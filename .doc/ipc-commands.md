# IPC Commands
> Last updated: 2026-03-25 (is_local routing for reading progress/session commands; library.db separation; folder naming gid-only) | Affects: src-tauri/src/commands/, src/lib/api/, src/lib/components/

## login
- **Signature:** `invoke("login", { ipbMemberId, ipbPassHash, igneous }) -> LoginResult`
- **Notes:** Validates cookies with test request. Persists on success.

## logout
- **Signature:** `invoke("logout") -> LoginResult`

## get_auth_status
- **Signature:** `invoke("get_auth_status") -> boolean`

## sync_gallery_page
- **Signature:** `invoke("sync_gallery_page", { page? }) -> SyncResult`
- **Notes:** Fetches one listing page (by page number), upserts into DB + FTS, downloads thumbnails. Rate-limited. Legacy single-page sync.

## sync_galleries
- **Signature:** `invoke("sync_galleries", { depth? }) -> SyncResult`
- **Used by:** `GalleryGrid.svelte` sync button
- **Notes:** Multi-page sync. Fetches `depth` listing pages sequentially (default 10, max 50) with rate limiting. Downloads thumbnails concurrently (up to 6 — matches JHentai/EhViewer safe limit for ehgt.org) with adaptive throttling (10s timeout, 30s backoff after 3 consecutive failures). Emits `sync-progress` Tauri events during sync. Returns total galleries synced.

## sync_next_page
- **Signature:** `invoke("sync_next_page") -> SyncPageResult { galleries: Gallery[], has_more: bool }`
- **Used by:** `GalleryGrid.svelte` (startup fresh sync + infinite scroll)
- **Notes:** Fetches one listing page from ExHentai using cursor-based pagination (tracked in `SyncCursor` app state — stores the next page URL from the `#unext` anchor). Upserts all galleries to DB (browse source), then re-reads from DB to get consistent state (thumb_path if already cached). Returns the full gallery array immediately. **Does NOT download thumbnails** — frontend calls `download_thumbnails_for_gids` for visible galleries only. Cursor advances automatically; reset via `reset_sync_cursor`.

## download_thumbnails_for_gids
- **Signature:** `invoke("download_thumbnails_for_gids", { gids: number[] }) -> number`
- **Used by:** `GalleryGrid.svelte` (viewport-driven thumbnail loading), `FavoritesPage.svelte` (IntersectionObserver-driven thumbnail loading)
- **Events emitted:** `thumbnail-ready` (per thumbnail, as each download completes)
- **Notes:** Downloads thumbnails for the specified gallery gids. Looks up galleries from DB to get thumb_urls. Spawns concurrent download in background (IPC returns immediately). Skips already-cached thumbnails (verified on disk with nonzero size). Up to 6 concurrent downloads (semaphore — matches JHentai/EhViewer safe limit for ehgt.org), 10s per-request timeout. Shared pause flag suspends new launches on rate-limit (10s) or 3 consecutive failures (30s). Frontend calls this with only the gids visible in the virtual scroll viewport, debounced at 150ms.

## reset_sync_cursor
- **Signature:** `invoke("reset_sync_cursor") -> void`
- **Used by:** `GalleryGrid.svelte` on every app startup
- **Notes:** Resets the `SyncCursor` so next `sync_next_page` starts from the first listing page.

## get_galleries
- **Signature:** `invoke("get_galleries", { offset, limit }) -> GalleryPage`

## get_galleries_by_gids
- **Signature:** `invoke("get_galleries_by_gids", { gids: number[] }) -> Gallery[]`
- **Used by:** `GalleryGrid.svelte` (session-scoped DB reload after sync)
- **Notes:** Returns galleries for the given gids, preserving input order. Includes tags. Used to reload authoritative DB data (thumb_paths, enriched metadata) scoped to only the gids seen in the current browsing session — prevents stale galleries from prior sessions leaking into the master list.

