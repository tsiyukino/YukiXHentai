# Image Pipeline
> Last updated: 2026-03-22 (viewport-driven thumbnails) | Affects: src-tauri/src/images/, src-tauri/src/download/, src-tauri/src/commands/

## Cache directory
- Default: `{platform_cache_dir}/yukixhentai/` (Windows: `%LOCALAPPDATA%/yukixhentai/`)
- Configurable via `[storage].cache_dir` in config.toml (set via `set_cache_dir` IPC).
- All three caches (originals, page-thumbs, thumbs) live under this base.
- Changing the cache dir requires restart (caches are initialized once at startup).
- `clear_image_cache` IPC command deletes all cached files and clears DB paths.

## OriginalsCache
- **Signature:** `struct OriginalsCache { base_dir: PathBuf }`
- **Used by:** `commands::get_gallery_image`, `commands::clear_image_cache`, Tauri managed state
- **Notes:** Stored at `{cache_dir}/yukixhentai/originals/`. Initialized once at app startup.

### Cache directory structure (originals)
```
{cache_dir}/yukixhentai/originals/{ab}/{cd}/{hex_gid}_{page_index:04}.{ext}
```
- Sharded by first 4 hex chars of zero-padded gid.

### path_for_image / save / find_cached
- Standard cache operations. find_cached checks jpg, png, webp, gif.

## PageThumbCache
- **Signature:** `struct PageThumbCache { base_dir: PathBuf }`
- **Used by:** `commands::get_page_thumbnail`, Tauri managed state
- **Notes:** Stored at `{cache_dir}/yukixhentai/page-thumbs/`. Caches gallery detail page preview thumbnails.

### Cache directory structure (page-thumbs)
```
{cache_dir}/yukixhentai/page-thumbs/{gid}/{page_index}.jpg
```
- Flat directory per gallery (not sharded — bounded by gallery page count).

### path_for_page / save / find_cached
- Standard cache operations. `find_cached` returns path string if file exists.

### Sprite thumbnail handling
- `gdtm` thumbnails are CSS sprite sheets (~20 thumbs per image, differentiated by `background-position`).
- `thumb_url` stored as `sprite:{url}:{offsetX}:{offsetY}:{width}:{height}` for sprites.
- `get_page_thumbnail` command detects `sprite:` prefix, downloads the sprite, crops individual thumbnail via `image` crate, saves the crop.
- `gdtl` thumbnails are individual URLs — downloaded directly without cropping.

## ThumbCache
- **Signature:** `struct ThumbCache { base_dir: PathBuf }`
- **Used by:** `commands::download_thumbnails_for_gids`, `commands::sync_galleries`, Tauri managed state
- **Notes:** Stored at `{cache_dir}/yukixhentai/thumbs/`.

### Cache directory structure (thumbs)
```
{cache_dir}/yukixhentai/thumbs/{ab}/{cd}/{abcdef0123456789}.jpg
```
- Sharded by first 4 hex chars of zero-padded gid (16 hex digits).

### path_for_gid / save / exists / exists_valid / get_path
- Standard cache operations for thumbnails.
- `exists` checks file exists with nonzero size.
- `exists_valid` additionally removes corrupted (zero-byte) files so they are re-downloaded.

## Viewport-driven thumbnail loading
- **`sync_next_page` does NOT download thumbnails.** It returns galleries immediately.
- Frontend `GalleryGrid` tracks which galleries are visible via `VirtualGrid`/`VirtualList` `onVisibleRangeChanged` callback.
- When visible galleries lack `thumb_path`, frontend calls `download_thumbnails_for_gids(gids)` IPC with only the visible gids.
- Call is debounced (150ms) to avoid spamming during fast scrolling.
- `requestedThumbGids` set prevents re-requesting thumbnails already in-flight.
- This means loading 400 galleries but only viewing 100 results in ~100 thumbnail downloads, not 400.

