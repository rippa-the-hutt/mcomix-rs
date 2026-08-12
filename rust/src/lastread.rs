//! "Last read page" bookkeeping, stored as a small JSON database.
//! Mirrors `mcomix/last_read_page.py` (which uses sqlite; JSON keeps this port
//! dependency-light for now).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LastReadDb {
    #[serde(flatten)]
    pub pages: HashMap<String, u32>,
}

impl LastReadDb {
    pub fn path() -> PathBuf {
        crate::prefs::data_dir().join("lastreadpage.json")
    }

    pub fn load() -> LastReadDb {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => LastReadDb::default(),
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
                    log::warn!("cannot write last-read db: {e}");
                }
            }
            Err(e) => log::warn!("cannot serialize last-read db: {e}"),
        }
    }

    pub fn get(path: &Path) -> Option<u32> {
        Self::load().pages.get(&path.to_string_lossy().into_owned()).copied()
    }

    pub fn set(path: &Path, page: u32) {
        let mut db = Self::load();
        db.pages.insert(path.to_string_lossy().into_owned(), page);
        db.save();
    }
}