## search_galleries
- **Signature:** `invoke("search_galleries", { filter, sort, offset, limit }) -> GalleryPage`

## save_preset / get_presets / delete_preset
- Save/load/delete filter presets. See Phase 3 docs.

## fetch_gallery_metadata
- **Signature:** `invoke("fetch_gallery_metadata", { gid: number, token: string }) -> Gallery`
- **Used by:** `GalleryDetail.svelte` (progressive loading step 2)
- **Notes:** Calls ExHentai JSON API (`api.php` method `gdata`). Returns enriched Gallery with uploader, file_size, full tags. No page view cost. Updates DB. ~200ms response.

## get_page_thumbnail
- **Signature:** `invoke("get_page_thumbnail", { gid: number, pageIndex: number, thumbUrl: string }) -> string`
- **Used by:** `GalleryDetail.svelte` (page preview grid, via IntersectionObserver lazy loading)
- **Notes:** Downloads page thumbnail via Rust HTTP client (with cookies). Caches in `page-thumbs/{gid}/{page}.jpg`. Returns local path. 10s timeout. Checks `PageThumbCancellation` — returns "Cancelled" error if gallery changed. `thumbUrl` is either a plain URL (gdtl) or `sprite:{url}:{offsetX}:{offsetY}:{width}:{height}` (gdtm) — sprites are downloaded and cropped via the `image` crate.

## set_active_detail_gallery
- **Signature:** `invoke("set_active_detail_gallery", { gid: number | null }) -> void`
- **Used by:** `GalleryDetail.svelte` (open/close), `GalleryReader.svelte` (gallery open/close)
- **Notes:** Sets active gallery for `PageThumbCancellation`. Both `get_page_thumbnail` and `get_gallery_pages_batch` check this — they return "Cancelled" if gid doesn't match. Detail calls with `gid` on open, null on full close. Reader calls with `gid` on gallery open (overrides the null set by detail's g=null branch), and relies on the detail to restore on reader close back to detail.

## get_gallery_pages
- **Signature:** `invoke("get_gallery_pages", { gid: number, token: string, forceRefresh?: boolean }) -> GalleryPages`
- **Used by:** `api/reader.ts` (reader open when not all pages are loaded yet)
- **Events emitted:** `gallery-pages-batch` (per detail page, as each is fetched from ExHentai)
- **Notes:** Returns full page list for a gallery. Fetches all detail pages from ExHentai if not cached in DB. Rate-limited. Used as fallback when reader opens before all batches have been fetched by the detail panel. Emits `gallery-pages-batch` events. Pass `forceRefresh: true` to bypass DB cache.

## get_gallery_pages_batch
- **Signature:** `invoke("get_gallery_pages_batch", { gid: number, token: string, detailPage: number, pagesPerBatch?: number }) -> GalleryPagesBatchResult`
- **Used by:** `GalleryDetail.svelte` (lazy scroll-driven batch loading), `GalleryReader.svelte` (strip batch loading)
- **Notes:** Fetches a single ExHentai detail page (p=N). Returns the entries on that page (20 or 40 depending on site settings), plus `total_pages`, `has_next_page`, `showkey` (on first page), and `detail_page`. `pagesPerBatch` is the count ExHentai returns per detail page — omit for p=0, pass `result.pages.length` from p=0 for all subsequent pages. This ensures correct page index assignment when the user has 40 thumbnails/page configured. Checks `PageThumbCancellation` — returns "Cancelled" error if gallery changed. Caches fetched pages to DB incrementally via upsert. **DB cache path:** When pages are cached in DB, `total_pages` is read from `galleries.file_count` (not from `all_pages.len()`) to return the correct gallery total even when only a partial batch is cached.

