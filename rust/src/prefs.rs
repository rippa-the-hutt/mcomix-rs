//! Preferences, stored as JSON in the config directory.
//! Mirrors `mcomix/preferences.py` (key names kept identical where possible so
//! users can migrate their old `preferences.conf` by hand).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Home directory: `$HOME` on Unix, `%APPDATA%` on Windows (like MComix).
pub fn home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Platform-aware config directory (~/.config/mcomix-rs, or %APPDATA%\mcomix-rs).
pub fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        home_dir().join("mcomix-rs")
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".config"))
            .join("mcomix-rs")
    }
}

/// Platform-aware data directory (~/.local/share/mcomix-rs, or %APPDATA%\mcomix-rs).
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        home_dir().join("mcomix-rs")
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".local").join("share"))
            .join("mcomix-rs")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct Prefs {
    // View mode
    pub zoom_mode: i32,
    pub default_double_page: bool,
    pub default_manga_mode: bool,
    pub default_fullscreen: bool,
    pub fit_to_size_mode: i32,
    pub fit_to_size_px: u32,
    pub rotation: i32,
    pub auto_rotate_from_exif: bool,
    pub auto_rotate_depending_on_size: i32,
    pub scale_up: bool,
    pub stretch: bool,
    pub keep_transformation: bool,
    pub double_step_in_double_page_mode: bool,
    pub virtual_double_page_for_fitting_images: i32,

    // Appearance
    pub bg_color: [u16; 3],
    pub thumb_bg_color: [u16; 3],
    pub smart_bg: bool,
    pub smart_thumb_bg: bool,
    pub thumbnail_bg_uses_main_color: bool,
    pub checkered_bg_for_transparent_images: bool,
    pub show_menubar: bool,
    pub show_toolbar: bool,
    pub show_statusbar: bool,
    pub show_thumbnails: bool,
    pub show_scrollbar: bool,
    pub hide_all: bool,
    pub hide_all_in_fullscreen: bool,
    pub statusbar_fields: u32,
    pub thumbnail_size: u32,
    pub show_page_numbers_on_thumbnails: bool,
    pub create_thumbnails: bool,

    // Navigation / scrolling
    pub number_of_pixels_to_scroll_per_key_event: u32,
    pub number_of_pixels_to_scroll_per_mouse_wheel_event: u32,
    pub smart_scroll: bool,
    pub invert_smart_scroll: bool,
    pub smart_scroll_percentage: f64,
    pub number_of_key_presses_before_page_turn: u32,
    pub flip_with_wheel: bool,
    pub wrap_mouse_scroll: bool,
    pub escape_quits: bool,
    pub auto_open_next_archive: bool,
    pub auto_open_next_directory: bool,

    // Slideshow
    pub slideshow_delay: u64,
    pub slideshow_can_go_to_next_archive: bool,

    // Files
    pub auto_load_last_file: bool,
    pub page_of_last_file: u32,
    pub path_to_last_file: String,
    pub store_recent_file_info: bool,
    pub sort_archive_by: i32,
    pub sort_archive_order: i32,
    pub sort_by: i32,
    pub sort_order: i32,
    pub cache: bool,
    pub max_pages_to_cache: u32,
    pub scaling_quality: i32,
    pub animation_mode: i32,

    // Open-with commands: (label, command)
    pub openwith_commands: Vec<(String, String)>,
    // Image enhancement
    pub brightness: f64,
    pub contrast: f64,
    pub auto_contrast: bool,
    pub show_osd: bool,
    pub lens_size: u32,
    pub lens_magnification: u32,
    pub language: String,
    /// GTK theme: "system" (follow the OS), "dark", or "light".
    pub gtk_theme: String,
    pub ask_resume_from_last_page: bool,

    // Window geometry
    pub window_x: i32,
    pub window_y: i32,
    pub window_width: i32,
    pub window_height: i32,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            zoom_mode: 0, // ZOOM_MODE_BEST
            default_double_page: false,
            default_manga_mode: false,
            default_fullscreen: false,
            fit_to_size_mode: 2, // ZOOM_MODE_HEIGHT
            fit_to_size_px: 1800,
            rotation: 0,
            auto_rotate_from_exif: true,
            auto_rotate_depending_on_size: 0, // AUTOROTATE_NEVER
            scale_up: false,
            stretch: false,
            keep_transformation: false,
            double_step_in_double_page_mode: true,
            virtual_double_page_for_fitting_images: 3, // SHOW_DOUBLE_AS_ONE_TITLE|WIDE
            // Dark grey (0x202020) instead of the old reddish default.
            bg_color: [0x2020, 0x2020, 0x2020],
            thumb_bg_color: [0x2020, 0x2020, 0x2020],
            smart_bg: false,
            smart_thumb_bg: false,
            thumbnail_bg_uses_main_color: false,
            checkered_bg_for_transparent_images: true,
            show_menubar: true,
            show_toolbar: true,
            show_statusbar: true,
            show_thumbnails: true,
            show_scrollbar: true,
            hide_all: false,
            hide_all_in_fullscreen: true,
            statusbar_fields: 63, // all fields
            thumbnail_size: 80,
            show_page_numbers_on_thumbnails: true,
            create_thumbnails: true,
            number_of_pixels_to_scroll_per_key_event: 50,
            number_of_pixels_to_scroll_per_mouse_wheel_event: 50,
            smart_scroll: true,
            invert_smart_scroll: false,
            smart_scroll_percentage: 0.5,
            number_of_key_presses_before_page_turn: 3,
            flip_with_wheel: true,
            wrap_mouse_scroll: false,
            escape_quits: false,
            auto_open_next_archive: true,
            auto_open_next_directory: true,
            slideshow_delay: 3000,
            slideshow_can_go_to_next_archive: true,
            auto_load_last_file: false,
            page_of_last_file: 1,
            path_to_last_file: String::new(),
            store_recent_file_info: true,
            sort_archive_by: 1, // SORT_NAME
            sort_archive_order: 2, // SORT_ASCENDING
            sort_by: 1,
            sort_order: 2,
            cache: true,
            max_pages_to_cache: 7,
            scaling_quality: 2, // BILINEAR
            animation_mode: 0,  // ANIMATION_DISABLED
            openwith_commands: Vec::new(),
            brightness: 1.0,
            contrast: 1.0,
            auto_contrast: false,
            show_osd: true,
            lens_size: 200,
            lens_magnification: 2,
            language: "auto".to_string(),
            gtk_theme: "system".to_string(),
            ask_resume_from_last_page: true,
            window_x: 0,
            window_y: 0,
            window_width: 1024,
            window_height: 720,
        }
    }
}

