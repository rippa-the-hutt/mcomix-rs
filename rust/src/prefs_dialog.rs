//! Preferences dialog + keybinding (shortcuts) editor.
//! Mirrors `mcomix/preferences_dialog.py` + `keybindings_editor.py`.

use std::cell::RefCell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk4::gdk;
use gtk4::prelude::*;

use crate::keybindings::{format_binding, Action, Binding, BindingMap};
use crate::prefs::Prefs;
use crate::zoom;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn rgba_from_prefs(c: &[u16; 3]) -> gdk::RGBA {
    gdk::RGBA::new(
        c[0] as f32 / 65535.0,
        c[1] as f32 / 65535.0,
        c[2] as f32 / 65535.0,
        1.0,
    )
}

fn prefs_from_rgba(r: &gdk::RGBA) -> [u16; 3] {
    [
        (r.red().clamp(0.0, 1.0) * 65535.0) as u16,
        (r.green().clamp(0.0, 1.0) * 65535.0) as u16,
        (r.blue().clamp(0.0, 1.0) * 65535.0) as u16,
    ]
}

fn page_grid() -> gtk::Grid {
    let g = gtk::Grid::new();
    g.set_margin_top(12);
    g.set_margin_bottom(12);
    g.set_margin_start(16);
    g.set_margin_end(16);
    g.set_row_spacing(8);
    g.set_column_spacing(12);
    g
}

fn add_row(grid: &gtk::Grid, label: &str, widget: Option<&impl IsA<gtk::Widget>>, row: &mut i32) {
    if label.is_empty() {
        // Spacer that pushes the widget to the right edge, matching rows that
        // carry a text label in this column.
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        grid.attach(&spacer, 0, *row, 1, 1);
    } else {
        let l = gtk::Label::new(Some(label));
        l.set_xalign(0.0);
        grid.attach(&l, 0, *row, 1, 1);
    }
    if let Some(w) = widget {
        w.set_halign(gtk::Align::End);
        grid.attach(w, 1, *row, 1, 1);
    }
    *row += 1;
}

fn check(label: &str, active: bool) -> gtk::CheckButton {
    let b = gtk::CheckButton::with_label(label);
    b.set_active(active);
    b
}

fn spin(value: f64, min: f64, max: f64, step: f64, digits: u32) -> gtk::SpinButton {
    let s = gtk::SpinButton::with_range(min, max, step);
    s.set_digits(digits);
    s.set_numeric(true);
    s.set_value(value);
    s
}

fn dropdown(labels: &[&str], selected: u32) -> gtk::DropDown {
    let d = gtk::DropDown::from_strings(labels);
    d.set_selected(selected);
    d
}

// ---------------------------------------------------------------------------
// the form
// ---------------------------------------------------------------------------

struct PrefsForm {
    // Appearance
    bg_color: gtk::ColorDialogButton,
    thumb_bg_color: gtk::ColorDialogButton,
    show_page_numbers: gtk::CheckButton,
    thumbnail_size: gtk::SpinButton,
    checkered_bg: gtk::CheckButton,
    // Behaviour
    escape_quits: gtk::CheckButton,
    auto_load_last_file: gtk::CheckButton,
    auto_open_next_archive: gtk::CheckButton,
    auto_open_next_directory: gtk::CheckButton,
    double_step: gtk::CheckButton,
    default_double_page: gtk::CheckButton,
    default_manga: gtk::CheckButton,
    // Display
    default_fullscreen: gtk::CheckButton,
    hide_all_fullscreen: gtk::CheckButton,
    auto_rotate_exif: gtk::CheckButton,
    show_scrollbar: gtk::CheckButton,
    show_osd: gtk::CheckButton,
    zoom_mode: gtk::DropDown,
    fit_to_size_mode: gtk::DropDown,
    fit_to_size_px: gtk::SpinButton,
    slideshow_delay: gtk::SpinButton,
    // Scrolling
    pixels_key: gtk::SpinButton,
    pixels_wheel: gtk::SpinButton,
    smart_pct: gtk::SpinButton,
    edge_presses: gtk::SpinButton,
    flip_with_wheel: gtk::CheckButton,
    invert_scroll: gtk::CheckButton,
    scale_up: gtk::CheckButton,
}

