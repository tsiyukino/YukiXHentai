use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{
    FilterParams, FilterPreset, Gallery, GalleryPage, GalleryPageEntry,
    ReadProgress, ReadingSession, SearchHistoryEntry, SortDirection, SortParams, Tag, TagSuggestion,
};

const CURRENT_SCHEMA_VERSION: i32 = 10;

/// Thread-safe database handle, stored as Tauri managed state.
pub struct DbState {
    pub conn: Mutex<Connection>,
}

impl DbState {
    /// Open (or create) the database and run migrations.
    pub fn open(data_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
        let db_path = data_dir.join("yukixhentai.db");
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

        // Enable WAL mode for better concurrent read performance.
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;

        let state = Self {
            conn: Mutex::new(conn),
        };
        state.run_migrations()?;
        Ok(state)
    }

    fn run_migrations(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            );",
        )
        .map_err(|e| e.to_string())?;

        let version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if version < 1 {
            conn.execute_batch(MIGRATION_V1).map_err(|e| e.to_string())?;
            conn.execute("INSERT INTO schema_version (version) VALUES (?1)", params![1])
                .map_err(|e| e.to_string())?;
        }

        if version < 2 {
            conn.execute_batch(MIGRATION_V2).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE schema_version SET version = ?1",
                params![2],
            )
            .map_err(|e| e.to_string())?;
        }

        if version < 3 {
            conn.execute_batch(MIGRATION_V3).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE schema_version SET version = ?1",
                params![3],
            )
            .map_err(|e| e.to_string())?;
        }

        if version < 4 {
            conn.execute_batch(MIGRATION_V4).map_err(|e| e.to_string())?;
            conn.execute("UPDATE schema_version SET version = ?1", params![4])
                .map_err(|e| e.to_string())?;
        }

        if version < 5 {
            conn.execute_batch(MIGRATION_V5).map_err(|e| e.to_string())?;
            conn.execute("UPDATE schema_version SET version = ?1", params![5])
                .map_err(|e| e.to_string())?;
        }

        if version < 6 {
            conn.execute_batch(MIGRATION_V6).map_err(|e| e.to_string())?;
            conn.execute("UPDATE schema_version SET version = ?1", params![6])
                .map_err(|e| e.to_string())?;
        }

        if version < 7 {
            conn.execute_batch(MIGRATION_V7).map_err(|e| e.to_string())?;
            conn.execute("UPDATE schema_version SET version = ?1", params![7])
                .map_err(|e| e.to_string())?;
        }

        if version < 8 {
            conn.execute_batch(MIGRATION_V8).map_err(|e| e.to_string())?;
            conn.execute("UPDATE schema_version SET version = ?1", params![8])
                .map_err(|e| e.to_string())?;
        }

        if version < 9 {
            conn.execute_batch(MIGRATION_V9).map_err(|e| e.to_string())?;
            conn.execute("UPDATE schema_version SET version = ?1", params![9])
                .map_err(|e| e.to_string())?;
        }

        if version < 10 {
            conn.execute_batch(MIGRATION_V10).map_err(|e| e.to_string())?;
            conn.execute("UPDATE schema_version SET version = ?1", params![10])
                .map_err(|e| e.to_string())?;
        }

        assert!(
            CURRENT_SCHEMA_VERSION == 10,
            "Add new migration branches above when bumping CURRENT_SCHEMA_VERSION"
        );

        Ok(())
    }

    /// Upsert a gallery and its tags with a specific metadata source.
    /// `metadata_source` should be "browse" or "api".
    pub fn upsert_gallery_with_source(&self, gallery: &Gallery, metadata_source: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO galleries (gid, token, title, title_jpn, category, thumb_url, thumb_path, uploader, posted, rating, file_count, file_size, metadata_source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(gid) DO UPDATE SET
                token=excluded.token, title=excluded.title, title_jpn=excluded.title_jpn,
                category=excluded.category, thumb_url=excluded.thumb_url,
                thumb_path=COALESCE(excluded.thumb_path, galleries.thumb_path),
                uploader=COALESCE(excluded.uploader, galleries.uploader),
                posted=excluded.posted, rating=excluded.rating,
                file_count=excluded.file_count,
                file_size=COALESCE(excluded.file_size, galleries.file_size),
                metadata_source=excluded.metadata_source,
                updated_at=excluded.updated_at",
            params![
                gallery.gid,
                gallery.token,
                gallery.title,
                gallery.title_jpn,
                gallery.category,
                gallery.thumb_url,
                gallery.thumb_path,
                gallery.uploader,
                gallery.posted,
                gallery.rating,
                gallery.file_count,
                gallery.file_size,
                metadata_source,
                now,
            ],
        )
        .map_err(|e| e.to_string())?;

        // Replace tags: delete old, insert new.
        conn.execute("DELETE FROM gallery_tags WHERE gid = ?1", params![gallery.gid])
            .map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare("INSERT INTO gallery_tags (gid, namespace, name) VALUES (?1, ?2, ?3)")
            .map_err(|e| e.to_string())?;
        for tag in &gallery.tags {
            stmt.execute(params![gallery.gid, tag.namespace, tag.name])
                .map_err(|e| e.to_string())?;
        }
        drop(stmt);

        // Update FTS index.
        let tags_str: String = gallery
            .tags
            .iter()
            .map(|t| format!("{}:{}", t.namespace, t.name))
            .collect::<Vec<_>>()
            .join(" ");

        // Delete old FTS entry (if any), then insert new one.
        conn.execute(
            "DELETE FROM galleries_fts WHERE rowid = ?1",
            params![gallery.gid],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO galleries_fts(rowid, title, title_jpn, tags) VALUES (?1, ?2, ?3, ?4)",
            params![
                gallery.gid,
                gallery.title,
                gallery.title_jpn.as_deref().unwrap_or(""),
                tags_str,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Upsert a gallery from browse results.
    /// Only overwrites metadata if the existing source is NOT 'api' (richer data preserved).
    pub fn upsert_gallery_browse(&self, gallery: &Gallery) -> Result<(), String> {
        // Check if we already have API-enriched data for this gallery.
        let existing_source = {
            let conn = self.conn.lock().map_err(|e| e.to_string())?;
            conn.query_row(
                "SELECT metadata_source FROM galleries WHERE gid = ?1",
                params![gallery.gid],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
        };

        if existing_source.as_deref() == Some("api") {
            // Already enriched — only update thumb_url/thumb_path (browse may have fresher thumbnails).
            let conn = self.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE galleries SET thumb_url = ?1, thumb_path = COALESCE(?2, galleries.thumb_path) WHERE gid = ?3",
                params![gallery.thumb_url, gallery.thumb_path, gallery.gid],
            )
            .map_err(|e| e.to_string())?;
            return Ok(());
        }

        self.upsert_gallery_with_source(gallery, "browse")
    }

    /// Upsert a gallery and its tags. Replaces existing data for the same gid.
    /// Legacy method — uses "api" source for backward compatibility with fetch_gallery_metadata.
    pub fn upsert_gallery(&self, gallery: &Gallery) -> Result<(), String> {
        self.upsert_gallery_with_source(gallery, "api")
    }

    /// Get galleries that need enrichment (metadata_source = 'browse'), ordered by most recently updated first.
    pub fn get_galleries_needing_enrichment(&self, limit: i64) -> Result<Vec<(i64, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT gid, token FROM galleries WHERE metadata_source = 'browse' ORDER BY updated_at DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }

    /// Update the local thumbnail path for a gallery.
    pub fn set_thumb_path(&self, gid: i64, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE galleries SET thumb_path = ?1 WHERE gid = ?2",
            params![path, gid],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Get a page of galleries ordered by posted date descending.
    pub fn get_galleries(&self, offset: i64, limit: i64) -> Result<GalleryPage, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let total_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM galleries", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT gid, token, title, title_jpn, category, thumb_url, thumb_path,
                        uploader, posted, rating, file_count, file_size
                 FROM galleries
                 ORDER BY posted DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| e.to_string())?;

        let gallery_rows: Vec<Gallery> = stmt
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
                    tags: Vec::new(), // filled below
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        // Load tags for each gallery.
        let mut tag_stmt = conn
            .prepare("SELECT namespace, name FROM gallery_tags WHERE gid = ?1")
            .map_err(|e| e.to_string())?;

        let mut galleries = gallery_rows;
        for gallery in &mut galleries {
            let tags: Vec<Tag> = tag_stmt
                .query_map(params![gallery.gid], |row| {
                    Ok(Tag {
                        namespace: row.get(0)?,
                        name: row.get(1)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            gallery.tags = tags;
        }

        Ok(GalleryPage {
            galleries,
            total_count,
        })
    }

    /// Get galleries by a list of gids, preserving the input order.
    pub fn get_galleries_by_gids(&self, gids: &[i64]) -> Result<Vec<Gallery>, String> {
        if gids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let placeholders: Vec<String> = gids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT gid, token, title, title_jpn, category, thumb_url, thumb_path,
                    uploader, posted, rating, file_count, file_size
             FROM galleries WHERE gid IN ({})",
            placeholders.join(", ")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let param_refs: Vec<Box<dyn rusqlite::types::ToSql>> =
            gids.iter().map(|g| Box::new(*g) as Box<dyn rusqlite::types::ToSql>).collect();
        let refs: Vec<&dyn rusqlite::types::ToSql> = param_refs.iter().map(|p| p.as_ref()).collect();
        let rows: Vec<Gallery> = stmt
            .query_map(rusqlite::params_from_iter(&refs), |row| {
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
                    tags: Vec::new(),
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        // Load tags.
        let mut tag_stmt = conn
            .prepare("SELECT namespace, name FROM gallery_tags WHERE gid = ?1")
            .map_err(|e| e.to_string())?;
        let mut galleries = rows;
        for gallery in &mut galleries {
            let tags: Vec<Tag> = tag_stmt
                .query_map(params![gallery.gid], |row| {
                    Ok(Tag { namespace: row.get(0)?, name: row.get(1)? })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            gallery.tags = tags;
        }

        // Re-order to match input gid order.
        let mut ordered = Vec::with_capacity(gids.len());
        for &gid in gids {
            if let Some(g) = galleries.iter().find(|g| g.gid == gid) {
                ordered.push(g.clone());
            }
        }
        Ok(ordered)
    }

    /// Search galleries using filter params, sort params, and pagination.
    pub fn search_galleries(
        &self,
        filter: &FilterParams,
        sort: &SortParams,
        offset: i64,
        limit: i64,
    ) -> Result<GalleryPage, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut where_clauses: Vec<String> = Vec::new();
        let mut params_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut joins = String::new();
        let mut param_idx = 1u32;

        // FTS5 free text search — join on matching rowids.
        if let Some(ref q) = filter.query {
            let trimmed = q.trim();
            if !trimmed.is_empty() {
                joins.push_str(&format!(
                    " INNER JOIN galleries_fts ON galleries_fts.rowid = g.gid AND galleries_fts MATCH ?{}",
                    param_idx
                ));
                params_values.push(Box::new(trimmed.to_string()));
                param_idx += 1;
            }
        }

        // Tag include: gallery must have ALL specified tags.
        for tag in &filter.tags_include {
            where_clauses.push(format!(
                "EXISTS (SELECT 1 FROM gallery_tags gt WHERE gt.gid = g.gid AND gt.namespace = ?{} AND gt.name = ?{})",
                param_idx, param_idx + 1
            ));
            params_values.push(Box::new(tag.namespace.clone()));
            params_values.push(Box::new(tag.name.clone()));
            param_idx += 2;
        }

        // Tag exclude: gallery must NOT have any of these tags.
        for tag in &filter.tags_exclude {
            where_clauses.push(format!(
                "NOT EXISTS (SELECT 1 FROM gallery_tags gt WHERE gt.gid = g.gid AND gt.namespace = ?{} AND gt.name = ?{})",
                param_idx, param_idx + 1
            ));
            params_values.push(Box::new(tag.namespace.clone()));
            params_values.push(Box::new(tag.name.clone()));
            param_idx += 2;
        }

        // Category filter.
        if !filter.categories.is_empty() {
            let placeholders: Vec<String> = filter
                .categories
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", param_idx + i as u32))
                .collect();
            where_clauses.push(format!("g.category IN ({})", placeholders.join(", ")));
            for cat in &filter.categories {
                params_values.push(Box::new(cat.clone()));
                param_idx += 1;
            }
        }

        // Rating range.
        if let Some(min) = filter.rating_min {
            where_clauses.push(format!("g.rating >= ?{}", param_idx));
            params_values.push(Box::new(min));
            param_idx += 1;
        }
        if let Some(max) = filter.rating_max {
            where_clauses.push(format!("g.rating <= ?{}", param_idx));
            params_values.push(Box::new(max));
            param_idx += 1;
        }

        // Page count range.
        if let Some(min) = filter.pages_min {
            where_clauses.push(format!("g.file_count >= ?{}", param_idx));
            params_values.push(Box::new(min));
            param_idx += 1;
        }
        if let Some(max) = filter.pages_max {
            where_clauses.push(format!("g.file_count <= ?{}", param_idx));
            params_values.push(Box::new(max));
            param_idx += 1;
        }

        // Date range.
        if let Some(min) = filter.date_min {
            where_clauses.push(format!("g.posted >= ?{}", param_idx));
            params_values.push(Box::new(min));
            param_idx += 1;
        }
        if let Some(max) = filter.date_max {
            where_clauses.push(format!("g.posted <= ?{}", param_idx));
            params_values.push(Box::new(max));
            param_idx += 1;
        }

        // Language filter (tag-based: "language:<value>").
        if let Some(ref lang) = filter.language {
            where_clauses.push(format!(
                "EXISTS (SELECT 1 FROM gallery_tags gt WHERE gt.gid = g.gid AND gt.namespace = 'language' AND gt.name = ?{})",
                param_idx
            ));
            params_values.push(Box::new(lang.clone()));
            param_idx += 1;
        }

        // Uploader filter.
        if let Some(ref uploader) = filter.uploader {
            where_clauses.push(format!("g.uploader = ?{}", param_idx));
            params_values.push(Box::new(uploader.clone()));
            param_idx += 1;
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };

        // Sort.
        let allowed_sort_fields = ["posted", "rating", "file_count", "title", "category", "gid"];
        let order_sql = if sort.fields.is_empty() {
            "ORDER BY g.posted DESC".to_string()
        } else {
            let parts: Vec<String> = sort
                .fields
                .iter()
                .filter(|f| allowed_sort_fields.contains(&f.field.as_str()))
                .map(|f| {
                    let dir = match f.direction {
                        SortDirection::Asc => "ASC",
                        SortDirection::Desc => "DESC",
                    };
                    format!("g.{} {}", f.field, dir)
                })
                .collect();
            if parts.is_empty() {
                "ORDER BY g.posted DESC".to_string()
            } else {
                format!("ORDER BY {}", parts.join(", "))
            }
        };

        // Count query.
        let count_sql = format!(
            "SELECT COUNT(*) FROM galleries g{joins}{where_sql}",
            joins = joins,
            where_sql = where_sql
        );

        // Data query.
        let data_sql = format!(
            "SELECT g.gid, g.token, g.title, g.title_jpn, g.category, g.thumb_url, g.thumb_path,
                    g.uploader, g.posted, g.rating, g.file_count, g.file_size
             FROM galleries g{joins}{where_sql}
             {order_sql}
             LIMIT ?{limit_idx} OFFSET ?{offset_idx}",
            joins = joins,
            where_sql = where_sql,
            order_sql = order_sql,
            limit_idx = param_idx,
            offset_idx = param_idx + 1,
        );

        // Build rusqlite param refs.
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_values.iter().map(|p| p.as_ref()).collect();

        // Count.
        let total_count: i64 = conn
            .query_row(&count_sql, rusqlite::params_from_iter(&param_refs), |row| {
                row.get(0)
            })
            .map_err(|e| format!("Count query failed: {}", e))?;

        // Data with pagination params appended.
        let mut all_params = params_values;
        all_params.push(Box::new(limit));
        all_params.push(Box::new(offset));
        let all_param_refs: Vec<&dyn rusqlite::types::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn
            .prepare(&data_sql)
            .map_err(|e| format!("Prepare failed: {}", e))?;

        let gallery_rows: Vec<Gallery> = stmt
            .query_map(rusqlite::params_from_iter(&all_param_refs), |row| {
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
                    tags: Vec::new(),
                })
            })
            .map_err(|e| format!("Query failed: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        // Load tags for each gallery.
        let mut tag_stmt = conn
            .prepare("SELECT namespace, name FROM gallery_tags WHERE gid = ?1")
            .map_err(|e| e.to_string())?;

        let mut galleries = gallery_rows;
        for gallery in &mut galleries {
            let tags: Vec<Tag> = tag_stmt
                .query_map(params![gallery.gid], |row| {
                    Ok(Tag {
                        namespace: row.get(0)?,
                        name: row.get(1)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            gallery.tags = tags;
        }

        Ok(GalleryPage {
            galleries,
            total_count,
        })
    }

    // ── Filter presets CRUD ───────────────────────────────────────────────

    pub fn save_preset(
        &self,
        name: &str,
        filter: &FilterParams,
        sort: &SortParams,
    ) -> Result<FilterPreset, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let filter_json = serde_json::to_string(filter).map_err(|e| e.to_string())?;
        let sort_json = serde_json::to_string(sort).map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO filter_presets (name, filter_json, sort_json) VALUES (?1, ?2, ?3)",
            params![name, filter_json, sort_json],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        Ok(FilterPreset {
            id,
            name: name.to_string(),
            filter: filter.clone(),
            sort: sort.clone(),
        })
    }

    pub fn get_presets(&self) -> Result<Vec<FilterPreset>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, name, filter_json, sort_json FROM filter_presets ORDER BY name")
            .map_err(|e| e.to_string())?;

        let presets = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let name: String = row.get(1)?;
                let filter_json: String = row.get(2)?;
                let sort_json: String = row.get(3)?;
                Ok((id, name, filter_json, sort_json))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        presets
            .into_iter()
            .map(|(id, name, fj, sj)| {
                let filter: FilterParams =
                    serde_json::from_str(&fj).map_err(|e| e.to_string())?;
                let sort: SortParams = serde_json::from_str(&sj).map_err(|e| e.to_string())?;
                Ok(FilterPreset {
                    id,
                    name,
                    filter,
                    sort,
                })
            })
            .collect()
    }

    pub fn delete_preset(&self, id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM filter_presets WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ── Gallery pages ─────────────────────────────────────────────────────

    /// Check if we already have cached page URLs for this gallery.
    pub fn has_gallery_pages(&self, gid: i64) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM gallery_pages WHERE gid = ?1",
                params![gid],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(count > 0)
    }

    /// Store page URLs for a gallery (bulk insert, replaces existing).
    /// Each tuple: (page_index, page_url, thumb_url, imgkey).
    pub fn set_gallery_pages(
        &self,
        gid: i64,
        pages: &[(i32, String, Option<String>, Option<String>)],
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM gallery_pages WHERE gid = ?1", params![gid])
            .map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "INSERT INTO gallery_pages (gid, page_index, page_url, thumb_url, imgkey) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|e| e.to_string())?;
        for (idx, url, thumb, imgkey) in pages {
            stmt.execute(params![gid, idx, url, thumb, imgkey])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Upsert page entries for a gallery (insert or update individual pages).
    /// Unlike set_gallery_pages which replaces all pages, this inserts/updates only the given pages.
    /// Each tuple: (page_index, page_url, thumb_url, imgkey).
    pub fn upsert_gallery_pages(
        &self,
        gid: i64,
        pages: &[(i32, String, Option<String>, Option<String>)],
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "INSERT OR REPLACE INTO gallery_pages (gid, page_index, page_url, thumb_url, imgkey) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|e| e.to_string())?;
        for (idx, url, thumb, imgkey) in pages {
            stmt.execute(params![gid, idx, url, thumb, imgkey])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Store the showkey for a gallery (extracted from the detail page JS).
    pub fn set_gallery_showkey(&self, gid: i64, showkey: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE galleries SET showkey = ?1 WHERE gid = ?2",
            params![showkey, gid],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Get the showkey for a gallery.
    pub fn get_gallery_showkey(&self, gid: i64) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT showkey FROM galleries WHERE gid = ?1",
            params![gid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())
        .map(|opt| opt.flatten())
    }

    /// Get all page entries for a gallery.
    pub fn get_gallery_pages(&self, gid: i64) -> Result<Vec<GalleryPageEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT page_index, page_url, image_path, thumb_url, imgkey FROM gallery_pages WHERE gid = ?1 ORDER BY page_index",
            )
            .map_err(|e| e.to_string())?;
        let pages = stmt
            .query_map(params![gid], |row| {
                Ok(GalleryPageEntry {
                    page_index: row.get(0)?,
                    page_url: row.get(1)?,
                    image_path: row.get(2)?,
                    thumb_url: row.get(3)?,
                    imgkey: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(pages)
    }

    /// Update the cached image path for a specific page.
    pub fn set_page_image_path(&self, gid: i64, page_index: i32, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE gallery_pages SET image_path = ?1 WHERE gid = ?2 AND page_index = ?3",
            params![path, gid, page_index],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Clear all cached file paths from the database (thumb_path on galleries, image_path on pages).
    pub fn clear_all_cache_paths(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("UPDATE galleries SET thumb_path = NULL", [])
            .map_err(|e| e.to_string())?;
        conn.execute("UPDATE gallery_pages SET image_path = NULL", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ── Reading progress ──────────────────────────────────────────────────

    pub fn get_read_progress(&self, gid: i64) -> Result<Option<ReadProgress>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT gid, last_page_read, total_pages, last_read_at, is_completed
                 FROM reading_progress WHERE gid = ?1",
            )
            .map_err(|e| e.to_string())?;
        let result = stmt
            .query_row(params![gid], |row| {
                Ok(ReadProgress {
                    gid: row.get(0)?,
                    last_page_read: row.get(1)?,
                    total_pages: row.get(2)?,
                    last_read_at: row.get(3)?,
                    is_completed: row.get::<_, i32>(4)? != 0,
                })
            })
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    pub fn update_read_progress(&self, progress: &ReadProgress) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO reading_progress (gid, last_page_read, total_pages, last_read_at, is_completed)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(gid) DO UPDATE SET
                last_page_read = excluded.last_page_read,
                total_pages = excluded.total_pages,
                last_read_at = excluded.last_read_at,
                is_completed = excluded.is_completed",
            params![
                progress.gid,
                progress.last_page_read,
                progress.total_pages,
                progress.last_read_at,
                progress.is_completed as i32,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Get read progress for multiple galleries at once (for grid display).
    pub fn get_read_progress_batch(&self, gids: &[i64]) -> Result<Vec<ReadProgress>, String> {
        if gids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let placeholders: Vec<String> = gids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT gid, last_page_read, total_pages, last_read_at, is_completed
             FROM reading_progress WHERE gid IN ({})",
            placeholders.join(", ")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let param_refs: Vec<Box<dyn rusqlite::types::ToSql>> =
            gids.iter().map(|g| Box::new(*g) as Box<dyn rusqlite::types::ToSql>).collect();
        let refs: Vec<&dyn rusqlite::types::ToSql> = param_refs.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(rusqlite::params_from_iter(&refs), |row| {
                Ok(ReadProgress {
                    gid: row.get(0)?,
                    last_page_read: row.get(1)?,
                    total_pages: row.get(2)?,
                    last_read_at: row.get(3)?,
                    is_completed: row.get::<_, i32>(4)? != 0,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }

    // ── Reading sessions ──────────────────────────────────────────────────

    pub fn start_reading_session(&self, gid: i64, opened_at: i64) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO reading_sessions (gid, opened_at, pages_read) VALUES (?1, ?2, 0)",
            params![gid, opened_at],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn end_reading_session(&self, session_id: i64, closed_at: i64, pages_read: i32) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE reading_sessions SET closed_at = ?1, pages_read = ?2 WHERE id = ?3",
            params![closed_at, pages_read, session_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_reading_history(&self, limit: i64, offset: i64) -> Result<Vec<ReadingSession>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, gid, opened_at, closed_at, pages_read
                 FROM reading_sessions ORDER BY opened_at DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| e.to_string())?;
        let sessions = stmt
            .query_map(params![limit, offset], |row| {
                Ok(ReadingSession {
                    id: row.get(0)?,
                    gid: row.get(1)?,
                    opened_at: row.get(2)?,
                    closed_at: row.get(3)?,
                    pages_read: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(sessions)
    }

    // ── Search history ───────────────────────────────────────────────────

    /// Add a search query to history. Deduplicates by updating timestamp if same query exists.
    pub fn add_search_history(&self, query: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Delete existing entry with same query (case-insensitive) to avoid duplicates.
        conn.execute(
            "DELETE FROM search_history WHERE LOWER(query) = LOWER(?1)",
            params![query],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO search_history (query, searched_at) VALUES (?1, ?2)",
            params![query, now],
        )
        .map_err(|e| e.to_string())?;

        // Keep only the last 20 entries.
        conn.execute(
            "DELETE FROM search_history WHERE id NOT IN (SELECT id FROM search_history ORDER BY searched_at DESC LIMIT 20)",
            [],
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Get recent search history entries (most recent first).
    pub fn get_search_history(&self, limit: i64) -> Result<Vec<SearchHistoryEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, query, searched_at FROM search_history ORDER BY searched_at DESC LIMIT ?1")
            .map_err(|e| e.to_string())?;
        let entries = stmt
            .query_map(params![limit], |row| {
                Ok(SearchHistoryEntry {
                    id: row.get(0)?,
                    query: row.get(1)?,
                    searched_at: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(entries)
    }

    /// Clear all search history.
    pub fn clear_search_history(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM search_history", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Autocomplete tag suggestions matching the given prefix/substring.
    /// Queries the gallery_tags table with LIKE matching on name and namespace:name.
    /// Returns up to `limit` distinct (namespace, name) pairs ordered by name.
    pub fn search_tags_autocomplete(&self, query: &str, limit: i64) -> Result<Vec<TagSuggestion>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let pattern = format!("%{}%", query.to_lowercase());
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT namespace, name FROM gallery_tags \
                 WHERE LOWER(name) LIKE ?1 OR LOWER(namespace || ':' || name) LIKE ?1 \
                 ORDER BY name LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let suggestions = stmt
            .query_map(params![pattern, limit], |row| {
                Ok(TagSuggestion {
                    namespace: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        Ok(suggestions)
    }
}

// ---------------------------------------------------------------------------
// Migration SQL
// ---------------------------------------------------------------------------

const MIGRATION_V1: &str = "
CREATE TABLE galleries (
    gid         INTEGER PRIMARY KEY,
    token       TEXT    NOT NULL,
    title       TEXT    NOT NULL,
    title_jpn   TEXT,
    category    TEXT    NOT NULL DEFAULT 'Unknown',
    thumb_url   TEXT    NOT NULL,
    thumb_path  TEXT,
    uploader    TEXT,
    posted      INTEGER NOT NULL DEFAULT 0,
    rating      REAL    NOT NULL DEFAULT 0.0,
    file_count  INTEGER NOT NULL DEFAULT 0,
    file_size   INTEGER
);

CREATE INDEX idx_galleries_posted ON galleries(posted DESC);
CREATE INDEX idx_galleries_category ON galleries(category);
CREATE INDEX idx_galleries_rating ON galleries(rating DESC);

CREATE TABLE gallery_tags (
    gid       INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    namespace TEXT    NOT NULL,
    name      TEXT    NOT NULL,
    PRIMARY KEY (gid, namespace, name)
);

CREATE INDEX idx_gallery_tags_name ON gallery_tags(namespace, name);
";

const MIGRATION_V2: &str = "
-- FTS5 virtual table for full-text search across titles and tags.
-- content='' makes it an external-content table (we manage content ourselves).
CREATE VIRTUAL TABLE IF NOT EXISTS galleries_fts USING fts5(
    title,
    title_jpn,
    tags,
    content='',
    contentless_delete=1,
    tokenize='unicode61'
);

-- Populate FTS from existing data.
INSERT INTO galleries_fts(rowid, title, title_jpn, tags)
SELECT g.gid,
       g.title,
       COALESCE(g.title_jpn, ''),
       COALESCE((SELECT GROUP_CONCAT(gt.namespace || ':' || gt.name, ' ')
                 FROM gallery_tags gt WHERE gt.gid = g.gid), '')
FROM galleries g;

-- Additional indexes for filter queries.
CREATE INDEX IF NOT EXISTS idx_galleries_uploader ON galleries(uploader);
CREATE INDEX IF NOT EXISTS idx_galleries_file_count ON galleries(file_count);
";

const MIGRATION_V3: &str = "
CREATE TABLE filter_presets (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    filter_json TEXT    NOT NULL,
    sort_json   TEXT    NOT NULL
);
";

const MIGRATION_V4: &str = "
CREATE TABLE gallery_pages (
    gid         INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    page_index  INTEGER NOT NULL,
    page_url    TEXT    NOT NULL,
    image_path  TEXT,
    PRIMARY KEY (gid, page_index)
);
";

const MIGRATION_V5: &str = "
CREATE TABLE reading_progress (
    gid             INTEGER PRIMARY KEY REFERENCES galleries(gid) ON DELETE CASCADE,
    last_page_read  INTEGER NOT NULL DEFAULT 0,
    total_pages     INTEGER NOT NULL DEFAULT 0,
    last_read_at    INTEGER NOT NULL DEFAULT 0,
    is_completed    INTEGER NOT NULL DEFAULT 0
);
";

const MIGRATION_V6: &str = "
CREATE TABLE reading_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    gid         INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    opened_at   INTEGER NOT NULL,
    closed_at   INTEGER,
    pages_read  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_reading_sessions_gid ON reading_sessions(gid);
CREATE INDEX idx_reading_sessions_opened ON reading_sessions(opened_at DESC);
";

const MIGRATION_V7: &str = "
ALTER TABLE gallery_pages ADD COLUMN thumb_url TEXT;
";

const MIGRATION_V8: &str = "
ALTER TABLE galleries ADD COLUMN showkey TEXT;
ALTER TABLE gallery_pages ADD COLUMN imgkey TEXT;
";

const MIGRATION_V9: &str = "
ALTER TABLE galleries ADD COLUMN metadata_source TEXT NOT NULL DEFAULT 'browse';
ALTER TABLE galleries ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_galleries_metadata_source ON galleries(metadata_source);
";

const MIGRATION_V10: &str = "
CREATE TABLE search_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    query       TEXT    NOT NULL,
    searched_at INTEGER NOT NULL
);
CREATE INDEX idx_search_history_searched_at ON search_history(searched_at DESC);
";
