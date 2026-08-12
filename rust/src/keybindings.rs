//! Configurable key bindings, mirroring `mcomix/keybindings.py`.
//!
//! Bindings are stored as accelerator strings (e.g. `Page_Down`,
//! `<Ctrl>S`, `KP_Add`) in `keybindings.conf` (JSON), keyed by action name.
//! Defaults match MComix's.

use std::collections::HashMap;
use std::path::PathBuf;

use gtk4::gdk::{self, Key, ModifierType};
use serde::{Deserialize, Serialize};

/// Mask of modifier bits we care about (Shift / Ctrl / Alt).
const MOD_MASK: ModifierType = ModifierType::SHIFT_MASK
    .union(ModifierType::CONTROL_MASK)
    .union(ModifierType::ALT_MASK);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    NextPage,
    PrevPage,
    NextPage10,
    PrevPage10,
    FirstPage,
    LastPage,
    GoToPage,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    SmartScrollUp,
    SmartScrollDown,
    ZoomIn,
    ZoomOut,
    ZoomOriginal,
    FitBest,
    FitWidth,
    FitHeight,
    FitSize,
    FitManual,
    Rotate90,
    Rotate270,
    Rotate180,
    FlipH,
    FlipV,
    ToggleDoublePage,
    ToggleManga,
    ToggleFullscreen,
    ToggleThumbnails,
    ToggleSlideshow,
    ToggleHideAll,
    ToggleMenubar,
    InvertScroll,
    ShowInfo,
    Minimize,
    ExitFullscreen,
    AddBookmark,
    EditBookmarks,
}

impl Action {
    /// Config-file key (mirrors the Python BINDING_INFO action names).
    pub fn name(&self) -> &'static str {
        use Action::*;
        match self {
            NextPage => "next_page",
            PrevPage => "previous_page",
            NextPage10 => "next_page_ff",
            PrevPage10 => "previous_page_ff",
            FirstPage => "first_page",
            LastPage => "last_page",
            GoToPage => "go_to",
            ScrollUp => "scroll_up",
            ScrollDown => "scroll_down",
            ScrollLeft => "scroll_left",
            ScrollRight => "scroll_right",
            SmartScrollUp => "smart_scroll_up",
            SmartScrollDown => "smart_scroll_down",
            ZoomIn => "zoom_in",
            ZoomOut => "zoom_out",
            ZoomOriginal => "zoom_original",
            FitBest => "best_fit_mode",
            FitWidth => "fit_width_mode",
            FitHeight => "fit_height_mode",
            FitSize => "fit_size_mode",
            FitManual => "fit_manual_mode",
            Rotate90 => "rotate_90",
            Rotate270 => "rotate_270",
            Rotate180 => "rotate_180",
            FlipH => "flip_horiz",
            FlipV => "flip_vert",
            ToggleDoublePage => "double_page",
            ToggleManga => "manga_mode",
            ToggleFullscreen => "fullscreen",
            ToggleThumbnails => "thumbnails",
            ToggleSlideshow => "slideshow",
            ToggleHideAll => "hide_all",
            ToggleMenubar => "menubar",
            InvertScroll => "invert_scroll",
            ShowInfo => "osd_panel",
            Minimize => "minimize",
            ExitFullscreen => "exit_fullscreen",
            AddBookmark => "add_bookmark",
            EditBookmarks => "edit_bookmarks",
        }
    }

    /// Human-readable label shown in the shortcuts editor.
    pub fn label(&self) -> &'static str {
        use Action::*;
        match self {
            NextPage => "Next page",
            PrevPage => "Previous page",
            NextPage10 => "Forward ten pages",
            PrevPage10 => "Back ten pages",
            FirstPage => "First page",
            LastPage => "Last page",
            GoToPage => "Go to page",
            ScrollUp => "Scroll up",
            ScrollDown => "Scroll down",
            ScrollLeft => "Scroll left",
            ScrollRight => "Scroll right",
            SmartScrollUp => "Smart scroll up",
            SmartScrollDown => "Smart scroll down",
            ZoomIn => "Zoom in",
            ZoomOut => "Zoom out",
            ZoomOriginal => "Normal size",
            FitBest => "Best fit mode",
            FitWidth => "Fit width mode",
            FitHeight => "Fit height mode",
            FitSize => "Fit size mode",
            FitManual => "Manual zoom mode",
            Rotate90 => "Rotate 90° clockwise",
            Rotate270 => "Rotate 90° counter-clockwise",
            Rotate180 => "Rotate 180°",
            FlipH => "Flip horizontally",
            FlipV => "Flip vertically",
            ToggleDoublePage => "Double page mode",
            ToggleManga => "Manga mode",
            ToggleFullscreen => "Fullscreen",
            ToggleThumbnails => "Show/hide thumbnails",
            ToggleSlideshow => "Start/stop slideshow",
            ToggleHideAll => "Show/hide all",
            ToggleMenubar => "Show/hide toolbar",
            InvertScroll => "Invert smart scroll",
            ShowInfo => "Show info panel",
            Minimize => "Minimize",
            ExitFullscreen => "Exit fullscreen",
            AddBookmark => "Add bookmark",
            EditBookmarks => "Edit bookmarks…",
        }
    }

    pub fn all() -> &'static [Action] {
        use Action::*;
        &[
            NextPage, PrevPage, NextPage10, PrevPage10, FirstPage, LastPage, GoToPage, ScrollUp,
            ScrollDown, ScrollLeft, ScrollRight, SmartScrollUp, SmartScrollDown, ZoomIn, ZoomOut,
            ZoomOriginal, FitBest, FitWidth, FitHeight, FitSize, FitManual, Rotate90, Rotate270,
            Rotate180, FlipH, FlipV, ToggleDoublePage, ToggleManga, ToggleFullscreen,
            ToggleThumbnails, ToggleSlideshow, ToggleHideAll, ToggleMenubar, InvertScroll,
            ShowInfo, Minimize, ExitFullscreen, AddBookmark, EditBookmarks,
        ]
    }
}

