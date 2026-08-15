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
- i18n: 23 languages embedded
- Click-to-advance and drag-to-pan, like the original

## Installation

### Linux

| Package | Where |
|---|---|
| **AppImage** (standalone) | GitHub Releases — download, `chmod +x`, run |
| **.deb** (Debian/Ubuntu ≥ 24.04) | GitHub Releases, or build with `cargo deb` |
| **Arch Linux (AUR)** | `mcomix-rs-bin` on the AUR — see below |
| **Arch Linux (manual)** | `rust/packaging/arch/PKGBUILD` via `makepkg -si` |
| **Tarball** (binary only) | GitHub Releases — needs the GTK4 runtime |

Arch Linux users can install the binary package straight from the
[AUR](https://aur.archlinux.org/packages/mcomix-rs-bin):

```bash
# Using yay
yay -S mcomix-rs-bin

# Using paru
paru -S mcomix-rs-bin
```

The AUR package downloads the prebuilt Linux artifact from the GitHub
release (no compilation) and verifies the sha256 checksums. It can be
installed alongside the original Python `mcomix3` package.

### Windows

- **Portable zip** — no-install folder with the `.exe` and all bundled DLLs/data.
- **Installer** — NSIS `mcomix-rs-setup-<ver>.exe`.

Both are produced from the GitHub [Releases](https://github.com/rippa-the-hutt/mcomix-rs/releases) workflow (MSYS2/MINGW64 build).

## Building from source

```bash
# Debian/Ubuntu dependencies
sudo apt install pkg-config libgtk-4-dev libgdk-pixbuf-2.0-dev libpango1.0-dev \
     libcairo2-dev libglib2.0-dev liblzma-dev libbz2-dev

cd rust
cargo build --release
./target/release/mcomix-rs "/path/to/comic.cbz"
```

Arch Linux:

```bash
# Arch dependencies
sudo pacman -S --needed base-devel gtk4 gdk-pixbuf2 glib2 pango cairo xz bzip2

cd rust
cargo build --release
./target/release/mcomix-rs "/path/to/comic.cbz"
```

### Building the Windows artifacts from Linux (cross-compile)

No Windows machine or runner needed — the portable zip and NSIS installer are
built with the MinGW-w64 cross compiler, MSYS2's `mingw-w64` GTK4 packages
used as a link sysroot, and `makensis` (which runs natively on Linux):

```bash
cd rust

# Debian/Ubuntu
sudo apt install mingw-w64 nsis zip zstd curl pkg-config

# Arch Linux
sudo pacman -S --needed mingw-w64-gcc nsis zip zstd curl pkgconf

bash packaging/windows/cross-build-linux.sh
# -> dist/mcomix-rs-<ver>-windows-x86_64.zip and dist/mcomix-rs-setup-<ver>.exe
```

## Continuous integration (CI)

All builds run on GitHub Actions (`.github/workflows/release.yml`). The
workflow is triggered by **pushing a `v*` tag** (e.g. `v0.3.2`) and can also
be started manually from the Actions tab ("workflow_dispatch").

| Job | Runs on | Produces |
|---|---|---|
| `linux` | ubuntu-24.04 | `.deb` (via `cargo deb`), standalone tarball |
| `appimage` | ubuntu-24.04 | self-contained `mcomix-rs-<ver>-x86_64.AppImage` |
| `windows-cross` | ubuntu-24.04 | Windows portable zip + NSIS installer, **cross-compiled from Linux** |
| `release` | ubuntu-24.04 | creates a **draft GitHub Release** with all artifacts |

Each build job runs `cargo test --release` too, so the test suite is part of
the gate.

### How to use it

1. **Push your changes** to `develop`, then merge into `main`
   (`git checkout main && git merge develop --no-ff && git push origin main`).
2. **Tag the release** and push the tag — this is what triggers the build:
   ```bash
   git tag v0.3.2
   git push origin v0.3.2
   ```
   (Force-moving an existing tag, `git tag -f v0.3.2 && git push -f origin v0.3.2`,
   re-runs the workflow — useful to retry after a fix.)
3. **Watch the run** on the repository's **Actions** tab. A green dot means
   all jobs passed; click a failing job to see its logs.
4. When everything is green, go to the **Releases** tab, open the **draft
   release**, review/edit the notes and **publish** it.
5. **Arch/AUR**: bump `pkgver` in `rust/packaging/arch/PKGBUILD` to the new
   version and regenerate the checksums with `updkgsums` (the hashes change
   with every release).

### Running the tests locally (offline)

The same test suite that CI runs can be executed on your machine with no
network access (after the dependencies are fetched once):

```bash
cd rust
cargo test          # unit + integration tests
cargo build         # debug binary
```

The only requirement is the GTK4 development toolchain (see "Building from
source"); the test fixtures live in the repository (`test/files/`), so
nothing is downloaded at test time. Optional archive backends (`unrar`,
`7z`, `mutool`) are not needed for the test suite.

### Running the packaging scripts locally

| Artifact | Command (from `rust/`) |
|---|---|
| `.deb` | `cargo install cargo-deb && cargo deb` |
| AppImage | `bash packaging/appimage/build-appimage.sh` |
| Windows zip + installer | `bash packaging/windows/cross-build-linux.sh` |
| Arch Linux package | `makepkg -si` with `packaging/arch/PKGBUILD` |

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