## Gallery thumbnail throttling (`download_thumbs_sequential`)
- Downloads are **sequential** (one at a time) — no concurrency.
- **200ms base delay** between requests.
- **10s timeout** per HTTP request — prevents silent hangs from ExHentai throttling.
- **No retries** on individual failures — the frontend can re-request on next scroll.
- **Adaptive backoff** on error/timeout: delay increases to 2s for the next 10 downloads.
- **3 consecutive failures** → 30s pause, then resume with 500ms delay for 10 downloads.
- **429/503/509 responses** → 10s immediate pause + 2s delay for next 10 downloads.
- **Content-type validation**: rejects non-`image/*` responses.
- **Diagnostic logging**: `THUMB_DOWNLOAD`, `THUMB_SUCCESS`, `THUMB_FAIL`, `THUMB_TIMEOUT`, `THUMB_RATE_LIMITED`, `THUMB_WRONG_TYPE`, `THUMB_SAVE_REJECTED`, `THUMB_BACKOFF`, `THUMB_BATCH`, `THUMB_BATCH_DONE`, `THUMB_COOLDOWN_END`.

## Page thumbnail concurrency
- Frontend fires up to **20 concurrent** `get_page_thumbnail` requests per batch.
- CDN thumbnails (ehgt.org) are not rate-limited — no backend semaphore needed.
- Full-size reader images remain at 3 concurrent (ImageDownloadQueue semaphore).

## Frontend display
- **Asset protocol:** Enabled in `tauri.conf.json` with `assetProtocol.enable: true` and scope `allow: ["**/*", "$APPCACHE/**", "$APPLOCALDATA/**", "$APPDATA/**"]`.
- **Cargo feature:** `protocol-asset` enabled on `tauri` dependency.
- Use `convertFileSrc(path)` from `@tauri-apps/api/core` to convert local paths to asset URLs.
- Utility: `src/lib/utils/thumb.ts::thumbSrc(thumbPath, thumbUrl)` for thumbnails. Returns empty string if no local path — remote URLs can't work in webview (no cookies). Components show skeleton until local path available.
- **Data validation:** `ThumbCache::save` and `OriginalsCache::save` reject empty data and HTML error pages.
- Reader images: `convertFileSrc(path)` called in `GalleryReader.svelte` after `get_gallery_image`. Reader clears all image state (`loadedImages`, `loadingPages`, etc.) when `activeGid` changes, preventing stale images from previous gallery.
- **Viewport-driven thumbnails:** Thumbnails are only requested for galleries visible in the virtual scroll viewport. When a quick filter is active, `onVisibleRangeChanged` operates on the filtered list, so hidden galleries never trigger thumbnail downloads. `thumbnail-ready` events update gallery `thumb_path` in the store.

## ImageDownloadQueue (download/mod.rs)
- **Signature:** `struct ImageDownloadQueue { sender: mpsc::Sender<DownloadRequest> }`
- **Used by:** `commands::get_gallery_image`, Tauri managed state
- **Notes:** Background tokio task processes download requests. Created in `lib.rs` `.setup()`.

### Concurrency & spacing
- Max 3 concurrent downloads via `tokio::sync::Semaphore`
- 300ms delay between starting downloads via `tokio::time::interval`
- 15s timeout per image download pipeline (fetch page + download image)
- Oneshot channel send failures are logged (caller may have cancelled)

### Download cancellation
- Each `DownloadRequest` carries an optional `cancel_flag: Arc<AtomicBool>`.
- `DownloadCancellation` (managed state) tracks flags per gallery gid.
- Queue checks the flag before waiting, before acquiring semaphore, and skips cancelled requests.
- `cancel_image_downloads` IPC sets the flag; `register_download_session` creates a new flag (cancelling old).
- Reader calls `register_download_session` on open, `cancel_image_downloads` on close/navigation.

### Download pipeline (per image)
1. Check `OriginalsCache::find_cached` — return immediately if hit
2. If `ShowPageParams { imgkey, showkey }` provided:
   a. Try `api_show_page(gid, page+1, imgkey, showkey)` → `ImagePageResult` (fast path, ~1 request)
   b. On 509 → trigger rate limit backoff
   c. On other error → fallback to step 3
