use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, State};

use crate::config::ConfigState;
use crate::db::DbState;
use crate::download::ImageDownloadQueue;
use crate::http;
use crate::http::{GdataRateLimiter, RateLimiter};
use crate::images::{OriginalsCache, PageThumbCache, ThumbCache};
use crate::models::{
    AdvancedSearchOptions, ExhCookies, ExhSearchResult, FilterParams, FilterPreset, Gallery,
    GalleryPage, GalleryPages, GalleryPageEntry, GalleryPagesBatchEvent,
    GalleryPagesBatchResult, LoginResult, ReadProgress, ReadingSession, SearchHistoryEntry,
    SortParams, SyncPageResult, SyncProgress, SyncResult, TagSuggestion,
};

/// Tracks which gallery's page thumbnails are currently being fetched,
/// so we can cancel them when the detail panel closes.
pub struct PageThumbCancellation {
    pub active_gid: Mutex<Option<i64>>,
}

/// Tracks cancellation flags for image downloads per gallery.
/// Used to stop reader image downloads when navigating away.
pub struct DownloadCancellation {
    /// Maps gid -> cancellation flag. True means cancelled.
    pub flags: Mutex<HashMap<i64, Arc<AtomicBool>>>,
}

impl DownloadCancellation {
    pub fn new() -> Self {
        Self {
            flags: Mutex::new(HashMap::new()),
        }
    }

    /// Register a new download session for a gallery. Returns the cancellation flag.
    pub fn register(&self, gid: i64) -> Arc<AtomicBool> {
        let mut flags = self.flags.lock().unwrap();
        // Cancel any previous session for this gid.
        if let Some(old) = flags.get(&gid) {
            old.store(true, Ordering::Relaxed);
        }
        let flag = Arc::new(AtomicBool::new(false));
        flags.insert(gid, flag.clone());
        flag
    }