/// A key + modifier combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub key: Key,
    pub mods: ModifierType,
}

/// Parse an accelerator string like `Page_Down`, `<Ctrl>S`, `<Shift>F11`.
fn parse_accel(s: &str) -> Option<Binding> {
    let mut mods = ModifierType::empty();
    let mut rest = s;
    while let Some(open) = rest.find('<') {
        let close = rest[open..].find('>')? + open;
        let modifier = &rest[open + 1..close];
        match modifier {
            "Ctrl" | "Control" => mods |= ModifierType::CONTROL_MASK,
            "Shift" => mods |= ModifierType::SHIFT_MASK,
            "Alt" | "Mod1" => mods |= ModifierType::ALT_MASK,
            _ => return None,
        }
        rest = &rest[close + 1..];
    }
    // Normalise single-letter names so "S" behaves like "s" (keyvals are
    // lowercase for letters; Shift is a modifier, not a separate keyval).
    let key_name = if rest.len() == 1 && rest.as_bytes()[0].is_ascii_alphabetic() {
        &rest.to_ascii_lowercase()
    } else {
        rest
    };
    let key = Key::from_name(key_name)?;
    Some(Binding { key, mods: mods & MOD_MASK })
}

/// Format a binding for display, e.g. `Ctrl+Shift+S`.
pub fn format_binding(b: &Binding) -> String {
    let mut parts = Vec::new();
    if b.mods.contains(ModifierType::CONTROL_MASK) {
        parts.push("Ctrl");
    }
    if b.mods.contains(ModifierType::SHIFT_MASK) {
        parts.push("Shift");
    }
    if b.mods.contains(ModifierType::ALT_MASK) {
        parts.push("Alt");
    }
    let key_name = b.key.name().unwrap_or_else(|| "?".into());
    parts.push(&key_name);
    parts.join("+")
}