impl PrefsForm {
    fn new(p: &Prefs) -> PrefsForm {
        PrefsForm {
            bg_color: gtk::ColorDialogButton::new(None),
            thumb_bg_color: gtk::ColorDialogButton::new(None),
            show_page_numbers: check("Show page numbers on thumbnails", p.show_page_numbers_on_thumbnails),
            thumbnail_size: spin(p.thumbnail_size as f64, 20.0, 500.0, 10.0, 0),
            checkered_bg: check("Checkered background for transparent images", p.checkered_bg_for_transparent_images),
            escape_quits: check("Escape key closes program", p.escape_quits),
            auto_load_last_file: check("Load last opened file at startup", p.auto_load_last_file),
            auto_open_next_archive: check("Automatically open the next archive", p.auto_open_next_archive),
            auto_open_next_directory: check("Automatically open the next directory", p.auto_open_next_directory),
            double_step: check("Double step in double page mode", p.double_step_in_double_page_mode),
            default_double_page: check("Use double page mode by default", p.default_double_page),
            default_manga: check("Use manga mode by default", p.default_manga_mode),
            default_fullscreen: check("Use fullscreen by default", p.default_fullscreen),
            hide_all_fullscreen: check("Automatically hide all toolbars in fullscreen", p.hide_all_in_fullscreen),
            auto_rotate_exif: check("Automatically rotate images according to their metadata", p.auto_rotate_from_exif),
            show_scrollbar: check("Show scrollbars", p.show_scrollbar),
            show_osd: check("Show on-screen page indicator", p.show_osd),
            zoom_mode: dropdown(
                &["Best fit", "Fit width", "Fit height", "Fit size", "Manual"],
                p.zoom_mode.clamp(0, 4) as u32,
            ),
            fit_to_size_mode: {
                let idx = match p.fit_to_size_mode {
                    zoom::ZOOM_MODE_WIDTH => 0,
                    zoom::ZOOM_MODE_HEIGHT => 1,
                    _ => 2,
                };
                dropdown(&["Fit width", "Fit height", "Best fit"], idx)
            },
            fit_to_size_px: spin(p.fit_to_size_px as f64, 10.0, 10000.0, 50.0, 0),
            slideshow_delay: spin(p.slideshow_delay as f64 / 1000.0, 0.1, 3600.0, 0.5, 1),
            pixels_key: spin(p.number_of_pixels_to_scroll_per_key_event as f64, 0.0, 500.0, 5.0, 0),
            pixels_wheel: spin(p.number_of_pixels_to_scroll_per_mouse_wheel_event as f64, 0.0, 500.0, 5.0, 0),
            smart_pct: spin(p.smart_scroll_percentage, 0.05, 1.0, 0.05, 2),
            edge_presses: spin(p.number_of_key_presses_before_page_turn as f64, 1.0, 10.0, 1.0, 0),
            flip_with_wheel: check("Flip page with mouse wheel at page edges", p.flip_with_wheel),
            invert_scroll: check("Invert smart scroll", p.invert_smart_scroll),
            scale_up: check("Allow upscaling small images in fit modes", p.scale_up),
        }
    }