    /// Cancel all downloads for a gallery.
    pub fn cancel(&self, gid: i64) {
        let flags = self.flags.lock().unwrap();
        if let Some(flag) = flags.get(&gid) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Cancel all active downloads.
    pub fn cancel_all(&self) {
        let flags = self.flags.lock().unwrap();
        for flag in flags.values() {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

/// Stores the default (platform) cache directory path for reference.
pub struct DefaultCacheDir {
    pub path: std::path::PathBuf,
}

/// Tracks the next-page cursor for incremental sync.
/// Stores the full next-page URL from the #unext pagination link.
pub struct SyncCursor {
    pub next_url: Mutex<Option<String>>,
}

// ── Auth commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn login(
    ipb_member_id: String,
    ipb_pass_hash: String,
    igneous: String,
    config_state: State<'_, ConfigState>,
) -> Result<LoginResult, String> {
    let cookies = ExhCookies {
        ipb_member_id,
        ipb_pass_hash,
        igneous,
    };

    match http::validate_cookies(&cookies).await {
        Ok(()) => {
            {
                let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
                config.auth.set_cookies(&cookies);
            }
            config_state.save()?;
            Ok(LoginResult {
                success: true,
                message: "Logged in successfully.".into(),
            })
        }
        Err(msg) => Ok(LoginResult {
            success: false,
            message: msg,
        }),
    }
}

#[tauri::command]
pub async fn logout(config_state: State<'_, ConfigState>) -> Result<LoginResult, String> {
    {
        let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.clear();
    }
    config_state.save()?;
    Ok(LoginResult {
        success: true,
        message: "Logged out.".into(),
    })
}

#[tauri::command]
pub async fn get_auth_status(config_state: State<'_, ConfigState>) -> Result<bool, String> {
    let config = config_state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.auth.has_cookies())
}

// ── Sync commands ──────────────────────────────────────────────────────────

/// Fetch one page of gallery listings from ExHentai, parse, upsert into DB,
/// and download thumbnails. Returns how many galleries were synced.
#[tauri::command]
pub async fn sync_gallery_page(
    _page: Option<u32>,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
    thumb_cache: State<'_, ThumbCache>,
) -> Result<SyncResult, String> {
    // Get cookies from config.
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config
            .auth
            .to_cookies()
            .ok_or_else(|| "Not logged in.".to_string())?
    };

    let client = http::build_client(&cookies)?;

    // Legacy: page parameter is ignored for main listing (cursor-based).
    // First page only.
    let listing = http::fetch_gallery_listing(
        &client,
        &rate_limiter,
        None,
    )
    .await?;

    let count = listing.galleries.len();

    // Upsert each gallery into the database (browse source — won't overwrite API-enriched data).
    for gallery in &listing.galleries {
        db_state.upsert_gallery_browse(gallery)?;
    }

    // Download thumbnails sequentially with adaptive throttling.
    download_thumbs_sequential(&client, &listing.galleries, &db_state, &thumb_cache, None).await;

    Ok(SyncResult {
        galleries_synced: count,
        has_next_page: listing.next_url.is_some(),
        message: if count > 0 {
            format!("Synced {} galleries.", count)
        } else {
            "No galleries found on this page.".into()
        },
    })
}

/// Download thumbnails sequentially with adaptive throttling.
/// Processes downloads one-at-a-time from a queue with adaptive delays.
/// If `app` is provided, emits "thumbnail-ready" events as each completes.
/// Returns the number of thumbnails successfully downloaded.
async fn download_thumbs_sequential(
    client: &reqwest::Client,
    galleries: &[Gallery],
    db_state: &DbState,
    thumb_cache: &ThumbCache,
    app: Option<&AppHandle>,
) -> usize {
    use crate::models::ThumbnailReadyEvent;

    let to_download: Vec<_> = galleries
        .iter()
        .filter(|g| !g.thumb_url.is_empty() && !thumb_cache.exists_valid(g.gid))
        .collect();

    tracing::info!(
        "THUMB_BATCH: total={}, already_cached={}, to_download={}",
        galleries.len(),
        galleries.len() - to_download.len(),
        to_download.len()
    );

    if to_download.is_empty() {
        return 0;
    }

    let mut downloaded = 0usize;
    let mut rejected = 0usize;
    let mut consecutive_failures = 0u32;
    let mut base_delay_ms = 200u64;
    let mut cooldown_remaining = 0u32; // downloads remaining at elevated delay

    for gallery in &to_download {
        let gid = gallery.gid;
        let thumb_url = &gallery.thumb_url;

        // Apply inter-request delay.
        let delay = if cooldown_remaining > 0 {
            cooldown_remaining = cooldown_remaining.saturating_sub(1);
            if cooldown_remaining == 0 {
                base_delay_ms = 200;
                tracing::info!("THUMB_COOLDOWN_END: back to {}ms delay", base_delay_ms);
            }
            base_delay_ms
        } else {
            base_delay_ms
        };
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

        tracing::info!("THUMB_DOWNLOAD: gid={}, url={}, delay_ms={}", gid, thumb_url, delay);

        // Single attempt with 10s timeout.
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.get(thumb_url.as_str()).send(),
        )
        .await;

        match result {
            Err(_) => {
                // Timeout.
                tracing::warn!("THUMB_TIMEOUT: gid={}, url={}", gid, thumb_url);
                consecutive_failures += 1;

                if consecutive_failures >= 3 {
                    tracing::warn!(
                        "THUMB_BACKOFF: pausing 30s after {} consecutive timeouts",
                        consecutive_failures
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    base_delay_ms = 500;
                    cooldown_remaining = 10;
                    consecutive_failures = 0;
                } else {
                    // Elevate delay for next 10 downloads.
                    base_delay_ms = 2000;
                    cooldown_remaining = 10;
                }
            }
            Ok(Err(e)) => {
                // Network error.
                tracing::warn!("THUMB_FAIL: gid={}, error=network: {}", gid, e);
                consecutive_failures += 1;
                base_delay_ms = 2000;
                cooldown_remaining = 10;

                if consecutive_failures >= 3 {
                    tracing::warn!(
                        "THUMB_BACKOFF: pausing 30s after {} consecutive failures",
                        consecutive_failures
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    base_delay_ms = 500;
                    cooldown_remaining = 10;
                    consecutive_failures = 0;
                }
            }
            Ok(Ok(response)) => {
                let status = response.status();
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();

                if !status.is_success() {
                    let body_preview = response
                        .text()
                        .await
                        .map(|t| t.chars().take(200).collect::<String>())
                        .unwrap_or_else(|_| "<unreadable>".to_string());
                    tracing::warn!(
                        "THUMB_FAIL: gid={}, status={}, content_type={}, body_preview={}",
                        gid, status, content_type, body_preview
                    );

                    consecutive_failures += 1;
                    if status.as_u16() == 429 || status.as_u16() == 503 || status.as_u16() == 509 {
                        tracing::warn!("THUMB_RATE_LIMITED: gid={}, status={}", gid, status);
                        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                        base_delay_ms = 2000;
                        cooldown_remaining = 10;
                    }

                    if consecutive_failures >= 3 {
                        tracing::warn!(
                            "THUMB_BACKOFF: pausing 30s after {} consecutive failures",
                            consecutive_failures
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        base_delay_ms = 500;
                        cooldown_remaining = 10;
                        consecutive_failures = 0;
                    }
                    continue;
                }

                // Validate content-type.
                if !content_type.is_empty() && !content_type.starts_with("image/") {
                    let body_preview = response
                        .text()
                        .await
                        .map(|t| t.chars().take(200).collect::<String>())
                        .unwrap_or_else(|_| "<unreadable>".to_string());
                    tracing::warn!(
                        "THUMB_WRONG_TYPE: gid={}, content_type={}, body_preview={}",
                        gid, content_type, body_preview
                    );
                    consecutive_failures += 1;
                    continue;
                }

                match response.bytes().await {
                    Ok(bytes) => {
                        tracing::info!("THUMB_SUCCESS: gid={}, size={}", gid, bytes.len());
                        consecutive_failures = 0;

                        match thumb_cache.save(gid, &bytes) {
                            Ok(path) => {
                                let _ = db_state.set_thumb_path(gid, &path);
                                if let Some(app) = app {
                                    let _ = app.emit(
                                        "thumbnail-ready",
                                        ThumbnailReadyEvent {
                                            gid,
                                            path,
                                        },
                                    );
                                }
                                downloaded += 1;
                            }
                            Err(e) => {
                                tracing::warn!("THUMB_SAVE_REJECTED: gid={}, error={}", gid, e);
                                rejected += 1;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("THUMB_FAIL: gid={}, error=read_bytes: {}", gid, e);
                        consecutive_failures += 1;
                    }
                }
            }
        }
    }
    tracing::info!("THUMB_BATCH_DONE: downloaded={}, rejected={}", downloaded, rejected);
    downloaded
}

/// Multi-page sync: fetches `depth` pages of gallery listings sequentially
/// (rate-limited), then downloads all thumbnails in parallel batches.
/// Emits "sync-progress" events to the frontend.
#[tauri::command]
pub async fn sync_galleries(
    depth: Option<u32>,
    app: AppHandle,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
    thumb_cache: State<'_, ThumbCache>,
) -> Result<SyncResult, String> {
    let max_pages = depth.unwrap_or(10).min(50);

    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config
            .auth
            .to_cookies()
            .ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    let mut all_galleries: Vec<Gallery> = Vec::new();
    let mut next_url: Option<String> = None;
    let mut pages_fetched: u32 = 0;

    // Phase 1: Fetch listing pages sequentially (rate-limited).
    for page_num in 1..=max_pages {
        let _ = app.emit("sync-progress", SyncProgress {
            current_page: page_num,
            total_pages: max_pages,
            thumbs_downloaded: 0,
            thumbs_total: 0,
            galleries_synced: all_galleries.len(),
            message: format!("Fetching page {}/{}...", page_num, max_pages),
            done: false,
        });

        let listing = http::fetch_gallery_listing(
            &client,
            &rate_limiter,
            next_url.as_deref(),
        )
        .await?;

        // Upsert each gallery into the database (browse source — won't overwrite API-enriched data).
        for gallery in &listing.galleries {
            db_state.upsert_gallery_browse(gallery)?;
        }

        let has_next = listing.next_url.is_some();
        next_url = listing.next_url;
        all_galleries.extend(listing.galleries);
        pages_fetched = page_num;

        if !has_next {
            break;
        }
    }

    // Phase 2: Download thumbnails in parallel (no rate limit needed for CDN).
    let thumbs_needed: Vec<_> = all_galleries
        .iter()
        .filter(|g| !g.thumb_url.is_empty() && !thumb_cache.exists(g.gid))
        .cloned()
        .collect();
    let thumbs_total = thumbs_needed.len();

    let _ = app.emit("sync-progress", SyncProgress {
        current_page: pages_fetched,
        total_pages: pages_fetched,
        thumbs_downloaded: 0,
        thumbs_total,
        galleries_synced: all_galleries.len(),
        message: format!("Downloading {} thumbnails...", thumbs_total),
        done: false,
    });

    // Download thumbnails sequentially with adaptive throttling.
    let thumbs_downloaded = download_thumbs_sequential(
        &client,
        &thumbs_needed,
        &db_state,
        &thumb_cache,
        None,
    )
    .await;

    let total_synced = all_galleries.len();
    let message = format!(
        "Synced {} galleries from {} pages. Downloaded {} thumbnails.",
        total_synced, pages_fetched, thumbs_downloaded
    );

    let _ = app.emit("sync-progress", SyncProgress {
        current_page: pages_fetched,
        total_pages: pages_fetched,
        thumbs_downloaded,
        thumbs_total,
        galleries_synced: total_synced,
        message: message.clone(),
        done: true,
    });

    Ok(SyncResult {
        galleries_synced: total_synced,
        has_next_page: next_url.is_some(),
        message,
    })
}

/// Fetch one page from ExHentai, upsert to DB, return galleries directly.
/// Thumbnails are NOT downloaded here — the frontend calls
/// `download_thumbnails_for_gids` for visible galleries only.
/// Page cursor advances automatically.
#[tauri::command]
pub async fn sync_next_page(
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
    sync_cursor: State<'_, SyncCursor>,
) -> Result<SyncPageResult, String> {
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    let cursor_url = {
        let cursor = sync_cursor.next_url.lock().map_err(|e| e.to_string())?;
        cursor.clone()
    };

    tracing::info!("[sync_next_page] fetching cursor={:?}", cursor_url);

    let listing = http::fetch_gallery_listing(&client, &rate_limiter, cursor_url.as_deref()).await?;

    let has_more = listing.next_url.is_some();
    tracing::info!("[sync_next_page] got {} galleries, has_more={}, next_url={:?}", listing.galleries.len(), has_more, &listing.next_url);

    // Destructure to avoid partial-move issues.
    let next_url = listing.next_url;
    let galleries = listing.galleries;

    {
        let mut cursor = sync_cursor.next_url.lock().map_err(|e| e.to_string())?;
        *cursor = next_url;
    }

    // Upsert each gallery (browse source — won't overwrite API-enriched data),
    // then re-read from DB to get consistent state (thumb_path if already cached).
    let mut result_galleries = Vec::with_capacity(galleries.len());
    for gallery in &galleries {
        db_state.upsert_gallery_browse(gallery)?;
        let db_galleries = db_state.get_galleries_by_gids(&[gallery.gid])?;
        if let Some(g) = db_galleries.into_iter().next() {
            result_galleries.push(g);
        }
    }
    tracing::info!("[sync_next_page] returning {} galleries (thumbnails are demand-driven)", result_galleries.len());

    // Thumbnails are NOT downloaded automatically here.
    // The frontend calls `download_thumbnails_for_gids` for visible galleries only.

    Ok(SyncPageResult { galleries: result_galleries, has_more })
}

/// Download thumbnails for specific gallery gids (demand-driven by frontend viewport).
/// Only downloads thumbnails that are not already cached on disk.
/// Processes sequentially with adaptive throttling to avoid CDN bans.
/// Emits `thumbnail-ready` events as each completes.
#[tauri::command]
pub async fn download_thumbnails_for_gids(
    gids: Vec<i64>,
    app: AppHandle,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    thumb_cache: State<'_, ThumbCache>,
) -> Result<usize, String> {
    if gids.is_empty() {
        return Ok(0);
    }

    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    // Look up galleries from DB to get their thumb_urls.
    let galleries = db_state.get_galleries_by_gids(&gids)?;

    tracing::info!(
        "[download_thumbnails_for_gids] requested={}, found_in_db={}",
        gids.len(),
        galleries.len()
    );

    let bg_galleries = galleries;
    let bg_db = Arc::clone(&db_state);
    let bg_thumb = thumb_cache.inner().clone();
    let bg_app = app.clone();

    // Spawn in background so IPC returns immediately.
    tokio::spawn(async move {
        download_thumbs_sequential(&client, &bg_galleries, &bg_db, &bg_thumb, Some(&bg_app)).await;
    });

    Ok(0)
}

/// Reset the sync cursor so the next sync_next_page starts from the beginning.
#[tauri::command]
pub async fn reset_sync_cursor(
    sync_cursor: State<'_, SyncCursor>,
) -> Result<(), String> {
    let mut cursor = sync_cursor.next_url.lock().map_err(|e| e.to_string())?;
    tracing::info!("[reset_sync_cursor] cursor was {:?}, resetting to None", *cursor);
    *cursor = None;
    Ok(())
}

// ── Browse commands ────────────────────────────────────────────────────────

/// Get a page of galleries from the local database.
#[tauri::command]
pub async fn get_galleries(
    offset: i64,
    limit: i64,
    db_state: State<'_, Arc<DbState>>,
) -> Result<GalleryPage, String> {
    db_state.get_galleries(offset, limit)
}

/// Get galleries by a list of gids, preserving input order.
#[tauri::command]
pub async fn get_galleries_by_gids(
    gids: Vec<i64>,
    db_state: State<'_, Arc<DbState>>,
) -> Result<Vec<Gallery>, String> {
    db_state.get_galleries_by_gids(&gids)
}

/// Search galleries with filters, sorting, and pagination.
#[tauri::command]
pub async fn search_galleries(
    filter: FilterParams,
    sort: SortParams,
    offset: i64,
    limit: i64,
    db_state: State<'_, Arc<DbState>>,
) -> Result<GalleryPage, String> {
    db_state.search_galleries(&filter, &sort, offset, limit)
}

// ── Preset commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn save_preset(
    name: String,
    filter: FilterParams,
    sort: SortParams,
    db_state: State<'_, Arc<DbState>>,
) -> Result<FilterPreset, String> {
    db_state.save_preset(&name, &filter, &sort)
}

#[tauri::command]
pub async fn get_presets(db_state: State<'_, Arc<DbState>>) -> Result<Vec<FilterPreset>, String> {
    db_state.get_presets()
}

#[tauri::command]
pub async fn delete_preset(id: i64, db_state: State<'_, Arc<DbState>>) -> Result<(), String> {
    db_state.delete_preset(id)
}

// ── Detail panel commands ─────────────────────────────────────────────────

/// Fetch extended gallery metadata via the ExHentai JSON API (fast, no HTML).
/// Updates the DB with the enriched data. Returns the updated Gallery.
#[tauri::command]
pub async fn fetch_gallery_metadata(
    gid: i64,
    token: String,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
) -> Result<Gallery, String> {
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    let galleries = http::api::api_gallery_metadata(&client, &[(gid, token)]).await?;
    let gallery = galleries.into_iter().next().ok_or_else(|| "Gallery not found or expunged".to_string())?;

    // Upsert enriched data to DB (preserves thumb_path).
    db_state.upsert_gallery(&gallery)?;

    // Re-read from DB to get consistent state with thumb_path.
    let db_galleries = db_state.get_galleries_by_gids(&[gid])?;
    db_galleries.into_iter().next().ok_or_else(|| "Gallery not found in DB after upsert".to_string())
}

/// Download a single page thumbnail. Returns local file path.
/// Downloads via Rust HTTP client (with cookies for ExHentai CDN).
/// Caches in page-thumbs/{gid}/{page}.jpg.
///
/// `thumb_url` is either:
/// - A plain URL for individual thumbnails (gdtl mode)
/// - `sprite:{url}:{offsetX}:{offsetY}:{width}:{height}` for sprite sheets (gdtm mode)
#[tauri::command]
pub async fn get_page_thumbnail(
    gid: i64,
    page_index: i32,
    thumb_url: String,
    config_state: State<'_, ConfigState>,
    page_thumb_cache: State<'_, PageThumbCache>,
    cancellation: State<'_, PageThumbCancellation>,
) -> Result<String, String> {
    // Check if this request has been cancelled.
    {
        let active = cancellation.active_gid.lock().map_err(|e| e.to_string())?;
        if *active != Some(gid) {
            return Err("Cancelled".into());
        }
    }

    // Check cache first.
    if let Some(path) = page_thumb_cache.find_cached(gid, page_index) {
        return Ok(path);
    }

    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    // Parse sprite info if present.
    let sprite_info = parse_sprite_thumb_url(&thumb_url);
    let download_url = sprite_info.as_ref().map_or(&thumb_url, |s| &s.url);

    // Download with 10s timeout — page thumbnails are from CDN, should be fast.
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.get(download_url).send(),
    )
    .await
    .map_err(|_| "Page thumbnail download timed out".to_string())?
    .map_err(|e| format!("Failed to download page thumbnail: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Page thumbnail returned status {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read page thumbnail bytes: {}", e))?;

    // Check cancellation again before saving.
    {
        let active = cancellation.active_gid.lock().map_err(|e| e.to_string())?;
        if *active != Some(gid) {
            return Err("Cancelled".into());
        }
    }

    // If this is a sprite sheet, crop the individual thumbnail.
    if let Some(info) = sprite_info {
        let cropped = crop_sprite_thumbnail(&bytes, &info)?;
        page_thumb_cache.save(gid, page_index, &cropped)
    } else {
        page_thumb_cache.save(gid, page_index, &bytes)
    }
}

/// Parsed sprite thumbnail info from `sprite:{url}:{offsetX}:{offsetY}:{width}:{height}`.
struct SpriteThumbInfo {
    url: String,
    offset_x: u32,
    offset_y: u32,
    width: u32,
    height: u32,
}

/// Parse a `sprite:...` encoded thumb URL. Returns None for plain URLs.
fn parse_sprite_thumb_url(thumb_url: &str) -> Option<SpriteThumbInfo> {
    let rest = thumb_url.strip_prefix("sprite:")?;
    // Format: {url}:{offsetX}:{offsetY}:{width}:{height}
    // URL may contain colons (https://...), so parse from the end.
    let parts: Vec<&str> = rest.rsplitn(5, ':').collect();
    if parts.len() < 5 {
        return None;
    }
    // rsplitn gives parts in reverse: height, width, offsetY, offsetX, url
    let height = parts[0].parse::<u32>().ok()?;
    let width = parts[1].parse::<u32>().ok()?;
    let offset_y = parts[2].parse::<u32>().ok()?;
    let offset_x = parts[3].parse::<u32>().ok()?;
    let url = parts[4].to_string();
    Some(SpriteThumbInfo { url, offset_x, offset_y, width, height })
}

/// Crop an individual thumbnail from a sprite sheet image.
fn crop_sprite_thumbnail(sprite_bytes: &[u8], info: &SpriteThumbInfo) -> Result<Vec<u8>, String> {
    use image::GenericImageView;

    let img = image::load_from_memory(sprite_bytes)
        .map_err(|e| format!("Failed to decode sprite image: {}", e))?;

    let (img_w, img_h) = img.dimensions();

    // Clamp crop region to image bounds.
    let x = info.offset_x.min(img_w.saturating_sub(1));
    let y = info.offset_y.min(img_h.saturating_sub(1));
    let w = info.width.min(img_w.saturating_sub(x));
    let h = info.height.min(img_h.saturating_sub(y));

    if w == 0 || h == 0 {
        return Err("Sprite crop region is empty".into());
    }

    let cropped = img.crop_imm(x, y, w, h);

    let mut buf = std::io::Cursor::new(Vec::new());
    cropped
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode cropped thumbnail: {}", e))?;
    Ok(buf.into_inner())
}

/// Set the active gallery for page thumbnail downloads.
/// Cancels any pending downloads for a different gallery.
#[tauri::command]
pub async fn set_active_detail_gallery(
    gid: Option<i64>,
    cancellation: State<'_, PageThumbCancellation>,
) -> Result<(), String> {
    let mut active = cancellation.active_gid.lock().map_err(|e| e.to_string())?;
    let prev = *active;
    *active = gid;
    tracing::info!("[set_active_detail_gallery] {:?} -> {:?}", prev, gid);
    Ok(())
}

// ── Reader commands ───────────────────────────────────────────────────────

/// Get the page list for a gallery. Fetches from ExHentai if not cached.
/// Pass `force_refresh: true` to bypass the DB cache and re-fetch from ExHentai.
/// Emits `gallery-pages-batch` events as each detail page is fetched, so the
/// frontend can start loading thumbnails before all pages are ready.
#[tauri::command]
pub async fn get_gallery_pages(
    gid: i64,
    token: String,
    force_refresh: Option<bool>,
    app: AppHandle,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
    page_thumb_cache: State<'_, PageThumbCache>,
) -> Result<GalleryPages, String> {
    // On force refresh, clear the page thumbnail cache for this gallery.
    if force_refresh.unwrap_or(false) {
        let _ = page_thumb_cache.clear_gallery(gid);
        tracing::info!("[get_gallery_pages] Force refresh: cleared page thumb cache for gid={}", gid);
    }

    // Check if we already have pages cached in DB (unless force refresh requested).
    if !force_refresh.unwrap_or(false) && db_state.has_gallery_pages(gid)? {
        let pages = db_state.get_gallery_pages(gid)?;
        tracing::info!(
            "[get_gallery_pages] GALLERY_PAGES_PARSED: gid={} total_pages={} source=db_cache",
            gid, pages.len()
        );
        let (gallery_title, showkey) = {
            let conn = db_state.conn.lock().map_err(|e| e.to_string())?;
            let title = conn.query_row(
                "SELECT title FROM galleries WHERE gid = ?1",
                rusqlite::params![gid],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_default();
            let sk: Option<String> = conn.query_row(
                "SELECT showkey FROM galleries WHERE gid = ?1",
                rusqlite::params![gid],
                |row| row.get(0),
            )
            .unwrap_or(None);
            (title, sk)
        };
        return Ok(GalleryPages {
            gid,
            token: token.clone(),
            title: gallery_title,
            total_pages: pages.len() as i32,
            pages,
            showkey,
        });
    }

    // Fetch from ExHentai.
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    let mut all_page_urls: Vec<String> = Vec::new();
    let mut all_thumb_urls: Vec<String> = Vec::new();
    let mut all_imgkeys: Vec<String> = Vec::new();
    let mut detail_page: u32 = 0;
    let mut total_pages: i32 = 0;
    let mut showkey: Option<String> = None;

    loop {
        let result = http::fetch_gallery_detail(
            &client,
            &rate_limiter,
            gid,
            &token,
            Some(detail_page),
        )
        .await?;

        // DEBUG: Log what we parsed from each detail page.
        let prev_count = all_page_urls.len();
        tracing::info!(
            "[get_gallery_pages] gid={} detail_page=p{}: parsed {} page_urls, {} thumb_urls, total_pages={}, next={:?}",
            gid, detail_page, result.page_urls.len(), result.thumb_urls.len(),
            result.total_pages, result.next_detail_page
        );
        for (i, (url, thumb)) in result.page_urls.iter().zip(result.thumb_urls.iter()).enumerate() {
            tracing::debug!(
                "[get_gallery_pages] gid={} page_index={} page_url={} thumb_url={}",
                gid, prev_count + i, url, thumb
            );
        }

        // Build batch page entries and emit event so frontend can start loading
        // thumbnails immediately without waiting for all detail pages.
        let batch_pages: Vec<GalleryPageEntry> = result.page_urls.iter().enumerate().map(|(i, url)| {
            let page_index = (prev_count + i) as i32;
            let thumb = result.thumb_urls.get(i).filter(|t| !t.is_empty()).cloned();
            let imgkey = result.imgkeys.get(i).filter(|k| !k.is_empty()).cloned();
            GalleryPageEntry {
                page_index,
                page_url: url.clone(),
                image_path: None,
                thumb_url: thumb,
                imgkey,
            }
        }).collect();

        if result.total_pages > total_pages {
            total_pages = result.total_pages;
        }
        if showkey.is_none() {
            showkey = result.showkey;
        }

        let _ = app.emit("gallery-pages-batch", GalleryPagesBatchEvent {
            gid,
            pages: batch_pages,
            showkey: if detail_page == 0 { showkey.clone() } else { None },
            total_pages,
        });

        all_page_urls.extend(result.page_urls);
        all_thumb_urls.extend(result.thumb_urls);
        all_imgkeys.extend(result.imgkeys);

        if result.next_detail_page.is_some() {
            detail_page += 1;
        } else {
            break;
        }
    }

    // DEBUG: Summary — count unique thumbnail URLs.
    {
        let unique_thumbs: std::collections::HashSet<&str> =
            all_thumb_urls.iter().map(|s| s.as_str()).collect();
        tracing::info!(
            "[get_gallery_pages] GALLERY_PAGES_PARSED: gid={} total_pages={} unique_thumb_urls={} detail_pages_fetched={} source=fetch",
            gid, all_page_urls.len(), unique_thumbs.len(), detail_page + 1
        );
    }

    // Store showkey in DB.
    if let Some(ref sk) = showkey {
        let _ = db_state.set_gallery_showkey(gid, sk);
    }

    // Store in DB with thumbnail URLs and imgkeys.
    let pages_data: Vec<(i32, String, Option<String>, Option<String>)> = all_page_urls
        .iter()
        .enumerate()
        .map(|(i, url)| {
            let thumb = all_thumb_urls
                .get(i)
                .filter(|t| !t.is_empty())
                .cloned();
            let imgkey = all_imgkeys
                .get(i)
                .filter(|k| !k.is_empty())
                .cloned();
            (i as i32, url.clone(), thumb, imgkey)
        })
        .collect();
    db_state.set_gallery_pages(gid, &pages_data)?;

    let pages = db_state.get_gallery_pages(gid)?;
    let gallery_title = {
        let conn = db_state.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT title FROM galleries WHERE gid = ?1",
            rusqlite::params![gid],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
    };

    Ok(GalleryPages {
        gid,
        token,
        title: gallery_title,
        total_pages: pages.len() as i32,
        pages,
        showkey,
    })
}

/// Fetch a single detail page (p=N) from ExHentai for a gallery.
/// Returns only the entries on that page, plus total_pages and has_next_page.
/// The frontend calls this on-demand as the user scrolls, instead of fetching all pages eagerly.
/// Also stores fetched pages in DB for caching.
///
/// `pages_per_batch`: number of pages ExHentai shows per detail page (20 or 40 depending on
/// the user's site settings). Frontend passes this after learning it from p=0. Defaults to 20.
#[tauri::command]
pub async fn get_gallery_pages_batch(
    gid: i64,
    token: String,
    detail_page: u32,
    pages_per_batch: Option<u32>,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
    cancellation: State<'_, PageThumbCancellation>,
) -> Result<GalleryPagesBatchResult, String> {
    let batch_size = pages_per_batch.unwrap_or(20) as usize;
    // Check cancellation — if we've navigated away, don't bother fetching.
    {
        let active = cancellation.active_gid.lock().map_err(|e| e.to_string())?;
        if *active != Some(gid) {
            return Err("Cancelled".into());
        }
    }

    // Check if we already have pages cached in DB for this range.
    if db_state.has_gallery_pages(gid)? {
        let all_pages = db_state.get_gallery_pages(gid)?;
        // Use the gallery's file_count as the authoritative total — all_pages.len() would
        // return only the number of incrementally-cached pages, not the real total.
        let file_count: i32 = {
            let conn = db_state.conn.lock().map_err(|e| e.to_string())?;
            conn.query_row(
                "SELECT file_count FROM galleries WHERE gid = ?1",
                rusqlite::params![gid],
                |row| row.get(0),
            ).unwrap_or(all_pages.len() as i32)
        };
        let total = if file_count > 0 { file_count } else { all_pages.len() as i32 };
        let start = (detail_page as usize) * batch_size;
        if start < all_pages.len() {
            let end = (start + batch_size).min(all_pages.len());
            let batch: Vec<GalleryPageEntry> = all_pages[start..end].to_vec();
            let has_next = end < all_pages.len();
            let showkey = if detail_page == 0 {
                let conn = db_state.conn.lock().map_err(|e| e.to_string())?;
                conn.query_row(
                    "SELECT showkey FROM galleries WHERE gid = ?1",
                    rusqlite::params![gid],
                    |row| row.get(0),
                ).unwrap_or(None)
            } else {
                None
            };
            return Ok(GalleryPagesBatchResult {
                gid,
                pages: batch,
                showkey,
                total_pages: total,
                has_next_page: has_next,
                detail_page,
            });
        }
    }

    // Not cached — fetch this single detail page from ExHentai.
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    tracing::info!("[get_gallery_pages_batch] gid={} fetching detail_page=p{}", gid, detail_page);

    let result = http::fetch_gallery_detail(
        &client,
        &rate_limiter,
        gid,
        &token,
        Some(detail_page),
    )
    .await?;

    // Check cancellation again after the network request.
    {
        let active = cancellation.active_gid.lock().map_err(|e| e.to_string())?;
        if *active != Some(gid) {
            return Err("Cancelled".into());
        }
    }

    // The start index for this batch.
    // Use the actual count parsed from p=0 (passed as pages_per_batch) to compute the offset.
    // This is necessary because ExHentai shows 20 or 40 thumbnails per detail page depending
    // on the user's site settings. If the caller doesn't know yet (p=0), we use the DB count
    // of already-stored pages (which is 0 for p=0, so base_index=0 is correct).
    let base_index = if detail_page == 0 {
        0
    } else if pages_per_batch.is_some() {
        (detail_page as usize) * batch_size
    } else {
        // Fallback: count pages already in DB to determine where this batch starts.
        let db_count = db_state.get_gallery_pages(gid).unwrap_or_default().len();
        if db_count > 0 { db_count } else { (detail_page as usize) * batch_size }
    };
    tracing::info!(
        "[get_gallery_pages_batch] GALLERY_DETAIL_PAGES_FETCHED: gid={} detail_page=p{} base_index={} pages_per_batch={:?}",
        gid, detail_page, base_index, pages_per_batch
    );

    let batch_pages: Vec<GalleryPageEntry> = result.page_urls.iter().enumerate().map(|(i, url)| {
        let page_index = (base_index + i) as i32;
        let thumb = result.thumb_urls.get(i).filter(|t| !t.is_empty()).cloned();
        let imgkey = result.imgkeys.get(i).filter(|k| !k.is_empty()).cloned();
        GalleryPageEntry {
            page_index,
            page_url: url.clone(),
            image_path: None,
            thumb_url: thumb,
            imgkey,
        }
    }).collect();

    let showkey = result.showkey;
    let has_next = result.next_detail_page.is_some();
    let total_pages = result.total_pages;

    // Store showkey in DB on first page.
    if detail_page == 0 {
        if let Some(ref sk) = showkey {
            let _ = db_state.set_gallery_showkey(gid, sk);
        }
    }

    // Persist these pages to DB incrementally.
    let pages_data: Vec<(i32, String, Option<String>, Option<String>)> = batch_pages.iter().map(|p| {
        (p.page_index, p.page_url.clone(), p.thumb_url.clone(), p.imgkey.clone())
    }).collect();
    // Use upsert so we can store pages incrementally without needing all at once.
    db_state.upsert_gallery_pages(gid, &pages_data)?;

    tracing::info!(
        "[get_gallery_pages_batch] GALLERY_DETAIL_PAGES_FETCHED: gid={} detail_page=p{} pages_parsed={} total_pages={} has_next={} base_index={}",
        gid, detail_page, batch_pages.len(), total_pages, has_next, base_index
    );

    Ok(GalleryPagesBatchResult {
        gid,
        pages: batch_pages,
        showkey,
        total_pages,
        has_next_page: has_next,
        detail_page,
    })
}

/// Get a single full-size image. Fetches from ExHentai if not cached.
/// Uses the download queue for concurrency control and rate limit handling.
/// If imgkey and showkey are provided, uses the fast showpage API first.
/// Returns the local file path.
#[tauri::command]
pub async fn get_gallery_image(
    gid: i64,
    page_index: i32,
    page_url: String,
    imgkey: Option<String>,
    showkey: Option<String>,
    originals_cache: State<'_, Arc<OriginalsCache>>,
    download_queue: State<'_, ImageDownloadQueue>,
    download_cancellation: State<'_, DownloadCancellation>,
) -> Result<String, String> {
    // Check if already cached on disk.
    if let Some(path) = originals_cache.find_cached(gid, page_index) {
        return Ok(path);
    }

    // Get or create cancellation flag for this gallery.
    let cancel_flag = {
        let flags = download_cancellation.flags.lock().map_err(|e| e.to_string())?;
        flags.get(&gid).cloned()
    };

    // Check if already cancelled before submitting.
    if let Some(ref flag) = cancel_flag {
        if flag.load(Ordering::Relaxed) {
            return Err("Download cancelled".to_string());
        }
    }

    // Build showpage params if both keys are available.
    let showpage_params = match (imgkey, showkey) {
        (Some(ik), Some(sk)) if !ik.is_empty() && !sk.is_empty() => {
            Some(crate::download::ShowPageParams {
                imgkey: ik,
                showkey: sk,
            })
        }
        _ => None,
    };

    // Submit to download queue and wait for result.
    let rx = download_queue.submit(gid, page_index, page_url, showpage_params, cancel_flag);
    rx.await.map_err(|_| "Download request cancelled".to_string())?
}

/// Cancel all in-progress and queued image downloads for a gallery.
/// Called when the reader closes or when navigating away from a gallery.
#[tauri::command]
pub async fn cancel_image_downloads(
    gid: Option<i64>,
    download_cancellation: State<'_, DownloadCancellation>,
) -> Result<(), String> {
    tracing::info!("[cancel_image_downloads] gid={:?}", gid);
    match gid {
        Some(gid) => download_cancellation.cancel(gid),
        None => download_cancellation.cancel_all(),
    }
    Ok(())
}

/// Register a new image download session for a gallery.
/// Returns immediately. Cancels any previous session for this gallery.
#[tauri::command]
pub async fn register_download_session(
    gid: i64,
    download_cancellation: State<'_, DownloadCancellation>,
) -> Result<(), String> {
    tracing::info!("[register_download_session] gid={}", gid);
    download_cancellation.register(gid);
    Ok(())
}

// ── Read progress commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn update_read_progress(
    progress: ReadProgress,
    db_state: State<'_, Arc<DbState>>,
) -> Result<(), String> {
    db_state.update_read_progress(&progress)
}

#[tauri::command]
pub async fn get_read_progress(
    gid: i64,
    db_state: State<'_, Arc<DbState>>,
) -> Result<Option<ReadProgress>, String> {
    db_state.get_read_progress(gid)
}

#[tauri::command]
pub async fn get_read_progress_batch(
    gids: Vec<i64>,
    db_state: State<'_, Arc<DbState>>,
) -> Result<Vec<ReadProgress>, String> {
    db_state.get_read_progress_batch(&gids)
}

// ── Reading session commands ──────────────────────────────────────────────

#[tauri::command]
pub async fn start_reading_session(
    gid: i64,
    opened_at: i64,
    db_state: State<'_, Arc<DbState>>,
) -> Result<i64, String> {
    db_state.start_reading_session(gid, opened_at)
}

#[tauri::command]
pub async fn end_reading_session(
    session_id: i64,
    closed_at: i64,
    pages_read: i32,
    db_state: State<'_, Arc<DbState>>,
) -> Result<(), String> {
    db_state.end_reading_session(session_id, closed_at, pages_read)
}

#[tauri::command]
pub async fn get_reading_history(
    limit: i64,
    offset: i64,
    db_state: State<'_, Arc<DbState>>,
) -> Result<Vec<ReadingSession>, String> {
    db_state.get_reading_history(limit, offset)
}

/// Convert an absolute filesystem path to a Tauri asset URL.
/// This allows the frontend to display cached thumbnails.
#[tauri::command]
pub async fn resolve_thumb_path(path: String) -> Result<String, String> {
    Ok(path)
}

// ── UI config commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn get_detail_preview_size(
    config_state: State<'_, ConfigState>,
) -> Result<u32, String> {
    let config = config_state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.ui.detail_preview_size)
}

#[tauri::command]
pub async fn set_detail_preview_size(
    size: u32,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    let clamped = size.clamp(80, 200);
    {
        let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.ui.detail_preview_size = clamped;
    }
    config_state.save()?;
    Ok(())
}

#[tauri::command]
pub async fn get_theme(
    config_state: State<'_, ConfigState>,
) -> Result<String, String> {
    let config = config_state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.ui.theme.clone())
}

