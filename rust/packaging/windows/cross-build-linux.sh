#!/usr/bin/env bash
# Cross-compile the Windows artifacts (portable zip + NSIS installer) from
# Linux, without any Windows runner.
#
# Approach:
#   - Rust target x86_64-pc-windows-gnu with the MinGW-w64 cross compiler
#   - MSYS2 mingw-w64 GTK4 packages downloaded from the MSYS2 mirror and used
#     as a link sysroot (import libs + pkg-config files)
#   - Runtime DLLs + GTK data bundled from the same sysroot
#   - NSIS installer produced by `makensis`, which runs natively on Linux
#
# Usage: bash packaging/windows/cross-build-linux.sh
# Requires: apt install mingw-w64 nsis zip zstd curl pkg-config

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"   # rust/ crate root
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
TARGET="x86_64-pc-windows-gnu"
MIRROR="https://mirror.msys2.org/mingw/mingw64"
WORK="${XDG_CACHE_HOME:-$HOME/.cache}/mcomix-rs-cross"
# MSYS2 packages extract to a top-level `mingw64/` tree under $WORK.
SYSROOT="$WORK"
MINGW="$SYSROOT/mingw64"
DIST_DIR="dist/mcomix-rs-win64"
PORTABLE="dist/mcomix-rs-${VERSION}-windows-x86_64.zip"
SETUP="dist/mcomix-rs-setup-${VERSION}.exe"

echo "==> Installing cross toolchain (mingw-w64, nsis, zip, zstd)"
sudo apt-get update -qq
sudo apt-get install -y -qq mingw-w64 nsis zip zstd curl pkg-config zlib1g-dev 2>/dev/null || \
sudo apt-get install -y -qq mingw-w64 nsis zip zstd curl pkg-config

echo "==> Adding Rust target $TARGET"
rustup target add "$TARGET"

# Packages needed to link + bundle GTK4 and friends (MSYS2 mingw-w64).
PACKAGES="
gtk4 gdk-pixbuf2 glib2 pango cairo harfbuzz freetype fontconfig fribidi
libpng libjpeg-turbo libtiff libwebp zlib zstd xz bzip2 expat brotli
libunistring graphite2 libffi pcre2 gettext libiconv libxml2 libdatrie
libthai libepoxy graphene adwaita-icon-theme gcc-libs winpthreads
vulkan-loader
"

mkdir -p "$WORK"
if [ ! -d "$MINGW/bin" ]; then
    echo "==> Fetching MSYS2 mingw-w64 packages into $WORK"
    for pkg in $PACKAGES; do
        # Pick the newest matching .pkg.tar.zst from the mirror listing
        # (the mirror redirects; -L follows it).
        file="$(curl -fsSL "$MIRROR/" | grep -oE "mingw-w64-x86_64-$pkg-[0-9][^\"]*\.pkg\.tar\.zst" | sort -V | tail -1 || true)"
        if [ -z "$file" ]; then
            echo "WARNING: no package found for $pkg"
            continue
        fi
        if [ ! -f "$WORK/$file" ]; then
            echo "  downloading $file"
            curl -fsSL -o "$WORK/$file" "$MIRROR/$file"
        fi
        tar --zstd -xf "$WORK/$file" -C "$WORK"
    done
fi

# MSYS2 .pc files use prefix=/mingw64; PKG_CONFIG_SYSROOT_DIR rewrites that to
# $SYSROOT/mingw64, and PKG_CONFIG_LIBDIR points at the cross .pc files.
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR="$MINGW/lib/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"
export PKG_CONFIG_PATH="$MINGW/lib/pkgconfig"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc"

echo "==> Cross-compiling (release)"
cargo build --release --target "$TARGET"

EXE="target/$TARGET/release/mcomix-rs.exe"

echo "==> Assembling portable bundle"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"
cp "$EXE" "$DIST_DIR/"

# Copy every DLL the exe depends on (transitively) from the sysroot.
collect_deps() { # exe -> list of DLL names (first level)
    x86_64-w64-mingw32-objdump -p "$1" | awk '/DLL Name:/{print $3}'
}
copied=""
copy_dll() { # name
    local dll="$1"
    if echo "$copied" | grep -qx "$dll"; then return; fi
    copied="$copied
$dll"
    local src="$MINGW/bin/$dll"
    if [ ! -f "$src" ]; then
        echo "WARNING: $dll not found in sysroot"
        return
    fi
    cp -n "$src" "$DIST_DIR/" || true
    # Recurse into the DLL's own dependencies.
    for dep in $(collect_deps "$src"); do
        copy_dll "$dep"
    done
}
for dep in $(collect_deps "$DIST_DIR/mcomix-rs.exe"); do
    copy_dll "$dep"
done

echo "==> Copying GTK runtime data"
SHARE="$MINGW/share"
# GSettings schemas (compile with the host tool if the package lacks the cache).
if [ -d "$SHARE/glib-2.0/schemas" ]; then
    mkdir -p "$DIST_DIR/share/glib-2.0/schemas"
    cp -r "$SHARE/glib-2.0/schemas/." "$DIST_DIR/share/glib-2.0/schemas/"
    if [ ! -f "$DIST_DIR/share/glib-2.0/schemas/gschemas.compiled" ] && command -v glib-compile-schemas >/dev/null; then
        glib-compile-schemas "$DIST_DIR/share/glib-2.0/schemas"
    fi
fi
# Adwaita icons (needed by GTK for fallback icons).
if [ -d "$SHARE/icons/Adwaita" ]; then
    mkdir -p "$DIST_DIR/share/icons"
    cp -r "$SHARE/icons/Adwaita" "$DIST_DIR/share/icons/"
fi
# gdk-pixbuf loaders (DLLs + cache).
LOADERS="$MINGW/lib/gdk-pixbuf-2.0/2.10.0/loaders"
if [ -d "$LOADERS" ]; then
    mkdir -p "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders"
    cp "$LOADERS"/*.dll "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders/" 2>/dev/null || true
    if [ -f "$LOADERS/loaders.cache" ]; then
        cp "$LOADERS/loaders.cache" "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders/"
    fi
fi

echo "==> Creating portable zip"
(cd dist && rm -f "$(basename "$PORTABLE")" && zip -qr "$(basename "$PORTABLE")" mcomix-rs-win64)

echo "==> Building NSIS installer (makensis on Linux)"
makensis -DVERSION="$VERSION" packaging/windows/installer.nsi

echo "==> Done."
ls -lh "$PORTABLE" "$SETUP"
