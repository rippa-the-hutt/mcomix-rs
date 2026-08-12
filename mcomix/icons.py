"""icons.py - Load MComix specific icons."""

import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, Gdk, GdkPixbuf
from importlib.resources import files

from mcomix import image_tools
from mcomix import log

def mcomix_icons():
    """ Returns a list of differently sized pixbufs for the
    application icon, generated from a single high-resolution source. """

    large = image_tools.load_pixbuf_data(
        (files('mcomix.images') / 'mcomix-large.png').read_bytes()
    )
    sizes = (16, 22, 24, 32, 48, 64, 96, 128, 256)
    pixbufs = [
        large.scale_simple(size, size, GdkPixbuf.InterpType.BILINEAR)
        for size in sizes
    ]

    return pixbufs

def load_icons():
    _icons = (('gimp-flip-horizontal.png',   'mcomix-flip-horizontal'),
              ('gimp-flip-vertical.png',     'mcomix-flip-vertical'),
              ('gimp-rotate-180.png',        'mcomix-rotate-180'),
              ('gimp-rotate-270.png',        'mcomix-rotate-270'),
              ('gimp-rotate-90.png',         'mcomix-rotate-90'),
              ('gimp-thumbnails.png',        'mcomix-thumbnails'),
              ('gimp-transform.png',         'mcomix-transform'),
              ('tango-enhance-image.png',    'mcomix-enhance-image'),
              ('tango-add-bookmark.png',     'mcomix-add-bookmark'),
              ('tango-archive.png',          'mcomix-archive'),
              ('tango-image.png',            'mcomix-image'),
              ('library.png',                'mcomix-library'),
              ('comments.png',               'mcomix-comments'),
              ('zoom.png',                   'mcomix-zoom'),
              ('lens.png',                   'mcomix-lens'),
              ('double-page.png',            'mcomix-double-page'),
              ('manga.png',                  'mcomix-manga'),
              ('fitbest.png',                'mcomix-fitbest'),
              ('fitwidth.png',               'mcomix-fitwidth'),
              ('fitheight.png',              'mcomix-fitheight'),
              ('fitmanual.png',              'mcomix-fitmanual'),
              ('fitsize.png',                'mcomix-fitsize'))

    # Load window title icons.
    pixbufs = mcomix_icons()
    Gtk.Window.set_default_icon_list(pixbufs)
    # Pre-initialize the missing pixbuf in the main thread
    image_tools.init_missing_pixbuf()
    # Load application icons.
    factory = Gtk.IconFactory()
    for filename, stockid in _icons:
        try:
            icon_data = (files('mcomix.images') / filename).read_bytes()
            pixbuf = image_tools.load_pixbuf_data(icon_data)
            iconset = Gtk.IconSet(pixbuf)
            factory.add(stockid, iconset)
        except Exception:
            log.warning(_('! Could not load icon "%s"'), filename)
    factory.add_default()



# vim: expandtab:sw=4:ts=4