impl Prefs {
    pub fn path() -> PathBuf {
        config_dir().join("preferences.conf")
    }

    pub fn load() -> Prefs {
        let path = Self::path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Prefs::default();
        };
        // Start from the full defaults and overlay only the keys actually
        // present in the file. This keeps correct values for fields added
        // after an older preferences.conf was written (plain `#[serde(default)]`
        // would fill missing fields with the *type* default, e.g. false/0).
        let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text)
        else {
            log::warn!("could not parse preferences file {:?}; using defaults", path);
            return Prefs::default();
        };
        let mut p = Prefs::default();
        macro_rules! ov {
            ($f:ident) => {
                if let Some(v) = map.get(stringify!($f)) {
                    if let Ok(x) = serde_json::from_value::<_>(v.clone()) {
                        p.$f = x;
                    }
                }
            };
        }
        ov!(zoom_mode);
        ov!(default_double_page);
        ov!(default_manga_mode);
        ov!(default_fullscreen);
        ov!(fit_to_size_mode);
        ov!(fit_to_size_px);
        ov!(rotation);
        ov!(auto_rotate_from_exif);
        ov!(auto_rotate_depending_on_size);
        ov!(scale_up);
        ov!(stretch);
        ov!(keep_transformation);
        ov!(double_step_in_double_page_mode);
        ov!(virtual_double_page_for_fitting_images);
        ov!(bg_color);
        ov!(thumb_bg_color);
        ov!(smart_bg);
        ov!(smart_thumb_bg);
        ov!(thumbnail_bg_uses_main_color);
        ov!(checkered_bg_for_transparent_images);
        ov!(show_menubar);
        ov!(show_toolbar);
        ov!(show_statusbar);
        ov!(show_thumbnails);
        ov!(show_scrollbar);
        ov!(hide_all);
        ov!(hide_all_in_fullscreen);
        ov!(statusbar_fields);
        ov!(thumbnail_size);
        ov!(show_page_numbers_on_thumbnails);
        ov!(create_thumbnails);
        ov!(number_of_pixels_to_scroll_per_key_event);
        ov!(number_of_pixels_to_scroll_per_mouse_wheel_event);
        ov!(smart_scroll);
        ov!(invert_smart_scroll);
        ov!(smart_scroll_percentage);
        ov!(flip_with_wheel);
        ov!(wrap_mouse_scroll);
        ov!(escape_quits);
        ov!(auto_open_next_archive);
        ov!(auto_open_next_directory);
        ov!(slideshow_delay);
        ov!(slideshow_can_go_to_next_archive);
        ov!(auto_load_last_file);
        ov!(page_of_last_file);
        ov!(path_to_last_file);
        ov!(store_recent_file_info);
        ov!(sort_archive_by);
        ov!(sort_archive_order);
        ov!(sort_by);
        ov!(sort_order);
        ov!(cache);
        ov!(max_pages_to_cache);
        ov!(scaling_quality);
        ov!(animation_mode);
        ov!(openwith_commands);
        ov!(brightness);
        ov!(contrast);
        ov!(auto_contrast);
        ov!(show_osd);
        ov!(lens_size);
        ov!(lens_magnification);
        ov!(language);
        ov!(gtk_theme);
        ov!(ask_resume_from_last_page);
        ov!(number_of_key_presses_before_page_turn);
        ov!(window_x);
        ov!(window_y);
        ov!(window_width);
        ov!(window_height);
        p
    }

    pub fn save(&self) {
        let dir = config_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            log::warn!("cannot create config dir {:?}: {e}", dir);
            return;
        }
        let path = Self::path();
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = fs::write(&path, text) {
                    log::warn!("cannot write preferences to {:?}: {e}", path);
                }
            }
            Err(e) => log::warn!("cannot serialize preferences: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_keep_defaults() {
        // A config written before newer fields existed must keep the
        // defaults for those fields (e.g. ask_resume_from_last_page = true).
        let text = r#"{"zoom_mode": 4, "window_width": 800}"#;
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(text).unwrap();
        let mut p = Prefs::default();
        macro_rules! ov {
            ($f:ident) => {
                if let Some(v) = map.get(stringify!($f)) {
                    if let Ok(x) = serde_json::from_value::<_>(v.clone()) {
                        p.$f = x;
                    }
                }
            };
        }
        ov!(zoom_mode);
        ov!(window_width);
        ov!(ask_resume_from_last_page);
        ov!(show_osd);
        ov!(lens_size);
        ov!(brightness);
        assert_eq!(p.zoom_mode, 4);
        assert_eq!(p.window_width, 800);
        assert!(p.ask_resume_from_last_page, "missing bool keeps default true");
        assert!(p.show_osd, "missing bool keeps default true");
        assert_eq!(p.lens_size, 200);
        assert!((p.brightness - 1.0).abs() < 1e-9);
    }
}
