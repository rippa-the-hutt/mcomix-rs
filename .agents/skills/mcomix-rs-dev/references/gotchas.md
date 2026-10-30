# Gotchas already hit in this port

Each entry is a real failure that reached a build or the maintainer. Check this
list before debugging something that "should work".

## GTK4 / gtk4-rs 0.9

- **`set_size_request` sets the *minimum*, not the size.** A `GtkPicture`'s
  natural size is the paintable's full resolution, so a 4000 px image overflows
  its box and sticks to the left. Verify with
  `pic.measure(Orientation::Horizontal, -1)` (returns `min=600, natural=4000`).
  Fix: give the picture a texture already scaled to the display size.
- **`GtkFlowBox::insert(child, pos)` appends when `pos >= child_count`.** For
  out-of-order arrivals, pre-create one cell per page in order and fill the
  pictures, instead of inserting by index.
- **`MenuButton` is not a `Button`** (no `emit_clicked()`); `Button::clicked()`
  does not exist in 0.9 — use `ButtonExt::emit_clicked()`.
- **`ColorDialogButton::new(None)` NULLs the construct-only `dialog` property**
  → `GTK_IS_COLOR_DIALOG (self)` assertion when clicked. Pass
  `Some(ColorDialog::new())`. Set the initial colour with `set_rgba(...)` from
  the *stored prefs* (reading the button's own `rgba()` at construction returns
  GTK's default red).
- **The CSS property for colour schemes differs across GTK versions**
  (`gtk-color-scheme` on newer builds vs `color-scheme`): probe at runtime by
  parsing a probe stylesheet and counting `connect_parsing_error` hits; fall
  back to `Settings::set_gtk_application_prefer_dark_theme`.
- **`SourceId::remove()` panics if the source already fired.** A timeout that
  returns `Break` destroys itself; guard the stored id with a "fired" flag
  (`AppState::osd_fired`).
- **Unrealised widgets measure 0 and have an allocation of (1,1).** When the app
  starts with a file argument (file manager / Open-with), the scrolled window is
  not mapped yet: computing the viewport from it yields ~50 px and the cover is
  pre-scaled to a thumbnail. Fall back to `window.default_size()` until the
  allocation is real.
- **gtk4-rs 0.9 has no `size-allocate` signal** — resize handling is polled
  (100 ms) on purpose.

## `Rc<RefCell<Ui>>` re-entrancy

- A signal handler that runs *while* you hold `borrow_mut()` panics with
  "RefCell already borrowed". Two concrete cases:
  - setting a `ToggleButton` programmatically fires `toggled` → handler →
    `borrow_mut()` again → use a suppression flag while syncing
    (`Ui::suppress_toggles`).
  - closing a dialog fires `close_request` synchronously → drop the borrow
    *before* calling `dlg.close()`.
- Signal handlers that fire during layout (e.g. `vadjustment`
  `value_changed` while inserting children) are safest handled from the poller
  instead of a signal.

## Widget wiring

- Creating a widget in `Ui::new` but initialising the struct field with a
  **new** widget (`osd: gtk::Label::new(...)` instead of `osd,`) leaves the
  visible widget orphaned: updates go to a widget that is not in the tree. This
  happened twice (OSD, lens) — always move the real clone into the struct.

## Preferences

- `#[serde(default)]` on the struct fills **missing** fields with the *type*
  default (`false`/`0`), not with the value from `Default::default()`. Any
  preference added after a user's config file exists would silently be off.
  `Prefs::load` therefore starts from `Prefs::default()` and overlays only the
  keys present in the file (`ov!` macro). **Add new fields to that list.**

## Archives

- `unrar lb -inul <file>` returns an **empty listing** on recent unrar versions
  (it suppresses the listing itself). Do not pass `-inul` to `lb`.
- `7z l -slt` prints the archive's own path as the first `Path =` line; only
  parse entries after the `----------` separator.
- `Archive::page_names()` filters to images, so embedded archive discovery uses
  `raw_names()`.
- Decoding/pre-scaling belongs to the worker; doing it in a signal handler or on
  the UI thread freezes the app (a 12 MP page costs ~700 ms decode + scale).

## Windows bundle

- A shipped 0.4.2 build failed on Windows with `libgcc_s_seh-1.dll not found`.
  MSYS2 had turned `gcc-libs` into an empty stub (`size = 0`) depending on
  `cc-libs` → `libgcc` + `libstdc++`; the script still listed only `gcc-libs` and
  the bundler merely *warned* about the missing DLLs. Never trust a hand-written
  package list: resolve `.PKGINFO` deps recursively and **fail the build** when
  an import cannot be satisfied (`packaging/windows/cross-build-linux.sh`).
- Bundling must also walk the **gdk-pixbuf loader** DLLs, not only the exe:
  `pixbufloader_svg.dll` needs `librsvg-2-2.dll`.
- Windows DLL names are case-insensitive but Linux paths are not: `libLerc.dll`
  satisfies an import of `liblerc.dll`. Compare lowercased when auditing.
- Antivirus/SmartScreen false positives come with the territory for unsigned
  MinGW binaries and NSIS installers; the installer installs per user (no UAC)
  and the release publishes `.sha256` files. Code signing is the real remedy.

## Toolbar overflow

- Deciding "does it fit?" from the *live* natural width oscillates: hiding a
  button shrinks the natural width, which un-hides it. Use the stored
  full-width value with engage/disengage margins, and refresh the per-button
  widths only while fully expanded (before that, widgets are unrealised and
  measure 0).

## Miscellaneous

- `trf()` must substitute `%d` as well as `%s`/`{}` (catalog strings use `%d`).
- Tests that depend on machine-local paths (e.g. a hand-made `/tmp/nav-test`)
  pass locally and fail in CI — build fixtures inside the test.
