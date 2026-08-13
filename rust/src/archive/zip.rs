//! CBZ / ZIP support via the pure-Rust `zip` crate.
//! Mirrors `mcomix/archive/zip.py`.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::{Archive, ArchiveError, sorted_pages};

pub struct ZipArchive {
    path: PathBuf,
    name: String,
    archive: zip::ZipArchive<File>,
    pages: Option<Vec<String>>,
}

impl ZipArchive {
    pub fn open(path: &Path) -> Result<ZipArchive, ArchiveError> {
        let file = File::open(path)?;
        let archive = zip::ZipArchive::new(file).map_err(|e| {
            ArchiveError::Other(format!("not a valid ZIP archive: {e}"))
        })?;
        Ok(ZipArchive {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            archive,
            pages: None,
        })
    }
}

impl Archive for ZipArchive {
    fn name(&self) -> &str {
        &self.name
    }

    fn raw_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        Ok(self
            .archive
            .file_names()
            .map(|s| s.to_string())
            .filter(|n| !n.ends_with('/'))
            .collect())
    }

    fn page_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        if self.pages.is_none() {
            let names = self.raw_names()?;
            self.pages = Some(sorted_pages(names));
        }
        Ok(self.pages.clone().unwrap_or_default())
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        let mut file = self.archive.by_name(name).map_err(|e| {
            ArchiveError::Other(format!("cannot read '{name}' from '{}': {e}", self.path.display()))
        })?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn archive_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("test")
            .join("files")
            .join("archives")
            .join(name)
    }

    #[test]
    fn lists_and_reads_zip() {
        let path = archive_path("01-ZIP-Normal.zip");
        if !path.exists() {
            eprintln!("skipping: test archive not present");
            return;
        }
        let mut ar = ZipArchive::open(&path).expect("open zip");
        let pages = ar.page_names().expect("list pages");
        assert!(!pages.is_empty(), "expected at least one image page");
        let bytes = ar.read(&pages[0]).expect("read first page");
        assert!(image::guess_format(&bytes).is_ok());
    }
}