#[tauri::command]
pub async fn set_theme(
    theme: String,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    let valid = match theme.as_str() {
        "light" | "dark" => theme.clone(),
        _ => "light".to_string(),
    };
    {
        let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.ui.theme = valid;
    }
    config_state.save()?;
    Ok(())
}

// ── Cache management commands ─────────────────────────────────────────

/// Get the current cache directory path.
/// Returns the custom path from config if set, otherwise the platform default.
#[tauri::command]
pub async fn get_cache_dir(
    config_state: State<'_, ConfigState>,
    default_cache_dir: State<'_, DefaultCacheDir>,
) -> Result<String, String> {
    let config = config_state.config.lock().map_err(|e| e.to_string())?;
    let path = config
        .storage
        .cache_dir
        .as_deref()
        .unwrap_or_else(|| default_cache_dir.path.to_str().unwrap_or(""));
    Ok(path.to_string())
}

/// Set a custom cache directory. Pass empty string to reset to default.
/// Does NOT move existing cache files — the app starts fresh in the new location.
#[tauri::command]
pub async fn set_cache_dir(
    path: String,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    {
        let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
        if path.is_empty() {
            config.storage.cache_dir = None;
        } else {
            // Validate the path exists or can be created.
            let p = std::path::Path::new(&path);
            if !p.exists() {
                std::fs::create_dir_all(p).map_err(|e| format!("Cannot create directory: {}", e))?;
            }
            config.storage.cache_dir = Some(path);
        }
    }
    config_state.save()?;
    Ok(())
}