    fn build_stack(&self) -> gtk::Stack {
        let stack = gtk::Stack::new();

        // ---- Appearance ----
        let mut row = 0;
        let grid = page_grid();
        {
            let btn = &self.bg_color;
            btn.set_rgba(&rgba_from_prefs(&self.bg_color_value()));
            add_row(&grid, "Background colour:", Some(btn), &mut row);
        }
        add_row(&grid, "Thumbnail background colour:", Some(&self.thumb_bg_color), &mut row);
        add_row(&grid, "", Some(&self.show_page_numbers), &mut row);
        add_row(&grid, "Thumbnail size (pixels):", Some(&self.thumbnail_size), &mut row);
        add_row(&grid, "", Some(&self.checkered_bg), &mut row);
        let page = gtk::ScrolledWindow::new();
        page.set_child(Some(&grid));
        page.set_vexpand(true);
        page.set_min_content_height(340);
        let sp = stack.add_named(&page, Some("appearance"));
        sp.set_title("Appearance");

        // ---- Behaviour ----
        let mut row = 0;
        let grid = page_grid();
        add_row(&grid, "", Some(&self.escape_quits), &mut row);
        add_row(&grid, "", Some(&self.auto_load_last_file), &mut row);
        add_row(&grid, "", Some(&self.auto_open_next_archive), &mut row);
        add_row(&grid, "", Some(&self.auto_open_next_directory), &mut row);
        add_row(&grid, "", Some(&self.double_step), &mut row);
        add_row(&grid, "", Some(&self.default_double_page), &mut row);
        add_row(&grid, "", Some(&self.default_manga), &mut row);
        let page = gtk::ScrolledWindow::new();
        page.set_child(Some(&grid));
        page.set_vexpand(true);
        page.set_min_content_height(340);
        let sp = stack.add_named(&page, Some("behaviour"));
        sp.set_title("Behaviour");

        // ---- Display ----
        let mut row = 0;
        let grid = page_grid();
        add_row(&grid, "", Some(&self.default_fullscreen), &mut row);
        add_row(&grid, "", Some(&self.hide_all_fullscreen), &mut row);
        add_row(&grid, "", Some(&self.auto_rotate_exif), &mut row);
        add_row(&grid, "", Some(&self.show_scrollbar), &mut row);
        add_row(&grid, "", Some(&self.show_osd), &mut row);
        add_row(&grid, "Default zoom mode:", Some(&self.zoom_mode), &mut row);
        add_row(&grid, "Fit to size mode:", Some(&self.fit_to_size_mode), &mut row);
        add_row(&grid, "Fixed size for this mode (px):", Some(&self.fit_to_size_px), &mut row);
        add_row(&grid, "Slideshow delay (seconds):", Some(&self.slideshow_delay), &mut row);
        let page = gtk::ScrolledWindow::new();
        page.set_child(Some(&grid));
        page.set_vexpand(true);
        page.set_min_content_height(340);
        let sp = stack.add_named(&page, Some("display"));
        sp.set_title("Display");

        // ---- Scrolling ----
        let mut row = 0;
        let grid = page_grid();
        add_row(&grid, "Pixels per key event:", Some(&self.pixels_key), &mut row);
        add_row(&grid, "Pixels per wheel event:", Some(&self.pixels_wheel), &mut row);
        add_row(&grid, "Smart scroll step (fraction of viewport):", Some(&self.smart_pct), &mut row);
        add_row(&grid, "Key presses before page turn at edges:", Some(&self.edge_presses), &mut row);
        add_row(&grid, "", Some(&self.flip_with_wheel), &mut row);
        add_row(&grid, "", Some(&self.invert_scroll), &mut row);
        add_row(&grid, "", Some(&self.scale_up), &mut row);
        let page = gtk::ScrolledWindow::new();
        page.set_child(Some(&grid));
        page.set_vexpand(true);
        page.set_min_content_height(340);
        let sp = stack.add_named(&page, Some("scrolling"));
        sp.set_title("Scrolling");

        stack
    }

    fn bg_color_value(&self) -> [u16; 3] {
        prefs_from_rgba(&self.bg_color.rgba())
    }

