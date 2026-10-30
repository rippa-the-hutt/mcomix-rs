#!/usr/bin/env bash
# Cross-compile the Windows artifacts (portable zip + NSIS installer) from
# Linux, without any Windows runner.
#
# Approach:
#   - Rust target x86_64-pc-windows-gnu with the MinGW-w64 cross compiler
#   - MSYS2 mingw-w64 packages downloaded from the MSYS2 mirror and used as a
#     link sysroot (import libs + pkg-config files). The dependency closure is
#     resolved recursively from each package's .PKGINFO, so split/meta packages
#     (e.g. gcc-libs -> cc-libs -> libgcc + libstdc++) are followed correctly.
#   - Runtime DLLs are bundled by walking the import tables of the exe *and* of
#     the gdk-pixbuf loaders; the build FAILS if any non-system import is left
#     unresolved (a missing libgcc_s_seh-1.dll once shipped and broke the app on
#     Windows).
#   - NSIS installer produced by `makensis`, which runs natively on Linux.
#
# Usage: bash packaging/windows/cross-build-linux.sh
# Requires: apt install mingw-w64 nsis zip zstd curl pkg-config python3

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
OBJDUMP="${OBJDUMP:-x86_64-w64-mingw32-objdump}"

echo "==> Installing cross toolchain (mingw-w64, nsis, zip, zstd)"
sudo apt-get update -qq
sudo apt-get install -y -qq mingw-w64 nsis zip zstd curl pkg-config python3 2>/dev/null || \
sudo apt-get install -y -qq mingw-w64 nsis zip zstd curl pkg-config python3

echo "==> Adding Rust target $TARGET"
rustup target add "$TARGET"

# Seed packages: the pkg-config chain we link against plus the toolkit data.
# Their own dependencies are resolved recursively below, so do NOT rely on this
# list being complete (MSYS2 splits packages over time: gcc-libs is now a stub
# with no files that depends on cc-libs -> libgcc + libstdc++).
PACKAGES="
gtk4 gdk-pixbuf2 glib2 gettext-runtime libiconv pango cairo harfbuzz
freetype fontconfig fribidi libpng libjpeg-turbo libtiff jbigkit lerc
libdeflate libwebp giflib zlib zstd xz bzip2 expat brotli libunistring
graphite2 libffi pcre2 libxml2 libdatrie libthai libepoxy graphene
json-glib pixman lzo2 librsvg gcc-libs vulkan-loader
adwaita-icon-theme iso-codes shared-mime-info
# GStreamer runtime: libgtk-4-1.dll hard-imports these (media support).
gstreamer gst-plugins-base gst-plugins-bad-libs orc libogg libvorbis
libtheora opus libnice gnutls gmp nettle libtasn1 p11-kit libidn2 libva
"

mkdir -p "$WORK"
LISTING="$WORK/listing.html"
FETCHED="$WORK/.fetched"

# The mirror redirects; -L follows it. Cache the (6 MB) listing for the run.
if [ ! -s "$LISTING" ] || [ "${REFRESH_LISTING:-1}" = "1" ]; then
    echo "==> Fetching package listing"
    curl -fsSL "$MIRROR/" -o "$LISTING"
fi
touch "$FETCHED"

# Map a package name to its archive name. The listing URL-encodes '+', so
# libstdc++ appears as libstdc%2B%2B.
pkg_file() { # name -> archive name (empty if absent)
    local name="$1" encoded base found
    encoded="$(printf '%s' "$name" | sed 's/+/%2B/g')"
    base="mingw-w64-x86_64-"
    found="$(grep -oE "${base}(${name}|${encoded})-[0-9][^\"]*\.pkg\.tar\.zst" "$LISTING" \
             | sort -V | tail -1 || true)"
    printf '%s' "$found"
}

pkg_deps() { # archive -> "name name ..." (mingw deps only, versions stripped)
    tar --zstd -xOf "$1" .PKGINFO 2>/dev/null \
        | sed -n 's/^depend = //p' \
        | sed 's/[<>=].*$//' \
        | sed -n 's/^mingw-w64-x86_64-//p'
}

