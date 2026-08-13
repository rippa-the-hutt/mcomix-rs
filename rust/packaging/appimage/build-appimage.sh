#!/usr/bin/env bash
# Build a self-contained AppImage for Linux (x86_64).
# Requires: cargo, pkg-config, libgtk-4-dev (build deps), and network access
# to fetch linuxdeploy / the GTK plugin / appimagetool on first run.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"   # rust/ crate root
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
APP="mcomix-rs"
TOOLS_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/mcomix-rs-appimage"
APPDIR="dist/appimage/AppDir"

echo "==> Building release binary"
cargo build --release

mkdir -p "$TOOLS_DIR" dist/appimage
cd dist/appimage

# ---- fetch tools (cached) ----
fetch() { # name url
    if [ ! -f "$TOOLS_DIR/$1" ]; then
        echo "==> Downloading $1"
        curl -fL --retry 3 -o "$TOOLS_DIR/$1" "$2"
        chmod +x "$TOOLS_DIR/$1"
    fi
}

fetch linuxdeploy-x86_64.AppImage \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
fetch linuxdeploy-plugin-gtk-x86_64.AppImage \
    https://github.com/linuxdeploy/linuxdeploy-plugin-gtk/releases/download/continuous/linuxdeploy-plugin-gtk-x86_64.AppImage
fetch appimagetool-x86_64.AppImage \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage

# The tools are AppImages; extract them so they run without FUSE.
export LINUXDEPLOY="$ROOT/dist/appimage/linuxdeploy"
"$TOOLS_DIR/linuxdeploy-x86_64.AppImage" --appimage-extract >/dev/null 2>&1
mv squashfs-root "$LINUXDEPLOY"
export GTK_PLUGIN="$ROOT/dist/appimage/gtk-plugin"
"$TOOLS_DIR/linuxdeploy-plugin-gtk-x86_64.AppImage" --appimage-extract >/dev/null 2>&1
mv squashfs-root "$GTK_PLUGIN"

echo "==> Assembling AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR"

# Desktop entry + icon.
mkdir -p "$APPDIR/usr/share/applications"
cp "$ROOT/packaging/linux/mcomix-rs.desktop" "$APPDIR/usr/share/applications/"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp "$ROOT/packaging/linux/icons/256x256/apps/mcomix3.png" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps/mcomix3.png"
mkdir -p "$APPDIR/usr/share/icons/hicolor/scalable/apps"

echo "==> Running linuxdeploy with the GTK plugin"
export LDAI_OUTPUT=AppImage
export NO_STRIP=true
"$LINUXDEPLOY/AppRun" \
    --appdir "$APPDIR" \
    --executable "$ROOT/target/release/mcomix-rs" \
    --desktop-file "$APPDIR/usr/share/applications/mcomix-rs.desktop" \
    --icon-file "$APPDIR/usr/share/icons/hicolor/256x256/apps/mcomix3.png" \
    --plugin gtk

echo "==> Building AppImage"
"$TOOLS_DIR/appimagetool-x86_64.AppImage" --appimage-extract-and-run "$APPDIR" \
    "$ROOT/dist/mcomix-rs-${VERSION}-x86_64.AppImage"

echo "==> Done:"
ls -lh "$ROOT"/dist/mcomix-rs-*.AppImage
