//! LHA / LZH support via external tools (`7z` preferred for Unicode support,
//! `lha` fallback), mirroring `mcomix/archive/lha_external.py` plus the
//! `sevenzip_external.py` preference.

use std::path::{Path, PathBuf};

use super::{Archive, ArchiveError, run_capture, sorted_pages, which};

pub struct LhaArchive {
    path: PathBuf,
    name: String,
    /// "7z" or "lha".
    backend: String,
    pages: Option<Vec<String>>,
}

impl LhaArchive {
    pub fn open(path: &Path) -> Result<LhaArchive, ArchiveError> {
        let backend = if which("7z").is_some()
            || which("7zz").is_some()
            || which("7za").is_some()
            || which("7zr").is_some()
        {
            "7z".to_string()
        } else if which("lha").is_some() {
            "lha".to_string()
        } else {
            return Err(ArchiveError::Other(
                "LHA support requires the '7z' or 'lha' executable to be installed".to_string(),
            ));
        };
        let archive = LhaArchive {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            backend,
            pages: None,
        };
        // Validate that the archive can be listed.
        let _ = archive.list_raw()?;
        Ok(archive)
    }

    fn list_raw(&self) -> Result<Vec<String>, ArchiveError> {
        let p = self.path.to_string_lossy().into_owned();
        let raw = match self.backend.as_str() {
            "7z" => run_capture("7z", &["l", "-slt", &p])?,
            _ => run_capture("lha", &["l", "-g", "-q2", &p])?,
        };
        let text = String::from_utf8_lossy(&raw);
        let mut names = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if self.backend == "7z" {
                if let Some(rest) = line.strip_prefix("Path = ") {
                    names.push(rest.to_string());
                }
            } else if let Some(rest) = line.strip_prefix("[generic]") {
                // `lha l -g -q2` verbose rows look like:
                //   [generic] <size> <uid> <gid> <date> <time> <name...>
                let tokens: Vec<&str> = rest.split_whitespace().collect();
                if tokens.len() > 5 {
                    names.push(tokens[5..].join(" "));
                }
            }
        }
        Ok(names)
    }
}

impl Archive for LhaArchive {
    fn name(&self) -> &str {
        &self.name
    }

    fn page_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        if self.pages.is_none() {
            self.pages = Some(sorted_pages(self.list_raw()?));
        }
        Ok(self.pages.clone().unwrap_or_default())
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        let p = self.path.to_string_lossy().into_owned();
        match self.backend.as_str() {
            "7z" => run_capture("7z", &["e", "-so", &p, name]),
            _ => run_capture("lha", &["p", "-q2", &p, name]),
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
    fn lists_and_reads_lha() {
        if which("7z").is_none() && which("lha").is_none() {
            eprintln!("skipping: neither 7z nor lha available");
            return;
        }
        let path = archive_path("Flat.lha");
        if !path.exists() {
            eprintln!("skipping: test archive not present");
            return;
        }
        let mut ar = LhaArchive::open(&path).expect("open lha");
        let pages = ar.page_names().expect("list pages");
        assert_eq!(pages.len(), 4);
        let bytes = ar.read(&pages[0]).expect("read first page");
        assert!(image::guess_format(&bytes).is_ok());
    }
}
