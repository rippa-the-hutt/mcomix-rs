# Porting MComix3 from Python/PyGObject to Rust — status & roadmap

The Rust port lives in [`rust/`](rust/). It targets **GTK 4** via `gtk4-rs`
(0.9, requires GTK ≥ 4.12) and mirrors the architecture and behaviour of the
Python `mcomix/` package so the user experience stays as close as possible.

## Milestone 1 (implemented, 2026-08)

Compiles and runs on Linux; opens CBZ/CB7/CBT/CBR/image directories; full
viewer navigation, zoom/fit modes, rotation, double-page + manga mode,
thumbnails, slideshow, fullscreen, preferences and last-read-page persistence.

```
rust/
├── Cargo.toml                  # gtk4-rs, image, zip, sevenz-rust, tar, serde, clap
├── src/
│   ├── main.rs                 # CLI (mirrors mcomix/run.py) + GApplication wiring
│   ├── app.rs                  # main window (mirrors mcomix/main.py + event.py)
│   ├── archive/                # archive support (mirrors mcomix/archive_tools.py)
│   │   ├── mod.rs              # magic-byte detection, Archive trait, dispatch
│   │   ├── zip.rs              # CBZ via `zip` crate
│   │   ├── tar.rs              # CBT + gzip/bzip2/xz via `tar`/`flate2`/`bzip2`/`xz2`
│   │   ├── sevenzip.rs         # CB7 via `sevenz-rust` (pure Rust)
│   │   └── rar.rs              # CBR via external `unrar` or `7z` (like the Python port)
│   ├── image_loader.rs         # decode/rotate/flip → gdk::MemoryTexture (mcomix/image_tools.py)
│   ├── zoom.rs                 # fit/zoom model (mcomix/zoom.py)
│   ├── prefs.rs                # JSON preferences (mcomix/preferences.py)
│   ├── lastread.rs             # last-read page DB (mcomix/last_read_page.py)
│   └── natsort.rs              # natural sort (mcomix/tools.py alphanumeric_compare)
└── packaging/
    ├── arch/PKGBUILD           # Arch Linux
    ├── linux/                  # .desktop, mime XML, hicolor icons
    └── windows/                # MSYS2 build script + NSIS installer
```

### Module / feature matrix (Python → Rust)

| Python module | Rust module | Status |
|---|---|---|
| `run.py`, `main.py` (window shell) | `main.rs`, `app.rs` | ✅ done (M1) |
| `event.py` (keybindings) | `app.rs` key controller | ✅ done (M1) |
| `archive_tools.py`, `archive/zip.py` | `archive/mod.rs`, `archive/zip.rs` | ✅ done |
| `archive/tar.py` (+gz/bz2/xz) | `archive/tar.rs` | ✅ done |
| `archive/sevenzip_external.py` | `archive/sevenzip.rs` | ✅ done (external `7z` first, pure-Rust `sevenz-rust` fallback) |
| `archive/rar_external.py` | `archive/rar.rs` | ✅ done (external unrar/7z) |
| `archive/lha_external.py` | — | ⏳ next (external 7z/lha) |
| `archive/pdf_external.py` | — | ⏳ next (mutool; or `mupdf` crate) |
| `archive_recursive.py` (archive-in-archive) | — | ⏳ later |
| `image_handler.py` (cache, prefetch) | `app.rs` (2-page cache only) | 🟡 partial; add LRU prefetch |
| `image_tools.py` | `image_loader.rs` | ✅ core; enhancers ⏳ |
| `zoom.py` | `zoom.rs` | ✅ core |
| `layout.py` + `scrolling.py` (smart scroll) | `app.rs` (simple scroll) | 🟡 partial; port smart-scroll |
| `thumbbar.py`, `thumbnail_tools.py` | `app.rs` async thumbnails | ✅ basic |
| `slideshow.py` | `app.rs` | ✅ basic |
| `preferences.py` | `prefs.rs` | ✅ (JSON, same keys) |
| `preferences_dialog.py` | — | ⏳ next |
| `last_read_page.py` | `lastread.rs` | ✅ (JSON instead of sqlite) |
| `bookmark_backend.py` + dialogs | — | ⏳ next (serde JSON) |
| `library/*` (sqlite book DB) | — | ⏳ later (rusqlite) |
| `keybindings.py` + editor | `app.rs` (hard-coded defaults) | 🟡 partial; editor ⏳ |
| `enhance_backend.py` (contrast/brightness…) | — | ⏳ later (`image` ops) |
| `lens.py` (magnifier) | — | ⏳ later (overlay) |
| `osd.py` | `app.rs` status/OSD-lite | 🟡 partial |
| `openwith.py` | — | ⏳ later |
| `archive_packer.py` (edit archive) | — | ⏳ later (`zip` write support) |
| `edit_*.py` (image editing) | — | ⏳ later |
| i18n (`messages/*.po`) | — | ⏳ later (gettext-rs or fluent) |

