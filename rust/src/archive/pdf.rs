//! PDF support via MuPDF's `mutool` (mirrors `mcomix/archive/pdf_external.py`,
//! simplified: fixed render DPI instead of the optimal-DPI trace pass).
//!
//! Pages are rendered on demand to stdout (`mutool draw -o -`), so no temp
//! directory is needed.

use std::path::{Path, PathBuf};

use super::{Archive, ArchiveError, run_capture, which};

/// Rendering resolution in DPI (72 * 3) — a good balance of quality/speed for
/// comics. The Python port computes an optimal DPI per page; that refinement
/// can be ported later if needed.
const PDF_RENDER_DPI: u32 = 216;

pub struct PdfArchive {
    path: PathBuf,
    name: String,
    pages: Option<Vec<String>>,
}

impl PdfArchive {
    pub fn open(path: &Path) -> Result<PdfArchive, ArchiveError> {
        if which("mutool").is_none() {
            return Err(ArchiveError::Other(
                "PDF support requires the 'mutool' executable (mupdf-tools)".to_string(),
            ));
        }
        Ok(PdfArchive {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            pages: None,
        })
    }
}

impl Archive for PdfArchive {
    fn name(&self) -> &str {
        &self.name
    }

    fn page_names(&mut self) -> Result<Vec<String>, ArchiveError> {
        if self.pages.is_none() {
            let p = self.path.to_string_lossy().into_owned();
            // `mutool show -- <pdf> pages` prints one `page N = ...` line per
            // page. Names match the Python port ("N.png").
            let raw = run_capture("mutool", &["show", "--", &p, "pages"])?;
            let text = String::from_utf8_lossy(&raw);
            let mut names = Vec::new();
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("page ") {
                    if let Some(num) = rest.split_whitespace().next() {
                        if num.chars().all(|c| c.is_ascii_digit()) {
                            names.push(format!("{num}.png"));
                        }
                    }
                }
            }
            self.pages = Some(names);
        }
        Ok(self.pages.clone().unwrap_or_default())
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, ArchiveError> {
        let page = name.strip_suffix(".png").ok_or_else(|| {
            ArchiveError::Other(format!("invalid PDF page name '{name}'"))
        })?;
        let p = self.path.to_string_lossy().into_owned();
        let dpi = PDF_RENDER_DPI.to_string();
        // `mutool draw -F png -r <dpi> -o - -- <pdf> <page>` writes the
        // rendered page to stdout.
        run_capture(
            "mutool",
            &["draw", "-F", "png", "-r", &dpi, "-o", "-", "--", &p, page],
        )
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
    fn lists_and_reads_pdf() {
        if which("mutool").is_none() {
            eprintln!("skipping: mutool not available");
            return;
        }
        let path = archive_path("01-PDF-Normal.pdf");
        if !path.exists() {
            eprintln!("skipping: test archive not present");
            return;
        }
        let mut ar = PdfArchive::open(&path).expect("open pdf");
        let pages = ar.page_names().expect("list pages");
        assert_eq!(pages.len(), 2);
        let bytes = ar.read(&pages[0]).expect("render first page");
        assert!(image::guess_format(&bytes).is_ok());
    }
}