/// Reduce a modifier mask to the ones we care about (Shift/Ctrl/Alt).
pub fn normalize_mods(mods: ModifierType) -> ModifierType {
    mods & MOD_MASK
}

/// The full binding map: a list of (binding, action) pairs, checked in order.
#[derive(Debug, Clone, Default)]
pub struct BindingMap {
    map: Vec<(Binding, Action)>,
}

#[derive(Serialize, Deserialize, Default)]
struct BindingFile {
    #[serde(flatten)]
    actions: HashMap<String, Vec<String>>,
}

impl BindingMap {
    /// Default MComix-compatible bindings.
    pub fn defaults() -> BindingMap {
        use Action::*;
        let defs: &[(&str, &[&str])] = &[
            ("next_page", &["Page_Down", "KP_Page_Down", "<Alt>Right"]),
            ("previous_page", &["Page_Up", "KP_Page_Up", "BackSpace", "<Alt>Left"]),
            ("next_page_ff", &["<Shift>Page_Down", "<Shift>KP_Page_Down"]),
            ("previous_page_ff", &["<Shift>Page_Up", "<Shift>KP_Page_Up", "<Shift>BackSpace"]),
            ("first_page", &["Home", "KP_Home"]),
            ("last_page", &["End", "KP_End"]),
            ("go_to", &["g"]),
            ("scroll_up", &["Up", "KP_Up"]),
            ("scroll_down", &["Down", "KP_Down"]),
            ("scroll_left", &["Left", "KP_Left"]),
            ("scroll_right", &["Right", "KP_Right"]),
            ("smart_scroll_up", &["<Shift>space"]),
            ("smart_scroll_down", &["space"]),
            ("zoom_in", &["plus", "KP_Add", "equal"]),
            ("zoom_out", &["minus", "KP_Subtract"]),
            ("zoom_original", &["<Control>0", "KP_0"]),
            ("best_fit_mode", &["b"]),
            ("fit_width_mode", &["w"]),
            ("fit_height_mode", &["h"]),
            ("fit_size_mode", &["s"]),
            ("fit_manual_mode", &["a"]),
            ("rotate_90", &["r"]),
            ("rotate_270", &["<Shift>r"]),
            ("rotate_180", &[]),
            ("flip_horiz", &[]),
            ("flip_vert", &[]),
            ("double_page", &["d"]),
            ("manga_mode", &["m"]),
            ("fullscreen", &["f", "F11"]),
            ("thumbnails", &["F9"]),
            ("slideshow", &["<Control>s"]),
            ("hide_all", &["i"]),
            ("menubar", &["<Control>m"]),
            ("invert_scroll", &["x"]),
            ("osd_panel", &["Tab"]),
            ("minimize", &["n"]),
            ("exit_fullscreen", &["Escape"]),
            ("add_bookmark", &["<Control>d"]),
            ("edit_bookmarks", &["<Control>b"]),
        ];
        let mut map = Vec::new();
        for (name, accels) in defs {
            let action = Action::all().iter().find(|a| a.name() == *name).copied().unwrap();
            for accel in *accels {
                if let Some(b) = parse_accel(accel) {
                    map.push((b, action));
                }
            }
        }
        BindingMap { map }
    }

    pub fn config_path() -> PathBuf {
        crate::prefs::config_dir().join("keybindings.conf")
    }

