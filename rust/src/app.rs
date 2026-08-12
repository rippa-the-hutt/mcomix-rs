//! Main window and application logic (Rust port of `mcomix/main.py`).
//!
//! Milestone-1 UI: toolbar, scrollable viewer with fit/zoom modes, double-page
//! + manga mode, thumbnails sidebar, statusbar, slideshow, fullscreen,
//! preferences and last-read-page persistence.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gio::prelude::*;
use gtk4 as gtk;
use gtk4::gdk;
use gtk4::prelude::*;
use log::{info, warn};

use crate::archive::{self, Archive};
use crate::image_loader;
use crate::lastread::LastReadDb;
use crate::prefs::Prefs;
use crate::zoom::{self, ZoomModel};

/// Where to position the viewport after a page change.
#[derive(Clone, Copy, PartialEq)]
enum ScrollDest {
    /// Top of the page (used when moving forward).
    Start,
    /// Bottom of the page (used when moving backward).
    End,
    /// Keep the current viewport position (transforms, toggles).
    Keep,
}

/// Command for the background page-decoding worker.
struct PageCmd {
    /// Display requests only: incremented per display push; results with an
    /// older id are dropped.
    req: u64,
    /// Archive generation (bumped when a new file is opened).
    gen: u64,
    /// Prefetch generation (bumped when transforms invalidate the cache).
    pgen: u64,
    /// True for display requests, false for background prefetch.
    display: bool,
    path: PathBuf,
    pages: Vec<String>,
    /// Page indices to decode.
    indices: Vec<usize>,
    rotation: i32,
    flip_h: bool,
    flip_v: bool,
}

/// One decoded page, ready to be turned into a texture on the main thread.
struct DecodedPage {
    idx: usize,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

struct PageResult {
    req: u64,
    gen: u64,
    pgen: u64,
    display: bool,
    pages: Vec<DecodedPage>,
}

pub struct AppState {
    pub archive: Option<Box<dyn Archive>>,
    pub path: Option<PathBuf>,
    pub pages: Vec<String>,
    pub page: usize,
    pub double_page: bool,
    pub manga: bool,
    pub slideshow: bool,
    pub slideshow_source: Option<glib::SourceId>,
    pub rotation: i32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub zoom: ZoomModel,
    pub prefs: Prefs,
    /// Textures of the (up to two) visible pages.
    pub textures: Vec<Option<gdk::Texture>>,
    /// Full-resolution sizes of the visible pages (after rotation).
    pub sizes: Vec<(u32, u32)>,
    /// Generation counter for the thumbnail worker; stale results are dropped.
    pub thumb_gen: u64,
    /// Sender for thumbnail pixels produced by the worker thread
    /// (generation, page index, width, height, tight RGBA8 data).
    pub thumb_tx: Option<std::sync::mpsc::Sender<(u64, usize, u32, u32, Vec<u8>)>>,
    /// Page index -> thumbnail cell. Cells are pre-created in page order so
    /// the sidebar is stable while thumbnails load; images fill in via
    /// `thumb_pics`.
    pub thumb_cells: Vec<Option<gtk::Box>>,
    /// Page index -> thumbnail picture widget (filled as thumbnails arrive).
    pub thumb_pics: Vec<Option<gtk::Picture>>,
    /// Last viewport size we laid out for (avoids redraw loops).
    pub last_viewport: Option<(i32, i32)>,
    /// Visibility of chrome elements before fullscreen hid them.
    pub saved_toolbar: bool,
    pub saved_status: bool,
    pub saved_thumbs: bool,
    /// True while fullscreen has hidden the toolbar/statusbar/thumbnails.
    pub chrome_hidden_by_fullscreen: bool,
    /// Size of the content laid out by the last redraw (used to scroll to the
    /// bottom of a freshly drawn page before GTK has updated the adjustments).
    pub last_content: (i32, i32),
    /// Background page-decoder channel (worker decodes off the UI thread).
    pub page_loader_tx: Option<std::sync::mpsc::Sender<PageCmd>>,
    /// Monotonic request id; results with an older id are dropped.
    pub page_req: u64,
    /// Bumped when a new archive is opened; invalidates in-flight decodes.
    pub page_gen: u64,
    /// True while a decode request is in flight (used to coalesce navigation).
    pub page_loading: bool,
    /// Coalesced newest request while a decode is still running.
    pub page_pending: Option<Vec<usize>>,
    /// Scroll destination to apply when the pending page actually displays.
    pub page_dest: Option<ScrollDest>,
    /// Last time the last-read position was persisted (throttled).
    pub last_save: Option<std::time::Instant>,
    /// LRU cache of decoded page textures (mirrors `max pages to cache`).
    pub cache: crate::lru::LruCache<usize, (gdk::Texture, u32, u32)>,
    /// Pages to prefetch in the background, in priority order.
    pub prefetch_queue: std::collections::VecDeque<usize>,
    /// True while a prefetch decode is in flight.
    pub prefetching: bool,
    /// Bumped when transforms invalidate the cache (stale prefetches dropped).
    pub prefetch_gen: u64,
    /// Configurable keyboard bindings.
    pub bindings: crate::keybindings::BindingMap,
}

impl Default for AppState {
    fn default() -> Self {
        let prefs = Prefs::load();
        let cache_capacity = prefs.max_pages_to_cache.max(1) as usize;
        AppState {
            archive: None,
            path: None,
            pages: Vec::new(),
            page: 0,
            double_page: prefs.default_double_page,
            manga: prefs.default_manga_mode,
            slideshow: false,
            slideshow_source: None,
            rotation: prefs.rotation,
            flip_h: false,
            flip_v: false,
            zoom: ZoomModel {
                fit_mode: prefs.zoom_mode,
                scale_up: prefs.scale_up,
                ..Default::default()
            },
            prefs,
            textures: vec![None, None],
            sizes: vec![(0, 0), (0, 0)],
            thumb_gen: 0,
            thumb_tx: None,
            last_viewport: None,
            saved_toolbar: true,
            saved_status: true,
            saved_thumbs: true,
            chrome_hidden_by_fullscreen: false,
            last_content: (1, 1),
            page_loader_tx: None,
            page_req: 0,
            page_gen: 0,
            page_loading: false,
            page_pending: None,
            page_dest: None,
            last_save: None,
            cache: crate::lru::LruCache::new(
                cache_capacity,
                384 * 1024 * 1024, // ~384 MB of decoded pages
            ),
            prefetch_queue: std::collections::VecDeque::new(),
            prefetching: false,
            prefetch_gen: 0,
            bindings: crate::keybindings::BindingMap::load(),
            thumb_cells: Vec::new(),
            thumb_pics: Vec::new(),
        }
    }
}

pub struct Ui {
    pub window: gtk::ApplicationWindow,
    pub scrolled: gtk::ScrolledWindow,
    pub content: gtk::Box,
    pub pics: Vec<gtk::Picture>,
    pub placeholder: gtk::Label,
    pub thumb_scroll: gtk::ScrolledWindow,
    pub thumb_box: gtk::FlowBox,
    pub status: gtk::Label,
    pub toolbar: gtk::Box,
    pub zoom_dropdown: gtk::DropDown,
    pub btn_open: gtk::Button,
    pub btn_prev: gtk::Button,
    pub btn_next: gtk::Button,
    pub btn_zoom_out: gtk::Button,
    pub btn_zoom_in: gtk::Button,
    pub btn_zoom_orig: gtk::Button,
    pub btn_rotate: gtk::Button,
    pub btn_library: gtk::Button,
    pub btn_prefs: gtk::Button,
    pub btn_double: gtk::ToggleButton,
    pub btn_manga: gtk::ToggleButton,
    pub btn_slideshow: gtk::ToggleButton,
    pub btn_fullscreen: gtk::ToggleButton,
    pub btn_thumbs: gtk::ToggleButton,
    /// Set while `sync_toggles` is updating button states programmatically, so
    /// `toggled` handlers can bail out without re-borrowing the RefCell.
    pub suppress_toggles: Rc<std::cell::Cell<bool>>,
    pub state: AppState,
}

impl Ui {
    pub fn new(
        app: &gtk::Application,
        open_path: Option<PathBuf>,
        start_page: u32,
    ) -> Rc<RefCell<Ui>> {
        let state = AppState::default();

        let window = gtk::ApplicationWindow::new(app);
        window.set_title(Some("MComix3"));
        window.set_default_size(state.prefs.window_width, state.prefs.window_height);

        // ---- toolbar ----
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        toolbar.set_margin_top(2);
        toolbar.set_margin_bottom(2);
        toolbar.set_margin_start(4);
        toolbar.set_margin_end(4);

        let btn_open = gtk::Button::from_icon_name("document-open");
        btn_open.set_tooltip_text(Some("Open"));
        toolbar.append(&btn_open);

        let btn_prev = gtk::Button::from_icon_name("go-previous");
        btn_prev.set_tooltip_text(Some("Previous page"));
        toolbar.append(&btn_prev);

        let btn_next = gtk::Button::from_icon_name("go-next");
        btn_next.set_tooltip_text(Some("Next page"));
        toolbar.append(&btn_next);

        let sep1 = gtk::Separator::new(gtk::Orientation::Vertical);
        toolbar.append(&sep1);

        let zoom_dropdown = gtk::DropDown::from_strings(&[
            "Best fit", "Fit width", "Fit height", "Fit size", "Manual",
        ]);
        zoom_dropdown.set_selected(state.zoom.fit_mode as u32);
        zoom_dropdown.set_tooltip_text(Some("Zoom mode"));
        toolbar.append(&zoom_dropdown);

        let btn_zoom_out = gtk::Button::from_icon_name("zoom-out");
        btn_zoom_out.set_tooltip_text(Some("Zoom out"));
        toolbar.append(&btn_zoom_out);

        let btn_zoom_in = gtk::Button::from_icon_name("zoom-in");
        btn_zoom_in.set_tooltip_text(Some("Zoom in"));
        toolbar.append(&btn_zoom_in);

        let btn_zoom_orig = gtk::Button::from_icon_name("zoom-original");
        btn_zoom_orig.set_tooltip_text(Some("Normal size"));
        toolbar.append(&btn_zoom_orig);

        let btn_rotate = gtk::Button::from_icon_name("object-rotate-right");
        btn_rotate.set_tooltip_text(Some("Rotate 90° clockwise"));
        toolbar.append(&btn_rotate);

        let sep2 = gtk::Separator::new(gtk::Orientation::Vertical);
        toolbar.append(&sep2);

        let btn_double = gtk::ToggleButton::with_label("Two pages");
        btn_double.set_tooltip_text(Some("Double page mode"));
        btn_double.set_active(state.double_page);
        toolbar.append(&btn_double);

        let btn_manga = gtk::ToggleButton::with_label("Manga");
        btn_manga.set_tooltip_text(Some("Manga (right-to-left) mode"));
        btn_manga.set_active(state.manga);
        toolbar.append(&btn_manga);

        let btn_slideshow = gtk::ToggleButton::new();
        btn_slideshow.set_icon_name("media-playback-start");
        btn_slideshow.set_tooltip_text(Some("Slideshow"));
        toolbar.append(&btn_slideshow);

        let btn_fullscreen = gtk::ToggleButton::new();
        btn_fullscreen.set_icon_name("view-fullscreen");
        btn_fullscreen.set_tooltip_text(Some("Fullscreen"));
        toolbar.append(&btn_fullscreen);

        let btn_thumbs = gtk::ToggleButton::new();
        btn_thumbs.set_icon_name("view-list-symbolic");
        btn_thumbs.set_tooltip_text(Some("Thumbnails"));
        btn_thumbs.set_active(state.prefs.show_thumbnails);
        toolbar.append(&btn_thumbs);

        let sep3 = gtk::Separator::new(gtk::Orientation::Vertical);
        toolbar.append(&sep3);

        let btn_library = gtk::Button::from_icon_name("folder");
        btn_library.set_tooltip_text(Some("Library (not ported yet)"));
        toolbar.append(&btn_library);

        let btn_prefs = gtk::Button::from_icon_name("preferences-system");
        btn_prefs.set_tooltip_text(Some("Preferences (not ported yet)"));
        toolbar.append(&btn_prefs);

        // Clicking toolbar buttons must not move keyboard focus away from the
        // viewer (so arrow/Page keys keep working after mouse usage).
        let mut child = toolbar.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            if let Ok(btn) = c.downcast::<gtk::Button>() {
                btn.set_focus_on_click(false);
            }
        }

