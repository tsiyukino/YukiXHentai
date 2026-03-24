# Models
> Last updated: 2026-03-23 (search types) | Affects: src-tauri/src/models/, src-tauri/src/commands/

## ExhCookies
- **Signature:** `{ ipb_member_id, ipb_pass_hash, igneous: String }`
- **Used by:** config, http, commands

## LoginResult
- **Signature:** `{ success: bool, message: String }`
- **Used by:** login/logout commands, frontend

## Gallery
- **Signature:** `{ gid: i64, token, title, title_jpn?, category, thumb_url, thumb_path?, uploader?, posted: i64, rating: f64, file_count: i32, file_size?: i64, tags: Vec<Tag> }`
- **Used by:** db, http/parser, commands, frontend

## Tag
- **Signature:** `{ namespace, name: String }`
- **Notes:** `Tag::parse("female:glasses")` splits on colon. No colon → namespace="misc".

## SyncResult
- **Signature:** `{ galleries_synced: usize, has_next_page: bool, message: String }`

## SyncProgress
- **Signature:** `{ current_page: u32, total_pages: u32, thumbs_downloaded: usize, thumbs_total: usize, galleries_synced: usize, message: String, done: bool }`
- **Used by:** `commands::sync_galleries`, frontend via Tauri event `sync-progress`
- **Notes:** Emitted as Tauri event during multi-page sync. Frontend listens via `listen("sync-progress")`.

## GalleryPage
- **Signature:** `{ galleries: Vec<Gallery>, total_count: i64 }`

## TagFilter / FilterParams / SortDirection / SortField / SortParams / FilterPreset
- See Phase 3 search types. FilterParams has: query, tags_include/exclude, categories, rating/pages/date ranges, language, uploader.

## GalleryPageEntry
- **Signature:** `{ page_index: i32, page_url: String, image_path: Option<String>, thumb_url: Option<String> }`
- **Used by:** db::get_gallery_pages, commands::get_gallery_pages, frontend reader + detail

## GalleryPages
- **Signature:** `{ gid: i64, token: String, title: String, pages: Vec<GalleryPageEntry>, total_pages: i32, showkey: Option<String> }`
- **Used by:** commands::get_gallery_pages, frontend reader

## GalleryPagesBatchResult
- **Signature:** `{ gid: i64, pages: Vec<GalleryPageEntry>, showkey: Option<String>, total_pages: i32, has_next_page: bool, detail_page: u32 }`
- **Used by:** commands::get_gallery_pages_batch, GalleryDetail.svelte
- **Notes:** Result of fetching a single ExH detail page. Frontend calls this on-demand per batch.

## ReadProgress
- **Signature:** `{ gid: i64, last_page_read: i32, total_pages: i32, last_read_at: i64, is_completed: bool }`
- **Used by:** commands, db, frontend reader + gallery card

## ReadingSession
- **Signature:** `{ id: i64, gid: i64, opened_at: i64, closed_at: Option<i64>, pages_read: i32 }`
- **Used by:** commands, db, (no UI yet)

## SyncPageResult
- **Signature:** `{ galleries: Vec<Gallery>, has_more: bool }`
- **Used by:** `commands::sync_next_page`, frontend infinite scroll
- **Notes:** Returns the full gallery array directly. Frontend appends to master list on receive.

## ThumbnailReadyEvent
- **Signature:** `{ gid: i64, path: String }`
- **Used by:** `commands::sync_next_page` (emitted as Tauri event `thumbnail-ready`)
- **Notes:** Emitted when a thumbnail download completes. Frontend updates the card's image in-place.

## GalleryEnrichedEvent
- **Signature:** `{ gallery: Gallery }`
- **Used by:** `commands::start_enrichment` (emitted as Tauri event `gallery-enriched`)
- **Notes:** Emitted for each gallery after it is enriched via the gdata API and upserted to DB. Frontend updates the gallery card in-place with richer metadata. Defined in commands/mod.rs (not models/).

## ImageDownloadProgressEvent
- **Signature:** `{ gid: i64, page_index: i32, status: String, path: Option<String>, error: Option<String> }`
- **Used by:** `download::ImageDownloadQueue` (emitted as Tauri event `image-download-progress`)
- **Status values:** `"queued"`, `"downloading"`, `"done"`, `"error"`, `"rate_limited"`
- **Notes:** Emitted during image downloads for reader progress display.

## AdvancedSearchOptions
- **Signature:** `{ search_name?: bool, search_tags?: bool, search_description?: bool, show_expunged?: bool, search_torrent_filenames?: bool, only_with_torrents?: bool, search_low_power_tags?: bool, search_downvoted_tags?: bool, minimum_rating?: u8 (2–5), min_pages?: u32, max_pages?: u32 }`
- **Used by:** `search_exhentai` command, `SearchPage.svelte`
- **Notes:** All fields optional. Bool defaults: name=true, tags=true, rest=false. minimum_rating/min_pages/max_pages: None = disabled.

## TagSuggestion
- **Signature:** `{ namespace: String, name: String }`
- **Used by:** `search_tags_autocomplete` command, `SearchPage.svelte`
- **Notes:** Autocomplete result from local gallery_tags DB table.

## ExhSearchResult
- **Signature:** `{ galleries: Vec<Gallery>, has_more: bool, next_url: Option<String> }`
- **Used by:** `search_exhentai` command
- **Notes:** Server-side search result. `has_more` and `next_url` both derived from #unext href. Frontend passes `next_url` back for cursor-based pagination on subsequent pages.

## SearchHistoryEntry
- **Signature:** `{ id: i64, query: String, searched_at: i64 }`
- **Used by:** `get_search_history` command, `SearchPage.svelte`