    fn collect(&self, base: &Prefs) -> Prefs {
        let mut p = base.clone();
        p.bg_color = prefs_from_rgba(&self.bg_color.rgba());
        p.thumb_bg_color = prefs_from_rgba(&self.thumb_bg_color.rgba());
        p.show_page_numbers_on_thumbnails = self.show_page_numbers.is_active();
        p.thumbnail_size = self.thumbnail_size.value() as u32;
        p.checkered_bg_for_transparent_images = self.checkered_bg.is_active();
        p.escape_quits = self.escape_quits.is_active();
        p.auto_load_last_file = self.auto_load_last_file.is_active();
        p.auto_open_next_archive = self.auto_open_next_archive.is_active();
        p.auto_open_next_directory = self.auto_open_next_directory.is_active();
        p.double_step_in_double_page_mode = self.double_step.is_active();
        p.default_double_page = self.default_double_page.is_active();
        p.default_manga_mode = self.default_manga.is_active();
        p.default_fullscreen = self.default_fullscreen.is_active();
        p.hide_all_in_fullscreen = self.hide_all_fullscreen.is_active();
        p.auto_rotate_from_exif = self.auto_rotate_exif.is_active();
        p.show_scrollbar = self.show_scrollbar.is_active();
        p.show_osd = self.show_osd.is_active();
        p.zoom_mode = self.zoom_mode.selected() as i32;
        p.fit_to_size_mode = match self.fit_to_size_mode.selected() {
            0 => zoom::ZOOM_MODE_WIDTH,
            1 => zoom::ZOOM_MODE_HEIGHT,
            _ => zoom::ZOOM_MODE_BEST,
        };
        p.fit_to_size_px = self.fit_to_size_px.value() as u32;
        p.slideshow_delay = (self.slideshow_delay.value() * 1000.0) as u64;
        p.number_of_pixels_to_scroll_per_key_event = self.pixels_key.value() as u32;
        p.number_of_pixels_to_scroll_per_mouse_wheel_event = self.pixels_wheel.value() as u32;
        p.smart_scroll_percentage = self.smart_pct.value();
        p.number_of_key_presses_before_page_turn = self.edge_presses.value() as u32;
        p.flip_with_wheel = self.flip_with_wheel.is_active();
        p.invert_smart_scroll = self.invert_scroll.is_active();
        p.scale_up = self.scale_up.is_active();
        p
    }
}

// ---------------------------------------------------------------------------
// shortcuts (keybindings) editor
// ---------------------------------------------------------------------------

fn capture_binding(parent: &impl IsA<gtk::Window>, on_done: Rc<dyn Fn(Binding)>) {
    let win = gtk::Window::new();
    win.set_title(Some("Press a key…"));
    win.set_transient_for(Some(parent));
    win.set_modal(true);
    win.set_resizable(false);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    let label = gtk::Label::new(Some(
        "Press the key or key combination to assign.\nPress Escape to cancel.",
    ));
    vbox.append(&label);
    let cancel = gtk::Button::with_label("Cancel");
    vbox.append(&cancel);
    win.set_child(Some(&vbox));

    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    let win2 = win.clone();
    key.connect_key_pressed(move |_c, keyval, _code, state| {
        use gdk::Key as K;
        if matches!(
            keyval,
            K::Shift_L | K::Shift_R | K::Control_L | K::Control_R | K::Alt_L | K::Alt_R | K::Meta_L | K::Meta_R
        ) {
            return glib::Propagation::Proceed;
        }
        let mods = crate::keybindings::normalize_mods(state);
        if keyval == K::Escape && mods.is_empty() {
            win2.close();
            return glib::Propagation::Stop;
        }
        on_done(Binding { key: keyval, mods });
        win2.close();
        glib::Propagation::Stop
    });
    win.add_controller(key);
    let win3 = win.clone();
    cancel.connect_clicked(move |_| win3.close());

    win.present();
}