        // ---- viewer ----
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        content.set_css_classes(&["viewer"]);
        content.set_focusable(true);
        content.set_halign(gtk::Align::Center);
        content.set_valign(gtk::Align::Center);
        content.set_hexpand(true);
        content.set_vexpand(true);

        let placeholder = gtk::Label::new(Some(
            "Open a comic book archive (CBZ, CBR, CB7, CBT, PDF) or an image directory.",
        ));
        placeholder.set_css_classes(&["placeholder"]);

        let pics = vec![gtk::Picture::new(), gtk::Picture::new()];
        for pic in &pics {
            pic.set_can_shrink(true);
            pic.set_content_fit(gtk::ContentFit::Contain);
            pic.set_halign(gtk::Align::Center);
            pic.set_valign(gtk::Align::Center);
        }

        let scrolled = gtk::ScrolledWindow::new();
        let policy = if state.prefs.show_scrollbar {
            gtk::PolicyType::Automatic
        } else {
            gtk::PolicyType::Never
        };
        scrolled.set_policy(policy, policy);
        scrolled.set_kinetic_scrolling(true);
        scrolled.set_child(Some(&content));
        scrolled.set_css_classes(&["viewer"]);

        // ---- thumbnails ----
        let thumb_box = gtk::FlowBox::new();
        thumb_box.set_max_children_per_line(1);
        thumb_box.set_selection_mode(gtk::SelectionMode::Single);
        thumb_box.set_activate_on_single_click(true);
        thumb_box.set_homogeneous(true);
        thumb_box.set_css_classes(&["thumbview"]);

        let thumb_scroll = gtk::ScrolledWindow::new();
        thumb_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        thumb_scroll.set_child(Some(&thumb_box));
        thumb_scroll.set_size_request(180, -1);
        thumb_scroll.set_css_classes(&["thumbview"]);

        // ---- paned ----
        let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        paned.set_start_child(Some(&thumb_scroll));
        paned.set_end_child(Some(&scrolled));
        paned.set_position(180);

        // ---- statusbar ----
        let status = gtk::Label::new(Some(""));
        status.set_xalign(0.0);
        status.set_margin_start(6);
        status.set_margin_top(2);
        status.set_margin_bottom(2);
        status.set_hexpand(true);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);

        let statusbar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        statusbar.append(&status);

        // ---- layout ----
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        vbox.append(&toolbar);
        vbox.append(&paned);
        vbox.append(&statusbar);
        window.set_child(Some(&vbox));

        apply_background_css(&state.prefs);

        let ui = Rc::new(RefCell::new(Ui {
            window,
            scrolled,
            content,
            pics,
            placeholder,
            thumb_scroll,
            thumb_box,
            status,
            toolbar,
            zoom_dropdown,
            btn_open,
            btn_prev,
            btn_next,
            btn_zoom_out,
            btn_zoom_in,
            btn_zoom_orig,
            btn_rotate,
            btn_library,
            btn_prefs,
            btn_double,
            btn_manga,
            btn_slideshow,
            btn_fullscreen,
            btn_thumbs,
            suppress_toggles: Rc::new(std::cell::Cell::new(false)),
            state,
        }));

        ui.borrow().connect_signals(ui.clone());

