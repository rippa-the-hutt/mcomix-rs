//! Library backend (SQLite via rusqlite), mirroring `mcomix/library/backend.py`.
//! Tables: book, collection, contain, info, watchlist, recent.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::archive::{self, ArchiveKind};

/// Virtual "All books" collection id.
pub const COLLECTION_ALL: i64 = -1;

#[derive(Debug, Clone)]
pub struct Book {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub pages: i64,
    pub format: Option<i64>,
    pub size: i64,
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub supercollection: Option<i64>,
}

fn kind_to_code(k: ArchiveKind) -> i64 {
    match k {
        ArchiveKind::Zip => 0,
        ArchiveKind::Rar => 1,
        ArchiveKind::Tar => 2,
        ArchiveKind::Gzip => 3,
        ArchiveKind::Bzip2 => 4,
        ArchiveKind::Xz => 5,
        ArchiveKind::Pdf => 6,
        ArchiveKind::SevenZip => 7,
        ArchiveKind::Lha => 8,
    }
}

pub struct LibraryDb {
    conn: Connection,
}

impl LibraryDb {
    pub fn open() -> rusqlite::Result<LibraryDb> {
        let dir = crate::prefs::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let conn = Connection::open(dir.join("library.db"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS book (
                id integer primary key,
                name text,
                path text unique,
                pages integer,
                format integer,
                size integer,
                added datetime default current_timestamp);
             CREATE TABLE IF NOT EXISTS collection (
                id integer primary key,
                name text unique,
                supercollection integer);
             CREATE TABLE IF NOT EXISTS contain (
                collection integer not null,
                book integer not null,
                primary key (collection, book));
             CREATE TABLE IF NOT EXISTS info (
                key text primary key,
                value text);
             CREATE TABLE IF NOT EXISTS watchlist (
                path text primary key,
                collection integer references collection (id) on delete set null,
                recursive boolean not null);
             CREATE TABLE IF NOT EXISTS recent (
                book integer primary key,
                page integer,
                time_set datetime);
             INSERT OR IGNORE INTO info (key, value) VALUES ('version', '1');",
        )?;
        Ok(LibraryDb { conn })
    }

    pub fn db_path() -> std::path::PathBuf {
        crate::prefs::data_dir().join("library.db")
    }

    // ---- books ----

    /// Add an archive to the library; returns the new book id (or the id of an
    /// existing book with the same path). `collection` may be None.
    pub fn add_book(&mut self, path: &str, collection: Option<i64>) -> Option<i64> {
        // Already present?
        if let Some(id) = self.get_book_id_by_path(path) {
            if let Some(c) = collection {
                let _ = self.add_book_to_collection(id, c);
            }
            return Some(id);
        }
        let name = Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        let kind = archive::detect(Path::new(path));
        let format = kind.map(kind_to_code);
        let pages = archive::open(Path::new(path))
            .and_then(|mut a| a.page_names())
            .map(|p| p.len() as i64)
            .unwrap_or(0);
        let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);

        self.conn
            .execute(
                "INSERT INTO book (name, path, pages, format, size) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, path, pages, format, size],
            )
            .ok()?;
        let id = self.conn.last_insert_rowid();
        if let Some(c) = collection {
            let _ = self.add_book_to_collection(id, c);
        }
        Some(id)
    }