/// Clear all cached image files (thumbnails, page thumbnails, originals)
/// and reset DB paths. Returns the number of bytes freed.
#[tauri::command]
pub async fn clear_image_cache(
    db_state: State<'_, Arc<DbState>>,
    thumb_cache: State<'_, ThumbCache>,
    page_thumb_cache: State<'_, PageThumbCache>,
    originals_cache: State<'_, Arc<OriginalsCache>>,
) -> Result<u64, String> {
    let mut bytes_freed: u64 = 0;

    // Helper to recursively delete a directory's contents and count bytes.
    fn remove_dir_contents(dir: &std::path::Path) -> u64 {
        let mut freed: u64 = 0;
        if dir.exists() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        freed += remove_dir_contents(&path);
                        let _ = std::fs::remove_dir(&path);
                    } else {
                        if let Ok(meta) = path.metadata() {
                            freed += meta.len();
                        }
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
        freed
    }

    // Clear each cache directory.
    bytes_freed += remove_dir_contents(thumb_cache.base_dir());
    bytes_freed += remove_dir_contents(page_thumb_cache.base_dir());
    bytes_freed += remove_dir_contents(originals_cache.base_dir());

    // Clear DB paths.
    db_state.clear_all_cache_paths()?;

    tracing::info!("Cleared image cache: freed {} bytes", bytes_freed);
    Ok(bytes_freed)
}

