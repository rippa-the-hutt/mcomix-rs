//! Transparently handle archives embedded inside archives, mirroring
//! `mcomix/archive/archive_recursive.py`. Embedded archives are extracted to
//! a temporary directory, opened recursively, and their pages are exposed
//! with a path prefix (the embedded archive's entry name).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{Archive, ArchiveError, detect, open, sorted_pages};

/// Extensions considered embedded-archive candidates (mirrors
/// `archive_tools.is_archive_file`).
const SUBARCHIVE_EXTENSIONS: &[&str] = &[
    "zip", "cbz", "rar", "cbr", "7z", "cb7", "tar", "cbt", "gz", "bz2", "xz", "lha", "lzh", "pdf",
];

fn is_subarchive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    match lower.rsplit('.').next() {
        Some(ext) => SUBARCHIVE_EXTENSIONS.contains(&ext),
        None => false,
    }
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct RecursiveArchive {
    name: String,
    /// All archives: index 0 is the main one; later entries are embedded
    /// archives (themselves possibly recursive).
    archives: Vec<Box<dyn Archive>>,
    /// Display name -> (archive index, entry name inside that archive).
    map: HashMap<String, (usize, String)>,
    tmp_dir: Option<PathBuf>,
    listed: bool,
    pages: Vec<String>,
}

impl RecursiveArchive {
    pub fn new(main: Box<dyn Archive>) -> RecursiveArchive {
        let name = main.name().to_string();
        let mut archives: Vec<Box<dyn Archive>> = Vec::new();
        archives.push(main);
        RecursiveArchive {
            name,
            archives,
            map: HashMap::new(),
            tmp_dir: None,
            listed: false,
            pages: Vec::new(),
        }
    }

    fn ensure_tmp(&mut self) -> PathBuf {
        if self.tmp_dir.is_none() {
            let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "mcomix-rs-sub-{}-{n}",
                std::process::id()
            ));
            let _ = std::fs::create_dir_all(&dir);
            self.tmp_dir = Some(dir.clone());
            dir
        } else {
            self.tmp_dir.clone().unwrap()
        }
    }

    /// List all pages, expanding embedded archives. Returns display names.
    fn do_list(&mut self) -> Result<Vec<String>, ArchiveError> {
        let tmp = self.ensure_tmp();
        let mut out: Vec<String> = Vec::new();
        // Work through the archive stack; archives[i].raw_names() gives all
        // entries of that archive.
        let mut i = 0usize;
        loop {
            if i >= self.archives.len() {
                break;
            }
            let entries = self.archives[i].raw_names()?;
            let mut subs: Vec<(String, usize)> = Vec::new();
            for entry in &entries {
                if is_subarchive(entry) {
                    subs.push((entry.clone(), i));
                } else {
                    out.push(entry.clone());
                    self.map.insert(entry.clone(), (i, entry.clone()));
                }
            }
            // Open embedded archives (after listing the current one, like the
            // Python implementation, which extracts sub-archives last).
            for (entry, parent) in subs {
                let bytes = match self.archives[parent].read(&entry) {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("cannot read embedded archive '{}': {e}", entry);
                        continue;
                    }
                };
                let ext = entry
                    .rsplit('.')
                    .next()
                    .unwrap_or("arc")
                    .to_ascii_lowercase();
                let path = tmp.join(format!(
                    "sub-{:04}.{ext}",
                    self.archives.len()
                ));
                if let Err(e) = std::fs::write(&path, &bytes) {
                    log::warn!("cannot write embedded archive {:?}: {e}", path);
                    continue;
                }
                let sub = match open(&path) {
                    Ok(a) => a,
                    Err(e) => {
                        log::warn!("cannot open embedded archive '{}': {e}", entry);
                        continue;
                    }
                };
                let sub_idx = self.archives.len();
                self.archives.push(sub);
                // List the sub-archive's pages (this also expands any nested
                // archives inside it).
                let sub_pages = match self.archives[sub_idx].page_names() {
                    Ok(p) => p,
                    Err(e) => {
                        log::warn!("cannot list embedded archive '{}': {e}", entry);
                        continue;
                    }
                };
                for sp in sub_pages {
                    let display = format!("{entry}/{sp}");
                    out.push(display.clone());
                    self.map.insert(display, (sub_idx, sp));
                }
            }
            i += 1;
        }
        // Natural sort of the flat display list (sub-archive groups sort by
        // their prefix).
        let mut sorted = sorted_pages(out);
        // sorted_pages filters images; embedded archives are already expanded
        // so nothing extra is lost. Keep only what was mapped.
        sorted.retain(|n| self.map.contains_key(n));
        Ok(sorted)
    }
}

impl Archive for RecursiveArchive {
    fn name(&self) -> &str {
        &self.name
    }

    fn raw_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        self.page_names()
    }

    fn page_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        if !self.listed {
            self.pages = self.do_list()?;
            self.listed = true;
        }
        Ok(self.pages.clone())
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        if !self.listed {
            self.page_names()?;
        }
        let (idx, entry) = self
            .map
            .get(name)
            .ok_or_else(|| ArchiveError::Other(format!("entry '{name}' not found")))?;
        let idx = *idx;
        let entry = entry.clone();
        self.archives[idx].read(&entry)
    }

    fn close(&mut self) {
        for a in &mut self.archives {
            a.close();
        }
        if let Some(dir) = self.tmp_dir.take() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

impl Drop for RecursiveArchive {
    fn drop(&mut self) {
        for a in &mut self.archives {
            a.close();
        }
        if let Some(dir) = self.tmp_dir.take() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("test")
            .join("files")
            .join("archives")
            .join(name)
    }

    #[test]
    fn expands_embedded_archives() {
        // embedded_red_and_blues_rar.rar contains red.png + blues.7z
        // (itself containing blue.png).
        let path = archive_path("embedded_red_and_blues_rar.rar");
        if !path.exists() {
            eprintln!("skipping: test archive not present");
            return;
        }
        if detect(&path).is_none() {
            eprintln!("skipping: no backend for this archive");
            return;
        }
        let mut ar = crate::archive::open(&path).expect("open recursive archive");
        let pages = ar.page_names().expect("list pages");
        eprintln!("pages: {pages:?}");
        assert!(
            pages.iter().any(|p| p == "red.png"),
            "expected red.png from the outer archive"
        );
        assert!(
            pages.iter().any(|p| p.ends_with("blue0.png")),
            "expected blue0.png from the embedded 7z archive"
        );
        // Reading the embedded page works through the mapping.
        let blue = pages.iter().find(|p| p.ends_with("blue0.png")).unwrap().clone();
        let bytes = ar.read(&blue).expect("read embedded page");
        assert!(image::guess_format(&bytes).is_ok());
    }
}
