//! About dialog (GTK4 has no GtkAboutDialog, so this is a custom window
//! mirroring `mcomix/about_dialog.py`).

use gtk4 as gtk;
use gtk4::gdk;
use gtk4::prelude::*;

/// The logo, embedded from the original MComix icon set.
const LOGO_PNG: &[u8] = include_bytes!("../../mcomix/images/mcomix-large.png");

pub fn show_about(parent: &impl IsA<gtk::Window>) {
    let dlg = gtk::Window::new();
    dlg.set_title(Some("About MComix-rs"));
    dlg.set_transient_for(Some(parent));
    dlg.set_modal(true);
    dlg.set_resizable(false);
    dlg.set_default_size(440, -1);

    // Logo.
    let logo = gtk::Picture::new();
    if let Ok(img) = image::load_from_memory(LOGO_PNG) {
        let rgba = img.to_rgba8();
        let tex = crate::image_loader::texture_from_rgba(&rgba);
        logo.set_paintable(Some(&tex));
    }
    logo.set_can_shrink(true);
    logo.set_content_fit(gtk::ContentFit::Contain);
    logo.set_size_request(128, 128);

    let name = gtk::Label::new(Some("MComix-rs"));
    name.add_css_class("about-name");
    let version = gtk::Label::new(Some(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
    version.add_css_class("dim-label");

    let comments = gtk::Label::new(Some(
        "MComix-rs is an image viewer specifically designed to handle comic books. \
         It reads ZIP, RAR, 7Z, tar, LHA and PDF archives, as well as plain image files. \
         This is the Rust / GTK4 port.",
    ));
    comments.set_wrap(true);
    comments.set_xalign(0.0);
    comments.set_max_width_chars(60);

    let license = gtk::Label::new(Some(
        "MComix-rs is licensed under the terms of the GNU General Public License. \
         A copy of this license can be obtained from \
         http://www.gnu.org/licenses/gpl-2.0.html",
    ));
    license.set_wrap(true);
    license.set_xalign(0.0);
    license.set_max_width_chars(60);

    let section = |title: &str, lines: &[&str]| -> gtk::Widget {
        let boxv = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let t = gtk::Label::new(Some(title));
        t.set_xalign(0.0);
        t.add_css_class("about-section");
        boxv.append(&t);
        for line in lines {
            let l = gtk::Label::new(Some(line));
            l.set_xalign(0.0);
            l.set_wrap(true);
            boxv.append(&l);
        }
        boxv.upcast()
    };

    let authors = section(
        "Authors",
        &[
            "Rippa The Hutt: Developer (Rust / GTK4 port)",
            "Pontus Ekberg: Original vision/developer of Comix",
            "Louis Casillas, Moritz Brunner, Ark, Benoit Pierre: MComix developers",
        ],
    );
    let artists = section("Artists", &["Victor Castillejo: Icon design"]);
    let translators = section(
        "Translators",
        &[
            "Christoph Wolk: German translation",
            "Raimondo Giammanco, Giovanni Scafora, GhePeU: Italian translation",
            "Arthur Nieuwland: Dutch translation",
            "Achraf Cherti, Benoît H., Joseph M. Sleiman: French translation",
            "… and many others — see the original MComix translation credits.",
        ],
    );

    let website = gtk::LinkButton::with_label(
        "https://github.com/rippa-the-hutt/mcomix-rs",
        "Project website",
    );

    let close = gtk::Button::with_label(&crate::i18n::tr("Close"));
    close.add_css_class("suggested-action");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vbox.set_margin_top(20);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(24);
    vbox.set_margin_end(24);
    vbox.append(&logo);
    vbox.append(&name);
    vbox.append(&version);
    vbox.append(&comments);
    vbox.append(&license);
    vbox.append(&authors);
    vbox.append(&artists);
    vbox.append(&translators);
    vbox.append(&website);
    let h = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    h.set_halign(gtk::Align::End);
    h.append(&close);
    vbox.append(&h);
    dlg.set_child(Some(&vbox));

    // Center the logo/name horizontally.
    logo.set_halign(gtk::Align::Center);
    name.set_halign(gtk::Align::Center);
    version.set_halign(gtk::Align::Center);
    website.set_halign(gtk::Align::Center);

    let dlg2 = dlg.clone();
    close.connect_clicked(move |_| dlg2.close());
    dlg.connect_close_request(|_| glib::Propagation::Proceed);

    // Escape closes.
    let esc = gtk::EventControllerKey::new();
    esc.set_propagation_phase(gtk::PropagationPhase::Capture);
    let dlg_esc = dlg.clone();
    esc.connect_key_pressed(move |_c, keyval, _code, _state| {
        if keyval == gdk::Key::Escape {
            dlg_esc.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    dlg.add_controller(esc);

    dlg.present();
}