    pub fn get_book_id_by_path(&self, path: &str) -> Option<i64> {
        self.conn
            .query_row("SELECT id FROM book WHERE path = ?1", params![path], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    fn row_to_book(row: &rusqlite::Row) -> rusqlite::Result<Book> {
        Ok(Book {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            pages: row.get(3)?,
            format: row.get(4)?,
            size: row.get(5)?,
        })
    }

    /// Books in a collection (None = all books). `filter` optionally filters
    /// by name substring.
    pub fn get_books_in_collection(
        &self,
        collection: Option<i64>,
        filter: Option<&str>,
    ) -> Vec<Book> {
        let sql = match collection {
            None => "SELECT id, name, path, pages, format, size FROM book".to_string(),
            Some(c) => format!(
                "SELECT book.id, book.name, book.path, book.pages, book.format, book.size \
                 FROM book JOIN contain ON contain.book = book.id \
                 WHERE contain.collection = {c}"
            ),
        };
        let sql = match filter {
            Some(f) => format!("{sql} WHERE name LIKE ?1"),
            None => sql,
        };
        let Ok(mut stmt) = self.conn.prepare(&sql) else {
            return Vec::new();
        };
        let rows = match filter {
            Some(f) => stmt.query_map(params![format!("%{f}%")], Self::row_to_book),
            None => stmt.query_map([], Self::row_to_book),
        };
        rows.map(|r| r.flatten().collect()).unwrap_or_default()
    }

    pub fn get_book(&self, id: i64) -> Option<Book> {
        self.conn
            .query_row(
                "SELECT id, name, path, pages, format, size FROM book WHERE id = ?1",
                params![id],
                Self::row_to_book,
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn remove_book(&mut self, id: i64) {
        let _ = self
            .conn
            .execute("DELETE FROM book WHERE id = ?1", params![id]);
        let _ = self.conn.execute("DELETE FROM contain WHERE book = ?1", params![id]);
        let _ = self.conn.execute("DELETE FROM recent WHERE book = ?1", params![id]);
    }

    // ---- collections ----

    pub fn get_collections(&self) -> Vec<Collection> {
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT id, name, supercollection FROM collection ORDER BY name")
        else {
            return Vec::new();
        };
        stmt.query_map([], |r| {
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                supercollection: r.get(2)?,
            })
        })
        .map(|r| r.flatten().collect())
        .unwrap_or_default()
    }

    pub fn add_collection(&mut self, name: &str) -> Option<i64> {
        self.conn
            .execute(
                "INSERT INTO collection (name) VALUES (?1)",
                params![name],
            )
            .ok()?;
        Some(self.conn.last_insert_rowid())
    }

    pub fn remove_collection(&mut self, id: i64) {
        let _ = self
            .conn
            .execute("DELETE FROM collection WHERE id = ?1", params![id]);
        let _ = self
            .conn
            .execute("DELETE FROM contain WHERE collection = ?1", params![id]);
    }

    pub fn add_book_to_collection(&mut self, book: i64, collection: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO contain (collection, book) VALUES (?1, ?2)",
            params![collection, book],
        )?;
        Ok(())
    }

    pub fn remove_book_from_collection(&mut self, book: i64, collection: i64) {
        let _ = self.conn.execute(
            "DELETE FROM contain WHERE collection = ?1 AND book = ?2",
            params![collection, book],
        );
    }

    // ---- recent ----

    /// Record that `path` was read up to `page` (1-based).
    pub fn record_recent(&mut self, path: &str, page: u32) {
        let _ = self.conn.execute(
            "INSERT OR REPLACE INTO recent (book, page, time_set) \
             SELECT id, ?1, datetime('now') FROM book WHERE path = ?2",
            params![page as i64, path],
        );
    }

    /// Last-read page (1-based) for a book, if any.
    pub fn get_recent_page(&self, id: i64) -> Option<u32> {
        self.conn
            .query_row("SELECT page FROM recent WHERE book = ?1", params![id], |r| r.get::<_, i64>(0))
            .optional()
            .ok()
            .flatten()
            .map(|p| p as u32)
    }

    /// Books with a recent entry, most recently read first.
    pub fn get_recent_books(&self, limit: i64) -> Vec<Book> {
        let Ok(mut stmt) = self
            .conn
            .prepare(
                "SELECT book.id, book.name, book.path, book.pages, book.format, book.size \
                 FROM book JOIN recent ON recent.book = book.id \
                 ORDER BY recent.time_set DESC LIMIT ?1",
            )
        else {
            return Vec::new();
        };
        stmt.query_map(params![limit], Self::row_to_book)
            .map(|r| r.flatten().collect())
            .unwrap_or_default()
    }

    // ---- watchlist ----

    pub fn watchlist_add(&mut self, path: &str, recursive: bool, collection: Option<i64>) {
        let _ = self.conn.execute(
            "INSERT OR REPLACE INTO watchlist (path, collection, recursive) VALUES (?1, ?2, ?3)",
            params![path, collection, recursive],
        );
    }

    pub fn watchlist_entries(&self) -> Vec<(String, bool, Option<i64>)> {
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT path, recursive, collection FROM watchlist")
        else {
            return Vec::new();
        };
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|r| r.flatten().collect())
            .unwrap_or_default()
    }

