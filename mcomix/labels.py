"""labels.py - Gtk.Label convenience classes."""

import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, Gdk, GdkPixbuf
from gi.repository import Pango

class FormattedLabel(Gtk.Label):

    """FormattedLabel keeps a label always formatted with some pango weight,
    style and scale, even when new text is set using set_text().
    """

    def __init__(self, text='', weight=Pango.Weight.NORMAL,
      style=Pango.Style.NORMAL, scale=1.0):
        super(FormattedLabel, self).__init__(text)
        self._weight = weight
        self._style = style
        self._scale = scale
        self._format()

    def set_text(self, text):
        Gtk.Label.set_text(self, text)
        self._format()

    def _format(self):
        text_len = len(self.get_text())
        attrlist = Pango.AttrList()
        if text_len > 0:
            weight_attr = Pango.attr_weight_new(self._weight)
            weight_attr.start_index = 0
            weight_attr.end_index = text_len
            attrlist.insert(weight_attr)
            
            style_attr = Pango.attr_style_new(self._style)
            style_attr.start_index = 0
            style_attr.end_index = text_len
            attrlist.insert(style_attr)
            
            scale_attr = Pango.attr_scale_new(self._scale)
            scale_attr.start_index = 0
            scale_attr.end_index = text_len
            attrlist.insert(scale_attr)
        self.set_attributes(attrlist)

class BoldLabel(FormattedLabel):

    """A FormattedLabel that is always bold and otherwise normal."""

    def __init__(self, text=''):
        super(BoldLabel, self).__init__(text=text, weight=Pango.Weight.BOLD)

class ItalicLabel(FormattedLabel):

    """A FormattedLabel that is always italic and otherwise normal."""

    def __init__(self, text=''):
        super(ItalicLabel, self).__init__(text=text, style=Pango.Style.ITALIC)


# vim: expandtab:sw=4:ts=4
