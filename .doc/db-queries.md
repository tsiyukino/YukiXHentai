# Database Queries
> Last updated: 2026-03-22 (metadata enrichment queries) | Affects: src-tauri/src/db/

## upsert_gallery_with_source
- **Signature:** `fn upsert_gallery_with_source(&self, gallery: &Gallery, metadata_source: &str) -> Result<(), String>`
- **Used by:** `upsert_gallery`, `upsert_gallery_browse`
- **Notes:** INSERT OR UPDATE on gid. Sets metadata_source and updated_at. Preserves thumb_path. Replaces tags. Updates FTS5.

## upsert_gallery_browse
- **Signature:** `fn upsert_gallery_browse(&self, gallery: &Gallery) -> Result<(), String>`
- **Used by:** `commands::sync_gallery_page`, `commands::sync_galleries`, `commands::sync_next_page`
- **Notes:** Browse-source upsert. If gallery already has metadata_source='api', only updates thumb_url/thumb_path (preserves richer API data). Otherwise delegates to `upsert_gallery_with_source` with "browse".

## upsert_gallery
- **Signature:** `fn upsert_gallery(&self, gallery: &Gallery) -> Result<(), String>`
- **Used by:** `commands::fetch_gallery_metadata`, `commands::start_enrichment`
- **Notes:** API-source upsert. Delegates to `upsert_gallery_with_source` with "api". Always overwrites.

## get_galleries_needing_enrichment
- **Signature:** `fn get_galleries_needing_enrichment(&self, limit: i64) -> Result<Vec<(i64, String)>, String>`
- **Used by:** `commands::start_enrichment`
- **Notes:** Returns (gid, token) pairs where metadata_source='browse', ordered by updated_at DESC.

## set_thumb_path
- **Signature:** `fn set_thumb_path(&self, gid: i64, path: &str) -> Result<(), String>`
- **Used by:** `commands::sync_gallery_page`

## get_galleries
- **Signature:** `fn get_galleries(&self, offset: i64, limit: i64) -> Result<GalleryPage, String>`
- **Used by:** `commands::get_galleries`
- **Notes:** Ordered by posted DESC. Loads tags per gallery.

## get_galleries_by_gids
- **Signature:** `fn get_galleries_by_gids(&self, gids: &[i64]) -> Result<Vec<Gallery>, String>`
- **Used by:** `commands::sync_next_page`
- **Notes:** Fetches galleries by specific gids with tags, preserving input order.

## search_galleries
- **Signature:** `fn search_galleries(&self, filter: &FilterParams, sort: &SortParams, offset: i64, limit: i64) -> Result<GalleryPage, String>`
- **Used by:** `commands::search_galleries`
- **Notes:** Dynamic query builder. FTS5 MATCH, tag EXISTS, category IN, range filters, ORDER BY.

## save_preset / get_presets / delete_preset
- **Signatures:** CRUD on filter_presets table. JSON serialization.

## has_gallery_pages
- **Signature:** `fn has_gallery_pages(&self, gid: i64) -> Result<bool, String>`
- **Notes:** Check if page URLs are cached for this gallery.

## set_gallery_pages
- **Signature:** `fn set_gallery_pages(&self, gid: i64, pages: &[(i32, String, Option<String>, Option<String>)]) -> Result<(), String>`
- **Notes:** Replaces all page URLs for a gallery (DELETE + INSERT).

## upsert_gallery_pages
- **Signature:** `fn upsert_gallery_pages(&self, gid: i64, pages: &[(i32, String, Option<String>, Option<String>)]) -> Result<(), String>`
- **Notes:** Insert or replace individual page entries (INSERT OR REPLACE). Used by `get_gallery_pages_batch` for incremental page storage without deleting existing pages.

## get_gallery_pages
- **Signature:** `fn get_gallery_pages(&self, gid: i64) -> Result<Vec<GalleryPageEntry>, String>`
- **Notes:** Ordered by page_index. Includes image_path if cached.

## set_page_image_path
- **Signature:** `fn set_page_image_path(&self, gid: i64, page_index: i32, path: &str) -> Result<(), String>`

## get_read_progress
- **Signature:** `fn get_read_progress(&self, gid: i64) -> Result<Option<ReadProgress>, String>`

## update_read_progress
- **Signature:** `fn update_read_progress(&self, progress: &ReadProgress) -> Result<(), String>`
- **Notes:** Upsert on gid.

## get_read_progress_batch
- **Signature:** `fn get_read_progress_batch(&self, gids: &[i64]) -> Result<Vec<ReadProgress>, String>`
- **Notes:** Dynamic IN clause for batch loading progress on gallery grid.

## start_reading_session
- **Signature:** `fn start_reading_session(&self, gid: i64, opened_at: i64) -> Result<i64, String>`
- **Notes:** Returns session ID.

## end_reading_session
- **Signature:** `fn end_reading_session(&self, session_id: i64, closed_at: i64, pages_read: i32) -> Result<(), String>`

## get_reading_history
- **Signature:** `fn get_reading_history(&self, limit: i64, offset: i64) -> Result<Vec<ReadingSession>, String>`
- **Notes:** Ordered by opened_at DESC.

## search_tags_autocomplete
- **Signature:** `fn search_tags_autocomplete(&self, query: &str, limit: i64) -> Result<Vec<TagSuggestion>, String>`
- **Used by:** `commands::search_tags_autocomplete`
- **Query:** `SELECT DISTINCT namespace, name FROM gallery_tags WHERE LOWER(name) LIKE ?1 OR LOWER(namespace || ':' || name) LIKE ?1 ORDER BY name LIMIT ?2`
- **Notes:** Pattern is `%{query.to_lowercase()}%`. Returns up to `limit` (10) results. Queries local DB only — no network call.