    pub fn watchlist_remove(&mut self, path: &str) {
        let _ = self
            .conn
            .execute("DELETE FROM watchlist WHERE path = ?1", params![path]);
    }

    /// Scan watched directories for archive files that are not yet in the
    /// library. Returns `(path, collection_id)` pairs for new files.
    pub fn scan_watchlist(&mut self) -> Vec<(String, Option<i64>)> {
        let entries = self.watchlist_entries();
        let mut found = Vec::new();
        for (dir, recursive, collection) in entries {
            if !Path::new(&dir).is_dir() {
                continue;
            }
            let mut candidates: Vec<std::path::PathBuf> = Vec::new();
            if recursive {
                let mut walk = vec![std::path::PathBuf::from(&dir)];
                while let Some(d) = walk.pop() {
                    if let Ok(rd) = std::fs::read_dir(&d) {
                        for e in rd.flatten() {
                            let p = e.path();
                            if p.is_dir() {
                                walk.push(p);
                            } else if p.is_file() && archive::detect(&p).is_some() {
                                candidates.push(p);
                            }
                        }
                    }
                }
            } else if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_file() && archive::detect(&p).is_some() {
                        candidates.push(p);
                    }
                }
            }
            for c in candidates {
                let p = c.to_string_lossy().into_owned();
                if self.get_book_id_by_path(&p).is_none() {
                    found.push((p, collection));
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("test")
            .join("files")
            .join("archives")
            .join(name)
    }

    #[test]
    fn backend_crud() {
        let mut db = LibraryDb {
            conn: Connection::open_in_memory().expect("open in-memory db"),
        };
        db.conn
            .execute_batch(
                "CREATE TABLE book (id integer primary key, name text, path text unique,
                    pages integer, format integer, size integer, added datetime);
                 CREATE TABLE collection (id integer primary key, name text unique, supercollection integer);
                 CREATE TABLE contain (collection integer not null, book integer not null,
                    primary key (collection, book));
                 CREATE TABLE watchlist (path text primary key, collection integer, recursive boolean);
                 CREATE TABLE recent (book integer primary key, page integer, time_set datetime);",
            )
            .expect("schema");

        let zip = archive_path("01-ZIP-Normal.zip");
        let zip = zip.to_string_lossy().into_owned();
        let id = db.add_book(&zip, None).expect("add book");
        assert!(db.get_book(id).is_some());
        assert!(db.get_book_id_by_path(&zip).is_some());
        let all = db.get_books_in_collection(None, None);
        assert_eq!(all.len(), 1);
        assert!(all[0].pages > 0, "pages should be counted");

        let col = db.add_collection("Favorites").expect("add collection");
        db.add_book_to_collection(id, col).ok();
        let in_col = db.get_books_in_collection(Some(col), None);
        assert_eq!(in_col.len(), 1);

        db.record_recent(&zip, 3);
        let recent = db.get_recent_books(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(db.get_recent_page(id), Some(3));

        db.remove_book(id);
        assert!(db.get_book(id).is_none());
        assert!(db.get_books_in_collection(Some(col), None).is_empty());
    }
}