## get_gallery_image
- **Signature:** `invoke("get_gallery_image", { gid: number, pageIndex: number, pageUrl: string, imgkey?: string, showkey?: string }) -> string`
- **Used by:** `api/reader.ts`, `GalleryReader.svelte`
- **Events emitted:** `image-download-progress` (per image, status updates during download)
- **Notes:** Returns local file path to full-size image. Checks cache first; if uncached, submits to `ImageDownloadQueue` and awaits result. When `imgkey` + `showkey` provided, queue uses showpage API fast path (single POST instead of HTML page fetch). Falls back to HTML scraping on API failure. Emits progress events for UI feedback. Checks `DownloadCancellation` flag — returns error if gallery downloads have been cancelled.

## cancel_image_downloads
- **Signature:** `invoke("cancel_image_downloads", { gid: number | null }) -> void`
- **Used by:** `GalleryReader.svelte` (onDestroy, handleClose, gallery change)
- **Notes:** Cancels all in-progress and queued image downloads for a gallery. Pass null to cancel all galleries. Sets the cancellation flag which is checked by the download queue before each download starts.

## register_download_session
- **Signature:** `invoke("register_download_session", { gid: number }) -> void`
- **Used by:** `GalleryReader.svelte` (on gallery change)
- **Notes:** Registers a new download session for a gallery. Cancels any previous session for the same gallery, preventing stale downloads from interfering.

## update_read_progress
- **Signature:** `invoke("update_read_progress", { progress: ReadProgress, isLocal: boolean }) -> void`
- **Notes:** `isLocal=true` routes to `library.db`; `isLocal=false` (default) routes to `yukixhentai.db`.

## get_read_progress
- **Signature:** `invoke("get_read_progress", { gid: number, isLocal: boolean }) -> ReadProgress | null`
- **Notes:** `isLocal=true` routes to `library.db`; `isLocal=false` (default) routes to `yukixhentai.db`.

## get_read_progress_batch
- **Signature:** `invoke("get_read_progress_batch", { gids: number[], isLocal: boolean }) -> ReadProgress[]`
- **Notes:** Batch load for grid display. `isLocal=true` routes to `library.db`.

## start_reading_session
- **Signature:** `invoke("start_reading_session", { gid: number, openedAt: number, isLocal: boolean }) -> number`
- **Notes:** Returns session ID. `isLocal=true` routes to `library.db`.

## end_reading_session
- **Signature:** `invoke("end_reading_session", { sessionId: number, closedAt: number, pagesRead: number, isLocal: boolean }) -> void`
- **Notes:** `isLocal=true` routes to `library.db`.

## get_reading_history
- **Signature:** `invoke("get_reading_history", { limit: number, offset: number }) -> ReadingSession[]`
- **Used by:** `HistoryPage.svelte`
- **Notes:** Displayed in History page with GID, date, pages read, duration. Reads from `yukixhentai.db` only (online history).

## get_detail_preview_size
- **Signature:** `invoke("get_detail_preview_size") -> number`
- **Used by:** `SettingsPage.svelte`, `GalleryDetail.svelte`
- **Notes:** Returns detail preview thumbnail size (px) from config. Default 120, range 80–200.

## set_detail_preview_size
- **Signature:** `invoke("set_detail_preview_size", { size: number }) -> void`
- **Used by:** `SettingsPage.svelte`, `GalleryDetail.svelte`
- **Notes:** Persists to `[ui].detail_preview_size` in config.toml.

## get_theme
- **Signature:** `invoke("get_theme") -> string`
- **Used by:** `+page.svelte` (onMount), `SettingsPage.svelte`
- **Notes:** Returns current theme from `[ui].theme` config. Default `"light"`.

## set_theme
- **Signature:** `invoke("set_theme", { theme: string }) -> void`
- **Used by:** `SettingsPage.svelte`, `Sidebar.svelte`
- **Notes:** Sets theme. Validates value (`"light"` | `"dark"`), invalid falls back to `"light"`. Persists to config.toml.

