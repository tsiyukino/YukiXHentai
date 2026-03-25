# Database Schema
> Last updated: 2026-03-25 (library.db: separate local library database) | Affects: src-tauri/src/db/

## library.db
`{library_dir}/library.db`  (default: `{data_local_dir}/yukixhentai/library/library.db`)
- Lives alongside the gallery folders inside the library root. Never cleared on exit.
- Separate database for local library data. Created at first launch.
- Schema version tracked in its own `schema_version` table. Current: v1.
- Tables: `galleries`, `gallery_tags`, `local_gallery_pages`, `reading_progress`, `reading_sessions`
- All rows in `galleries` here are local (no `is_local` column needed).
- `reading_progress` and `reading_sessions` here track local gallery reading only.
- IPC commands for local galleries route to this DB; online commands route to `yukixhentai.db`.

---

Database: `{data_local_dir}/yukixhentai/yukixhentai.db`
- WAL mode enabled
- Foreign keys enabled
- Migration system: `schema_version` table tracks applied version number

## schema_version
- **Schema:** `CREATE TABLE schema_version (version INTEGER NOT NULL)`
- **Used by:** `db::DbState::run_migrations`
- **Notes:** Single row tracking the current schema version. Current: v12.

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
    updated_at       INTEGER NOT NULL DEFAULT 0,
    description      TEXT,
    is_local         INTEGER NOT NULL DEFAULT 0,
    local_folder     TEXT,
    origin           TEXT,
    remote_gid       INTEGER
);
```
- **Indexes:** posted DESC, category, rating DESC, uploader, file_count, metadata_source, is_local
- **Used by:** `db::DbState`, sync, browse, search, reader, enrichment, local gallery commands
- **Notes:** `gid` = ExHentai gallery ID. Upsert preserves existing thumb_path. `showkey` extracted from detail page JS, used by showpage API. `metadata_source` is "browse"/"api"/"local". `is_local=1` means locally-imported gallery (new installs write local galleries to `library.db` instead; this column is retained for existing data). `local_folder` is absolute path to gallery folder on disk. `description` from API or local import. `origin` is the source site identifier (e.g. `"exhentai"`) set on galleries downloaded via the queue; NULL for manually-imported folders. `remote_gid` is the ID on the origin site. Migration v9 adds metadata_source/updated_at; v12 adds description/is_local/local_folder; v13 adds origin/remote_gid.

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
- **Notes:** Upsert on gid. is_completed stored as 0/1. **Not cleared on exit** — cleaned by `clean_old_reading_history` (orphaned rows older than `history.retention_days`). Migration v5.

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
- **Notes:** One row per reading session. closed_at NULL until session ends. **Not cleared on exit** — auto-cleaned on exit by `clean_old_reading_history` (rows with `opened_at` older than `history.retention_days`). Migration v6.

## cloud_favorites
- **Schema:**
```sql
CREATE TABLE cloud_favorites (
    gid         INTEGER PRIMARY KEY REFERENCES galleries(gid) ON DELETE CASCADE,
    token       TEXT    NOT NULL,
    favcat      INTEGER NOT NULL DEFAULT 0,
    favnote     TEXT    NOT NULL DEFAULT '',
    added_at    INTEGER NOT NULL DEFAULT 0
);
```
- **Indexes:** `idx_cloud_favorites_favcat(favcat)`, `idx_cloud_favorites_added(added_at DESC)`
- **Used by:** `db::upsert_cloud_favorite`, `db::remove_cloud_favorite`, `db::get_cloud_favorite`, `db::list_cloud_favorites`
- **Notes:** Local cache of cloud favorite status. `favcat` is 0–9 folder index. `added_at` is Unix timestamp. Migration v11.

## favorite_folders
- **Schema:**
```sql
CREATE TABLE favorite_folders (
    idx     INTEGER PRIMARY KEY,
    name    TEXT    NOT NULL,
    count   INTEGER NOT NULL DEFAULT 0
);
```
- **Used by:** `db::upsert_favorite_folder`, `db::get_favorite_folders`
- **Notes:** Cached folder names and counts from `favorites.php`. idx 0–9. Migration v11.

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
- **Notes:** Stores last 20 search queries. Deduplicates by updating timestamp (case-insensitive). Auto-trims to 20 entries on insert. **Not cleared on exit** — persists until user explicitly clears. Migration v10.

## read_cache_index (v12)
- **Schema:**
```sql
CREATE TABLE read_cache_index (
    cache_key   TEXT    PRIMARY KEY,
    file_path   TEXT    NOT NULL,
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    last_access INTEGER NOT NULL DEFAULT 0
);
```
- **Indexes:** `idx_read_cache_last_access(last_access)`
- **Used by:** `db::read_cache_upsert`, `db::read_cache_touch`, `db::read_cache_stats`, `db::read_cache_evict_lru`, `db::read_cache_clear_and_unlink`; `images::read_cache` helpers; `get_read_cache_stats`, `clear_read_cache` commands
- **Notes:** Tracks originals cache entries for LRU eviction. `cache_key` = `"{gid}_{page_index}"`. `last_access` Unix timestamp updated on read/write. LRU eviction triggered after each save.

## local_gallery_pages (v12)
- **Schema:**
```sql
CREATE TABLE local_gallery_pages (
    gid         INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    page_index  INTEGER NOT NULL,
    file_path   TEXT    NOT NULL,
    source_url  TEXT,
    width       INTEGER,
    height      INTEGER,
    PRIMARY KEY (gid, page_index)
);
```
- **Used by:** `db::get_local_gallery_pages`, `db::insert_local_pages`, `db::remove_local_page`, `db::reorder_local_pages`
- **Notes:** Stores file paths for locally-imported gallery images. `page_index` is 0-based. Renumbered on insert/delete to keep indices contiguous.

## local_tags (v12)
- **Schema:**
```sql
CREATE TABLE local_tags (
    gid         INTEGER NOT NULL REFERENCES galleries(gid) ON DELETE CASCADE,
    namespace   TEXT    NOT NULL DEFAULT '',
    name        TEXT    NOT NULL,
    PRIMARY KEY (gid, namespace, name)
);
```
- **Used by:** `db::update_gallery_metadata`, `db::upsert_local_gallery`, `db::get_local_galleries`
- **Notes:** User-editable tags for local galleries (separate from gallery_tags which is overwritten on remote sync). Combined with gallery_tags in `get_local_galleries` query.