        // Persistent thumbnail channel + main-loop poller.
        // Background page-decoder channel + worker, created once for the app
        // lifetime (the worker re-opens the archive when the path changes).
        {
            let (thumb_tx, thumb_rx) = std::sync::mpsc::channel::<(u64, usize, u32, u32, Vec<u8>)>();
            ui.borrow_mut().state.thumb_tx = Some(thumb_tx);

            let (page_tx, page_cmd_rx) = std::sync::mpsc::channel::<PageCmd>();
            let (page_res_tx, page_res_rx) = std::sync::mpsc::channel::<PageResult>();
            ui.borrow_mut().state.page_loader_tx = Some(page_tx);

            std::thread::spawn(move || {
                let mut ar: Option<Box<dyn Archive>> = None;
                let mut ar_path: Option<PathBuf> = None;
                while let Ok(cmd) = page_cmd_rx.recv() {
                    if ar_path.as_deref() != Some(cmd.path.as_path()) {
                        ar = archive::open(&cmd.path).ok();
                        ar_path = Some(cmd.path.clone());
                        if ar.is_none() {
                            warn!(
                                "page worker cannot open {}",
                                cmd.path.display()
                            );
                        }
                    }
                    let mut out = Vec::new();
                    if let Some(ar) = ar.as_mut() {
                        for &idx in &cmd.indices {
                            if let Some(name) = cmd.pages.get(idx) {
                                if let Ok(bytes) = ar.read(name) {
                                    if let Ok(img) = image_loader::decode_rgba(&bytes) {
                                        let img = image_loader::transform(
                                            &img,
                                            cmd.rotation,
                                            cmd.flip_h,
                                            cmd.flip_v,
                                        );
                                        let (w, h) = img.dimensions();
                                        out.push(DecodedPage {
                                            idx,
                                            w,
                                            h,
                                            rgba: img.into_raw(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    if page_res_tx
                        .send(PageResult {
                            req: cmd.req,
                            gen: cmd.gen,
                            pgen: cmd.pgen,
                            display: cmd.display,
                            pages: out,
                        })
                        .is_err()
                    {
                        break; // main thread is gone
                    }
                }
            });

            let r = ui.clone();
            glib::timeout_add_local(Duration::from_millis(30), move || {
                let mut u = r.borrow_mut();
                while let Ok((g, idx, w, h, rgba)) = thumb_rx.try_recv() {
                    if g != u.state.thumb_gen {
                        continue;
                    }
                    if let Some(img) = image::RgbaImage::from_raw(w, h, rgba) {
                        let tex = image_loader::texture_from_rgba(&img);
                        u.add_thumb(idx, &tex);
                    }
                }
                while let Ok(res) = page_res_rx.try_recv() {
                    u.apply_page_result(res);
                }
                glib::ControlFlow::Continue
            });
        }

        // Initial file.
        {
            let mut u = ui.borrow_mut();
            let prefs = u.state.prefs.clone();
            let path = open_path.or_else(|| {
                if prefs.auto_load_last_file && !prefs.path_to_last_file.is_empty() {
                    let p = PathBuf::from(&prefs.path_to_last_file);
                    if p.exists() {
                        Some(p)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            if let Some(p) = path {
                let page = if prefs.auto_load_last_file {
                    prefs.page_of_last_file
                } else {
                    start_page.max(1)
                };
                u.open_path_with_page(p, page);
            } else {
                u.redraw();
            }
        }

        ui
    }

    /// Wire up all signals. Called once after the widget tree exists.
    fn connect_signals(&self, rc: Rc<RefCell<Ui>>) {
        // ---- toolbar buttons ----
        self.btn_open.connect_clicked({
            let r = rc.clone();
            move |_| {
                let ui = r.borrow();
                ui.open_dialog(r.clone());
            }
        });
        self.btn_prev.connect_clicked({
            let r = rc.clone();
            move |_| r.borrow_mut().next_page(-1)
        });
        self.btn_next.connect_clicked({
            let r = rc.clone();
            move |_| r.borrow_mut().next_page(1)
        });
        self.btn_zoom_out.connect_clicked({
            let r = rc.clone();
            move |_| r.borrow_mut().zoom_out()
        });
        self.btn_zoom_in.connect_clicked({
            let r = rc.clone();
            move |_| r.borrow_mut().zoom_in()
        });
        self.btn_zoom_orig.connect_clicked({
            let r = rc.clone();
            move |_| r.borrow_mut().zoom_original()
        });
        self.btn_rotate.connect_clicked({
            let r = rc.clone();
            move |_| r.borrow_mut().rotate_90()
        });
        self.btn_library.connect_clicked({
            let r = rc.clone();
            move |_| {
                let ui = r.borrow();
                ui.notice("The library has not been ported yet; it is planned for a later milestone.");
            }
        });
        self.btn_prefs.connect_clicked({
            let r = rc.clone();
            move |_| {
                let ui = r.borrow();
                let prefs = ui.state.prefs.clone();
                let window = ui.window.clone();
                drop(ui);
                let rc2 = r.clone();
                crate::prefs_dialog::show_dialog(
                    &window,
                    &prefs,
                    Rc::new(move |p: &crate::prefs::Prefs| {
                        let mut ui = rc2.borrow_mut();
                        ui.apply_prefs(p);
                        ui.state.bindings = crate::keybindings::BindingMap::load();
                    }),
                );
            }
        });

        // ---- zoom dropdown ----
        self.zoom_dropdown.connect_selected_notify({
            let r = rc.clone();
            move |dd| {
                let mut ui = r.borrow_mut();
                ui.set_fit_mode(dd.selected() as i32);
            }
        });

        // ---- toggles ----
        let suppress = self.suppress_toggles.clone();
        self.btn_double.connect_toggled({
            let r = rc.clone();
            let suppress = suppress.clone();
            move |b| {
                if suppress.get() {
                    return;
                }
                let mut ui = r.borrow_mut();
                if b.is_active() != ui.state.double_page {
                    ui.toggle_double_page();
                }
            }
        });
        let suppress = self.suppress_toggles.clone();
        self.btn_manga.connect_toggled({
            let r = rc.clone();
            let suppress = suppress.clone();
            move |b| {
                if suppress.get() {
                    return;
                }
                let mut ui = r.borrow_mut();
                if b.is_active() != ui.state.manga {
                    ui.toggle_manga();
                }
            }
        });
        let suppress = self.suppress_toggles.clone();
        self.btn_slideshow.connect_toggled({
            let r = rc.clone();
            let suppress = suppress.clone();
            move |b| {
                if suppress.get() {
                    return;
                }
                let mut ui = r.borrow_mut();
                if b.is_active() != ui.state.slideshow {
                    ui.toggle_slideshow(r.clone());
                }
            }
        });
        let suppress = self.suppress_toggles.clone();
        self.btn_fullscreen.connect_toggled({
            let r = rc.clone();
            let suppress = suppress.clone();
            move |b| {
                if suppress.get() {
                    return;
                }
                let mut ui = r.borrow_mut();
                let is_fs = ui.window.is_fullscreen();
                if b.is_active() != is_fs {
                    ui.toggle_fullscreen();
                }
            }
        });
        let suppress = self.suppress_toggles.clone();
        self.btn_thumbs.connect_toggled({
            let r = rc.clone();
            let suppress = suppress.clone();
            move |b| {
                if suppress.get() {
                    return;
                }
                let mut ui = r.borrow_mut();
                if b.is_active() != ui.thumb_scroll.is_visible() {
                    ui.toggle_thumbnails();
                }
            }
        });

        // ---- thumbnail activation ----
        self.thumb_box.connect_child_activated({
            let r = rc.clone();
            move |_fb, child| {
                let mut ui = r.borrow_mut();
                if let Some(idx) = ui.page_of_child(&child) {
                    ui.goto_index(idx, ScrollDest::Start);
                }
            }
        });

        // ---- keyboard (configurable bindings) ----
        // Capture phase: handle keys before any focused child widget can
        // consume them, so navigation always works.
        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        let r = rc.clone();
        key_ctrl.connect_key_pressed(move |_c, keyval, _code, state| {
            let mut ui = r.borrow_mut();
            let action = ui.state.bindings.lookup(keyval, state);
            log::debug!("key event {:?} -> {:?}", keyval.name(), action);
            if let Some(action) = action {
                ui.handle_action(action, r.clone());
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.window.add_controller(key_ctrl);

        // ---- resize refit (polled; gtk4-rs does not expose size-allocate) ----
        {
            let r = rc.clone();
            glib::timeout_add_local(Duration::from_millis(100), move || {
                r.borrow_mut().schedule_redraw();
                glib::ControlFlow::Continue
            });
        }

        // ---- mouse wheel ----
        let scroll_ctrl =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll_ctrl.connect_scroll({
            let r = rc.clone();
            move |_c, _dx, dy| {
                let mut ui = r.borrow_mut();
                if dy > 0.0 {
                    ui.scroll_down();
                } else if dy < 0.0 {
                    ui.scroll_up();
                }
                glib::Propagation::Proceed
            }
        });
        self.scrolled.add_controller(scroll_ctrl);

        // ---- fullscreen chrome handling (deferred to avoid re-borrow) ----
        self.window.connect_fullscreened_notify({
            let r = rc.clone();
            move |_w| {
                let r2 = r.clone();
                glib::idle_add_local_once(move || {
                    r2.borrow_mut().on_fullscreen_changed();
                });
            }
        });

        // ---- mouse: grab-to-pan + click-to-advance ----
        // Mirrors the Python port (`event.py` mouse_press/mouse_move/
        // mouse_release): while the left button is held, the page follows the
        // cursor 1:1 (incremental delta) with a "grabbing" cursor; a
        // press+release without movement is a click and advances the page.
        let dragging: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));
        let pointer_inside: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(true));
        let last_pos: Rc<std::cell::Cell<(f64, f64)>> =
            Rc::new(std::cell::Cell::new((0.0, 0.0)));
        // Total pointer travel during the press, measured in the stable
        // scrolled-window coordinate space (the page pans 1:1, so coordinates
        // relative to the *content* don't change — that's why GestureClick's
        // own press/release positions can't be used to detect a drag).
        let total_travel: Rc<std::cell::Cell<f64>> = Rc::new(std::cell::Cell::new(0.0));
        let grab_cursor = gdk::Cursor::from_name("grabbing", None);
        let scrolled = self.scrolled.clone();

        let click = gtk::GestureClick::new();
        click.set_button(1);
        {
            let dragging = dragging.clone();
            let pointer_inside = pointer_inside.clone();
            let last_pos = last_pos.clone();
            let scrolled = scrolled.clone();
            let grab_cursor = grab_cursor.clone();
            let total_travel = total_travel.clone();
            click.connect_pressed(move |_g, _n, x, y| {
                dragging.set(true);
                pointer_inside.set(true);
                last_pos.set((x, y));
                total_travel.set(0.0);
                scrolled.set_cursor(grab_cursor.as_ref());
            });
        }
        {
            let dragging = dragging.clone();
            let total_travel = total_travel.clone();
            let r = rc.clone();
            let scrolled = scrolled.clone();
            click.connect_released(move |_g, _n, _x, _y| {
                dragging.set(false);
                scrolled.set_cursor(None);
                // Click (no meaningful drag) = advance, like the Python version.
                if total_travel.get() < 4.0 {
                    r.borrow_mut().next_page(1);
                }
            });
        }
        self.scrolled.add_controller(click);

        let motion = gtk::EventControllerMotion::new();
        {
            let dragging_motion = dragging.clone();
            let inside_motion = pointer_inside.clone();
            let last_pos_motion = last_pos.clone();
            let travel_motion = total_travel.clone();
            let r_motion = rc.clone();
            motion.connect_motion(move |_c, x, y| {
                if !dragging_motion.get() {
                    return;
                }
                let (px, py) = last_pos_motion.get();
                if !inside_motion.replace(true) {
                    // Pointer just re-entered mid-drag: avoid a jump.
                    last_pos_motion.set((x, y));
                    return;
                }
                travel_motion.set(travel_motion.get() + (x - px).abs() + (y - py).abs());
                let mut ui = r_motion.borrow_mut();
                let hadj = ui.scrolled.hadjustment();
                let vadj = ui.scrolled.vadjustment();
                // Incremental delta, exactly like Python's `scroll(last - current)`.
                hadj.set_value(hadj.value() + (px - x));
                vadj.set_value(vadj.value() + (py - y));
                last_pos_motion.set((x, y));
            });
            let inside_leave = pointer_inside.clone();
            motion.connect_leave(move |_c| {
                inside_leave.set(false);
            });
        }
        self.scrolled.add_controller(motion);

        // ---- save on close ----
        self.window.connect_close_request({
            let r = rc.clone();
            move |_w| {
                let mut ui = r.borrow_mut();
                ui.save_on_exit();
                glib::Propagation::Proceed
            }
        });
    }

    // ================= file handling =================

    pub fn open_dialog(&self, rc: Rc<RefCell<Ui>>) {
        let dialog = gtk::FileDialog::builder()
            .title("Open comic book or image directory")
            .modal(true)
            .build();
        let last = self.state.prefs.path_to_last_file.clone();
        if let Some(parent) = Path::new(&last).parent() {
            if parent.is_dir() {
                dialog.set_initial_folder(Some(&gio::File::for_path(parent)));
            }
        }
        dialog.open(
            Some(&self.window),
            None::<&gio::Cancellable>,
            glib::clone!(@strong rc => move |res| {
                if let Ok(file) = res {
                    if let Some(p) = file.path() {
                        rc.borrow_mut().open_path(p);
                    }
                }
            }),
        );
    }

    pub fn open_path(&mut self, path: PathBuf) {
        let page = if self.state.prefs.path_to_last_file == path.to_string_lossy() {
            self.state.prefs.page_of_last_file
        } else {
            1
        };
        self.open_path_with_page(path, page);
    }

    pub fn open_path_with_page(&mut self, path: PathBuf, start_page: u32) {
        if let Some(mut a) = self.state.archive.take() {
            a.close();
        }
        self.state.pages.clear();
        self.state.page = 0;
        self.state.textures = vec![None, None];
        self.state.sizes = vec![(0, 0), (0, 0)];
        self.state.thumb_gen += 1;

        match archive::open(&path) {
            Ok(mut ar) => {
                let name = ar.name().to_string();
                match ar.page_names() {
                    Ok(pages) if !pages.is_empty() => {
                        let kind = archive::detect(&path)
                            .map(|k| k.to_string())
                            .unwrap_or_else(|| "images".to_string());
                        log::info!(
                            "opened {}: {} pages ({kind})",
                            path.display(),
                            pages.len()
                        );
                        self.state.pages = pages;
                        let last = (start_page.max(1) as usize).saturating_sub(1);
                        self.state.page = last.min(self.state.pages.len() - 1);
                        self.state.archive = Some(ar);
                        self.state.path = Some(path.clone());
                        // Invalidate any in-flight decode of the previous file.
                        self.state.page_gen += 1;
                        self.state.prefetch_gen += 1;
                        self.state.page_loading = false;
                        self.state.page_pending = None;
                        self.state.page_dest = None;
                        self.state.prefetching = false;
                        self.state.prefetch_queue.clear();
                        self.state.cache.clear();
                        self.state.textures = vec![None, None];
                        self.state.sizes = vec![(0, 0), (0, 0)];
                        self.window.set_title(Some(&format!("{name} — MComix3")));
                        self.update_status();
                        self.record_position();
                        self.spawn_thumbnails();
                        self.request_pages(ScrollDest::Start);
                    }
                    Ok(_) => {
                        self.notice(&format!("No images found in '{}'.", path.display()));
                    }
                    Err(e) => {
                        self.notice(&format!("Could not list '{}': {}", path.display(), e));
                    }
                }
            }
            Err(e) => {
                self.notice(&format!("Cannot open '{}': {}", path.display(), e));
            }
        }
    }

    // ================= page decoding (async, background worker) =================

    /// Request an asynchronous decode of the visible page(s). While a decode
    /// is in flight, further requests are coalesced to the newest target, so
    /// holding PageDown never blocks the UI thread.
    fn request_pages(&mut self, dest: ScrollDest) {
        let n = self.state.pages.len();
        if n == 0 {
            return;
        }
        let mut indices = vec![self.state.page];
        if self.state.double_page && n > 1 {
            indices.push((self.state.page + 1).min(n - 1));
        }
        self.state.page_dest = Some(dest);
        self.push_load(indices, true);
    }

    fn push_load(&mut self, indices: Vec<usize>, display: bool) {
        let Some(tx) = self.state.page_loader_tx.clone() else {
            return;
        };
        if display && self.state.page_loading {
            self.state.page_pending = Some(indices);
            return;
        }
        let Some(path) = self.state.path.clone() else {
            return;
        };
        let pages = self.state.pages.clone();
        if display {
            self.state.page_req += 1;
        }
        let cmd = PageCmd {
            req: self.state.page_req,
            gen: self.state.page_gen,
            pgen: self.state.prefetch_gen,
            display,
            path,
            pages,
            indices,
            rotation: self.state.rotation,
            flip_h: self.state.flip_h,
            flip_v: self.state.flip_v,
        };
        if display {
            self.state.page_loading = true;
        } else {
            self.state.prefetching = true;
        }
        if tx.send(cmd).is_err() {
            if display {
                self.state.page_loading = false;
            } else {
                self.state.prefetching = false;
            }
        }
    }

    fn apply_page_result(&mut self, res: PageResult) {
        if !res.display {
            // Prefetch result: fill the cache only.
            if res.gen != self.state.page_gen || res.pgen != self.state.prefetch_gen {
                self.state.prefetching = false;
                self.pump_prefetch();
                return;
            }
            for p in res.pages {
                self.cache_decoded(p);
            }
            self.state.prefetching = false;
            self.pump_prefetch();
            return;
        }

        self.state.page_loading = false;
        // Drop results that are stale (old archive) or superseded by a newer
        // navigation/transform request.
        if res.gen != self.state.page_gen || res.req != self.state.page_req {
            if let Some(p) = self.state.page_pending.take() {
                self.push_load(p, true);
            }
            return;
        }
        self.state.textures = vec![None, None];
        self.state.sizes = vec![(0, 0), (0, 0)];
        for p in res.pages {
            let idx = p.idx;
            // Map the page index back to its display slot (0 or 1).
            if let Some((tex, w, h)) = self.cache_decoded(p) {
                if idx >= self.state.page && idx - self.state.page < 2 {
                    self.state.textures[idx - self.state.page] = Some(tex);
                    self.state.sizes[idx - self.state.page] = (w, h);
                }
            }
        }
        self.redraw_force();
        if let Some(dest) = self.state.page_dest.take() {
            self.scroll_to_destination(dest);
        }
        self.record_position();
        if let Some(p) = self.state.page_pending.take() {
            self.push_load(p, true);
        }
        self.do_caching();
    }

    /// Turn a decoded page into a texture and store it in the LRU cache.
    /// Returns `None` if the page could not be decoded (nothing is cached).
    fn cache_decoded(&mut self, p: DecodedPage) -> Option<(gdk::Texture, u32, u32)> {
        let (w, h) = (p.w, p.h);
        let img = image::RgbaImage::from_raw(p.w, p.h, p.rgba)?;
        let tex = image_loader::texture_from_rgba(&img);
        let bytes = (w as usize) * (h as usize) * 4;
        self.state
            .cache
            .put(p.idx, (tex.clone(), w, h), bytes);
        Some((tex, w, h))
    }

    /// Drop all cached textures and pending prefetches (used when the decoded
    /// representation changes, e.g. rotation or flips).
    fn invalidate_cache(&mut self) {
        self.state.cache.clear();
        self.state.prefetch_queue.clear();
        self.state.prefetch_gen += 1;
    }

    /// Recompute the set of pages that should be cached around the current
    /// page, evict the rest, and prefetch whatever is missing (one decode in
    /// flight at a time, priority: next page, previous page, then the rest).
    fn do_caching(&mut self) {
        let n = self.state.pages.len();
        if n == 0 {
            return;
        }
        let page = self.state.page;
        let page_width = if self.state.double_page { 2 } else { 1 };
        let cap = self.state.prefs.max_pages_to_cache.max(1) as usize;

        let start = page as isize - page_width as isize;
        let mut wanted: Vec<usize> = Vec::new();
        for off in 0..cap {
            let idx = start + off as isize;
            if idx >= 0 && (idx as usize) < n {
                wanted.push(idx as usize);
            }
        }
        wanted.sort_by_key(|&i| {
            if i == page {
                0
            } else if i == page + page_width {
                1
            } else if i + page_width == page {
                2
            } else {
                3
            }
        });

        // Drop pages that fell out of the window.
        self.state.cache.retain(|k| wanted.contains(k));

        // Queue the missing ones.
        self.state.prefetch_queue.clear();
        for &i in &wanted {
            if !self.state.cache.contains(&i) {
                self.state.prefetch_queue.push_back(i);
            }
        }
        self.pump_prefetch();
    }

    fn pump_prefetch(&mut self) {
        if self.state.prefetching {
            return;
        }
        let Some(idx) = self.state.prefetch_queue.pop_front() else {
            return;
        };
        self.push_load(vec![idx], false);
    }

    pub fn redraw(&mut self) {
        let n = self.state.pages.len();
        // Remove previous children (pictures + placeholder).
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }
        if n == 0 {
            self.content.append(&self.placeholder);
            self.update_status();
            return;
        }

        let alloc = self.scrolled.allocation();
        let vw = (alloc.width() as f64 - 4.0).max(50.0);
        let vh = (alloc.height() as f64 - 4.0).max(50.0);

        let double = self.state.double_page && n > 1;
        // Display order: in manga mode the current page sits on the right.
        let order: Vec<usize> = if double {
            if self.state.manga {
                vec![1, 0]
            } else {
                vec![0, 1]
            }
        } else {
            vec![0]
        };
        let slot_sizes: Vec<(u32, u32)> = order.iter().map(|&i| self.state.sizes[i]).collect();
        let has_sizes = slot_sizes.iter().any(|s| s.0 > 0 && s.1 > 0);
        let zoomed = if has_sizes {
            self.state.zoom.zoomed_sizes(
                &slot_sizes,
                (vw, vh),
                2.0,
                double,
                self.state.prefs.fit_to_size_mode,
                self.state.prefs.fit_to_size_px,
            )
        } else {
            Vec::new()
        };

        let mut total_w = 0.0_f64;
        let mut total_h = 0.0_f64;
        for (slot, &i) in order.iter().enumerate() {
            let pic = &self.pics[i];
            match (self.state.textures[i].as_ref(), zoomed.get(slot)) {
                (Some(tex), Some((w, h))) => {
                    pic.set_paintable(Some(tex));
                    pic.set_size_request(*w as i32, *h as i32);
                    self.content.append(pic);
                    total_w += *w as f64;
                    total_h = total_h.max(*h as f64);
                }
                _ => {
                    pic.set_paintable(None::<&gdk::Texture>);
                }
            }
        }
        if double {
            total_w += 2.0; // spacing between pages
        }
        let (tw, th) = (total_w.max(1.0) as i32, total_h.max(1.0) as i32);
        self.content.set_size_request(tw, th);
        self.state.last_content = (tw, th);
        self.content.set_direction(if self.state.manga {
            gtk::TextDirection::Rtl
        } else {
            gtk::TextDirection::Ltr
        });
        self.update_status();
    }

    /// Force a redraw even if the viewport size did not change.
    pub fn redraw_force(&mut self) {
        self.state.last_viewport = None;
        self.redraw();
    }

    /// Redraw only if the viewport actually changed size (avoids loops).
    pub fn schedule_redraw(&mut self) {
        let alloc = self.scrolled.allocation();
        let vp = (alloc.width(), alloc.height());
        if self.state.last_viewport == Some(vp) {
            return;
        }
        self.state.last_viewport = Some(vp);
        self.redraw();
    }

    fn update_status(&mut self) {
        let n = self.state.pages.len();
        if n == 0 {
            self.status.set_text("");
            return;
        }
        let page = self.state.page + 1;
        let (w, h) = self.state.sizes[0];
        let name = self
            .state
            .pages
            .get(self.state.page)
            .cloned()
            .unwrap_or_default();
        let fname = Path::new(&name)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        self.status
            .set_text(&format!("Page {page} / {n}    {w} × {h} px    {fname}"));
    }

    /// Execute a keyboard-bound action.
    pub fn handle_action(&mut self, action: crate::keybindings::Action, rc: Rc<RefCell<Ui>>) {
        use crate::keybindings::Action as A;
        match action {
            A::NextPage => self.next_page(1),
            A::PrevPage => self.next_page(-1),
            A::NextPage10 => self.next_page(10),
            A::PrevPage10 => self.next_page(-10),
            A::FirstPage => self.first_page(),
            A::LastPage => self.last_page(),
            A::GoToPage => self.page_select_dialog(rc),
            A::ScrollUp => self.scroll_up(),
            A::ScrollDown => self.scroll_down(),
            A::ScrollLeft => {
                if self.h_scrollable() {
                    if self.state.manga {
                        self.scroll_right();
                    } else {
                        self.scroll_left();
                    }
                } else if self.state.manga {
                    self.next_page(1);
                } else {
                    self.next_page(-1);
                }
            }
            A::ScrollRight => {
                if self.h_scrollable() {
                    if self.state.manga {
                        self.scroll_left();
                    } else {
                        self.scroll_right();
                    }
                } else if self.state.manga {
                    self.next_page(-1);
                } else {
                    self.next_page(1);
                }
            }
            A::SmartScrollUp => self.smart_scroll_up(),
            A::SmartScrollDown => self.smart_scroll_down(),
            A::ZoomIn => self.zoom_in(),
            A::ZoomOut => self.zoom_out(),
            A::ZoomOriginal => self.zoom_original(),
            A::FitBest => self.set_fit_mode(zoom::ZOOM_MODE_BEST),
            A::FitWidth => self.set_fit_mode(zoom::ZOOM_MODE_WIDTH),
            A::FitHeight => self.set_fit_mode(zoom::ZOOM_MODE_HEIGHT),
            A::FitSize => self.set_fit_mode(zoom::ZOOM_MODE_SIZE),
            A::FitManual => self.set_fit_mode(zoom::ZOOM_MODE_MANUAL),
            A::Rotate90 => self.rotate_90(),
            A::Rotate270 => self.rotate_270(),
            A::Rotate180 => self.rotate_180(),
            A::FlipH => self.flip_horizontally(),
            A::FlipV => self.flip_vertically(),
            A::ToggleDoublePage => self.toggle_double_page(),
            A::ToggleManga => self.toggle_manga(),
            A::ToggleFullscreen => self.toggle_fullscreen(),
            A::ToggleThumbnails => self.toggle_thumbnails(),
            A::ToggleSlideshow => self.toggle_slideshow(rc),
            A::ToggleHideAll => self.toggle_hide_all(),
            A::ToggleMenubar => self.toggle_menubar(),
            A::InvertScroll => {
                self.state.prefs.invert_smart_scroll = !self.state.prefs.invert_smart_scroll;
            }
            A::ShowInfo => self.show_info_panel(),
            A::Minimize => self.window.minimize(),
            A::ExitFullscreen => {
                if self.window.is_fullscreen() {
                    self.window.unfullscreen();
                } else if self.state.prefs.escape_quits {
                    self.window.close();
                }
            }
        }
    }

    /// Apply new preference values (from the preferences dialog) to the UI.
    pub fn apply_prefs(&mut self, new: &crate::prefs::Prefs) {
        self.state.prefs = new.clone();
        self.state.prefs.save();
        self.state.zoom.fit_mode = self.state.prefs.zoom_mode;
        self.state.zoom.scale_up = self.state.prefs.scale_up;
        self.zoom_dropdown
            .set_selected(self.state.prefs.zoom_mode.clamp(0, 4) as u32);
        apply_background_css(&self.state.prefs);
        let policy = if self.state.prefs.show_scrollbar {
            gtk::PolicyType::Automatic
        } else {
            gtk::PolicyType::Never
        };
        self.scrolled.set_policy(policy, policy);
        self.thumb_scroll.set_visible(self.state.prefs.show_thumbnails);
        self.sync_toggles();
        self.redraw_force();
    }

    // ================= navigation =================

    pub fn next_page(&mut self, n: i32) {
        let count = self.state.pages.len();
        if count == 0 {
            return;
        }
        let step = if self.state.double_page && self.state.prefs.double_step_in_double_page_mode {
            2
        } else {
            1
        };
        let target = self.state.page as i32 + n * step;
        if target >= count as i32 {
            if n > 0 && self.state.prefs.auto_open_next_archive && self.open_next_in_dir() {
                return;
            }
            return;
        }
        if target < 0 {
            if n < 0 && self.state.prefs.auto_open_next_archive && self.open_prev_in_dir() {
                return;
            }
            return;
        }
        let dest = if n >= 0 { ScrollDest::Start } else { ScrollDest::End };
        self.goto_index(target as usize, dest);
    }

    pub fn first_page(&mut self) {
        self.goto_index(0, ScrollDest::Start);
    }

    pub fn last_page(&mut self) {
        if !self.state.pages.is_empty() {
            self.goto_index(self.state.pages.len() - 1, ScrollDest::End);
        }
    }

    pub fn goto_index(&mut self, idx: usize, dest: ScrollDest) {
        let n = self.state.pages.len();
        if n == 0 {
            return;
        }
        // While a page is still decoding, ignore navigation input: the user
        // expects "flip -> wait for the page to appear -> flip again", not
        // queued/skipped pages. (Decoding runs off the UI thread, so the UI
        // stays responsive; the flip just happens when the page is ready.)
        if self.state.page_loading {
            return;
        }
        let idx = idx.min(n - 1);
        if idx == self.state.page {
            return;
        }
        self.state.page = idx;
        log::debug!("goto page {} / {}", idx + 1, n);

        // Fast path: the target page(s) are already decoded in the cache.
        let mut visible = vec![idx];
        if self.state.double_page && n > 1 {
            visible.push((idx + 1).min(n - 1));
        }
        if visible.iter().all(|i| self.state.cache.contains(i)) {
            let mut textures = vec![None, None];
            let mut sizes = vec![(0, 0), (0, 0)];
            for (slot, &i) in visible.iter().enumerate() {
                if let Some((tex, w, h)) = self.state.cache.get(&i).cloned() {
                    textures[slot] = Some(tex);
                    sizes[slot] = (w, h);
                }
            }
            self.state.textures = textures;
            self.state.sizes = sizes;
            self.update_status();
            self.redraw_force();
            self.scroll_to_destination(dest);
            self.record_position();
            self.follow_thumbnail(idx);
            self.do_caching();
            return;
        }

        // Slow path: decode in the background (footer and thumbnail selection
        // follow immediately; the picture swaps in when ready).
        self.update_status();
        self.follow_thumbnail(idx);
        self.request_pages(dest);
    }

    /// Position the viewport: top for forward navigation, bottom for backward.
    fn scroll_to_destination(&mut self, dest: ScrollDest) {
        let hadj = self.scrolled.hadjustment();
        let vadj = self.scrolled.vadjustment();
        match dest {
            ScrollDest::Start => {
                hadj.set_value(0.0);
                vadj.set_value(0.0);
            }
            ScrollDest::End => {
                hadj.set_value(0.0);
                // Use the content size we just laid out: the scrolled window's
                // adjustment upper is not updated synchronously yet.
                let max_v =
                    (self.state.last_content.1 as f64 - vadj.page_size()).max(0.0);
                vadj.set_value(max_v);
            }
            ScrollDest::Keep => {}
        }
    }

    /// Highlight the thumbnail of page `idx` and scroll it into view.
    ///
    /// While paging faster than the thumbnail worker can generate thumbnails,
    /// the target cell may not exist yet; in that case the sidebar is scrolled
    /// to the proportional position so it still "follows" the current page.
    fn follow_thumbnail(&mut self, idx: usize) {
        let total = self.state.pages.len().max(1);
        if let Some(cell) = self.state.thumb_cells.get(idx).and_then(|c| c.as_ref()) {
            if let Some(child) = cell
                .parent()
                .and_then(|p| p.downcast::<gtk::FlowBoxChild>().ok())
            {
                self.thumb_box.select_child(&child);
            }
            self.scroll_thumb_into_view(idx);
            return;
        }
        // Not generated yet: approximate scroll position.
        let adj = self.thumb_scroll.vadjustment();
        let upper = adj.upper();
        let page = adj.page_size();
        if upper > page {
            let target = (idx as f64 / total as f64) * (upper - page);
            adj.set_value(target);
        }
    }

    /// Scroll the thumbnail sidebar so the thumbnail at `idx` is visible.
    /// (gtk4-rs 0.9 has no gtk_widget_scroll_to, so we move the vadjustment.)
    fn scroll_thumb_into_view(&mut self, idx: usize) {
        let Some(cell) = self.state.thumb_cells.get(idx).and_then(|c| c.as_ref()) else {
            return;
        };
        let Some(child) = cell
            .parent()
            .and_then(|p| p.downcast::<gtk::FlowBoxChild>().ok())
        else {
            return;
        };
        let alloc = child.allocation();
        let adj = self.thumb_scroll.vadjustment();
        let page = adj.page_size();
        let cur = adj.value();
        let y = alloc.y() as f64;
        let h = alloc.height() as f64;
        let max_scroll = (adj.upper() - page).max(0.0);
        let new = if y < cur {
            y
        } else if y + h > cur + page {
            (y + h - page).clamp(0.0, max_scroll)
        } else {
            cur
        };
        if new != cur {
            adj.set_value(new);
        }
    }

    fn open_next_in_dir(&mut self) -> bool {
        let Some(path) = self.state.path.clone() else {
            return false;
        };
        let Some(dir) = path.parent() else {
            return false;
        };
        self.open_sibling(dir, 1)
    }

    fn open_prev_in_dir(&mut self) -> bool {
        let Some(path) = self.state.path.clone() else {
            return false;
        };
        let Some(dir) = path.parent() else {
            return false;
        };
        self.open_sibling(dir, -1)
    }

    fn open_sibling(&mut self, dir: &Path, delta: i32) -> bool {
        let current = self.state.path.clone().unwrap_or_default();
        let mut candidates: Vec<PathBuf> = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && archive::detect(&p).is_some() {
                candidates.push(p);
            }
        }
        if candidates.is_empty() {
            return false;
        }
        let mut names: Vec<String> = candidates
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        crate::natsort::natural_sort(&mut names);
        let Some(pos) = names.iter().position(|n| Path::new(n) == &current) else {
            return false;
        };
        let target = pos as i32 + delta;
        if target < 0 || target >= names.len() as i32 {
            return false;
        }
        let path = PathBuf::from(&names[target as usize]);
        info!("opening sibling archive: {}", path.display());
        // Going forward, start at the first page; going backward, start at
        // the last page (open_path_with_page clamps to the last page for
        // oversized page numbers).
        let start_page = if delta < 0 { u32::MAX } else { 1 };
        self.open_path_with_page(path, start_page);
        true
    }

    // ================= zoom / view =================

    pub fn set_fit_mode(&mut self, mode: i32) {
        self.state.zoom.set_fit_mode(mode);
        self.state.prefs.zoom_mode = mode;
        self.state.prefs.save();
        self.zoom_dropdown.set_selected(mode as u32);
        self.redraw_force();
        self.scroll_to_start();
    }

    pub fn zoom_in(&mut self) {
        self.state.zoom.zoom_in();
        self.redraw_force();
    }

    pub fn zoom_out(&mut self) {
        self.state.zoom.zoom_out();
        self.redraw_force();
    }

    pub fn zoom_original(&mut self) {
        self.state.zoom.reset_user_zoom();
        self.set_fit_mode(zoom::ZOOM_MODE_MANUAL);
    }

    pub fn rotate_90(&mut self) {
        self.state.rotation = (self.state.rotation + 90).rem_euclid(360);
        self.state.prefs.rotation = self.state.rotation;
        self.state.prefs.save();
        self.invalidate_cache();
        self.request_pages(ScrollDest::Keep);
    }

    pub fn rotate_270(&mut self) {
        self.state.rotation = (self.state.rotation + 270).rem_euclid(360);
        self.state.prefs.rotation = self.state.rotation;
        self.state.prefs.save();
        self.invalidate_cache();
        self.request_pages(ScrollDest::Keep);
    }

    pub fn rotate_180(&mut self) {
        self.state.rotation = (self.state.rotation + 180).rem_euclid(360);
        self.state.prefs.rotation = self.state.rotation;
        self.state.prefs.save();
        self.invalidate_cache();
        self.request_pages(ScrollDest::Keep);
    }

    pub fn flip_horizontally(&mut self) {
        self.state.flip_h = !self.state.flip_h;
        self.invalidate_cache();
        self.request_pages(ScrollDest::Keep);
    }

    pub fn flip_vertically(&mut self) {
        self.state.flip_v = !self.state.flip_v;
        self.invalidate_cache();
        self.request_pages(ScrollDest::Keep);
    }

    pub fn toggle_double_page(&mut self) {
        self.state.double_page = !self.state.double_page;
        self.sync_toggles();
        self.request_pages(ScrollDest::Start);
    }

    pub fn toggle_manga(&mut self) {
        self.state.manga = !self.state.manga;
        self.sync_toggles();
        self.redraw_force();
        self.scroll_to_start();
    }

    pub fn toggle_fullscreen(&mut self) {
        if self.window.is_fullscreen() {
            self.window.unfullscreen();
        } else {
            self.window.fullscreen();
        }
        // The actual chrome show/hide happens in on_fullscreen_changed().
    }

    /// Called (via an idle handler) whenever the window's fullscreen state
    /// changes: in fullscreen mode, hide toolbar/statusbar/thumbnails so only
    /// the current page is shown (unless the user disabled hide-all).
    pub fn on_fullscreen_changed(&mut self) {
        let fs = self.window.is_fullscreen();
        if fs {
            if self.state.prefs.hide_all_in_fullscreen && !self.state.chrome_hidden_by_fullscreen {
                self.state.saved_toolbar = self.toolbar.is_visible();
                self.state.saved_status = self.status.is_visible();
                self.state.saved_thumbs = self.thumb_scroll.is_visible();
                self.toolbar.set_visible(false);
                self.status.set_visible(false);
                self.thumb_scroll.set_visible(false);
                self.state.chrome_hidden_by_fullscreen = true;
            }
        } else if self.state.chrome_hidden_by_fullscreen {
            self.toolbar.set_visible(self.state.saved_toolbar);
            self.status.set_visible(self.state.saved_status);
            self.thumb_scroll.set_visible(self.state.saved_thumbs);
            self.state.chrome_hidden_by_fullscreen = false;
        }
        self.sync_toggles();
        self.schedule_redraw();
    }

    pub fn toggle_thumbnails(&mut self) {
        let visible = !self.thumb_scroll.is_visible();
        self.thumb_scroll.set_visible(visible);
        self.state.prefs.show_thumbnails = visible;
        self.state.prefs.save();
        self.sync_toggles();
    }

    pub fn toggle_menubar(&mut self) {
        // There is no menubar in the milestone-1 layout; the toolbar plays that
        // role, so Ctrl+M toggles it like MComix's menubar key.
        self.toolbar.set_visible(!self.toolbar.is_visible());
    }

    pub fn toggle_hide_all(&mut self) {
        let hidden = !self.toolbar.is_visible();
        self.toolbar.set_visible(hidden);
        self.status.set_visible(hidden);
        self.thumb_scroll.set_visible(hidden);
    }

    pub fn toggle_stretch(&mut self) {
        self.state.prefs.stretch = !self.state.prefs.stretch;
        self.redraw_force();
    }

    pub fn toggle_keep_transformation(&mut self) {
        self.state.prefs.keep_transformation = !self.state.prefs.keep_transformation;
    }

    pub fn toggle_lens(&mut self) {
        self.notice("The magnifying lens is not ported yet.");
    }

    pub fn show_info_panel(&mut self) {
        // OSD-ish: dump current file info into the status bar.
        let path = self
            .state
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        self.status.set_text(&format!(
            "{} | zoom log {} fit {} | page {}",
            path,
            self.state.zoom.user_zoom_log,
            self.state.zoom.fit_mode,
            self.state.page + 1
        ));
    }

    fn sync_toggles(&mut self) {
        self.suppress_toggles.set(true);
        self.btn_double.set_active(self.state.double_page);
        self.btn_manga.set_active(self.state.manga);
        self.btn_slideshow.set_active(self.state.slideshow);
        self.btn_fullscreen.set_active(self.window.is_fullscreen());
        self.btn_thumbs.set_active(self.thumb_scroll.is_visible());
        self.suppress_toggles.set(false);
    }

    // ================= scrolling =================

    /// True when the page is wider than the viewport (horizontal overflow).
    fn h_scrollable(&self) -> bool {
        let a = self.scrolled.hadjustment();
        a.upper() > a.page_size() + 1.0
    }

    pub fn scroll_up(&mut self) {
        let adj = self.scrolled.vadjustment();
        let px = self.state.prefs.number_of_pixels_to_scroll_per_key_event as f64;
        let upper = adj.upper() - adj.page_size();
        // At the top edge of a zoomed-in page, go to the previous page (which
        // is shown from the bottom, see ScrollDest::End).
        if self.state.prefs.flip_with_wheel && adj.value() - px <= 1.0 && upper > 0.0 {
            self.next_page(-1);
        } else {
            adj.set_value(adj.value() - px);
        }
    }

    pub fn scroll_down(&mut self) {
        let adj = self.scrolled.vadjustment();
        let px = self.state.prefs.number_of_pixels_to_scroll_per_key_event as f64;
        let upper = adj.upper() - adj.page_size();
        if self.state.prefs.flip_with_wheel && adj.value() + px >= upper - 1.0 && upper > 0.0 {
            self.next_page(1);
        } else {
            adj.set_value(adj.value() + px);
        }
    }

    pub fn scroll_left(&mut self) {
        let adj = self.scrolled.hadjustment();
        adj.set_value(adj.value() - self.state.prefs.number_of_pixels_to_scroll_per_key_event as f64);
    }

    pub fn scroll_right(&mut self) {
        let adj = self.scrolled.hadjustment();
        adj.set_value(adj.value() + self.state.prefs.number_of_pixels_to_scroll_per_key_event as f64);
    }

    pub fn smart_scroll_down(&mut self) {
        let adj = self.scrolled.vadjustment();
        let upper = adj.upper() - adj.page_size();
        let step = (self.scrolled.allocation().height() as f64
            * self.state.prefs.smart_scroll_percentage)
            .max(1.0);
        if adj.value() + step >= upper - 1.0 {
            self.next_page(1);
        } else {
            adj.set_value(adj.value() + step);
        }
    }

    pub fn smart_scroll_up(&mut self) {
        let adj = self.scrolled.vadjustment();
        let step = (self.scrolled.allocation().height() as f64
            * self.state.prefs.smart_scroll_percentage)
            .max(1.0);
        if adj.value() - step <= 1.0 {
            self.next_page(-1);
        } else {
            adj.set_value(adj.value() - step);
        }
    }

    fn scroll_to_start(&mut self) {
        self.scrolled.hadjustment().set_value(0.0);
        self.scrolled.vadjustment().set_value(0.0);
    }

    // ================= page select dialog =================

    pub fn page_select_dialog(&self, rc: Rc<RefCell<Ui>>) {
        let n = self.state.pages.len();
        if n == 0 {
            return;
        }
        let dlg = gtk::Window::new();
        dlg.set_title(Some("Go to page"));
        dlg.set_transient_for(Some(&self.window));
        dlg.set_modal(true);
        dlg.set_resizable(false);

        let spin = gtk::SpinButton::with_range(1.0, n as f64, 1.0);
        spin.set_value((self.state.page + 1) as f64);

        let ok = gtk::Button::with_label("Go");
        let cancel = gtk::Button::with_label("Cancel");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_top(12);
        vbox.set_margin_bottom(12);
        vbox.set_margin_start(12);
        vbox.set_margin_end(12);
        vbox.append(&spin);
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        hbox.set_halign(gtk::Align::End);
        hbox.append(&cancel);
        hbox.append(&ok);
        vbox.append(&hbox);
        dlg.set_child(Some(&vbox));

        let dlg_ok = dlg.clone();
        ok.connect_clicked(move |_| {
            let page = spin.value() as usize;
            rc.borrow_mut().goto_index(page.saturating_sub(1), ScrollDest::Start);
            dlg_ok.close();
        });
        let dlg_cancel = dlg.clone();
        cancel.connect_clicked(move |_| dlg_cancel.close());
        dlg.connect_close_request(|_| glib::Propagation::Proceed);

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

        dlg.present();
    }

    // ================= slideshow =================

    pub fn toggle_slideshow(&mut self, rc: Rc<RefCell<Ui>>) {
        if self.state.slideshow {
            self.stop_slideshow();
        } else {
            self.start_slideshow(rc);
        }
    }

    fn start_slideshow(&mut self, rc: Rc<RefCell<Ui>>) {
        if self.state.pages.is_empty() || self.state.slideshow {
            return;
        }
        self.state.slideshow = true;
        self.sync_toggles();
        let delay = self.state.prefs.slideshow_delay.max(100);
        let r = rc.clone();
        let id = glib::timeout_add_local(Duration::from_millis(delay), move || {
            let mut ui = r.borrow_mut();
            if !ui.state.slideshow {
                return glib::ControlFlow::Break;
            }
            ui.next_page(1);
            glib::ControlFlow::Continue
        });
        self.state.slideshow_source = Some(id);
        info!("slideshow started");
    }

    fn stop_slideshow(&mut self) {
        self.state.slideshow = false;
        if let Some(id) = self.state.slideshow_source.take() {
            id.remove();
        }
        self.sync_toggles();
        info!("slideshow stopped");
    }

    // ================= thumbnails =================

    fn spawn_thumbnails(&mut self) {
        self.thumb_box.remove_all();
        self.state.thumb_cells.clear();
        self.state.thumb_pics.clear();
        self.state.thumb_gen += 1;
        let gen = self.state.thumb_gen;
        let Some(path) = self.state.path.clone() else {
            return;
        };
        let pages = self.state.pages.clone();
        let Some(tx) = self.state.thumb_tx.clone() else {
            return;
        };

        // Pre-create one cell per page, in page order, with an empty picture
        // placeholder. Thumbnails fill these in as they decode, so the sidebar
        // is a stable, correctly numbered list from the start.
        for idx in 0..pages.len() {
            let (cell, pic) = self.make_thumb_cell(idx);
            self.state.thumb_cells.push(Some(cell.clone()));
            self.state.thumb_pics.push(Some(pic));
            self.thumb_box.insert(&cell, -1);
        }
        self.follow_thumbnail(self.state.page);

        // Generation order: the currently open page first, then expand
        // outward (+1, -1, +2, -2, ...) so thumbnails near the reading
        // position are ready immediately when the user navigates.
        let total = pages.len();
        let cur = self.state.page.min(total - 1);
        let mut order: Vec<usize> = Vec::with_capacity(total);
        let mut seen = vec![false; total];
        order.push(cur);
        seen[cur] = true;
        for step in 1..total {
            let nxt = cur as isize + step as isize;
            if nxt >= 0 && (nxt as usize) < total && !seen[nxt as usize] {
                seen[nxt as usize] = true;
                order.push(nxt as usize);
            }
            let prev = cur as isize - step as isize;
            if prev >= 0 && (prev as usize) < total && !seen[prev as usize] {
                seen[prev as usize] = true;
                order.push(prev as usize);
            }
        }
        let order = std::sync::Arc::new(order);

        // Parallel generation: several workers each open their own archive
        // handle and claim the next page index from the shared order.
        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(4);
        let next_pos = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for _ in 0..n_threads {
            let tx = tx.clone();
            let path = path.clone();
            let pages = pages.clone();
            let order = order.clone();
            let next_pos = next_pos.clone();
            std::thread::spawn(move || {
                let mut ar = match archive::open(&path) {
                    Ok(a) => a,
                    Err(e) => {
                        warn!("thumbnail worker cannot open {}: {e}", path.display());
                        return;
                    }
                };
                if let Err(e) = ar.page_names() {
                    warn!("thumbnail worker listing failed: {e}");
                    return;
                }
                loop {
                    let pos = next_pos.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&idx) = order.get(pos) else {
                        break;
                    };
                    if let Some(name) = pages.get(idx) {
                        if let Ok(bytes) = ar.read(name) {
                            // gdk-pixbuf scaled decode (fast), pure-Rust fallback.
                            let thumb = crate::thumb_cache::load(&path, idx).or_else(|| {
                                image_loader::thumbnail_pixbuf_rgba(&bytes, 160, 160)
                                    .or_else(|| image_loader::thumbnail_rgba_fallback(&bytes, 160, 160))
                            });
                            if let Some((w, h, rgba)) = thumb {
                                crate::thumb_cache::store(&path, idx, w, h, &rgba);
                                if tx.send((gen, idx, w, h, rgba)).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    /// Build an (empty) thumbnail cell: picture placeholder + page number.
    fn make_thumb_cell(&self, idx: usize) -> (gtk::Box, gtk::Picture) {
        let size = self.state.prefs.thumbnail_size.max(40) as i32;
        let cell = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let pic = gtk::Picture::new();
        pic.set_can_shrink(true);
        pic.set_content_fit(gtk::ContentFit::Contain);
        pic.set_size_request(size, size);
        cell.append(&pic);
        if self.state.prefs.show_page_numbers_on_thumbnails {
            let lab = gtk::Label::new(Some(&format!("{}", idx + 1)));
            lab.set_css_classes(&["page-num"]);
            cell.append(&lab);
        }
        if let Some(name) = self.state.pages.get(idx) {
            cell.set_tooltip_text(Some(name));
        }
        (cell, pic)
    }

    /// Fill the pre-created cell for page `idx` with its decoded thumbnail.
    fn add_thumb(&mut self, idx: usize, tex: &gdk::Texture) {
        if let Some(pic) = self.state.thumb_pics.get(idx).and_then(|p| p.as_ref()) {
            pic.set_paintable(Some(tex));
        }
        if idx == self.state.page {
            self.follow_thumbnail(idx);
        }
    }

    /// Find the page index of a FlowBoxChild (by matching its cell widget).
    fn page_of_child(&self, child: &gtk::FlowBoxChild) -> Option<usize> {
        for (idx, cell) in self.state.thumb_cells.iter().enumerate() {
            if let Some(cell) = cell {
                if let Some(parent) = cell.parent() {
                    if parent.downcast::<gtk::FlowBoxChild>().ok().as_ref() == Some(child) {
                        return Some(idx);
                    }
                }
            }
        }
        None
    }

    // ================= persistence / misc =================

    /// Persist the current reading position, throttled to at most one write
    /// per second (rapid paging must not hammer the disk).
    fn record_position(&mut self) {
        let Some(path) = self.state.path.clone() else {
            return;
        };
        let page = (self.state.page + 1) as u32;
        let path_str = path.to_string_lossy().into_owned();
        let changed = self.state.prefs.path_to_last_file != path_str
            || self.state.prefs.page_of_last_file != page;
        let due = self
            .state
            .last_save
            .map(|t| t.elapsed().as_secs() >= 1)
            .unwrap_or(true);
        if changed && due {
            self.state.prefs.path_to_last_file = path_str;
            self.state.prefs.page_of_last_file = page;
            self.state.prefs.save();
            LastReadDb::set(&path, page);
            self.state.last_save = Some(std::time::Instant::now());
        }
    }

    pub fn save_on_exit(&mut self) {
        self.stop_slideshow();
        let (w, h) = self.window.default_size();
        self.state.prefs.window_width = w;
        self.state.prefs.window_height = h;
        self.state.prefs.zoom_mode = self.state.zoom.fit_mode;
        if let Some(path) = self.state.path.clone() {
            let page = (self.state.page + 1) as u32;
            self.state.prefs.path_to_last_file = path.to_string_lossy().into_owned();
            self.state.prefs.page_of_last_file = page;
            LastReadDb::set(&path, page);
        }
        self.state.prefs.save();
    }

    pub fn notice(&self, msg: &str) {
        let dialog = gtk::AlertDialog::builder().message(msg).modal(true).build();
        dialog.show(Some(&self.window));
        info!("{msg}");
    }

    /// Present the window (called after construction) and focus the viewer so
    /// keyboard navigation works immediately.
    pub fn present(&self) {
        self.window.present();
        self.content.grab_focus();
    }
}

/// Apply background colours from preferences via CSS.
fn apply_background_css(prefs: &Prefs) {
    let to_rgb = |c: u16| (c as f64 / 65535.0 * 255.0).round() as u32;
    let (r, g, b) = (
        to_rgb(prefs.bg_color[0]),
        to_rgb(prefs.bg_color[1]),
        to_rgb(prefs.bg_color[2]),
    );
    let (tr, tg, tb) = (
        to_rgb(prefs.thumb_bg_color[0]),
        to_rgb(prefs.thumb_bg_color[1]),
        to_rgb(prefs.thumb_bg_color[2]),
    );
    let css = format!(
        ".viewer {{ background-color: rgb({r},{g},{b}); }} \
         .thumbview {{ background-color: rgb({tr},{tg},{tb}); }} \
         .placeholder {{ font-size: 14pt; color: rgba(128,128,128,1); }} \
         .page-num {{ font-size: 8pt; }}"
    );
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    if let Some(display) = gdk::Display::default() {
        gtk::StyleContext::add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
