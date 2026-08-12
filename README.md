# MComix3

A user-friendly, customizable comic book image viewer, forked from [MComix](https://sourceforge.net/p/mcomix/).

MComix3 is specifically designed to handle comic books (both Western comics and manga) and supports a variety of container formats including **CBR**, **CBZ**, **CB7**, **CBT**, **LHA** and **PDF**.

It is written in **Python 3** and uses **GTK+ 3** through the **PyGObject** bindings.

---

## Installation

### From the Arch User Repository (AUR)

Arch Linux users can install MComix3 directly from the [AUR](https://aur.archlinux.org/packages/mcomix3):

```bash
# Using yay
yay -S mcomix3

# Using paru
paru -S mcomix3

# Manual build (requires base-devel)
git clone https://aur.archlinux.org/mcomix3.git
cd mcomix3
makepkg -si
```
### Debian/Ubuntu

Debian/Ubuntu users can install the software with the .deb package provided in [Releases](https://github.com/rippa-the-hutt/mcomix3/releases).

### From Git

```bash
git clone https://github.com/rippa-the-hutt/mcomix3.git
cd mcomix3
python3 mcomixstarter.py
```

Simply run `mcomixstarter.py`:

```bash
python3 mcomixstarter.py
```

No installation required. You can also create a symlink to `mcomixstarter.py` from anywhere in your `PATH`.

## Dependencies

MComix3 requires **Python 3.7+**, **PyGObject** (GTK+ 3 bindings), and **Pillow** (Python Imaging Library Fork).

- On **Debian/Ubuntu**: `apt install python3 python3-gi python3-gi-cairo gir1.2-gtk-3.0 python3-pil python3-pil.imagetk`
- On **Fedora**: `dnf install python3 python3-gobject gtk3 python3-pillow`
- On **Arch**: `pacman -S python python-gobject gtk3 python-pillow`

### Archive backends

| Format | Required tool |
|---|---|
| **CBZ / ZIP** | Built-in Python `zipfile` support |
| **CBR / RAR** | `unrar` or `rar` (or `libunrar`) |
| **CB7 / 7Zip** | `7z` (p7zip) |
| **CBT / TAR** | Built-in Python `tarfile` support |
| **LHA / LZA** | `lha` or `7z` |
| **PDF** | `mutool` and `mudraw` (MuPDF) |

## Changes from MComix

MComix3 is a **Python 3 / GTK3 port** of the original MComix codebase with numerous bug fixes:

- Python 2 → Python 3 migration (`map`, `filter`, `cmp`, `str`/`bytes` fixes)
- PyGTK (GTK2) → PyGObject (GTK3) migration (API compatibility fixes)
- Fixed archive format detection (magic byte comparisons)
- Fixed smooth scroll support for modern mice
- Fixed drag-scroll hangs
- Fixed various crashes on exit and dialog initialization
- Fixed keyboard navigation with arrow keys
- Fixed thumbnail generation in file browser and library
- Library dialog is functional

## Credits

**Python 3 port:** Rippa The Hutt

MComix3 is a fork of [MComix](https://sourceforge.net/p/mcomix/), which itself was a fork of [Comix](https://comix.sourceforge.net/).

**MComix3 developer:**
- Rippa The Hutt

**Original MComix developers:**
- Louis Casillas
- Moritz Brunner
- Ark
- Benoit Pierre

**Original Comix developer:**
- Pontus Ekberg

Thanks to everyone who contributed translations, suggestions, bug reports, fixes and donations!

Icons with a filename starting with "gimp" are taken from The GIMP, and icons with a filename starting with "tango" are taken from the Tango Desktop Project. Most other icons are made by Victor Castillejo, creator of the GNOME-Colors icon theme.
