//! Bookmarks, mirroring `mcomix/bookmark_backend.py` + `bookmark_menu_item.py`.
//! Persisted as JSON (the Python version used pickle).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
    pub page: u32,
    pub numpages: u32,
    pub archive_type: Option<String>,
    pub date_added: u64, // Unix epoch seconds
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Bookmarks {
    pub items: Vec<Bookmark>,
}

impl Bookmarks {
    pub fn path() -> PathBuf {
        crate::prefs::data_dir().join("bookmarks.json")
    }

    pub fn load() -> Bookmarks {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Bookmarks::default(),
        }
    }

    pub fn save(&self) {
        let dir = crate::prefs::data_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            log::warn!("cannot create data dir {:?}: {e}", dir);
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = fs::write(Self::path(), text) {
                    log::warn!("cannot write bookmarks: {e}");
                }
            }
            Err(e) => log::warn!("cannot serialize bookmarks: {e}"),
        }
    }

    pub fn add(&mut self, b: Bookmark) {
        self.items.push(b);
        self.save();
    }

    pub fn remove_path(&mut self, path: &str) {
        let before = self.items.len();
        self.items.retain(|b| b.path != path);
        if self.items.len() != before {
            self.save();
        }
    }

    pub fn remove(&mut self, path: &str, page: u32) {
        let before = self.items.len();
        self.items.retain(|b| !(b.path == path && b.page == page));
        if self.items.len() != before {
            self.save();
        }
    }

    pub fn clear(&mut self) {
        if !self.items.is_empty() {
            self.items.clear();
            self.save();
        }
    }

    pub fn same_path(&self, path: &str) -> Vec<&Bookmark> {
        self.items.iter().filter(|b| b.path == path).collect()
    }
}

/// Format a Unix-epoch timestamp as `YYYY-MM-DD` (Howard Hinnant's
/// civil-from-days algorithm).
pub fn epoch_to_date(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u64;
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formatting() {
        // 2026-08-12 00:00:00 UTC
        let secs = 1786492800;
        assert_eq!(epoch_to_date(secs), "2026-08-12");
        assert_eq!(epoch_to_date(0), "1970-01-01");
    }

    #[test]
    fn roundtrip_json() {
        let mut b = Bookmarks::default();
        b.add(Bookmark {
            name: "test".into(),
            path: "/tmp/x.cbz".into(),
            page: 5,
            numpages: 20,
            archive_type: Some("ZIP".into()),
            date_added: 1783987200,
        });
        let text = serde_json::to_string(&b).unwrap();
        let back: Bookmarks = serde_json::from_str(&text).unwrap();
        assert_eq!(back.items.len(), 1);
        assert_eq!(back.items[0].page, 5);
        assert_eq!(back.same_path("/tmp/x.cbz").len(), 1);
    }
}
