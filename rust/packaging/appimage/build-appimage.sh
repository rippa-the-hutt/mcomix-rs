#!/usr/bin/env bash
# Build a self-contained AppImage for Linux (x86_64) without linuxdeploy:
# the AppDir is assembled manually (binary + runtime libs via ldd + GTK data),
# and appimagetool produces the final .AppImage.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"   # rust/ crate root
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
TOOLS_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/mcomix-rs-appimage"
APPDIR="dist/appimage/AppDir"
APPIMAGE="dist/mcomix-rs-${VERSION}-x86_64.AppImage"

echo "==> Building release binary"
cargo build --release

mkdir -p "$TOOLS_DIR" dist/appimage

# ---- appimagetool (cached) ----
if [ ! -f "$TOOLS_DIR/appimagetool-x86_64.AppImage" ]; then
    echo "==> Downloading appimagetool"
    curl -fL --retry 3 -o "$TOOLS_DIR/appimagetool-x86_64.AppImage" \
        https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
    chmod +x "$TOOLS_DIR/appimagetool-x86_64.AppImage"
fi

echo "==> Assembling AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib" "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp target/release/mcomix-rs "$APPDIR/usr/bin/"
cp "$ROOT/packaging/linux/mcomix-rs.desktop" "$APPDIR/usr/share/applications/"
# appimagetool requires a .desktop file at the AppDir root.
cp "$ROOT/packaging/linux/mcomix-rs.desktop" "$APPDIR/mcomix-rs.desktop"
cp "$ROOT/packaging/linux/icons/256x256/apps/mcomix3.png" \
    "$APPDIR/usr/share/icons/hicolor/256x256/apps/mcomix3.png"

# ---- bundle the runtime libraries (via ldd), excluding glibc internals ----
echo "==> Bundling shared libraries"
is_system_lib() {
    case "$(basename "$1")" in
        ld-linux*|libc.so*|libm.so*|libdl.so*|libpthread.so*|librt.so*|libresolv.so*|libutil.so*|libnsl.so*|libnss_*|libnsl*|libBrokenLocale*|libanl.so*) return 0 ;;
        *) return 1 ;;
    esac
}
ldd target/release/mcomix-rs | awk '/=> \//{print $3}' | sort -u | while read -r lib; do
    if is_system_lib "$lib"; then
        continue
    fi
    cp -n "$lib" "$APPDIR/usr/lib/" 2>/dev/null || true
done
# glibc is excluded above, but libgcc_s/libstdc++ ARE bundled (portability).

# ---- GTK runtime data ----
SHARE=/usr/share
# GSettings schemas.
if [ -f "$SHARE/glib-2.0/schemas/gschemas.compiled" ]; then
    mkdir -p "$APPDIR/usr/share/glib-2.0/schemas"
    cp "$SHARE/glib-2.0/schemas/gschemas.compiled" "$APPDIR/usr/share/glib-2.0/schemas/"
fi
# Adwaita icons.
if [ -d "$SHARE/icons/Adwaita" ]; then
    mkdir -p "$APPDIR/usr/share/icons"
    cp -r "$SHARE/icons/Adwaita" "$APPDIR/usr/share/icons/"
fi
# gdk-pixbuf loaders + the query tool (to regenerate the cache at runtime).
PIXBUF_LIBDIR="$(pkg-config --variable=libdir gdk-pixbuf-2.0 2>/dev/null || echo /usr/lib/x86_64-linux-gnu)"
PIXBUF_VERSION="$(pkg-config --modversion gdk-pixbuf-2.0 2>/dev/null || echo 2.10.0)"
LOADERS_SRC="$PIXBUF_LIBDIR/gdk-pixbuf-2.0/$PIXBUF_VERSION/loaders"
if [ -d "$LOADERS_SRC" ]; then
    mkdir -p "$APPDIR/usr/lib/gdk-pixbuf-2.0/$PIXBUF_VERSION/loaders"
    cp "$LOADERS_SRC"/*.so "$APPDIR/usr/lib/gdk-pixbuf-2.0/$PIXBUF_VERSION/loaders/" 2>/dev/null || true
fi
if command -v gdk-pixbuf-query-loaders >/dev/null 2>&1; then
    cp "$(command -v gdk-pixbuf-query-loaders)" "$APPDIR/usr/bin/"
    ldd "$(command -v gdk-pixbuf-query-loaders)" | awk '/=> \//{print $3}' | sort -u | while read -r lib; do
        if ! is_system_lib "$lib"; then
            cp -n "$lib" "$APPDIR/usr/lib/" 2>/dev/null || true
        fi
    done
fi

# ---- AppRun ----
cat > "$APPDIR/AppRun" << 'APPEOF'
#!/bin/sh
SELF="$(readlink -f "$0")"
HERE="${SELF%/*}"
export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export GSETTINGS_SCHEMA_DIR="$HERE/usr/share/glib-2.0/schemas"
export XDG_DATA_DIRS="$HERE/usr/share${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
# Regenerate the gdk-pixbuf loader cache with the AppImage's own paths.
if [ -x "$HERE/usr/bin/gdk-pixbuf-query-loaders" ]; then
    GDK_PIXBUF_MODULEDIR="$HERE/usr/lib/gdk-pixbuf-2.0"/*/loaders \
        "$HERE/usr/bin/gdk-pixbuf-query-loaders" \
        > "$HERE/usr/lib/gdk-pixbuf-2.0"/*/loaders.cache 2>/dev/null || true
    export GDK_PIXBUF_MODULE_FILE=$(ls "$HERE/usr/lib/gdk-pixbuf-2.0"/*/loaders.cache 2>/dev/null | head -1)
fi
exec "$HERE/usr/bin/mcomix-rs" "$@"
APPEOF
chmod +x "$APPDIR/AppRun"

echo "==> Building AppImage"
"$TOOLS_DIR/appimagetool-x86_64.AppImage" --appimage-extract-and-run "$APPDIR" "$APPIMAGE"

echo "==> Done:"
ls -lh "$APPIMAGE"