## start_enrichment
- **Signature:** `invoke("start_enrichment") -> number`
- **Used by:** `GalleryGrid.svelte` (fire-and-forget after sync)
- **Events emitted:** `gallery-enriched` (per gallery, as each is enriched via gdata API)
- **Notes:** Background enrichment of galleries with metadata_source='browse'. Fetches full metadata via gdata API in batches of 25. Uses `GdataRateLimiter` (4 requests burst, then 5s cooldown). Updates DB with metadata_source='api'. Returns count of galleries enriched. Does not block UI. Up to 250 galleries per invocation.

## resolve_thumb_path
- **Signature:** `invoke("resolve_thumb_path", { path: string }) -> string`
- **Notes:** Identity function. Frontend uses convertFileSrc() instead.

## get_cache_dir
- **Signature:** `invoke("get_cache_dir") -> string`
- **Used by:** `SettingsPage.svelte`
- **Notes:** Returns the current cache directory path. Returns custom path from `[storage].cache_dir` config if set, otherwise the platform default.

## set_cache_dir
- **Signature:** `invoke("set_cache_dir", { path: string }) -> void`
- **Used by:** `SettingsPage.svelte`
- **Notes:** Sets a custom cache directory. Pass empty string to reset to platform default. Does NOT move existing cache files — starts fresh in new location. Validates path exists or creates it. Persists to `[storage].cache_dir` in config.toml. Requires restart to take effect.

## clear_image_cache
- **Signature:** `invoke("clear_image_cache") -> number`
- **Used by:** `SettingsPage.svelte`
- **Notes:** Deletes all cached files (thumbnails, page thumbnails, originals) and clears DB paths (thumb_path, image_path). Returns bytes freed. Files re-download on demand.

## search_exhentai
- **Signature:** `invoke("search_exhentai", { query: string, nextUrl?: string | null, categoryMask?: number, advancedOptions?: AdvancedSearchOptions }) -> ExhSearchResult`
- **Used by:** `SearchPage.svelte`
- **Notes:** Fetches search results from ExHentai's server-side search. First page: builds URL from query params. `query` is the fully-assembled f_search string (free text + include/exclude tags joined by the frontend). `advancedOptions` maps to all advanced search URL params (see `AdvancedSearchOptions` in models.md). Subsequent pages: uses `nextUrl` directly (cursor from #unext href). Upserts results to DB (browse source). Saves query to search_history on first page only. Rate-limited.

## search_tags_autocomplete
- **Signature:** `invoke("search_tags_autocomplete", { query: string }) -> TagSuggestion[]`
- **Used by:** `SearchPage.svelte` tag input
- **Notes:** Queries `gallery_tags` table for entries matching `query` as substring of `name` or `namespace:name`. Returns up to 10 distinct `{ namespace, name }` results ordered by name. Used for tag include/exclude chip autocomplete; debounced 200ms on the frontend.

## get_favorite_status
- **Signature:** `invoke("get_favorite_status", { gid: number }) -> FavoriteStatus`
- **Used by:** `GalleryDetail.svelte` (on gallery open, fast local DB read)
- **Notes:** Returns local cached favorite status. No network call. `favcat: null` = not favorited.

## add_favorite
- **Signature:** `invoke("add_favorite", { gid: number, token: string, favcat: number, favnote: string }) -> void`
- **Used by:** `FavoriteDialog.svelte`
- **Notes:** POSTs to ExHentai `gallerypopups.php`, then upserts to local `cloud_favorites` DB. Rate-limited.

## remove_favorite
- **Signature:** `invoke("remove_favorite", { gid: number, token: string }) -> void`
- **Used by:** `FavoriteDialog.svelte`
- **Notes:** POSTs `favcat=favdel` to ExHentai, then removes from local DB. Rate-limited.

## fetch_favorites
- **Signature:** `invoke("fetch_favorites", { favcat?: number | null, nextUrl?: string | null }) -> FavoritesResult`
- **Used by:** `FavoritesPage.svelte`
- **Notes:** GETs `favorites.php` from ExHentai. Cursor-based pagination via `nextUrl`. Upserts galleries to DB. Returns galleries + folder metadata + pagination cursor. Rate-limited.