echo "==> Resolving the MSYS2 package closure"
queue="$PACKAGES"
while [ -n "$(printf '%s' "$queue" | tr -d '[:space:]')" ]; do
    pkg="$(printf '%s' "$queue" | tr ' ' '\n' | sed '/^$/d' | head -1)"
    queue="$(printf '%s' "$queue" | tr ' ' '\n' | sed '/^$/d' | tail -n +2 | tr '\n' ' ')"
    case " $(tr '\n' ' ' < "$FETCHED") " in *" $pkg "*) continue ;; esac
    echo "$pkg" >> "$FETCHED"

    file="$(pkg_file "$pkg")"
    if [ -z "$file" ]; then
        echo "  WARNING: no package found for $pkg"
        continue
    fi
    if [ ! -f "$WORK/$file" ]; then
        echo "  downloading $file"
        curl -fsSL -o "$WORK/$file" "$MIRROR/$file"
    fi
    # Extract into a scratch dir so this package's .PKGINFO can be read, then
    # merge its mingw64/ tree into the sysroot.
    rm -rf "$WORK/.pkg"
    mkdir -p "$WORK/.pkg"
    tar --zstd -xf "$WORK/$file" -C "$WORK/.pkg"
    if [ -d "$WORK/.pkg/mingw64" ]; then
        cp -a "$WORK/.pkg/mingw64/." "$MINGW/"
    fi
    deps="$(pkg_deps "$WORK/$file" | tr '\n' ' ')"
    if [ -n "${deps// /}" ]; then
        queue="$queue $deps"
    fi
done
rm -rf "$WORK/.pkg"
echo "    $(wc -l < "$FETCHED") packages, $([ -d "$MINGW/bin" ] && ls "$MINGW/bin" | wc -l || echo 0) DLLs in the sysroot"

# MSYS2 .pc files use prefix=/mingw64; PKG_CONFIG_SYSROOT_DIR rewrites that to
# $SYSROOT/mingw64, and PKG_CONFIG_LIBDIR points at the cross .pc files.
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR="$MINGW/lib/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"
export PKG_CONFIG_PATH="$MINGW/lib/pkgconfig"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc"

echo "==> Verifying pkg-config closure"
missing=""
for mod in gtk4 gdk-pixbuf-2.0 glib-2.0 gio-2.0 pango pangocairo cairo \
           cairo-gobject harfbuzz freetype2 fontconfig graphene-gobject-1.0 vulkan; do
    if ! pkg-config --exists "$mod"; then
        missing="$missing $mod"
    fi
done
if [ -n "$missing" ]; then
    echo "ERROR: missing pkg-config modules:$missing"
    exit 1
fi

echo "==> Cross-compiling (release)"
cargo build --release --target "$TARGET"

EXE="target/$TARGET/release/mcomix-rs.exe"

echo "==> Assembling portable bundle"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"
cp "$EXE" "$DIST_DIR/"

# Windows system DLLs are provided by the OS and must NOT be bundled. This list
# is shared by the bundler and the final verification step.
is_system_dll() {
    case "$(printf '%s' "$1" | tr 'A-Z' 'a-z')" in
        kernel32.dll|user32.dll|gdi32.dll|advapi32.dll|shell32.dll|ole32.dll|oleaut32.dll|shlwapi.dll|ws2_32.dll|comdlg32.dll|comctl32.dll|crypt32.dll|d3d11.dll|d3d12.dll|dcomp.dll|dwmapi.dll|hid.dll|imm32.dll|opengl32.dll|setupapi.dll|shcore.dll|winmm.dll|winspool.drv|msimg32.dll|dwrite.dll|rpcrt4.dll|usp10.dll|dxgi.dll|cfgmgr32.dll|ntdll.dll|msvcrt.dll|dnsapi.dll|iphlpapi.dll|gdiplus.dll|uxtheme.dll|version.dll|bcrypt.dll|bcryptprimitives.dll|userenv.dll|mpr.dll|winhttp.dll|secur32.dll|wldap32.dll|normaliz.dll|psapi.dll|powrprof.dll|dbghelp.dll|propsys.dll|authz.dll|netapi32.dll|api-ms-win-*) return 0 ;;
        *) return 1 ;;
    esac
}

collect_deps() { # PE file -> first-level imported DLL names
    "$OBJDUMP" -p "$1" 2>/dev/null | awk '/DLL Name:/{print $3}'
}

declare -A copied=()
missing_dlls=()
copy_dll() { # dll name (recursively)
    local dll="$1" src dep
    [ -n "${copied[$dll]:-}" ] && return 0
    copied[$dll]=1
    is_system_dll "$dll" && return 0
    src="$MINGW/bin/$dll"
    if [ ! -f "$src" ]; then
        missing_dlls+=("$dll")
        return 0
    fi
    cp -f "$src" "$DIST_DIR/"
    while read -r dep; do
        [ -n "$dep" ] && copy_dll "$dep"
    done < <(collect_deps "$src")
}

echo "==> Bundling runtime DLLs"
while read -r dep; do
    [ -n "$dep" ] && copy_dll "$dep"
done < <(collect_deps "$DIST_DIR/mcomix-rs.exe")

echo "==> Copying GTK runtime data"
SHARE="$MINGW/share"
if [ -d "$SHARE/glib-2.0/schemas" ]; then
    mkdir -p "$DIST_DIR/share/glib-2.0/schemas"
    cp -r "$SHARE/glib-2.0/schemas/." "$DIST_DIR/share/glib-2.0/schemas/"
    if [ ! -f "$DIST_DIR/share/glib-2.0/schemas/gschemas.compiled" ] && command -v glib-compile-schemas >/dev/null; then
        glib-compile-schemas "$DIST_DIR/share/glib-2.0/schemas"
    fi
