# Architecture

## Module map (`rust/src/`)

| Module | Responsibility |
|---|---|
| `main.rs` | CLI parsing (clap), `GApplication` setup (`HANDLES_OPEN`), i18n init |
| `app.rs` | `Ui` (all widgets) + `AppState`; page worker; redraw; spreads; navigation; scrolling; lens; OSD; page-select, enhance, open-with dialogs; toolbar overflow |
| `archive/mod.rs` | Format detection by magic bytes, the `Archive` trait (`raw_names`, `page_names`, `read`), `open`/`open_plain`, `DirArchive` |
| `archive/{zip,tar,sevenzip,rar,lha,pdf}.rs` | One backend each; RAR/LHA/PDF shell out to `unrar`/`7z`/`mutool` |
| `archive/recursive.rs` | Archives inside archives: extracts embedded archives to a temp dir and prefixes their entries |
| `image_loader.rs` | Decode / rotate / flip / enhance; gdk-pixbuf scaled thumbnails; textures |
| `layout.rs` | Ported box model: `Box2`, `Scrolling` (Bresenham smart scroll), `FiniteLayout` |
| `zoom.rs` | Fit modes and the user zoom log (`ZoomModel::zoomed_sizes`) |
| `prefs.rs` | `Prefs` struct + JSON load/save (defaults + overlay semantics) |
| `prefs_dialog.rs` | Preferences window (Appearance/Behaviour/Display/Scrolling/Shortcuts) |
| `keybindings.rs` | `Action`, `BindingMap`, accelerator parsing, JSON persistence |
| `library.rs` / `library_dialog.rs` | SQLite library backend (rusqlite) and its window |
| `bookmarks.rs` | Bookmarks store (JSON) |
| `lastread.rs` | Last-read page per book (JSON) |
| `i18n.rs` | `.mo` parser, language resolution, `tr()` / `trf()` |
| `thumb_cache.rs` | On-disk thumbnail cache (`~/.cache/mcomix-rs/thumbnails`) |
| `lru.rs` | LRU cache with a byte budget (page cache) |
| `natsort.rs` | Natural sort |
| `about_dialog.rs` | About window (uses `mcomix/images/mcomix-large.png`) |

`build.rs` embeds `mcomix/messages/*/LC_MESSAGES/mcomix.mo` into the binary.

## Threads and channels

- **Page worker** (one thread): receives `PageCmd` (path, pages, indices,
  rotation/flip/enhance, viewport + zoom settings) and returns `PageResult`.
  For display requests it **pre-scales** the page to the target size with
  `image::thumbnail` and sends both the display-sized image and the full-res
  RGBA. The main thread only builds textures from them.
- **Thumbnail manager** (one thread): driven by a "wanted" set (window around
  the current page + the visible sidebar region), checks the disk cache,
  decodes with gdk-pixbuf scaled decoding, and is **paused while a page decode
  is in flight** so navigation always has the CPU.
- **Pollers** (main loop): every 30 ms drain the page/thumbnail result
  channels and refresh the thumbnail wanted-set; every 100 ms update the
  toolbar overflow and re-check the viewport size (`gtk4-rs 0.9` exposes no
  `size-allocate` signal, hence polling).
- Generation counters (`page_gen`, `prefetch_gen`, `thumb_gen`, `page_req`)
  invalidate stale results after opening a file or changing transforms.

## Caches

- **Page cache** (`AppState::cache`, `lru.rs`): page index → (full-res w, h,
  tight RGBA8). `max_pages_to_cache` entries and a byte budget. Prefetch fills
  it in the background; the lens reads full-res pixels from it.
- **Display textures** (`AppState::textures`): slot → (page index, display w,
  display h, texture). Rebuilt only when the page or the zoomed size changes.
- **Thumbnail cache** (disk): keyed by archive path + mtime + page index.

## Redraw and geometry

`Ui::redraw()`:

1. computes the viewport from the scrolled window allocation;
2. computes the **spread** (`spread_of`) and its visual order (`visual_order`);
3. asks `ZoomModel::zoomed_sizes` for the display size of each page;
4. builds a `FiniteLayout` (union-wrapped) used by smart scrolling;
5. resolves a texture per slot (reusing `state.textures`, otherwise scaling
   from the page cache) and appends the pictures **in visual order** into an
   LTR `GtkBox`;
6. records `state.displayed[slot] = (x, y, w, h)` for the lens.

The content widget is sized to the pages (`total_w`/`total_h`), **not** to the
layout union box (that is the scrollable wrapper, ≥ viewport, and would pin
pages to the left).

## Spreads and stepping

- `show_cover_alone()` → the `SHOW_DOUBLE_AS_ONE_TITLE` bit (default on) and
  the path is an archive.
- `spread_of(n, page, double_page, cover_alone)`: `[page]` in single mode;
  with the cover offset `[0]`, `(1,2)`, `(3,4)`, …; without it `(0,1)`, `(2,3)`.
- `visual_order(len, manga)`: in manga the first page of the spread is appended
  last so it renders on the **right**.
- `step_magnitude(page, forward, double_step, cover_alone, n)`: mirrors the
  Python `flip_page` double-step rule (an extra page is skipped except across
  the cover).

`sizes` holds **full-resolution** dimensions; always derive display sizes from
them via the zoom model.

## Navigation at book boundaries

`next_book`/`previous_book` mirror the Python `next_book` logic: next/previous
archive in the same directory (when `auto_open_next_archive`, or the slideshow
with `slideshow_can_go_to_next_archive`), else next/previous **directory**
(`auto_open_next_directory`, `sibling_directory` + `first_or_last_matching`).
When a transition succeeds, the finished book's saved position is cleared
(`clear_position_for`), so it no longer offers to resume.