## get_favorite_folders
- **Signature:** `invoke("get_favorite_folders") -> FavoriteFolder[]`
- **Used by:** `FavoriteDialog.svelte`, `FavoritesPage.svelte`
- **Notes:** Returns cached folder names/counts from local DB. No network call.

## get_search_history
- **Signature:** `invoke("get_search_history", { limit?: number }) -> SearchHistoryEntry[]`
- **Used by:** `SearchPage.svelte` (search input focus dropdown)
- **Notes:** Returns last N search queries (default 20) from search_history table, most recent first.

## clear_search_history
- **Signature:** `invoke("clear_search_history") -> void`
- **Used by:** `SearchPage.svelte`
- **Notes:** Deletes all entries from search_history table.

## get_read_cache_stats
- **Signature:** `invoke("get_read_cache_stats") -> ReadCacheStats`
- **Notes:** Returns `{ used_bytes, max_bytes, file_count }`. `max_bytes` derived from `storage.read_cache_max_mb` config.

## set_read_cache_max_mb
- **Signature:** `invoke("set_read_cache_max_mb", { maxMb: number }) -> void`
- **Notes:** Updates `storage.read_cache_max_mb` config (clamped 128–4096). Persists to config.toml.

## clear_read_cache
- **Signature:** `invoke("clear_read_cache") -> number`
- **Notes:** Deletes all originals cache files tracked in `read_cache_index`, clears `gallery_pages.image_path` entries, removes files from disk. Returns bytes freed.

## get_local_galleries
- **Signature:** `invoke("get_local_galleries", { offset: number, limit: number }) -> GalleryPage`
- **Notes:** Returns galleries where `is_local=1`, ordered by posted DESC. Tags include both `gallery_tags` and `local_tags`.

## get_local_gallery_pages
- **Signature:** `invoke("get_local_gallery_pages", { gid: number }) -> LocalPage[]`
- **Notes:** Returns all pages for a local gallery from `local_gallery_pages`, ordered by page_index.

## update_gallery_metadata
- **Signature:** `invoke("update_gallery_metadata", { gid: number, patch: GalleryMetadataPatch }) -> void`
- **Notes:** Updates only the provided fields. For local galleries, rewrites metadata.json in the gallery folder.

## reorder_local_pages
- **Signature:** `invoke("reorder_local_pages", { gid: number, newOrder: number[] }) -> void`
- **Notes:** `newOrder` is a list of current `page_index` values in the desired new order. Rewrites metadata.json.

## insert_local_pages
- **Signature:** `invoke("insert_local_pages", { gid: number, filePaths: string[], insertAfterIndex: number }) -> LocalPage[]`
- **Notes:** Copies files to the gallery folder, reads image dimensions, inserts rows. Returns inserted LocalPage objects. Existing pages after insertAfterIndex are renumbered. Rewrites metadata.json.

## remove_local_page
- **Signature:** `invoke("remove_local_page", { gid: number, pageIndex: number, deleteFile: bool }) -> void`
- **Notes:** Removes the page row. If `deleteFile=true`, also deletes the file from disk. Renumbers remaining pages. Rewrites metadata.json.

## set_local_gallery_cover
- **Signature:** `invoke("set_local_gallery_cover", { gid: number, filePath: string }) -> string`
- **Notes:** Copies the file to `{gallery_folder}/cover.{ext}`, generates a thumbnail (updates `galleries.thumb_path`). Returns the new thumb_path.

## import_local_folder
- **Signature:** `invoke("import_local_folder", { folderPath: string }) -> ImportPreview`
- **Notes:** Scans the folder for image files (natural sort). Reads metadata.json if present. Returns preview without importing. No DB changes.