fi
if [ -d "$SHARE/icons/Adwaita" ]; then
    mkdir -p "$DIST_DIR/share/icons"
    cp -r "$SHARE/icons/Adwaita" "$DIST_DIR/share/icons/"
fi

# gdk-pixbuf loaders. Their own imports must be bundled too (the SVG loader
# needs librsvg, which is how a missing DLL slipped through before).
LOADERS="$MINGW/lib/gdk-pixbuf-2.0/2.10.0/loaders"
if [ -d "$LOADERS" ]; then
    mkdir -p "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders"
    cp "$LOADERS"/*.dll "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders/" 2>/dev/null || true
    for loader in "$LOADERS"/*.dll; do
        [ -f "$loader" ] || continue
        while read -r dep; do
            [ -n "$dep" ] && copy_dll "$dep"
        done < <(collect_deps "$loader")
    done
fi

echo "==> Verifying the bundle"
if [ ${#missing_dlls[@]} -gt 0 ]; then
    printf 'ERROR: missing DLL(s) in the sysroot: %s\n' "${missing_dlls[*]}"
    printf 'Add the providing MSYS2 package (see .PKGINFO deps) and retry.\n'
    exit 1
fi
# Independent check: every import of every shipped PE file must resolve to a
# bundled DLL or a Windows system DLL.
python3 - "$DIST_DIR" "$OBJDUMP" << 'PYEOF'
import glob, os, subprocess, sys

dist, objdump = sys.argv[1], sys.argv[2]
os.chdir(dist)
bundled = {os.path.basename(p).lower() for p in glob.glob('*.dll')}
bundled |= {os.path.basename(p).lower()
            for p in glob.glob('lib/gdk-pixbuf-2.0/*/loaders/*.dll')}
system = {l.strip().lower() for l in """kernel32.dll user32.dll gdi32.dll advapi32.dll
shell32.dll ole32.dll oleaut32.dll shlwapi.dll ws2_32.dll comdlg32.dll comctl32.dll
crypt32.dll d3d11.dll d3d12.dll dcomp.dll dwmapi.dll hid.dll imm32.dll opengl32.dll
setupapi.dll shcore.dll winmm.dll winspool.drv msimg32.dll dwrite.dll rpcrt4.dll
usp10.dll dxgi.dll cfgmgr32.dll ntdll.dll msvcrt.dll dnsapi.dll iphlpapi.dll
gdiplus.dll uxtheme.dll version.dll bcrypt.dll bcryptprimitives.dll userenv.dll
mpr.dll winhttp.dll secur32.dll wldap32.dll normaliz.dll psapi.dll powrprof.dll
dbghelp.dll propsys.dll authz.dll netapi32.dll""".split() if l.strip()}

def imports(path):
    out = subprocess.run([objdump, '-p', path], capture_output=True, text=True).stdout
    return [line.split()[-1] for line in out.splitlines() if 'DLL Name:' in line]

pe_files = glob.glob('*.exe') + glob.glob('*.dll') + glob.glob('lib/gdk-pixbuf-2.0/*/loaders/*.dll')
unresolved = {}
for f in pe_files:
    for dep in imports(f):
        d = dep.lower()
        if d.startswith('api-ms-win-') or d in system or d in bundled:
            continue
        unresolved.setdefault(d, set()).add(os.path.basename(f))

if unresolved:
    print(f"ERROR: {len(unresolved)} unresolved import(s) in the bundle:")
    for dep, who in sorted(unresolved.items()):
        print(f"  {dep}  <- {', '.join(sorted(who))}")
    sys.exit(1)
print(f"    ok: {len(pe_files)} PE files, {len(bundled)} bundled DLLs, all imports resolved")
PYEOF

echo "==> Creating portable zip"
(cd dist && rm -f "$(basename "$PORTABLE")" && zip -qr "$(basename "$PORTABLE")" mcomix-rs-win64)
(cd dist && sha256sum "$(basename "$PORTABLE")" > "$(basename "$PORTABLE").sha256")

echo "==> Building NSIS installer (makensis on Linux)"
makensis \
    -DVERSION="$VERSION" \
    -DSOURCE_DIR="$ROOT/dist/mcomix-rs-win64" \
    -DOUTFILE="$ROOT/dist/mcomix-rs-setup-${VERSION}.exe" \
    packaging/windows/installer.nsi
(cd dist && sha256sum "mcomix-rs-setup-${VERSION}.exe" > "mcomix-rs-setup-${VERSION}.exe.sha256")

echo "==> Done."
ls -lh "$PORTABLE" "$SETUP" dist/*.sha256
