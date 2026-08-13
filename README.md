# MComix-rs

A user-friendly, customizable comic book image viewer for Linux and Windows,
written in **Rust** with **GTK4** (`gtk4-rs`). It is a from-scratch port of
[MComix3](https://github.com/rippa-the-hutt/mcomix3) (Python/PyGObject),
designed to keep the same look, feel and features while being far easier to
build and distribute.

MComix-rs is specifically designed for comic books (both Western comics and
manga) and supports **CBZ**, **CBR**, **CB7**, **CBT**, **LHA**, **PDF** and
plain image directories — including archives embedded inside archives.

---

## Features

- GTK4 interface with a scrollable viewer, toolbar, thumbnail sidebar and status bar
- Fit modes (best / width / height / size / manual), zoom in/out, rotation, flips
- Double-page and manga (right-to-left) modes
- Smart scrolling with MComix-style edge-flip protection
- Lazy thumbnail generation (gdk-pixbuf scaled decode + on-disk cache)
- Background page decoding with a page cache and prefetch — the UI never freezes
- Magnifying lens, image enhancement (brightness/contrast/auto-contrast)
- Slideshow, fullscreen, on-screen page indicator (OSD)
- Bookmarks, library (SQLite) with cover grid and watch folders
- "Continue reading?" resume prompt, last-read-page tracking
- Configurable keybindings and preferences (JSON)
- i18n: 23 languages embedded (same catalogs as MComix3)
- Click-to-advance and drag-to-pan, like the original

## Installation

### Linux

| Package | Where |
|---|---|
| **AppImage** (standalone) | GitHub Releases — download, `chmod +x`, run |
| **.deb** (Debian/Ubuntu ≥ 24.04) | GitHub Releases, or build with `cargo deb` |
| **Arch Linux** | `rust/packaging/arch/PKGBUILD` via `makepkg -si` |
| **Tarball** (binary only) | GitHub Releases — needs the GTK4 runtime |

### Windows

- **Portable zip** — no-install folder with the `.exe` and all bundled DLLs/data.
- **Installer** — NSIS `mcomix-rs-setup-<ver>.exe`.

Both are produced from the GitHub Releases workflow (MSYS2/MINGW64 build).

## Building from source

```bash
# Debian/Ubuntu dependencies
sudo apt install pkg-config libgtk-4-dev libgdk-pixbuf-2.0-dev libpango1.0-dev \
     libcairo2-dev libglib2.0-dev liblzma-dev libbz2-dev

cd rust
cargo build --release
./target/release/mcomix-rs "/path/to/comic.cbz"
```

Optional external tools for archive backends: `unrar` or `7z` (RAR/LHA), and
`mutool` (PDF). Everything else (ZIP, 7z, TAR, gzip/bzip2/xz) is handled by
bundled pure-Rust libraries.

## Usage

```text
mcomix-rs [OPTIONS] [PATH]
  -s, --slideshow       start in slideshow mode
  -f, --fullscreen      start fullscreen
  -m, --manga           manga (right-to-left) mode
  -d, --double-page     double page mode
  -b / -w / -h          zoom best / width / height
  -p, --page N          open at page N
  -W, --loglevel LEVEL  all | debug | info | warn | error
```

Key bindings (configurable in Preferences → Shortcuts): `Page_Down`/`space`
next, `Page_Up`/`BackSpace` previous, `Home`/`End` first/last, `g` go-to,
`d` double page, `m` manga, `b/w/h/s/a` fit modes, `+`/`-` zoom, `r` rotate,
`f`/`F11` fullscreen, `l` magnifying lens, `i` hide all, `Tab` info,
`F9` thumbnails, `Ctrl+S` slideshow, `Ctrl+D` bookmark, `Ctrl+B` edit
bookmarks, arrows scroll (edge-flip with repeated presses).

## Data & configuration

- Preferences: `~/.config/mcomix-rs/preferences.conf` (JSON)
- Keybindings: `~/.config/mcomix-rs/keybindings.conf`
- Library: `~/.local/share/mcomix-rs/library.db`, thumbnails in `~/.cache/mcomix-rs/`
- Windows: `%APPDATA%\mcomix-rs\`

## Documentation

See [PORTING.md](PORTING.md) for the module-by-module status of the Python →
Rust port and the roadmap.

## Credits

- **Rippa The Hutt** — developer of MComix-rs
- **Pontus Ekberg** — original vision/developer of Comix
- **Louis Casillas, Moritz Brunner, Ark, Benoit Pierre** — MComix developers
- **Victor Castillejo** — icon design
- All the MComix translators (see the About dialog)

MComix-rs is licensed under the GNU General Public License, version 2 or later.