3. Fallback: `fetch_image_url(page_url)` → `ImagePageResult { image_url, nl_key }` (2 requests)
4. `download_image(image_url)` → bytes
5. On failure + `nl_key` present: `fetch_image_url_with_nl(page_url, nl_key)` → retry from alternate server
6. Save to `OriginalsCache`, update DB `image_path`
7. Emit `image-download-progress` event

### Reader preload strategy
- Current page N: urgent download
- Pages N+1..N+3: preload ahead (3 pages)
- Page N-1: preload behind (1 page)
- Frontend passes `imgkey`/`showkey` to `get_gallery_image` for showpage API fast path
- Progress bar shown in reader during loading

### 509 rate limit handling
- HTTP 509 status or 509.gif in response → `"509_RATE_LIMITED"` error
- On 509: pause all downloads with exponential backoff (5s → 10s → 20s → 30s cap)
- `QueueStatus { rate_limited, rate_limited_until, backoff_secs }` tracks pause state

### Events
- Emits `image-download-progress` Tauri event: `{ gid, page_index, status, path?, error? }`
- Status values: `"queued"`, `"downloading"`, `"done"`, `"error"`, `"rate_limited"`

## Detail page lazy batch loading
- Frontend calls `get_gallery_pages_batch(gid, token, detailPage, pagesPerBatch?)` on demand as user scrolls.
- Each call fetches one ExH detail page (p=N, 20 or 40 entries depending on site settings).
- After p=0 responds, frontend records `pagesPerBatch = result.pages.length` and passes it to all subsequent calls.
- Backend uses `pagesPerBatch` to compute correct `base_index = detailPage * pagesPerBatch` for page index assignment.
- IntersectionObserver sentinels at each `pagesPerBatch`-page boundary trigger the next fetch.
- Pages are stored in DB incrementally via upsert (not bulk replace).
- On navigation away: `setActiveDetailGallery(null)` cancels all in-flight batch fetches and thumbnail downloads.
- `get_gallery_pages` (fetch-all) is NOT used when opening the reader — reader receives stubs for unloaded pages and fetches batches on demand.

## Reader batch loading (preview strip)
- `GalleryReader` receives a `GalleryPages` with `total_pages = N` (truth from HTML parse, stored in `detailBatchState.totalPageCount`) and `pages[]` where unloaded entries are stubs (`{page_url:"", thumb_url:null, ...}`).
- `detailBatchState` store carries `{ gid, token, showkey, pagesPerBatch, totalPageCount, fetchedDetailPages: Set, pageEntries: Record }` — same references as the detail page's local state, mutated in-place.
- `totalPageCount` is set once in `fetchBatch` from `result.total_pages` (HTML parse). Never recalculated. Stored in `detailBatchState.totalPageCount` so both components agree on the count across sessions.
- Reader sets up IntersectionObserver sentinels on `data-strip-sentinel` attributes ONLY when `showControls=true` and `stripEl` is bound (strip is in the DOM). Sentinels queried via `stripEl.querySelector(...)` with `root: stripEl, rootMargin: "0px 600px 0px 600px"`.
- When a sentinel for batch dp enters view, `fetchStripBatch(gid, dp)` is called — mirrors `fetchBatch` in GalleryDetail.
- Fetched entries written into `gallery.pages[idx]` (reader's local state) and `detailBatchState.pageEntries` (shared). `fetchedDetailPages.add(dp)` marks the batch done for both sides.
- If the detail page has already fetched a batch while the reader is open, the reader skips re-fetching and calls `syncBatchStateToGallery` to merge known entries.
- On close to detail page: `detailBatchState` is preserved; detail's open effect detects same-gallery by `detailBatchState.gid === g.gid` and restores `totalPageCount`/`fetchedDetailPages`/`pageEntries` from store.
- On close to home/search (no source gallery): `setActiveDetailGallery(null)` + `detailBatchState.set(null)` cancel everything.
