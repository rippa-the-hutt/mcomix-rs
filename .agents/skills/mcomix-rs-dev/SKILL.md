---
name: mcomix-rs-dev
description: Build, test, run and safely modify the mcomix-rs Rust/GTK4 comic reader (port of MComix3). Use when editing Rust code in this repository, adding features, or fixing viewer, archive, navigation, zoom, thumbnail, layout or preferences bugs.
---

# Working on mcomix-rs

## Build, test, run

```bash
cd rust
cargo test                # 41 unit tests, offline (fixtures in ../test/files)
cargo build               # debug binary (deps are optimised, see below)
./target/debug/mcomix-rs -W info "/path/to/comic.cbz"   # -W debug for detail
```

CLI: `-s` slideshow, `-f` fullscreen, `-m` manga, `-d` double page,
`-b/-w/-h` zoom best/width/height, `-p N` start page, `-l` library,
`-W all|debug|info|warn|error` (default `info`).

Note: `[profile.dev.package."*"] opt-level = 2` in `Cargo.toml` keeps
dependency code (image decoding/scaling) fast in debug builds — without it,
resizing a 12 MP page takes ~1 s unoptimised.

## Where things are

- Crate: `rust/` (binary `mcomix-rs`). Resources stay at the repo root:
  `mcomix/images/` (icons, `include_bytes!`), `mcomix/messages/` (23 `.mo`
  catalogs embedded by `build.rs`), `test/files/` (fixtures).
- `src/app.rs` (~4.2k lines) holds the widget tree, `AppState`, the page
  worker, redraw, spreads, scrollers, lens, OSD and most dialogs.

Read `references/architecture.md` (module map + data flow) before larger work,
and `references/gotchas.md` before touching GTK/threading code — it lists traps
this port already hit, which are easy to reintroduce.

## Conventions

- **New preference:** field in `src/prefs.rs` **and** an entry in the `ov!(...)`
  list inside `Prefs::load` — otherwise an existing `preferences.conf` silently
  keeps the *type* default (this disabled the resume prompt and OSD for a
  while). Then surface it in `src/prefs_dialog.rs`.
- **New keybinding:** `Action` variant in `src/keybindings.rs` (`name()`,
  `label()`, `all()`, defaults) plus a match arm in `Ui::handle_action`.
- **Heavy work never runs on the UI thread:** decoding, transforming and
  pre-scaling happen in the page worker (`PageCmd` → `PageResult`); the main
  thread only wraps textures and drives widgets.
- **Match the Python reference semantics** for anything user-visible
  (navigation, spreads, scrolling, resume prompt, archive handling). See the
  `mcomix-rs-python-parity` skill.
- Keep code style consistent with the surrounding file; `cargo build` should
  stay free of new warnings.