## confirm_import_local_folder
- **Signature:** `invoke("confirm_import_local_folder", { folderPath: string, gid: number, token: string, title: string, category: string }) -> Gallery`
- **Notes:** Copies images to `{library_dir}/{gid}/`, upserts gallery in `library.db` (is_local=1), inserts local_gallery_pages rows, generates thumbnail from first image, writes metadata.json. Returns the full Gallery object.

## parse_download_queue_json
- **Signature:** `invoke("parse_download_queue_json", { jsonText: string }) -> QueueEntry[]`
- **Notes:** Parses a JSON array of `{ gid, token?, title? }` objects. Checks DB for `already_local` flag on each entry.

## resolve_gallery_token
- **Signature:** `invoke("resolve_gallery_token", { gid: number }) -> ResolvedGallery`
- **Notes:** Fetches `https://exhentai.org/g/{gid}/` and parses the canonical `<link>` tag to extract the token. Rate-limited. Returns `{ gid, token?, title?, error? }`.

## submit_download_queue
- **Signature:** `invoke("submit_download_queue", { entries: SubmitEntry[], downloadOriginals: boolean, subfolder?: string }) -> SubmitResult`
- **Used by:** `GalleryDetail.svelte` (Download button), `QueueDownloadPage.svelte` (Queue all ready)
- **Events emitted:** `local-download-progress` (per page during download, and on completion)
- **Notes:** Enqueues galleries that are not already local. Returns `{ queued, skipped_already_local }`. The background worker in `LocalDownloadQueue` processes jobs sequentially: fetches gdata metadata, fetches all detail pages to get page URLs + imgkeys, downloads each full image, downloads the gallery cover thumbnail (`meta.thumb_url`) into `{gallery_folder}/cover_thumb.{ext}` (falls back to first page image if unavailable), saves to library folder, registers as local gallery in DB.

## get_download_queue_status
- **Signature:** `invoke("get_download_queue_status") -> DownloadQueueStatus`
- **Notes:** Returns current snapshot of `LocalDownloadQueue` state. `downloading=1` means a gallery is actively being downloaded; `queued` is the pending count.

## pause_download_queue
- **Signature:** `invoke("pause_download_queue") -> void`

## resume_download_queue
- **Signature:** `invoke("resume_download_queue") -> void`

## cancel_download_queue
- **Signature:** `invoke("cancel_download_queue", { gid?: number | null }) -> void`
- **Notes:** If `gid` is provided, cancels only that gallery. If null/undefined, cancels all pending.

## get_library_dir
- **Signature:** `invoke("get_library_dir") -> string`
- **Used by:** `SettingsPage.svelte`
- **Notes:** Returns the current library directory. Returns custom path from `[storage].library_dir` config if set, otherwise the platform default (`{data_local_dir}/yukixhentai/library/`).

## set_library_dir
- **Signature:** `invoke("set_library_dir", { path: string }) -> void`
- **Used by:** `SettingsPage.svelte`
- **Notes:** Sets a custom library directory. Pass empty string to reset to platform default. Does NOT move existing gallery files. Validates path exists or creates it. Persists to `[storage].library_dir` in config.toml.

## delete_local_gallery
- **Signature:** `invoke("delete_local_gallery", { gid: number }) -> void`
- **Used by:** `GalleryDetail.svelte` (Delete button for local galleries)
- **Notes:** Deletes the gallery row from DB (cascades to `local_gallery_pages`, tags, history, etc.) and removes the `local_folder` directory from disk. Errors if disk deletion fails. After delete, increments `libraryRefreshTick` store so `LocalPage.svelte` reloads immediately.

## sync_local_gallery
- **Signature:** `invoke("sync_local_gallery", { gid: number }) -> void`
- **Used by:** `GalleryDetail.svelte` (Sync button, only shown when `origin` and `remote_gid` are set)
- **Notes:** Placeholder — checks that `origin` and `remote_gid` exist, then returns OK. Actual sync logic (per-origin site) will be added in a future phase. Only local galleries with `origin`+`remote_gid` set (i.e. downloaded via queue, not manually imported) can be synced.
