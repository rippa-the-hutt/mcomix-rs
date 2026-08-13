//! CBT / TAR support (plain, gzip, bzip2, xz compressed).
//! Mirrors `mcomix/archive/tar.py` plus the XZ fallback in `sevenzip_external.py`.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use super::{Archive, ArchiveError, ArchiveKind, sorted_pages};

pub struct TarArchive {
    path: PathBuf,
    name: String,
    kind: Option<ArchiveKind>,
    pages: Option<Vec<String>>,
}

fn open_tar_reader(
    path: &Path,
    kind: Option<ArchiveKind>,
) -> Result<Box<dyn Read + Send>, ArchiveError> {
    let file = File::open(path)?;
    let reader: Box<dyn Read + Send> = match kind {
        None => Box::new(BufReader::new(file)),
        Some(ArchiveKind::Gzip) => Box::new(flate2::read::GzDecoder::new(file)),
        Some(ArchiveKind::Bzip2) => Box::new(bzip2::read::BzDecoder::new(file)),
        Some(ArchiveKind::Xz) => Box::new(xz2::read::XzDecoder::new(file)),
        _ => Box::new(BufReader::new(file)),
    };
    Ok(reader)
}

impl TarArchive {
    pub fn open(path: &Path, kind: Option<ArchiveKind>) -> Result<TarArchive, ArchiveError> {
        // Validate that it really is a tar stream.
        let mut reader = open_tar_reader(path, kind)?;
        let mut head = [0u8; 512];
        let n = reader.read(&mut head)?;
        if n < 512 || !(head[257..262] == *b"ustar") {
            return Err(ArchiveError::Other(format!(
                "'{}' is not a valid TAR archive",
                path.display()
            )));
        }
        Ok(TarArchive {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            kind,
            pages: None,
        })
    }

    fn read_all_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        let reader = open_tar_reader(&self.path, self.kind)?;
        let mut archive = tar::Archive::new(reader);
        let mut names = Vec::new();
        let entries = archive.entries()?;
        for entry in entries {
            let entry = entry?;
            let p = entry.path()?.into_owned();
            names.push(p.to_string_lossy().into_owned());
        }
        Ok(names)
    }
}

impl Archive for TarArchive {
    fn name(&self) -> &str {
        &self.name
    }

    fn raw_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        let mut names = self.read_all_names()?;
        names.retain(|n| !n.ends_with('/'));
        Ok(names)
    }

    fn page_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        if self.pages.is_none() {
            self.pages = Some(sorted_pages(self.raw_names()?));
        }
        Ok(self.pages.clone().unwrap_or_default())
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        let reader = open_tar_reader(&self.path, self.kind)?;
        let mut archive = tar::Archive::new(reader);
        let mut buf = Vec::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let p = entry.path()?.into_owned();
            if p.to_string_lossy() == name {
                entry.read_to_end(&mut buf)?;
                return Ok(buf);
            }
        }
        Err(ArchiveError::Other(format!(
            "entry '{name}' not found in '{}'",
            self.path.display()
        )))
    }
}
