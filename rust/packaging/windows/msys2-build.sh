#!/usr/bin/env bash
# Build mcomix-rs for Windows inside an MSYS2 MINGW64 shell and produce:
#   1. a portable folder + zip (no-install .exe bundle), and
#   2. an NSIS installer .exe
#
# CI installs the toolchain first; to do it manually run:
#   pacman -S --needed mingw-w64-x86_64-gcc mingw-w64-x86_64-pkgconf \
#       mingw-w64-x86_64-gtk4 mingw-w64-x86_64-ntldd mingw-w64-x86_64-nsis \
#       mingw-w64-x86_64-zip
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"   # rust/ crate root
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
DIST_DIR="dist/mcomix-rs-win64"
PORTABLE="dist/mcomix-rs-${VERSION}-windows-x86_64.zip"
SETUP="dist/mcomix-rs-setup-${VERSION}.exe"

echo "==> Building release binary"
cargo build --release

echo "==> Assembling portable bundle in $DIST_DIR"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"
cp target/release/mcomix-rs.exe "$DIST_DIR/"

# Copy every DLL the exe depends on (transitively), as reported by ntldd.
if command -v ntldd >/dev/null 2>&1; then
    ntldd -R target/release/mcomix-rs.exe \
        | sed -n 's/.*=> \(.*\.dll\).*/\1/p' \
        | sort -u \
        | while read -r dll; do
            if [ -f "$dll" ]; then
                cp -n "$dll" "$DIST_DIR/" || true
            fi
          done
else
    echo "WARNING: ntldd not found; DLL bundling skipped. Install mingw-w64-x86_64-ntldd."
fi

# GTK4 runtime data files.
MINGW="$(cd /mingw64 && pwd)"
SHARE="$MINGW/share"
if [ -d "$SHARE/glib-2.0/schemas" ]; then
    mkdir -p "$DIST_DIR/share"
    cp -r "$SHARE/glib-2.0/schemas" "$DIST_DIR/share/"
fi
if [ -d "$SHARE/icons/Adwaita" ]; then
    mkdir -p "$DIST_DIR/share/icons"
    cp -r "$SHARE/icons/Adwaita" "$DIST_DIR/share/icons/"
fi
# gdk-pixbuf image loaders.
LOADERS="$MINGW/lib/gdk-pixbuf-2.0/2.10.0/loaders"
if [ -d "$LOADERS" ]; then
    mkdir -p "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders"
    cp "$LOADERS"/*.dll "$DIST_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders/" 2>/dev/null || true
fi

echo "==> Creating portable zip"
(cd dist && zip -qr "mcomix-rs-${VERSION}-windows-x86_64.zip" mcomix-rs-win64)

if command -v makensis >/dev/null 2>&1; then
    echo "==> Building NSIS installer"
    makensis -DVERSION="$VERSION" packaging/windows/installer.nsi
    echo "    -> $SETUP"
else
    echo "WARNING: makensis not found; installer skipped. Install mingw-w64-x86_64-nsis."
fi

echo "==> Done."
ls -lh "$PORTABLE" "$SETUP" 2>/dev/null || true