## Build & run

```bash
# Linux (Debian/Ubuntu deps)
sudo apt install pkg-config libgtk-4-dev libgdk-pixbuf-2.0-dev libpango1.0-dev \
     libcairo2-dev libglib2.0-dev liblzma-dev libbz2-dev
cd rust
cargo run --release -- /path/to/comic.cbz
```

CLI mirrors the Python app:

```text
mcomix-rs [OPTIONS] [PATH]
  -s, --slideshow       start in slideshow mode
  -f, --fullscreen      start fullscreen
  -m, --manga           manga (right-to-left) mode
  -d, --double-page     double page mode
  -b/-w/-h              zoom best / width / height
  -p, --page N          open at page N
  -W, --loglevel LEVEL  all | debug | info | warn | error (default: info)
```

Key bindings match MComix: `Page_Down`/`space` next, `Page_Up`/`BackSpace` prev,
`Home`/`End` first/last, `g` go-to, `d` double page, `m` manga, `b/w/h/s/a` fit
modes, `+`/`-` zoom, `r`/`Shift+r` rotate, `f`/`F11` fullscreen, `i` hide all,
`n` minimize, `Tab` info, `F9` thumbnails, `Ctrl+S` slideshow, arrows scroll,
`Ctrl+0` normal size.

## Packaging

| Target | Where | How |
|---|---|---|
| Debian/Ubuntu `.deb` | `rust/` + `rust/debian/` | `cargo install cargo-deb && cargo deb` (needs GTK ≥ 4.12 ⇒ Debian 13+/Ubuntu 24.04+) |
| Arch Linux | `rust/packaging/arch/PKGBUILD` | `makepkg -si` (drop into a dir with the source) |
| Standalone Linux | tarball job in CI | binary only; needs GTK runtime installed |
| Windows portable zip | `rust/packaging/windows/msys2-build.sh` | MSYS2 MINGW64 build; bundles GTK DLLs + data ⇒ no-install folder/zip |
| Windows NSIS installer | `installer.nsi` | produced by the same script ⇒ `mcomix-rs-setup-<ver>.exe` |
| GitHub Releases | `.github/workflows/release.yml` | tag `v0.1.0` ⇒ builds .deb, tarball, zip, installer; attaches to a draft release |

Windows notes:

- Build toolchain: MSYS2 `mingw-w64-x86_64-{gcc,pkgconf,gtk4,rust,ntldd,nsis,zip}`.
- The portable folder contains `mcomix-rs.exe` + all required DLLs + GTK data
  (`share/glib-2.0/schemas`, `share/icons/Adwaita`, gdk-pixbuf loaders).
- `gtk4-rs` needs GTK ≥ 4.12; MSYS2 ships current GTK 4, so that is satisfied.

## Preferences / data migration

- Config: `~/.config/mcomix-rs/preferences.conf` (JSON — same key names as the
  Python `preferences.conf`, so settings can be copied over).
- Data: `~/.local/share/mcomix-rs/lastreadpage.json` (the Python app used
  sqlite for this; JSON keeps the port dependency-free).
- Windows: `%APPDATA%\mcomix-rs\` for both.

## What's next (suggested order)

1. **LHA + PDF readers** (external `7z`/`lha`, `mutool`; later `mupdf` crate).
2. **Smart scrolling + page cache** (prefetch next/prev pages in a worker;
   LRU of ~7 decoded pages like `max pages to cache`).
3. **Preferences dialog + keybinding editor** (mirror `preferences_dialog.py`).
4. **Bookmarks** (serde JSON; port `bookmark_backend.py` + dialogs).
5. **Library** (rusqlite backend; port `mcomix/library/*`).
6. **Recursive archives, image enhancement, magnifying lens, OSD, openwith.**
7. **i18n** (reuse the existing `.po` files via gettext-rs).
8. **AppImage** for a truly standalone Linux distribution.