    pub fn load() -> BindingMap {
        let mut map = BindingMap::defaults();
        let path = Self::config_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return map;
        };
        let Ok(file) = serde_json::from_str::<BindingFile>(&text) else {
            return map;
        };
        for (name, accels) in file.actions {
            let Some(action) = Action::all().iter().find(|a| a.name() == name).copied() else {
                continue;
            };
            // Replace the default bindings for this action.
            map.map.retain(|(_, a)| *a != action);
            for accel in accels {
                if let Some(b) = parse_accel(&accel) {
                    map.map.push((b, action));
                }
            }
        }
        map
    }

    pub fn save(&self) {
        let dir = crate::prefs::config_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("cannot create config dir {:?}: {e}", dir);
            return;
        }
        let mut actions: HashMap<String, Vec<String>> = HashMap::new();
        for (b, a) in &self.map {
            actions
                .entry(a.name().to_string())
                .or_default()
                .push(format_binding(b));
        }
        match serde_json::to_string_pretty(&BindingFile { actions }) {
            Ok(text) => {
                if let Err(e) = std::fs::write(Self::config_path(), text) {
                    log::warn!("cannot write keybindings: {e}");
                }
            }
            Err(e) => log::warn!("cannot serialize keybindings: {e}"),
        }
    }

    /// Look up the action bound to a key/modifier combination.
    pub fn lookup(&self, key: Key, mods: ModifierType) -> Option<Action> {
        let mods = mods & MOD_MASK;
        self.map
            .iter()
            .find(|(b, _)| b.key == key && b.mods == mods)
            .map(|(_, a)| *a)
    }

    pub fn bindings_for(&self, action: Action) -> Vec<Binding> {
        self.map
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(b, _)| *b)
            .collect()
    }

    /// Replace the first binding of `action` (or add one if it has none).
    pub fn set_binding(&mut self, action: Action, binding: Binding) {
        if let Some((b, _)) = self.map.iter_mut().find(|(_, a)| *a == action) {
            *b = binding;
        } else {
            self.map.push((binding, action));
        }
    }

    pub fn reset_action(&mut self, action: Action) {
        self.map.retain(|(_, a)| *a != action);
        let defaults = BindingMap::defaults();
        for (b, a) in defaults.map {
            if a == action {
                self.map.push((b, a));
            }
        }
    }

    pub fn reset_all(&mut self) {
        *self = BindingMap::defaults();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4::gdk::Key;

    #[test]
    fn parses_accelerators() {
        let b = parse_accel("<Ctrl><Shift>S").unwrap();
        assert_eq!(b.key, Key::s);
        assert!(b.mods.contains(ModifierType::CONTROL_MASK));
        assert!(b.mods.contains(ModifierType::SHIFT_MASK));

        let b = parse_accel("Page_Down").unwrap();
        assert_eq!(b.key, Key::Page_Down);
        assert_eq!(b.mods, ModifierType::empty());
    }

    #[test]
    fn lookup_matches_modifiers_exactly() {
        let map = BindingMap::defaults();
        assert_eq!(map.lookup(Key::Page_Down, ModifierType::empty()), Some(Action::NextPage));
        assert_eq!(map.lookup(Key::s, ModifierType::CONTROL_MASK), Some(Action::ToggleSlideshow));
        assert_eq!(map.lookup(Key::s, ModifierType::empty()), Some(Action::FitSize));
        // Plain Right is scroll; Alt+Right is next page (dynamic).
        assert_eq!(map.lookup(Key::Right, ModifierType::empty()), Some(Action::ScrollRight));
        assert_eq!(map.lookup(Key::Right, ModifierType::ALT_MASK), Some(Action::NextPage));
    }

    #[test]
    fn roundtrip_format() {
        let b = parse_accel("<Ctrl>0").unwrap();
        assert_eq!(format_binding(&b), "Ctrl+0");
    }

    #[test]
    fn set_and_reset() {
        let mut map = BindingMap::defaults();
        let b = parse_accel("q").unwrap();
        map.set_binding(Action::GoToPage, b);
        assert_eq!(map.lookup(Key::q, ModifierType::empty()), Some(Action::GoToPage));
        assert!(map.lookup(Key::g, ModifierType::empty()).is_none());
        map.reset_action(Action::GoToPage);
        assert_eq!(map.lookup(Key::g, ModifierType::empty()), Some(Action::GoToPage));
    }

    #[test]
    fn gdk_init_available() {
        // Key::from_name requires GDK to be initialised; tests run in the same
        // process as the app after gtk init, so this should always succeed.
        assert!(Key::from_name("Page_Down").is_some());
    }
}
