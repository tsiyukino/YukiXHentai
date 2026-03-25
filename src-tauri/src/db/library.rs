/// Separate SQLite database for local gallery library data.
/// Path: {library_dir}/library.db  (lives alongside the gallery folders)

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{
    Gallery, GalleryMetadataPatch, GalleryPage, LocalPage, ReadProgress, ReadingSession, Tag,
};

const CURRENT_SCHEMA_VERSION: i32 = 1;

/// Thread-safe database handle for the local library database.
pub struct LibraryDbState {
    pub conn: Mutex<Connection>,
}

impl LibraryDbState {
    /// Open (or create) library.db inside `library_dir`.
    /// `library_dir` is the root gallery library folder (e.g. {data_local_dir}/yukixhentai/library/).
    pub fn open(library_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&library_dir).map_err(|e| e.to_string())?;
        let db_path = library_dir.join("library.db");
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL;").map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA foreign_keys=ON;").map_err(|e| e.to_string())?;
        let state = Self { conn: Mutex::new(conn) };
        state.run_migrations()?;
        Ok(state)
    }

    fn run_migrations(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);",
        ).map_err(|e| e.to_string())?;

        let version: i32 = conn
            .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;

        if version < 1 {
            conn.execute_batch(MIGRATION_V1).map_err(|e| e.to_string())?;
            conn.execute("INSERT INTO schema_version (version) VALUES (?1)", params![1])
                .map_err(|e| e.to_string())?;
        }

        assert!(
            CURRENT_SCHEMA_VERSION == 1,
            "Add new migration branches above when bumping CURRENT_SCHEMA_VERSION"
        );
        Ok(())
    }

    /// Upsert a local gallery and its tags.
    pub fn upsert_local_gallery(
        &self,
        gallery: &Gallery,
        local_folder: &str,
        description: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO galleries (gid, token, title, title_jpn, category, thumb_url, thumb_path,
                                    uploader, posted, rating, file_count, file_size,
                                    updated_at, local_folder, description, origin, remote_gid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(gid) DO UPDATE SET
                token=excluded.token, title=excluded.title, title_jpn=excluded.title_jpn,
                category=excluded.category, thumb_url=excluded.thumb_url,
                thumb_path=COALESCE(excluded.thumb_path, galleries.thumb_path),
                uploader=COALESCE(excluded.uploader, galleries.uploader),
                posted=excluded.posted, rating=excluded.rating,
                file_count=excluded.file_count,
                file_size=COALESCE(excluded.file_size, galleries.file_size),
                updated_at=excluded.updated_at,
                local_folder=excluded.local_folder,
                description=COALESCE(excluded.description, galleries.description),
                origin=COALESCE(excluded.origin, galleries.origin),
                remote_gid=COALESCE(excluded.remote_gid, galleries.remote_gid)",
            params![
                gallery.gid, gallery.token, gallery.title, gallery.title_jpn,
                gallery.category, gallery.thumb_url, gallery.thumb_path,
                gallery.uploader, gallery.posted, gallery.rating,
                gallery.file_count, gallery.file_size,
                now, local_folder, description,
                gallery.origin, gallery.remote_gid,
            ],
        ).map_err(|e| e.to_string())?;

        // Replace tags.
        conn.execute("DELETE FROM gallery_tags WHERE gid = ?1", params![gallery.gid])
            .map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("INSERT OR IGNORE INTO gallery_tags (gid, namespace, name) VALUES (?1, ?2, ?3)")
            .map_err(|e| e.to_string())?;
        for tag in &gallery.tags {
            stmt.execute(params![gallery.gid, tag.namespace, tag.name])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Get a single local gallery by gid.
    pub fn get_gallery_by_gid(&self, gid: i64) -> Result<Option<Gallery>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let result = conn.query_row(
            "SELECT gid, token, title, title_jpn, category, thumb_url, thumb_path,
                    uploader, posted, rating, file_count, file_size, description,
                    origin, remote_gid, local_folder
             FROM galleries WHERE gid = ?1",
            params![gid],
            |row| {
                Ok(Gallery {
                    gid: row.get(0)?,
                    token: row.get(1)?,
                    title: row.get(2)?,
                    title_jpn: row.get(3)?,
                    category: row.get(4)?,
                    thumb_url: row.get(5)?,
                    thumb_path: row.get(6)?,
                    uploader: row.get(7)?,
                    posted: row.get(8)?,
                    rating: row.get(9)?,
                    file_count: row.get(10)?,
                    file_size: row.get(11)?,
                    is_local: Some(1),
                    description: row.get(12)?,
                    origin: row.get(13)?,
                    remote_gid: row.get(14)?,
                    tags: Vec::new(),
                })
            },
        ).optional().map_err(|e| e.to_string())?;

        if let Some(mut gallery) = result {
            let tags = conn
                .prepare("SELECT namespace, name FROM gallery_tags WHERE gid = ?1 ORDER BY namespace, name")
                .map_err(|e| e.to_string())?
                .query_map(params![gid], |row| Ok(Tag { namespace: row.get(0)?, name: row.get(1)? }))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            gallery.tags = tags;
            Ok(Some(gallery))
        } else {
            Ok(None)
        }
    }

    /// Check if a gid exists in the library.
    pub fn is_local(&self, gid: i64) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM galleries WHERE gid = ?1", params![gid], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(count > 0)
    }

    /// Get a page of local galleries ordered by posted DESC.
    pub fn get_local_galleries(&self, offset: i64, limit: i64) -> Result<GalleryPage, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let total_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM galleries", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            "SELECT gid, token, title, title_jpn, category, thumb_url, thumb_path,
                    uploader, posted, rating, file_count, file_size, description,
                    origin, remote_gid
             FROM galleries ORDER BY posted DESC LIMIT ?1 OFFSET ?2",
        ).map_err(|e| e.to_string())?;

        let mut gallery_rows: Vec<Gallery> = stmt
            .query_map(params![limit, offset], |row| {
                Ok(Gallery {
                    gid: row.get(0)?,
                    token: row.get(1)?,
                    title: row.get(2)?,
                    title_jpn: row.get(3)?,
                    category: row.get(4)?,
                    thumb_url: row.get(5)?,
                    thumb_path: row.get(6)?,
                    uploader: row.get(7)?,
                    posted: row.get(8)?,
                    rating: row.get(9)?,
                    file_count: row.get(10)?,
                    file_size: row.get(11)?,
                    is_local: Some(1),
                    description: row.get(12)?,
                    origin: row.get(13)?,
                    remote_gid: row.get(14)?,
                    tags: Vec::new(),
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        // Load tags for each gallery.
        for gallery in &mut gallery_rows {
            let tags = conn
                .prepare("SELECT namespace, name FROM gallery_tags WHERE gid = ?1 ORDER BY namespace, name")
                .map_err(|e| e.to_string())?
                .query_map(params![gallery.gid], |row| Ok(Tag { namespace: row.get(0)?, name: row.get(1)? }))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            gallery.tags = tags;
        }

        Ok(GalleryPage { galleries: gallery_rows, total_count })
    }

    /// Get all local pages for a gallery.
    pub fn get_local_gallery_pages(&self, gid: i64) -> Result<Vec<LocalPage>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT gid, page_index, file_path, source_url, width, height
             FROM local_gallery_pages WHERE gid = ?1 ORDER BY page_index",
        ).map_err(|e| e.to_string())?;
        let pages = stmt
            .query_map(params![gid], |row| {
                Ok(LocalPage {
                    gid: row.get(0)?,
                    page_index: row.get(1)?,
                    file_path: row.get(2)?,
                    source_url: row.get(3)?,
                    width: row.get(4)?,
                    height: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(pages)
    }

    /// Insert new pages after insert_after_index (-1 to prepend as index 0).
    pub fn insert_local_pages(
        &self,
        gid: i64,
        pages: &[(String, Option<String>, Option<i32>, Option<i32>)],
        insert_after_index: i32,
    ) -> Result<Vec<LocalPage>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let insert_start = insert_after_index + 1;
        let shift_count = pages.len() as i32;

        // Shift existing pages up to make room.
        conn.execute(
            "UPDATE local_gallery_pages SET page_index = page_index + ?1
             WHERE gid = ?2 AND page_index >= ?3",
            params![shift_count, gid, insert_start],
        ).map_err(|e| e.to_string())?;

        let mut inserted = Vec::new();
        let mut stmt = conn.prepare(
            "INSERT INTO local_gallery_pages (gid, page_index, file_path, source_url, width, height)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        ).map_err(|e| e.to_string())?;

        for (i, (fp, su, w, h)) in pages.iter().enumerate() {
            let idx = insert_start + i as i32;
            stmt.execute(params![gid, idx, fp, su, w, h]).map_err(|e| e.to_string())?;
            inserted.push(LocalPage {
                gid,
                page_index: idx,
                file_path: fp.clone(),
                source_url: su.clone(),
                width: *w,
                height: *h,
            });
        }
        Ok(inserted)
    }

    /// Remove a single page and renumber remaining pages. Returns the file_path if found.
    pub fn remove_local_page(&self, gid: i64, page_index: i32) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let file_path: Option<String> = conn.query_row(
            "SELECT file_path FROM local_gallery_pages WHERE gid = ?1 AND page_index = ?2",
            params![gid, page_index],
            |row| row.get(0),
        ).optional().map_err(|e| e.to_string())?;

        conn.execute(
            "DELETE FROM local_gallery_pages WHERE gid = ?1 AND page_index = ?2",
            params![gid, page_index],
        ).map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE local_gallery_pages SET page_index = page_index - 1
             WHERE gid = ?1 AND page_index > ?2",
            params![gid, page_index],
        ).map_err(|e| e.to_string())?;

        Ok(file_path)
    }

    /// Reorder pages by providing new_order as a list of old page_index values in desired order.
    pub fn reorder_local_pages(&self, gid: i64, new_order: &[i32]) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let existing: Vec<(i32, String, Option<String>, Option<i32>, Option<i32>)> = conn
            .prepare("SELECT page_index, file_path, source_url, width, height FROM local_gallery_pages WHERE gid = ?1 ORDER BY page_index")
            .map_err(|e| e.to_string())?
            .query_map(params![gid], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let by_idx: std::collections::HashMap<i32, (String, Option<String>, Option<i32>, Option<i32>)> = existing
            .into_iter()
            .map(|(idx, fp, su, w, h)| (idx, (fp, su, w, h)))
            .collect();

        conn.execute("DELETE FROM local_gallery_pages WHERE gid = ?1", params![gid])
            .map_err(|e| e.to_string())?;

        let mut stmt = conn.prepare(
            "INSERT INTO local_gallery_pages (gid, page_index, file_path, source_url, width, height)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        ).map_err(|e| e.to_string())?;

        for (new_idx, &old_idx) in new_order.iter().enumerate() {
            if let Some((fp, su, w, h)) = by_idx.get(&old_idx) {
                stmt.execute(params![gid, new_idx as i32, fp, su, w, h])
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Update metadata fields. Only Some fields are changed.
    pub fn update_gallery_metadata(&self, gid: i64, patch: &GalleryMetadataPatch) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        if let Some(ref title) = patch.title {
            conn.execute("UPDATE galleries SET title = ?1 WHERE gid = ?2", params![title, gid])
                .map_err(|e| e.to_string())?;
        }
        if let Some(ref title_jpn) = patch.title_jpn {
            conn.execute("UPDATE galleries SET title_jpn = ?1 WHERE gid = ?2", params![title_jpn, gid])
                .map_err(|e| e.to_string())?;
        }
        if let Some(ref category) = patch.category {
            conn.execute("UPDATE galleries SET category = ?1 WHERE gid = ?2", params![category, gid])
                .map_err(|e| e.to_string())?;
        }
        if let Some(ref uploader) = patch.uploader {
            conn.execute("UPDATE galleries SET uploader = ?1 WHERE gid = ?2", params![uploader, gid])
                .map_err(|e| e.to_string())?;
        }
        if let Some(ref description) = patch.description {
            conn.execute("UPDATE galleries SET description = ?1 WHERE gid = ?2", params![description, gid])
                .map_err(|e| e.to_string())?;
        }

        // Add tags.
        if let Some(ref tags_add) = patch.tags_add {
            let mut stmt = conn.prepare(
                "INSERT OR IGNORE INTO gallery_tags (gid, namespace, name) VALUES (?1, ?2, ?3)"
            ).map_err(|e| e.to_string())?;
            for tag in tags_add {
                stmt.execute(params![gid, tag.namespace, tag.name]).map_err(|e| e.to_string())?;
            }
        }

        // Remove tags.
        if let Some(ref tags_remove) = patch.tags_remove {
            let mut stmt = conn.prepare(
                "DELETE FROM gallery_tags WHERE gid = ?1 AND namespace = ?2 AND name = ?3"
            ).map_err(|e| e.to_string())?;
            for tag in tags_remove {
                stmt.execute(params![gid, tag.namespace, tag.name]).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    /// Update the thumb_path for a gallery.
    pub fn set_thumb_path(&self, gid: i64, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("UPDATE galleries SET thumb_path = ?1 WHERE gid = ?2", params![path, gid])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Update file_count for a gallery.
    pub fn set_file_count(&self, gid: i64, count: i32) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("UPDATE galleries SET file_count = ?1 WHERE gid = ?2", params![count, gid])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Get the local_folder path for a gallery.
    pub fn get_local_folder(&self, gid: i64) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT local_folder FROM galleries WHERE gid = ?1",
            params![gid],
            |row| row.get::<_, Option<String>>(0),
        ).optional()
        .map_err(|e| e.to_string())
        .map(|opt| opt.flatten())
    }

    /// Delete a gallery (cascades to all child tables).
    pub fn delete_gallery(&self, gid: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM galleries WHERE gid = ?1", params![gid])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ── Reading progress ─────────────────────────────────────────────────

    pub fn update_read_progress(&self, progress: &ReadProgress) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO reading_progress (gid, last_page_read, total_pages, last_read_at, is_completed)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(gid) DO UPDATE SET
                last_page_read=excluded.last_page_read,
                total_pages=excluded.total_pages,
                last_read_at=excluded.last_read_at,
                is_completed=excluded.is_completed",
            params![
                progress.gid, progress.last_page_read, progress.total_pages,
                progress.last_read_at, progress.is_completed as i32,
            ],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_read_progress(&self, gid: i64) -> Result<Option<ReadProgress>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT gid, last_page_read, total_pages, last_read_at, is_completed
             FROM reading_progress WHERE gid = ?1",
            params![gid],
            |row| Ok(ReadProgress {
                gid: row.get(0)?,
                last_page_read: row.get(1)?,
                total_pages: row.get(2)?,
                last_read_at: row.get(3)?,
                is_completed: row.get::<_, i32>(4)? != 0,
            }),
        ).optional().map_err(|e| e.to_string())
    }

    pub fn get_read_progress_batch(&self, gids: &[i64]) -> Result<Vec<ReadProgress>, String> {
        if gids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let placeholders = gids.iter().enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT gid, last_page_read, total_pages, last_read_at, is_completed
             FROM reading_progress WHERE gid IN ({})",
            placeholders
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let params_vec: Vec<&dyn rusqlite::ToSql> = gids.iter().map(|g| g as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params_vec.as_slice(), |row| {
            Ok(ReadProgress {
                gid: row.get(0)?,
                last_page_read: row.get(1)?,
                total_pages: row.get(2)?,
                last_read_at: row.get(3)?,
                is_completed: row.get::<_, i32>(4)? != 0,
            })
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
        Ok(rows)
    }

    // ── Reading sessions ─────────────────────────────────────────────────

    pub fn start_reading_session(&self, gid: i64, opened_at: i64) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO reading_sessions (gid, opened_at) VALUES (?1, ?2)",
            params![gid, opened_at],
        ).map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn end_reading_session(&self, session_id: i64, closed_at: i64, pages_read: i32) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE reading_sessions SET closed_at = ?1, pages_read = ?2 WHERE id = ?3",
            params![closed_at, pages_read, session_id],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_reading_history(&self, limit: i64, offset: i64) -> Result<Vec<ReadingSession>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, gid, opened_at, closed_at, pages_read
             FROM reading_sessions ORDER BY opened_at DESC LIMIT ?1 OFFSET ?2",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(ReadingSession {
                id: row.get(0)?,
                gid: row.get(1)?,
                opened_at: row.get(2)?,
                closed_at: row.get(3)?,
                pages_read: row.get(4)?,
            })
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
        Ok(rows)
    }
}

// ── Migration SQL ─────────────────────────────────────────────────────────

const MIGRATION_V1: &str = "
CREATE TABLE galleries (
    gid          INTEGER PRIMARY KEY,
    token        TEXT    NOT NULL DEFAULT '',
    title        TEXT    NOT NULL,
    title_jpn    TEXT,
    category     TEXT    NOT NULL DEFAULT 'Unknown',
    thumb_url    TEXT    NOT NULL DEFAULT '',
    thumb_path   TEXT,
    uploader     TEXT,
    posted       INTEGER NOT NULL DEFAULT 0,
    rating       REAL    NOT NULL DEFAULT 0.0,
    file_count   INTEGER NOT NULL DEFAULT 0,
    file_size    INTEGER,
    updated_at   INTEGER NOT NULL DEFAULT 0,
    description  TEXT,
    local_folder TEXT,
    origin       TEXT,
    remote_gid   INTEGER
);

CREATE INDEX idx_lib_galleries_posted   ON galleries(posted DESC);
CREATE INDEX idx_lib_galleries_category ON galleries(category);
CREATE INDEX idx_lib_galleries_rating   ON galleries(rating DESC);
CREATE INDEX idx_lib_galleries_origin   ON galleries(origin);

CREATE TABLE gallery_tags (
    gid       INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    namespace TEXT    NOT NULL DEFAULT '',
    name      TEXT    NOT NULL,
    PRIMARY KEY (gid, namespace, name)
);

CREATE TABLE local_gallery_pages (
    gid        INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    page_index INTEGER NOT NULL,
    file_path  TEXT    NOT NULL,
    source_url TEXT,
    width      INTEGER,
    height     INTEGER,
    PRIMARY KEY (gid, page_index)
);

CREATE TABLE reading_progress (
    gid            INTEGER PRIMARY KEY REFERENCES galleries(gid) ON DELETE CASCADE,
    last_page_read INTEGER NOT NULL DEFAULT 0,
    total_pages    INTEGER NOT NULL DEFAULT 0,
    last_read_at   INTEGER NOT NULL DEFAULT 0,
    is_completed   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE reading_sessions (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    gid        INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    opened_at  INTEGER NOT NULL,
    closed_at  INTEGER,
    pages_read INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_lib_sessions_gid    ON reading_sessions(gid);
CREATE INDEX idx_lib_sessions_opened ON reading_sessions(opened_at DESC);
";
