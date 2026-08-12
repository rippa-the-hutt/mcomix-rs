//! On-disk thumbnail cache, mirroring MComix's `~/.thumbnails` behaviour so
//! re-opening a comic shows thumbnails instantly instead of regenerating them.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn cache_dir() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::prefs::home_dir());
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::prefs::home_dir().join(".cache"));
    base.join("mcomix-rs").join("thumbnails")
}

/// Cache key: archive path + mtime + page index (archive edits invalidate).
fn key(path: &Path, page: usize) -> u64 {
    let mut h = DefaultHasher::new();
    path.to_string_lossy().hash(&mut h);
    let mtime = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    mtime.hash(&mut h);
    page.hash(&mut h);
    h.finish()
}

fn file_path(path: &Path, page: usize) -> PathBuf {
    cache_dir().join(format!("{:016x}.thumb", key(path, page)))
}

/// Load a cached thumbnail: `(width, height, tight RGBA8)`.
pub fn load(path: &Path, page: usize) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = fs::read(file_path(path, page)).ok()?;
    if bytes.len() < 8 {
        return None;
    }
    let w = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    let h = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let rgba = bytes[8..].to_vec();
    if (w as usize) * (h as usize) * 4 != rgba.len() {
        return None;
    }
    Some((w, h, rgba))
}

/// Store a thumbnail (8-byte header of width/height, then tight RGBA8).
pub fn store(path: &Path, page: usize, w: u32, h: u32, rgba: &[u8]) {
    let dir = cache_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        log::debug!("cannot create thumbnail cache dir: {e}");
        return;
    }
    let mut data = Vec::with_capacity(8 + rgba.len());
    data.extend_from_slice(&w.to_le_bytes());
    data.extend_from_slice(&h.to_le_bytes());
    data.extend_from_slice(rgba);
    let _ = fs::write(file_path(path, page), data);
}
