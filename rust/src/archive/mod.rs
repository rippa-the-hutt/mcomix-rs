//! Comic archive support: format detection and per-format readers.
//!
//! Mirrors `mcomix/archive_tools.py` + `mcomix/archive/*`.

pub mod lha;
pub mod pdf;
pub mod rar;
pub mod sevenzip;
pub mod tar;
pub mod zip;

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::natsort;

/// All supported image file extensions (subset of what Pillow + GTK understand).
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff", "ico", "pcx", "ppm", "pbm",
    "pgm", "pnm", "tga", "qoi",
];

/// True if `name` looks like an image file (extension check, case-insensitive).
pub fn is_image_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(ext) = lower.rsplit('.').next() else {
        return false;
    };
    IMAGE_EXTENSIONS.contains(&ext)
}

/// Archive container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    Rar,
    Tar,
    SevenZip,
    Lha,
    Pdf,
    Gzip,
    Bzip2,
    Xz,
}

impl fmt::Display for ArchiveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ArchiveKind::Zip => "ZIP",
            ArchiveKind::Rar => "RAR",
            ArchiveKind::Tar => "TAR",
            ArchiveKind::SevenZip => "7Z",
            ArchiveKind::Lha => "LHA",
            ArchiveKind::Pdf => "PDF",
            ArchiveKind::Gzip => "GZIP",
            ArchiveKind::Bzip2 => "BZIP2",
            ArchiveKind::Xz => "XZ",
        };
        f.write_str(s)
    }
}

/// Errors produced while opening or reading archives.
#[derive(Debug)]
pub enum ArchiveError {
    Unsupported(String),
    Io(std::io::Error),
    Other(String),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::Unsupported(s) => write!(f, "unsupported archive: {s}"),
            ArchiveError::Io(e) => write!(f, "I/O error: {e}"),
            ArchiveError::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

impl From<std::io::Error> for ArchiveError {
    fn from(e: std::io::Error) -> Self {
        ArchiveError::Io(e)
    }
}

/// A comic archive: a list of page names plus on-demand byte access.
pub trait Archive: Send {
    /// The archive file name (as opened).
    fn name(&self) -> &str;

    /// Sorted list of image entries inside the archive.
    fn page_names(&mut self) -> Result<Vec<String>, ArchiveError>;

    /// Read the raw bytes of one entry (by name, as returned by `page_names`).
    fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError>;

    /// Number of image pages.
    fn num_pages(&mut self) -> Result<usize, ArchiveError> {
        Ok(self.page_names()?.len())
    }