// ── Metadata enrichment ──────────────────────────────────────────────────

/// Event emitted when a gallery has been enriched with full metadata.
#[derive(Debug, Clone, serde::Serialize)]
struct GalleryEnrichedEvent {
    gallery: Gallery,
}

/// Start background enrichment of galleries that only have browse-level metadata.
/// Fetches full metadata via the gdata API in batches of 25.
/// Emits `gallery-enriched` events as each batch completes.
/// Returns the number of galleries enriched.
#[tauri::command]
pub async fn start_enrichment(
    app: AppHandle,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    gdata_rate_limiter: State<'_, Arc<GdataRateLimiter>>,
) -> Result<u32, String> {
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    // Get galleries needing enrichment (up to 250 at a time to keep it bounded).
    let to_enrich = db_state.get_galleries_needing_enrichment(250)?;
    if to_enrich.is_empty() {
        return Ok(0);
    }

    tracing::info!("[enrichment] starting enrichment for {} galleries", to_enrich.len());

    let mut total_enriched: u32 = 0;

    // Process in batches of 25 (gdata API limit).
    for chunk in to_enrich.chunks(25) {
        let gids_tokens: Vec<(i64, String)> = chunk.to_vec();

        // Respect the burst rate limiter.
        gdata_rate_limiter.wait().await;

        match http::api::api_gallery_metadata(&client, &gids_tokens).await {
            Ok(enriched_galleries) => {
                for gallery in &enriched_galleries {
                    // Upsert with 'api' source (overwrites browse data).
                    if let Err(e) = db_state.upsert_gallery(gallery) {
                        tracing::warn!("[enrichment] failed to upsert gid={}: {}", gallery.gid, e);
                        continue;
                    }
                    // Re-read from DB to include thumb_path.
                    if let Ok(db_galleries) = db_state.get_galleries_by_gids(&[gallery.gid]) {
                        if let Some(g) = db_galleries.into_iter().next() {
                            let _ = app.emit("gallery-enriched", GalleryEnrichedEvent { gallery: g });
                        }
                    }
                    total_enriched += 1;
                }
            }
            Err(e) => {
                tracing::warn!("[enrichment] gdata API batch failed: {}", e);
                // Don't abort — continue with remaining batches.
            }
        }
    }

    tracing::info!("[enrichment] completed: enriched {} galleries", total_enriched);
    Ok(total_enriched)
}

