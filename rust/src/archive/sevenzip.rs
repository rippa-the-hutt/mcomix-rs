//! CB7 / 7z support.
//!
//! Prefers the external `7z` binary (like the Python port's
//! `sevenzip_external.py`), which is the most reliable across the wild variety
//! of 7z archives (solid, PPMd, BCJ filters, encryption headers, …). Falls
//! back to the pure-Rust `sevenz-rust` crate when no 7z binary is available
//! (e.g. minimal Windows bundles).

use std::io::Read;
use std::path::{Path, PathBuf};

use super::{Archive, ArchiveError, run_capture, sorted_pages, which};

pub struct SevenZipArchive {
    path: PathBuf,
    name: String,
    /// True when the external `7z` binary is used instead of sevenz-rust.
    external: bool,
    pages: Option<Vec<String>>,
}

const SEVENZ_BINARIES: &[&str] = &["7z", "7zz", "7za", "7zr"];

fn find_sevenz() -> Option<String> {
    SEVENZ_BINARIES
        .iter()
        .find(|b| which(b).is_some())
        .map(|s| s.to_string())
}

impl SevenZipArchive {
    pub fn open(path: &Path) -> Result<SevenZipArchive, ArchiveError> {
        let external = find_sevenz();
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        match &external {
            Some(prog) => {
                let p = path.to_string_lossy().into_owned();
                // Validate that the archive can be listed.
                run_capture(prog, &["l", "-slt", &p])
                    .map_err(|e| ArchiveError::Other(format!("not a valid 7z archive: {e}")))?;
            }
            None => {
                sevenz_rust::SevenZReader::<std::fs::File>::open(
                    path,
                    sevenz_rust::Password::empty(),
                )
                .map_err(|e| ArchiveError::Other(format!("not a valid 7z archive: {e}")))?;
            }
        }
        Ok(SevenZipArchive {
            path: path.to_path_buf(),
            name,
            external: external.is_some(),
            pages: None,
        })
    }
}

impl Archive for SevenZipArchive {
    fn name(&self) -> &str {
        &self.name
    }

    fn page_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        if self.pages.is_none() {
            let names = if self.external {
                let prog = find_sevenz().unwrap();
                let p = self.path.to_string_lossy().into_owned();
                let raw = run_capture(&prog, &["l", "-slt", &p])?;
                let text = String::from_utf8_lossy(&raw);
                let mut names = Vec::new();
                for line in text.lines() {
                    if let Some(rest) = line.strip_prefix("Path = ") {
                        names.push(rest.to_string());
                    }
                }
                names
            } else {
                let mut reader = sevenz_rust::SevenZReader::<std::fs::File>::open(
                    &self.path,
                    sevenz_rust::Password::empty(),
                )
                .map_err(|e| ArchiveError::Other(format!("cannot open 7z: {e}")))?;
                let mut names = Vec::new();
                reader
                    .for_each_entries(|entry, _| {
                        names.push(entry.name.clone());
                        Ok(true)
                    })
                    .map_err(|e| ArchiveError::Other(format!("cannot list 7z entries: {e}")))?;
                names
            };
            self.pages = Some(sorted_pages(names));
        }
        Ok(self.pages.clone().unwrap_or_default())
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        if self.external {
            let prog = find_sevenz().unwrap();
            let p = self.path.to_string_lossy().into_owned();
            run_capture(&prog, &["e", "-so", &p, name])
        } else {
            let mut reader = sevenz_rust::SevenZReader::<std::fs::File>::open(
                &self.path,
                sevenz_rust::Password::empty(),
            )
            .map_err(|e| ArchiveError::Other(format!("cannot open 7z: {e}")))?;
            let mut buf: Vec<u8> = Vec::new();
            let mut found = false;
            reader
                .for_each_entries(|entry, r| {
                    if entry.name == name && !found {
                        let mut tmp = Vec::new();
                        r.read_to_end(&mut tmp).map_err(sevenz_rust::Error::io)?;
                        buf = tmp;
                        found = true;
                        // Returning false stops iteration once the entry is found.
                        return Ok(false);
                    }
                    Ok(true)
                })
                .map_err(|e| ArchiveError::Other(format!("cannot read 7z entry: {e}")))?;
            if found {
                Ok(buf)
            } else {
                Err(ArchiveError::Other(format!(
                    "entry '{name}' not found in '{}'",
                    self.path.display()
                )))
            }
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
    fn lists_and_reads_7z() {
        let path = archive_path("04-7Z-Normal.7z");
        if !path.exists() {
            eprintln!("skipping: test archive not present");
            return;
        }
        let mut ar = SevenZipArchive::open(&path).expect("open 7z");
        let pages = ar.page_names().expect("list pages");
        assert!(!pages.is_empty(), "expected at least one image page");
        let bytes = ar.read(&pages[0]).expect("read first page");
        assert!(!bytes.is_empty());
        assert!(
            image::guess_format(&bytes).is_ok(),
            "first page should be a decodable image"
        );
    }

    #[test]
    fn lists_and_reads_solid_7z() {
        let path = archive_path("SolidFlat.7z");
        if !path.exists() {
            eprintln!("skipping: test archive not present");
            return;
        }
        let mut ar = SevenZipArchive::open(&path).expect("open solid 7z");
        let pages = ar.page_names().expect("list pages");
        assert!(!pages.is_empty());
        let bytes = ar.read(&pages[0]).expect("read first page of solid 7z");
        assert!(image::guess_format(&bytes).is_ok());
    }
}
