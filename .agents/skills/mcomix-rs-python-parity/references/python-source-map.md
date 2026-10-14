# Python → Rust module map

Reference commit: the parent of `bdbc2e1` (last commit containing `mcomix/*.py`).
`git rev-parse $(git log --diff-filter=D --format=%H -1 -- mcomix/main.py)^`

| Python (`mcomix/…`) | Rust (`rust/src/…`) | Notes |
|---|---|---|
| `run.py`, `main.py` | `main.rs`, `app.rs` | CLI + window; `flip_page`, `next_book`, `new_page`, `scroll_to_predefined` |
| `event.py` | `app.rs` | keybindings, scroll-with-flipping (N-press edge protection), smart scrolling, drag-to-pan, click-to-advance |
| `image_handler.py` | `app.rs`, `lru.rs` | page cache, prefetch window, `get_virtual_double_page` |
| `image_tools.py` | `image_loader.rs` | decode, rotate/flip, enhance, auto-contrast, thumbnails |
| `box.py`, `layout.py`, `scrolling.py` | `layout.rs` | `Box2`, `FiniteLayout`, `Scrolling` (Bresenham grid) |
| `zoom.py` | `zoom.rs` | fit modes, user zoom log, scale-up rules |
| `preferences.py` | `prefs.rs` | same key names; JSON instead of JSON+pickle migration |
| `preferences_dialog.py` | `prefs_dialog.rs` | tabs: Appearance/Behaviour/Display/Scrolling/Shortcuts |
| `keybindings.py`, `keybindings_editor.py` | `keybindings.rs`, `prefs_dialog.rs` | accelerator strings, JSON `keybindings.conf`, editor in the Shortcuts tab |
| `file_handler.py` | `app.rs` | open file/archive, next/previous archive and directory, resume prompt, `_get_index_for_page` |
| `archive_tools.py`, `archive/*.py` | `archive/*.rs` | magic detection, backends, recursive archives |
| `thumbbar.py`, `thumbnail_tools.py` | `app.rs`, `thumb_cache.rs` | lazy windowed thumbnails, gdk-pixbuf scaled decode, disk cache |
| `last_read_page.py` | `lastread.rs` | JSON instead of sqlite |
| `bookmark_backend.py`, `bookmark_menu*.py`, `bookmark_dialog.py` | `bookmarks.rs`, `app.rs` | JSON instead of pickle; popover + edit dialog |
| `library/*.py` | `library.rs`, `library_dialog.rs` | same schema (book/collection/contain/recent/watchlist) |
| `osd.py` | `app.rs` | overlay label, auto-hide, `show_osd` preference |
| `lens.py` | `app.rs` | cursor-following magnifier (crop from full-res pixels) |
| `openwith.py`, `openwith_menu.py` | `app.rs` | remembered commands + ad-hoc command dialog |
| `slideshow.py` | `app.rs` | timer-driven page flips |
| `i18n.py` + `messages/*.mo` | `i18n.rs` + `build.rs` | same catalogs, embedded in the binary |
| `about_dialog.py`, `strings.py` | `about_dialog.rs` | custom window (GTK4 has no `GtkAboutDialog`) |
| `cursor_handler.py` | `app.rs` | grab cursor while panning |
| `enhance_backend.py` | `image_loader.rs` | brightness/contrast/auto-contrast (saturation/sharpness still ⏳) |
| `layout.py` expand_area / `SHOW_DOUBLE_AS_ONE_WIDE` | — | ⏳ wide pages shown alone is not implemented |

Deliberate differences (documented): JSON instead of pickle/sqlite for
bookmarks, last-read page and keybindings; language catalogs embedded rather
than read from disk; RAR/LHA/PDF backends shell out to the same external tools
as Python; PDF DPI parsing handles `transform=`.
