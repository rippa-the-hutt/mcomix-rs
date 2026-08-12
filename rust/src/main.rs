//! MComix3 — Rust port. Entry point / CLI parsing.
//!
//! CLI mirrors `mcomix/run.py`:
//!   mcomix-rs [OPTIONS] [PATH]

mod app;
mod archive;
mod image_loader;
mod lastread;
mod natsort;
mod prefs;
mod zoom;

use std::path::PathBuf;

use clap::{ArgAction, Parser};
use gio::prelude::*;

use crate::prefs::Prefs;

/// View images and comic book archives.
#[derive(Parser, Clone, Debug)]
#[command(
    name = "mcomix-rs",
    version,
    about = "View images and comic book archives.",
    disable_help_flag = true,
    disable_version_flag = true,
)]
struct Args {
    /// Show this help and exit.
    #[arg(long = "help", action = ArgAction::Help)]
    help: Option<bool>,

    /// Show the version number and exit.
    #[arg(short = 'v', long = "version", action = ArgAction::Version)]
    version: Option<bool>,

    /// Start the application in slideshow mode.
    #[arg(short = 's', long = "slideshow")]
    slideshow: bool,

    /// Show the library on startup (not ported yet).
    #[arg(short = 'l', long = "library")]
    library: bool,

    /// Start the application in fullscreen mode.
    #[arg(short = 'f', long = "fullscreen")]
    fullscreen: bool,

    /// Start the application in manga mode.
    #[arg(short = 'm', long = "manga")]
    manga: bool,

    /// Start the application in double page mode.
    #[arg(short = 'd', long = "double-page")]
    double_page: bool,

    /// Start with zoom set to best fit mode.
    #[arg(short = 'b', long = "zoom-best")]
    zoom_best: bool,

    /// Start with zoom set to fit width.
    #[arg(short = 'w', long = "zoom-width")]
    zoom_width: bool,

    /// Start with zoom set to fit height.
    #[arg(short = 'h', long = "zoom-height")]
    zoom_height: bool,

    /// Open the archive at this page (1-based).
    #[arg(short = 'p', long = "page", default_value_t = 1)]
    page: u32,

    /// Sets the desired output log level.
    #[arg(short = 'W', value_name = "[ all | debug | info | warn | error ]")]
    loglevel: Option<String>,

    /// Path to a comic archive or image directory.
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    // Log level: -W overrides; default to `info` so the user sees what the app
    // is doing (mirrors run.py's -W option, but defaults are friendlier).
    let level = match args.loglevel.as_deref() {
        Some("all") | Some("trace") => log::LevelFilter::Trace,
        Some("debug") => log::LevelFilter::Debug,
        Some("info") => log::LevelFilter::Info,
        Some("warn") => log::LevelFilter::Warn,
        Some("error") => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };
    env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp(None)
        .init();
    log::info!("MComix3 (Rust port) {} starting", env!("CARGO_PKG_VERSION"));

    // Register with GIO as an application that can open files, so passing a
    // comic path on the command line (or from a file manager) works.
    let app = gtk4::Application::builder()
        .application_id("org.mcomix.mcomix_rs")
        .flags(gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let args_for_activate = args.clone();
    app.connect_activate(move |app| {
        let prefs = Prefs::load();
        let path = args_for_activate.path.clone();
        build_window(app, path, args_for_activate.page, &prefs, &args_for_activate);
    });

    let args_for_open = args.clone();
    app.connect_open(move |app, files, _hint| {
        let path = files.first().and_then(|f| f.path());
        let prefs = Prefs::load();
        build_window(app, path, 1, &prefs, &args_for_open);
    });

    app.run_with_args(&[] as &[&str]);
}

/// Create the main window (with an optional initial file).
fn build_window(
    app: &gtk4::Application,
    open_path: Option<PathBuf>,
    page: u32,
    prefs: &Prefs,
    args: &Args,
) {
    // CLI view-mode overrides.
    let mut start_fullscreen = args.fullscreen || prefs.default_fullscreen;
    let mut start_slideshow = args.slideshow;
    let start_manga = args.manga || prefs.default_manga_mode;
    let start_double = args.double_page || prefs.default_double_page;
    if args.zoom_best {
        override_zoom(prefs, zoom::ZOOM_MODE_BEST);
    } else if args.zoom_width {
        override_zoom(prefs, zoom::ZOOM_MODE_WIDTH);
    } else if args.zoom_height {
        override_zoom(prefs, zoom::ZOOM_MODE_HEIGHT);
    }

    if args.library {
        log::warn!("--library is not ported yet; opening normally.");
    }

    let ui = app::Ui::new(app, open_path, page);
    {
        let mut ui_ref = ui.borrow_mut();
        if start_fullscreen {
            ui_ref.toggle_fullscreen();
        }
        if start_manga != ui_ref.state.manga {
            ui_ref.toggle_manga();
        }
        if start_double != ui_ref.state.double_page {
            ui_ref.toggle_double_page();
        }
        if start_slideshow {
            ui_ref.toggle_slideshow(ui.clone());
        }
    }
    let _ = (&mut start_fullscreen, &mut start_slideshow);
    ui.borrow().present();
}

/// Persist a CLI zoom-mode override so the dropdown and viewer agree.
fn override_zoom(prefs: &Prefs, mode: i32) {
    let mut p = prefs.clone();
    p.zoom_mode = mode;
    p.save();
}