fn build_action_row(
    parent: &impl IsA<gtk::Window>,
    bindings: Rc<RefCell<BindingMap>>,
    action: Action,
) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_margin_top(3);
    row.set_margin_bottom(3);
    row.set_margin_start(6);
    row.set_margin_end(6);

    let name = gtk::Label::new(Some(action.label()));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    row.append(&name);

    let binding_label = gtk::Label::new(Some(""));
    binding_label.set_xalign(0.0);
    binding_label.set_css_classes(&["keybinding"]);
    row.append(&binding_label);

    let refresh: Rc<dyn Fn()> = {
        let bindings = bindings.clone();
        let binding_label = binding_label.clone();
        Rc::new(move || {
            let map = bindings.borrow();
            let list = map.bindings_for(action);
            let text = if list.is_empty() {
                "—".to_string()
            } else {
                list.iter().map(format_binding).collect::<Vec<_>>().join(", ")
            };
            binding_label.set_text(&text);
        })
    };
    refresh();

    let change = gtk::Button::with_label("Change…");
    {
        let bindings = bindings.clone();
        let refresh = refresh.clone();
        let p = parent.clone().upcast::<gtk::Window>();
        change.connect_clicked(move |_| {
            let bindings = bindings.clone();
            let refresh = refresh.clone();
            capture_binding(&p, Rc::new(move |b| {
                bindings.borrow_mut().set_binding(action, b);
                bindings.borrow().save();
                refresh();
            }));
        });
    }
    row.append(&change);

    let reset = gtk::Button::with_label("Reset");
    {
        let bindings = bindings.clone();
        let refresh = refresh.clone();
        reset.connect_clicked(move |_| {
            bindings.borrow_mut().reset_action(action);
            bindings.borrow().save();
            refresh();
        });
    }
    row.append(&reset);

    row.upcast::<gtk::Widget>()
}

fn shortcuts_page(parent: &impl IsA<gtk::Window>, bindings: Rc<RefCell<BindingMap>>) -> gtk::Widget {
    let list = gtk::ListBox::new();
    for action in Action::all() {
        list.append(&build_action_row(parent, bindings.clone(), *action));
    }
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_child(Some(&list));
    scroller.set_vexpand(true);
    scroller.set_min_content_height(340);
    scroller.upcast::<gtk::Widget>()
}

// ---------------------------------------------------------------------------
// dialog entry point
// ---------------------------------------------------------------------------

pub fn show_dialog(parent: &impl IsA<gtk::Window>, prefs: &Prefs, on_apply: Rc<dyn Fn(&Prefs)>) {
    let form = PrefsForm::new(prefs);
    let bindings = Rc::new(RefCell::new(BindingMap::load()));

    let stack = form.build_stack();
    let shortcuts = shortcuts_page(parent, bindings);
    let sp = stack.add_named(&shortcuts, Some("shortcuts"));
    sp.set_title("Shortcuts");
    stack.set_visible_child_name("appearance");

    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&stack));

    let cancel = gtk::Button::with_label("Cancel");
    let ok = gtk::Button::with_label("OK");
    ok.add_css_class("suggested-action");

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.set_halign(gtk::Align::End);
    footer.set_margin_top(8);
    footer.set_margin_bottom(8);
    footer.set_margin_start(16);
    footer.set_margin_end(16);
    footer.append(&cancel);
    footer.append(&ok);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vbox.append(&switcher);
    vbox.append(&stack);
    vbox.append(&footer);
    stack.set_vexpand(true);

    let dlg = gtk::Window::new();
    dlg.set_title(Some("Preferences"));
    dlg.set_transient_for(Some(parent));
    dlg.set_modal(true);
    dlg.set_default_size(620, 560);
    dlg.set_child(Some(&vbox));

    // Owned copies for the signal closures.
    let prefs = prefs.clone();

    let dlg_cancel = dlg.clone();
    cancel.connect_clicked(move |_| dlg_cancel.close());
    let dlg_ok = dlg.clone();
    ok.connect_clicked(move |_| {
        let p = form.collect(&prefs);
        on_apply(&p);
        dlg_ok.close();
    });
    // Escape closes the dialog.
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

    dlg.connect_close_request(|_| glib::Propagation::Proceed);

    dlg.present();
}