// ── ExHentai server-side search ───────────────────────────────────────────

/// Search ExHentai's server-side search. Fetches HTML from exhentai.org/?f_search=...
/// Parses results using the same gallery listing parser.
/// Saves results to local DB for caching/enrichment.
/// Also saves the query to search history.
#[tauri::command]
pub async fn search_exhentai(
    query: String,
    next_url: Option<String>,
    category_mask: Option<u32>,
    advanced_options: Option<AdvancedSearchOptions>,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
) -> Result<ExhSearchResult, String> {
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    let is_first_page = next_url.is_none();
    let cat_mask = category_mask.unwrap_or(0);
    let adv = advanced_options.unwrap_or_default();

    // First page: build URL from query params. Subsequent pages: use #unext URL directly.
    let url = match next_url {
        Some(ref u) => u.clone(),
        None => http::build_search_url(&query, cat_mask, &adv),
    };

    let listing = http::fetch_search_results(
        &client,
        &rate_limiter,
        &url,
    )
    .await?;

    let has_more = listing.next_url.is_some();
    let result_next_url = listing.next_url;

    // Upsert galleries to DB (browse source) for caching.
    for gallery in &listing.galleries {
        db_state.upsert_gallery_browse(gallery)?;
    }

    // Re-read from DB to get consistent state (thumb_path if cached).
    let gids: Vec<i64> = listing.galleries.iter().map(|g| g.gid).collect();
    let result_galleries = if gids.is_empty() {
        Vec::new()
    } else {
        db_state.get_galleries_by_gids(&gids)?
    };

    // Save to search history (only on first page).
    if is_first_page && !query.trim().is_empty() {
        let _ = db_state.add_search_history(&query);
    }

    Ok(ExhSearchResult {
        galleries: result_galleries,
        has_more,
        next_url: result_next_url,
    })
}

// ── Search history commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn get_search_history(
    limit: Option<i64>,
    db_state: State<'_, Arc<DbState>>,
) -> Result<Vec<SearchHistoryEntry>, String> {
    db_state.get_search_history(limit.unwrap_or(20))
}

#[tauri::command]
pub async fn clear_search_history(
    db_state: State<'_, Arc<DbState>>,
) -> Result<(), String> {
    db_state.clear_search_history()
}

/// Return tag autocomplete suggestions matching the query string.
/// Queries the local gallery_tags table (substring match on name and namespace:name).
#[tauri::command]
pub async fn search_tags_autocomplete(
    query: String,
    db_state: State<'_, Arc<DbState>>,
) -> Result<Vec<TagSuggestion>, String> {
    db_state.search_tags_autocomplete(&query, 10)
}
