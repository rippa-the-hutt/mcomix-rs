//! Library window, mirroring `mcomix/library/main_dialog.py` + the areas.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gio::prelude::*;
use gtk4 as gtk;
use gtk4::gdk;
use gtk4::prelude::*;

use crate::app::Ui;
use crate::library::{Book, LibraryDb, COLLECTION_ALL};

/// Sentinel collection id for the virtual "Recent" view.
const COLLECTION_RECENT_VIEW: i64 = -2;

struct LibState {
    db: LibraryDb,
    current: i64,
    last_activated: Option<i64>,
    /// path -> picture widget (for cover fills).
    cover_pics: HashMap<String, gtk::Picture>,
}

pub fn show_library(rc: Rc<RefCell<Ui>>) {
    let Ok(db) = LibraryDb::open() else {
        rc.borrow()
            .notice("Could not open the library database.");
        return;
    };
    let state = Rc::new(RefCell::new(LibState {
        db,
        current: COLLECTION_ALL,
        last_activated: None,
        cover_pics: HashMap::new(),
    }));

    let win = gtk::Window::new();
    win.set_title(Some(&crate::i18n::tr("Library")));
    win.set_default_size(980, 640);
    win.set_transient_for(Some(&rc.borrow().window));

    // ---- collections list ----
    let collection_list = gtk::ListBox::new();
    let collection_scroller = gtk::ScrolledWindow::new();
    collection_scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    collection_scroller.set_child(Some(&collection_list));
    collection_scroller.set_size_request(200, -1);
    collection_scroller.set_vexpand(true);
    collection_scroller.set_min_content_height(300);

    let new_collection = gtk::Button::with_label(&crate::i18n::tr("New collection…"));

    let left = gtk::Box::new(gtk::Orientation::Vertical, 4);
    left.append(&collection_scroller);
    left.append(&new_collection);
    left.set_vexpand(true);

    // ---- book grid ----
    let book_flow = gtk::FlowBox::new();
    book_flow.set_max_children_per_line(6);
    book_flow.set_selection_mode(gtk::SelectionMode::Single);
    book_flow.set_activate_on_single_click(true);
    let book_scroller = gtk::ScrolledWindow::new();
    book_scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    book_scroller.set_child(Some(&book_flow));
    book_scroller.set_vexpand(true);
    book_scroller.set_hexpand(true);
    book_scroller.set_min_content_width(480);
    book_scroller.set_min_content_height(360);

    // ---- controls ----
    let add_books = gtk::Button::with_label(&crate::i18n::tr("Add books…"));
    let remove_book = gtk::Button::with_label(&crate::i18n::tr("Remove from library"));
    let watch_dir = gtk::Button::with_label(&crate::i18n::tr("Watch directory…"));
    let scan = gtk::Button::with_label(&crate::i18n::tr("Scan for new books"));
    let status = gtk::Label::new(Some(""));
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    controls.set_margin_top(6);
    controls.set_margin_bottom(6);
    controls.set_margin_start(6);
    controls.set_margin_end(6);
    controls.append(&add_books);
    controls.append(&remove_book);
    controls.append(&watch_dir);
    controls.append(&scan);
    controls.append(&status);

    let main = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    main.append(&left);
    main.append(&book_scroller);
    main.set_hexpand(true);
    main.set_vexpand(true);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vbox.append(&main);
    vbox.append(&controls);
    win.set_child(Some(&vbox));

    // ---- helpers ----
    fn refresh_collections(state: &Rc<RefCell<LibState>>, list: &gtk::ListBox) {
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let collections = {
            let mut s = state.borrow_mut();
            let cols = s.db.get_collections();
            cols.clone()
        };
        let add_row = |list: &gtk::ListBox, name: &str, id: i64| {
            let row = gtk::Label::new(Some(name));
            row.set_xalign(0.0);
            row.set_margin_top(4);
            row.set_margin_bottom(4);
            row.set_margin_start(8);
            row.set_margin_end(8);
            list.append(&row);
            // store id on the row via the child widget's data
            let _ = id;
        };
        add_row(list, &crate::i18n::tr("All books"), COLLECTION_ALL);
        add_row(list, &crate::i18n::tr("Recent"), COLLECTION_RECENT_VIEW);
        for c in &collections {
            add_row(list, &c.name, c.id);
        }
    }

    fn current_books(state: &Rc<RefCell<LibState>>, filter: Option<&str>) -> Vec<Book> {
        let mut s = state.borrow_mut();
        match s.current {
            COLLECTION_RECENT_VIEW => s.db.get_recent_books(100),
            COLLECTION_ALL => s.db.get_books_in_collection(None, filter),
            c => s.db.get_books_in_collection(Some(c), filter),
        }
    }

    // Refresh the book grid (and kick off cover generation).
    fn refresh_books(state: &Rc<RefCell<LibState>>, flow: &gtk::FlowBox, win: &gtk::Window) {
        while let Some(child) = flow.first_child() {
            flow.remove(&child);
        }
        let books = current_books(state, None);
        {
            let mut s = state.borrow_mut();
            s.cover_pics.clear();
        }

        // (cover tx, rx) for background generation.
        let (cover_tx, cover_rx) = std::sync::mpsc::channel::<(String, u32, u32, Vec<u8>)>();

        for b in &books {
            let cell = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let pic = gtk::Picture::new();
            pic.set_can_shrink(true);
            pic.set_content_fit(gtk::ContentFit::Contain);
            pic.set_size_request(120, 160);
            cell.append(&pic);
            let name = gtk::Label::new(Some(&b.name));
            name.set_max_width_chars(16);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            cell.append(&name);
            cell.set_tooltip_text(Some(&b.path));
            flow.append(&cell);
            state.borrow_mut().cover_pics.insert(b.path.clone(), pic);
        }

        let paths: Vec<String> = books.iter().map(|b| b.path.clone()).collect();
        if paths.is_empty() {
            return;
        }
        std::thread::spawn(move || {
            for path in paths {
                let thumb = crate::thumb_cache::load(std::path::Path::new(&path), 0)
                    .or_else(|| {
                        crate::archive::open(std::path::Path::new(&path))
                            .ok()
                            .and_then(|mut ar| {
                                ar.page_names()
                                    .ok()
                                    .and_then(|pages| pages.first().cloned())
                                    .and_then(|name| ar.read(&name).ok())
                            })
                            .and_then(|bytes| {
                                image_loader_thumb(&bytes)
                            })
                    });
                if let Some((w, h, rgba)) = thumb {
                    crate::thumb_cache::store(std::path::Path::new(&path), 0, w, h, &rgba);
                    if cover_tx.send((path, w, h, rgba)).is_err() {
                        break;
                    }
                }
            }
        });

        // Poller fills pictures; stops when the window closes.
        let state = state.clone();
        let win = win.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
            if !win.is_visible() {
                return glib::ControlFlow::Break;
            }
            let mut s = state.borrow_mut();
            while let Ok((path, w, h, rgba)) = cover_rx.try_recv() {
                if let Some(pic) = s.cover_pics.get(&path) {
                    if let Some(img) = image::RgbaImage::from_raw(w, h, rgba) {
                        let tex = crate::image_loader::texture_from_rgba(&img);
                        pic.set_paintable(Some(&tex));
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    fn image_loader_thumb(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        crate::image_loader::thumbnail_pixbuf_rgba(bytes, 120, 160)
            .or_else(|| crate::image_loader::thumbnail_rgba_fallback(bytes, 120, 160))
    }

    // Wire listbox selection -> refresh books.
    collection_list.connect_row_selected({
        let state = state.clone();
        let book_flow = book_flow.clone();
        let win = win.clone();
        move |_list, row| {
            let Some(row) = row else { return };
            let name = row.child().and_then(|c| c.downcast::<gtk::Label>().ok()).map(|l| l.text().to_string()).unwrap_or_default();
            let id = match name.as_str() {
                n if n == crate::i18n::tr("All books") => COLLECTION_ALL,
                n if n == crate::i18n::tr("Recent") => COLLECTION_RECENT_VIEW,
                _ => {
                    let s = state.borrow();
                    s.db.get_collections().iter().find(|c| c.name == name).map(|c| c.id).unwrap_or(COLLECTION_ALL)
                }
            };
            state.borrow_mut().current = id;
            refresh_books(&state, &book_flow, &win);
        }
    });

    // Book click -> open in the main window.
    book_flow.connect_child_activated({
        let state = state.clone();
        let rc = rc.clone();
        let win = win.clone();
        move |_flow, child| {
            let path = child
                .child()
                .and_then(|c| c.downcast::<gtk::Box>().ok())
                .and_then(|b| b.tooltip_text())
                .map(|t| t.to_string());
            let Some(path) = path else { return };
            let mut s = state.borrow_mut();
            let id = s.db.get_book_id_by_path(&path);
            let page = id.and_then(|i| s.db.get_recent_page(i)).unwrap_or(1);
            s.last_activated = id;
            drop(s);
            let mut ui = rc.borrow_mut();
            // Route through the generic open so the resume prompt appears for
            // previously-read books (as with the Open dialog / CLI).
            ui.open_path(std::path::PathBuf::from(&path), rc.clone());
            drop(ui);
            // Close the library once the comic is opened.
            win.close();
        }
    });

    // Add books.
    add_books.connect_clicked({
        let state = state.clone();
        let book_flow = book_flow.clone();
        let win = win.clone();
        let status = status.clone();
        move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Add books to the library")
                .modal(true)
                .build();
            let state = state.clone();
            let book_flow = book_flow.clone();
            let win2 = win.clone();
            let status = status.clone();
            dialog.open_multiple(Some(&win), None::<&gio::Cancellable>, move |res| {
                let Ok(files) = res else { return };
                let mut added = 0usize;
                {
                    let mut s = state.borrow_mut();
                    let collection = if s.current >= 0 { Some(s.current) } else { None };
                    for i in 0..files.n_items() {
                        let item = files.item(i);
                        let Some(f) = item.and_then(|it| it.downcast::<gio::File>().ok()) else {
                            continue;
                        };
                        if let Some(p) = f.path() {
                            if s.db.add_book(&p.to_string_lossy(), collection).is_some() {
                                added += 1;
                            }
                        }
                    }
                }
                status.set_text(&format!("Added {added} book(s)."));
                refresh_books(&state, &book_flow, &win2);
            });
        }
    });

    // Remove the last-activated book.
    remove_book.connect_clicked({
        let state = state.clone();
        let book_flow = book_flow.clone();
        let win = win.clone();
        let status = status.clone();
        move |_| {
            let mut s = state.borrow_mut();
            if let Some(id) = s.last_activated.take() {
                s.db.remove_book(id);
                status.set_text("Book removed from the library.");
            } else {
                status.set_text("No book selected.");
            }
            drop(s);
            refresh_books(&state, &book_flow, &win);
        }
    });

    // Watch a directory.
    watch_dir.connect_clicked({
        let state = state.clone();
        let status = status.clone();
        let win = win.clone();
        move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Watch a directory for new comics")
                .modal(true)
                .build();
            let state = state.clone();
            let status = status.clone();
            dialog.select_folder(Some(&win), None::<&gio::Cancellable>, move |res| {
                if let Ok(f) = res {
                    if let Some(dir) = f.path() {
                        state.borrow_mut().db.watchlist_add(&dir.to_string_lossy(), true, None);
                        status.set_text(&format!("Watching '{}'.", dir.display()));
                    }
                }
            });
        }
    });

    // Scan watched directories.
    scan.connect_clicked({
        let state = state.clone();
        let book_flow = book_flow.clone();
        let win = win.clone();
        let status = status.clone();
        move |_| {
            let mut s = state.borrow_mut();
            let new_files = s.db.scan_watchlist();
            let mut added = 0usize;
            for (path, collection) in new_files {
                if s.db.add_book(&path, collection).is_some() {
                    added += 1;
                }
            }
            status.set_text(&format!("Added {added} new book(s)."));
            drop(s);
            refresh_books(&state, &book_flow, &win);
        }
    });

    // New collection.
    new_collection.connect_clicked({
        let state = state.clone();
        let collection_list = collection_list.clone();
        let win = win.clone();
        move |_| {
            let dlg = gtk::Window::new();
            dlg.set_title(Some(&crate::i18n::tr("New collection")));
            dlg.set_transient_for(Some(&win));
            dlg.set_modal(true);
            dlg.set_resizable(false);
            let entry = gtk::Entry::new();
            entry.set_placeholder_text(Some("Collection name"));
            let ok = gtk::Button::with_label(&crate::i18n::tr("Create"));
            let cancel = gtk::Button::with_label(&crate::i18n::tr("Cancel"));
            let boxv = gtk::Box::new(gtk::Orientation::Vertical, 8);
            boxv.set_margin_top(12);
            boxv.set_margin_bottom(12);
            boxv.set_margin_start(12);
            boxv.set_margin_end(12);
            boxv.append(&entry);
            let h = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            h.set_halign(gtk::Align::End);
            h.append(&cancel);
            h.append(&ok);
            boxv.append(&h);
            dlg.set_child(Some(&boxv));
            let state = state.clone();
            let collection_list = collection_list.clone();
            let dlg2 = dlg.clone();
            ok.connect_clicked(move |_| {
                let name = entry.text().to_string();
                if !name.is_empty() {
                    let mut s = state.borrow_mut();
                    s.db.add_collection(&name);
                }
                refresh_collections(&state, &collection_list);
                dlg2.close();
            });
            let dlg3 = dlg.clone();
            cancel.connect_clicked(move |_| dlg3.close());
            dlg.connect_close_request(|_| glib::Propagation::Proceed);
            dlg.present();
        }
    });

    refresh_collections(&state, &collection_list);
    refresh_books(&state, &book_flow, &win);

    // Escape closes the library window.
    let esc = gtk::EventControllerKey::new();
    esc.set_propagation_phase(gtk::PropagationPhase::Capture);
    let win_esc = win.clone();
    esc.connect_key_pressed(move |_c, keyval, _code, _state| {
        if keyval == gdk::Key::Escape {
            win_esc.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    win.add_controller(esc);

    win.connect_close_request(|_| glib::Propagation::Proceed);
    win.present();
}
