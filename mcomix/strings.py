# -*- coding: utf-8 -*-
""" strings.py - Constant strings that need internationalization.
    This file should only be imported after gettext has been correctly initialized
    and installed in the global namespace. """

from mcomix.constants import ZIP, RAR, TAR, GZIP, BZIP2, XZ, PDF, SEVENZIP, LHA, ZIP_EXTERNAL

ARCHIVE_DESCRIPTIONS = {
                        ZIP         : _('ZIP archive'),
                        RAR         : _('RAR archive'),
                        TAR         : _('Tar archive'),
                        GZIP        : _('Gzip compressed tar archive'),
                        BZIP2       : _('Bzip2 compressed tar archive'),
                        XZ          : _('XZ compressed tar archive'),
                        PDF         : _('PDF document'),
                        SEVENZIP    : _('7z archive'),
                        LHA         : _('LHA archive'),
                        ZIP_EXTERNAL: _('ZIP archive'),
                       }

AUTHORS = (
            ('Pontus Ekberg', _('Original vision/developer of Comix')),
            ('Louis Casillas', _('MComix3 developer')),
            ('Moritz Brunner', _('MComix3 developer')),
            ('Ark', _('MComix3 developer')),
            ('Benoit Pierre', _('MComix3 developer')),
            ('Rippa The Hutt', _('Python 3 port developer')),
          )
TRANSLATORS = (
            ('Emfox Zho', _('Simplified Chinese translation')),
            ('Xie Yanbo', _('Simplified Chinese translation')),
            ('Zach Cheung', _('Simplified Chinese translation')),
            ('Manuel QuiÃ±ones', _('Spanish translation')),
            ('Carlos Feli', _('Spanish translation')),
            ('Marcelo GÃ³es', _('Brazilian Portuguese translation')),
            ('Christoph Wolk', _('German translation and Nautilus thumbnailer')),
            ('Chris Leick', _('German translation')),
            ('Raimondo Giammanco', _('Italian translation')),
            ('Giovanni Scafora', _('Italian translation')),
            ('GhePeU', _('Italian translation')),
            ('Arthur Nieuwland', _('Dutch translation')),
            ('Achraf Cherti', _('French translation')),
            ('BenoÃ®t H.', _('French translation')),
            ('Joseph M. Sleiman', _('French translation')),
            ('FrÃ©dÃ©ric Chateaux', _('French translation')),
            ('Kamil Leduchowski', _('Polish translatin')),
            ('Darek Jakoniuk', _('Polish translation')),
            ('Paul Chatzidimitrio', _('Greek translation')),
            ('Carles Escrig Royo', _('Catalan translation')),
            ('Hsin-Lin Cheng', _('Traditional Chinese translation')),
            ('Wayne S', _('Traditional Chinese translation')),
            ('Mamoru Tasaka', _('Japanese translation')),
            ('Keita Haga', _('Japanese translation')),
            ('Toshiharu Kudoh', _('Japanese translation')),
            ('ErnÅ Drabik', _('Hungarian translation')),
            ('Artyom Smirnov', _('Russian translation')),
            ('ÐÐ²Ð³ÐµÐ½Ð¸Ð¹ ÐÐµÐ¶Ð½Ð¸Ð½', _('Russian translation')),
            ('Adrian C.', _('Croatian translation')),
            ('ê¹ë¯¼ê¸°', _('Korean translation')),
            ('Gyeongmin Bak', _('Korean translation')),
            ('Maryam Sanaat', _('Persian translation')),
            ('Andhika Padmawan', _('Indonesian translation')),
            ('Jan Nekvasil', _('Czech translation')),
            ('ÐÐ»ÐµÐºÑÐ°Ð½Ð´Ñ ÐÐ°ÑÑ', _('Ukrainian translation')),
            ('Roxerio Roxo Carrillo', _('Galician translation')),
            ('Jonatan Nyberg, Martin Karlsson', _('Swedish translation')),
            ('Isratine Citizen', _('Hebrew translation')),
            ('Zygi Mantus', _('Lithuanian translation')),
          )
ARTISTS = (
            ('Victor Castillejo', _('Icon design')),
          )

# vim: expandtab:sw=4:ts=4
