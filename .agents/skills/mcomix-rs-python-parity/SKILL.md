---
name: mcomix-rs-python-parity
description: Compare mcomix-rs behaviour with the original Python MComix3 implementation and port semantics faithfully. Use when a behaviour differs from the Python/original MComix, when porting a feature, or when you need to know exactly what the reference implementation does (navigation, spreads, scrolling, caching, resume prompt, archives, library, dialogs).
---

# Checking parity with the Python original

mcomix-rs is a port of **MComix3** (Python 3 + PyGObject/GTK3). The Python code
was removed from the working tree in commit `bdbc2e1` ("Remove the Python
implementation; keep resource directories in place") but is fully available in
git history — **always read it before changing ported behaviour**, it is the
specification.

## Getting the Python source

```bash
# Reference commit = the last commit that still contained the Python sources.
REF=$(git rev-parse $(git log --diff-filter=D --format=%H -1 -- mcomix/main.py)^)
echo "$REF"                      # -> e33235a… (v0.2.0 era)

# List everything:   ~92 modules under mcomix/
git ls-tree -r --name-only "$REF" | grep '^mcomix/.*\.py$'

# Read a module
git show "$REF:mcomix/main.py"

# Or extract the whole tree into a scratch dir for grepping
bash .agents/skills/mcomix-rs-python-parity/scripts/python-source.sh /tmp/mcomix-py
```

Auxiliary: `win32/`, `mime/`, `debian/`, `arch/` and `setup.py` also exist in
that commit (Python-era packaging). Upstream project:
<https://github.com/rippa-the-hutt/mcomix3>.

## Where to look for a given behaviour

| Question | Python file |
|---|---|
| Window layout, `flip_page`, `next_book`, resume prompt trigger, OSD text | `mcomix/main.py` |
| Keybindings, scroll-with-flipping, smart scrolling, drag-to-pan, click-to-advance | `mcomix/event.py` |
| Two-page/manga rules (`get_virtual_double_page`), page cache, prefetch window | `mcomix/image_handler.py` |
| Box model, layout, smart-scroll grid | `mcomix/box.py`, `mcomix/layout.py`, `mcomix/scrolling.py` |
| Fit/zoom limits | `mcomix/zoom.py` |
| Opening files, next/previous archive + directory, resume prompt text | `mcomix/file_handler.py` |
| Preferences (names, defaults) | `mcomix/preferences.py` |
| Constants (bit fields, zoom modes, autrotate) | `mcomix/constants.py` |
| Archive detection and backends | `mcomix/archive_tools.py`, `mcomix/archive/*.py` |
| Thumbnails sidebar | `mcomix/thumbbar.py`, `mcomix/thumbnail_tools.py` |
| Library, bookmarks, lens, OSD, open-with | `mcomix/library/*.py`, `mcomix/bookmark_*.py`, `mcomix/lens.py`, `mcomix/osd.py`, `mcomix/openwith*.py` |

`references/python-source-map.md` has the detail (including which Rust module
each Python module maps to) and `PORTING.md` keeps the status matrix.

## Rules for a faithful port

- **Copy the semantics, not the structure.** Python's GC + signals vs Rust's
  `Rc<RefCell<..>>` means the architecture differs; the *behaviour* must match.
- **Keep the same preference names and defaults** (JSON keys in
  `preferences.conf` mirror `prefs[...]`), so users can copy settings over.
- **Bit-field preferences stay bit fields** (e.g.
  `virtual double page for fitting images`: `SHOW_DOUBLE_AS_ONE_TITLE = 1`,
  `SHOW_DOUBLE_AS_ONE_WIDE = 2`).
- **Watch for 1-based vs 0-based pages**: Python uses 1-based page numbers,
  Rust uses 0-based indices (`goto_index`), and `start_page`/`-1` sentinels mean
  "last page".
- **Note where Python itself is stale**, so you do not replicate a bug blindly:
  e.g. its PDF trace pass matches only `matrix=`, while modern mutool emits
  `transform=` (mcomix-rs parses both).
- When the port intentionally differs, **document it** in `PORTING.md` and say
  why in the commit message.