    /// Release any held resources (temp dirs, file handles).
    fn close(&mut self) {}
}

/// Detect the archive kind from magic bytes (mirrors `archive_tools.archive_mime_type`).
pub fn detect(path: &Path) -> Option<ArchiveKind> {
    let mut f = File::open(path).ok()?;
    let mut magic = [0u8; 262];
    let n = f.read(&mut magic).ok()?;
    let magic = &magic[..n];

    // ZIP: PK\x03\x04 (normal), PK\x05\x06 (empty), PK\x07\x08 (spanned)
    if magic.starts_with(b"PK\x03\x04") || magic.starts_with(b"PK\x05\x06") || magic.starts_with(b"PK\x07\x08") {
        return Some(ArchiveKind::Zip);
    }
    // RAR
    if magic.starts_with(b"Rar!\x1a\x07") {
        return Some(ArchiveKind::Rar);
    }
    // 7z
    if magic.starts_with(b"7z\xbc\xaf") {
        return Some(ArchiveKind::SevenZip);
    }
    // PDF
    if magic.starts_with(b"%PDF") {
        return Some(ArchiveKind::Pdf);
    }
    // LHA / LZH (offset 2: "-l")
    if magic.len() > 4 && &magic[2..4] == b"-l" {
        return Some(ArchiveKind::Lha);
    }
    // Tar (ustar magic at offset 257)
    if magic.len() > 262 && &magic[257..262] == b"ustar" {
        if magic.starts_with(b"BZh") {
            return Some(ArchiveKind::Bzip2);
        }
        if magic.starts_with(&[0x1f, 0x8b]) {
            return Some(ArchiveKind::Gzip);
        }
        return Some(ArchiveKind::Tar);
    }
    // Tar compressed with xz / lzma (not recognized by `tar` directly)
    if magic.starts_with(&[0xfd, b'7', b'z', b'X', b'Z']) || magic.starts_with(&[0x5d, 0x00, 0x00, 0x80, 0x00]) {
        return Some(ArchiveKind::Xz);
    }
    None
}

/// Open an archive and return a boxed reader.
pub fn open(path: &Path) -> Result<Box<dyn Archive>, ArchiveError> {
    if path.is_dir() {
        return Ok(Box::new(DirArchive::open(path)));
    }
    let kind = detect(path).ok_or_else(|| ArchiveError::Unsupported(path.display().to_string()))?;
    open_with_kind(path, kind)
}

/// Open an archive of an already-detected kind.
pub fn open_with_kind(path: &Path, kind: ArchiveKind) -> Result<Box<dyn Archive>, ArchiveError> {
    log::info!("detected {kind} archive: {}", path.display());
    match kind {
        ArchiveKind::Zip | ArchiveKind::Gzip | ArchiveKind::Bzip2 | ArchiveKind::Xz => {
            // Gzip/bzip2/xz always wrap a tar in the comic world; the zip reader
            // handles plain CBZ. Compressed tars are dispatched to the tar reader.
            if kind == ArchiveKind::Zip {
                Ok(Box::new(zip::ZipArchive::open(path)?))
            } else {
                Ok(Box::new(tar::TarArchive::open(path, Some(kind))?))
            }
        }
        ArchiveKind::Tar => Ok(Box::new(tar::TarArchive::open(path, None)?)),
        ArchiveKind::SevenZip => Ok(Box::new(sevenzip::SevenZipArchive::open(path)?)),
        ArchiveKind::Rar => Ok(Box::new(crate::archive::rar::RarArchive::open(path)?)),
        ArchiveKind::Lha => Ok(Box::new(crate::archive::lha::LhaArchive::open(path)?)),
        ArchiveKind::Pdf => Ok(Box::new(crate::archive::pdf::PdfArchive::open(path)?)),
    }
}

/// Convenience: sort page names naturally and filter to images only.
pub fn sorted_pages(names: Vec<String>) -> Vec<String> {
    let mut v: Vec<String> = names.into_iter().filter(|n| is_image_file(n)).collect();
    natsort::natural_sort(&mut v);
    v
}

/// Find an executable on PATH.
pub(crate) fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{cmd}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Run a command and capture stdout (and stderr on failure).
pub(crate) fn run_capture(prog: &str, args: &[&str]) -> Result<Vec<u8>, ArchiveError> {
    let out = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| ArchiveError::Other(format!("failed to run {prog}: {e}")))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(ArchiveError::Other(format!(
            "{prog} failed: {}",
            stderr.trim()
        )));
    }
    Ok(out.stdout)
}

/// A directory of loose images (opened directly, like MComix does).
pub struct DirArchive {
    dir: PathBuf,
    name: String,
    pages: Option<Vec<String>>,
}

impl DirArchive {
    pub fn open(path: &Path) -> DirArchive {
        DirArchive {
            dir: path.to_path_buf(),
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            pages: None,
        }
    }
}

impl Archive for DirArchive {
    fn name(&self) -> &str {
        &self.name
    }

    fn page_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        if self.pages.is_none() {
            let mut names = Vec::new();
            for entry in std::fs::read_dir(&self.dir)? {
                let entry = entry?;
                let p = entry.path();
                if p.is_file() {
                    names.push(p.to_string_lossy().into_owned());
                }
            }
            self.pages = Some(sorted_pages(names));
        }
        Ok(self.pages.clone().unwrap_or_default())
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        Ok(std::fs::read(name)?)
    }
}
