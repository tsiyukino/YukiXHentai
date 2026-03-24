# Database Schema
> Last updated: 2026-03-23 (migration v10: search_history) | Affects: src-tauri/src/db/

Database: `{data_local_dir}/yukixhentai/yukixhentai.db`
- WAL mode enabled
- Foreign keys enabled
- Migration system: `schema_version` table tracks applied version number

## schema_version
- **Schema:** `CREATE TABLE schema_version (version INTEGER NOT NULL)`
- **Used by:** `db::DbState::run_migrations`
- **Notes:** Single row tracking the current schema version. Current: v10.

## galleries
- **Schema:**
```sql
CREATE TABLE galleries (
    gid              INTEGER PRIMARY KEY,
    token            TEXT    NOT NULL,
    title            TEXT    NOT NULL,
    title_jpn        TEXT,
    category         TEXT    NOT NULL DEFAULT 'Unknown',
    thumb_url        TEXT    NOT NULL,
    thumb_path       TEXT,
    showkey          TEXT,
    uploader         TEXT,
    posted           INTEGER NOT NULL DEFAULT 0,
    rating           REAL    NOT NULL DEFAULT 0.0,
    file_count       INTEGER NOT NULL DEFAULT 0,
    file_size        INTEGER,
    metadata_source  TEXT    NOT NULL DEFAULT 'browse',
    updated_at       INTEGER NOT NULL DEFAULT 0
);
```
- **Indexes:** posted DESC, category, rating DESC, uploader, file_count, metadata_source
- **Used by:** `db::DbState`, sync, browse, search, reader, enrichment commands
- **Notes:** `gid` = ExHentai gallery ID. Upsert preserves existing thumb_path. `showkey` extracted from detail page JS (`var showkey="..."`), used by showpage API. `metadata_source` is "browse" (from HTML listing) or "api" (enriched via gdata API). Browse upserts do not overwrite API-enriched data. `updated_at` is Unix timestamp of last upsert. Migration v9 adds metadata_source/updated_at.

## gallery_tags
- **Schema:** `(gid INTEGER, namespace TEXT, name TEXT, PK(gid,namespace,name))`
- **Indexes:** `idx_gallery_tags_name(namespace, name)`
- **Used by:** `db::DbState`, tag queries, search filter
- **Notes:** Replaced wholesale on upsert. FK → galleries(gid) CASCADE.

## galleries_fts (FTS5)
- **Schema:** `fts5(title, title_jpn, tags, content='', contentless_delete=1, tokenize='unicode61')`
- **Used by:** `search_galleries`, `upsert_gallery`
- **Notes:** rowid = galleries.gid. Updated on every upsert.

## filter_presets
- **Schema:** `(id AUTOINCREMENT, name TEXT, filter_json TEXT, sort_json TEXT)`
- **Used by:** preset CRUD commands
- **Notes:** JSON-serialized FilterParams/SortParams. Migration v3.

## gallery_pages
- **Schema:**
```sql
CREATE TABLE gallery_pages (
    gid         INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    page_index  INTEGER NOT NULL,
    page_url    TEXT    NOT NULL,
    image_path  TEXT,
    thumb_url   TEXT,
    imgkey      TEXT,
    PRIMARY KEY (gid, page_index)
);
```
- **Used by:** `db::get_gallery_pages`, `db::set_gallery_pages`, `db::set_page_image_path`
- **Notes:** Caches image page URLs from gallery detail page. image_path set after download. thumb_url stores page thumbnail URL from detail page grid. imgkey extracted from page URL (`/s/{imgkey}/{gid}-{page}`), used by showpage API. Migration v4 (base), v7 (thumb_url), v8 (imgkey).

## reading_progress
- **Schema:**
```sql
CREATE TABLE reading_progress (
    gid             INTEGER PRIMARY KEY REFERENCES galleries(gid) ON DELETE CASCADE,
    last_page_read  INTEGER NOT NULL DEFAULT 0,
    total_pages     INTEGER NOT NULL DEFAULT 0,
    last_read_at    INTEGER NOT NULL DEFAULT 0,
    is_completed    INTEGER NOT NULL DEFAULT 0
);
```
- **Used by:** `db::get_read_progress`, `db::update_read_progress`, `db::get_read_progress_batch`
- **Notes:** Upsert on gid. is_completed stored as 0/1. Migration v5.

## reading_sessions
- **Schema:**
```sql
CREATE TABLE reading_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    gid         INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    opened_at   INTEGER NOT NULL,
    closed_at   INTEGER,
    pages_read  INTEGER NOT NULL DEFAULT 0
);
```
- **Indexes:** `idx_reading_sessions_gid`, `idx_reading_sessions_opened(opened_at DESC)`
- **Used by:** `db::start_reading_session`, `db::end_reading_session`, `db::get_reading_history`
- **Notes:** One row per reading session. closed_at NULL until session ends. Migration v6.

## search_history
- **Schema:**
```sql
CREATE TABLE search_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    query       TEXT    NOT NULL,
    searched_at INTEGER NOT NULL
);
```
- **Indexes:** `idx_search_history_searched_at(searched_at DESC)`
- **Used by:** `db::add_search_history`, `db::get_search_history`, `db::clear_search_history`
- **Notes:** Stores last 20 search queries. Deduplicates by updating timestamp (case-insensitive). Auto-trims to 20 entries on insert. Migration v10.
