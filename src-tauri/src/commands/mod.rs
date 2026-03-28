use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, State};

use crate::config::ConfigState;
use crate::db::DbState;
use crate::db::library::LibraryDbState;
use crate::download::ImageDownloadQueue;
use crate::http;
use crate::http::{GdataRateLimiter, RateLimiter};
use crate::images::{OriginalsCache, PageThumbCache, ThumbCache};
use crate::models::{
    AdvancedSearchOptions, CloudFavorite, DownloadQueueStatus, ExhCookies, ExhSearchResult,
    FavoriteFolder, FavoritesResult, FavoriteStatus, FilterParams, FilterPreset, Gallery,
    GalleryMetadataPatch, GalleryPage, GalleryPages, GalleryPageEntry, GalleryPagesBatchEvent,
    GalleryPagesBatchResult, ImportPreview, LocalPage, LoginResult, ReadCacheStats, ReadProgress,
    ReadingSession, ResolvedGallery, SearchHistoryEntry, SortParams, SubmitEntry, SubmitResult,
    SyncPageResult, SyncProgress, SyncResult, Tag, TagSuggestion,
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

/// Shared HTTP client for thumbnail downloads (gallery covers + page previews).
/// Built once on login and reused across all calls so the connection pool
/// (keepalive to s.exhentai.org / ehgt.org) is preserved between batches.
/// Wrapped in RwLock so login can atomically replace it.
pub struct ThumbClient {
    pub client: tokio::sync::RwLock<Option<reqwest::Client>>,
}

impl ThumbClient {
    pub fn new() -> Self {
        Self { client: tokio::sync::RwLock::new(None) }
    }

    pub async fn set(&self, client: reqwest::Client) {
        *self.client.write().await = Some(client);
    }

    pub async fn clear(&self) {
        *self.client.write().await = None;
    }

    /// Return a clone of the client, or build a cookieless fallback.
    pub async fn get_or_fallback(&self) -> reqwest::Client {
        if let Some(c) = self.client.read().await.as_ref() {
            return c.clone();
        }
        reqwest::Client::new()
    }
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
    thumb_client: State<'_, ThumbClient>,
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
            // Build and cache a shared thumbnail client for this session.
            if let Ok(client) = http::build_client(&cookies) {
                thumb_client.set(client).await;
            }
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

/// Open a WebviewWindow navigated to the E-Hentai login page.
/// The webview handles Cloudflare challenges natively.
/// After the user logs in, injected JS encodes document.cookie into a
/// redirect URL; on_navigation intercepts it, extracts the cookies, emits
/// a `webview-login-cookies` Tauri event to the main window, and closes
/// the login webview.
///
/// The frontend listens for `webview-login-cookies` and calls `login` with
/// the received cookie values.
#[tauri::command]
pub async fn open_login_window(app: AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    {
        tracing::info!("[open_login_window] command invoked");
        use tauri::{utils::config::WebviewUrl, webview::WebviewWindowBuilder, Emitter, Manager};
        use url::Url;

        const COOKIE_PARAM: &str = "__exh_cookies__";
        const LOGIN_URL: &str = "https://forums.e-hentai.org/index.php?act=Login&CODE=00";

        if let Some(w) = app.get_webview_window("exh-login") {
            let _ = w.close();
        }

        let app_for_event = app.clone();

        let win = WebviewWindowBuilder::new(
            &app,
            "exh-login",
            WebviewUrl::External("about:blank".parse::<Url>().expect("valid URL")),
        )
        .title("E-Hentai Login")
        .inner_size(900.0, 700.0)
        .resizable(true)
        .on_navigation(move |url: &Url| {
            tracing::info!("[open_login_window] on_navigation: {}", url);
            let query = url.query().unwrap_or("");
            if !query.contains(COOKIE_PARAM) {
                return true;
            }

            let cookie_str = url
                .query_pairs()
                .find(|(k, _)| k == COOKIE_PARAM)
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default();

            tracing::info!("[open_login_window] received cookies: {} chars", cookie_str.len());

            let mut ipb_member_id = String::new();
            let mut ipb_pass_hash = String::new();
            let mut igneous = String::new();

            for part in cookie_str.split(';') {
                let part = part.trim();
                if let Some(v) = part.strip_prefix("ipb_member_id=") {
                    ipb_member_id = v.to_string();
                } else if let Some(v) = part.strip_prefix("ipb_pass_hash=") {
                    ipb_pass_hash = v.to_string();
                } else if let Some(v) = part.strip_prefix("igneous=") {
                    if v != "mystery" && v != "deleted" {
                        igneous = v.to_string();
                    }
                }
            }

            tracing::info!(
                "[open_login_window] parsed: member_id={} pass_hash={} igneous={}",
                if ipb_member_id.is_empty() { "<empty>" } else { "<set>" },
                if ipb_pass_hash.is_empty() { "<empty>" } else { "<set>" },
                if igneous.is_empty() { "<empty>" } else { "<set>" },
            );

            let _ = app_for_event.emit("webview-login-cookies", serde_json::json!({
                "ipb_member_id": ipb_member_id,
                "ipb_pass_hash": ipb_pass_hash,
                "igneous": igneous,
            }));

            if let Some(w) = app_for_event.get_webview_window("exh-login") {
                let _ = w.close();
            }

            false
        })
        .on_page_load(|webview, payload| {
            use tauri::webview::PageLoadEvent;
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            let url = payload.url().to_string();
            tracing::info!("[open_login_window] page loaded: {}", url);
            if url.starts_with("https://forums.e-hentai.org/") && !url.contains("__exh_cookies__") {
                let js = r#"
                    (function() {
                        var c = encodeURIComponent(document.cookie);
                        if (c.indexOf('ipb_member_id') !== -1 && c.indexOf('ipb_pass_hash') !== -1) {
                            window.location.href = 'https://forums.e-hentai.org/?__exh_cookies__=' + c;
                        }
                    })();
                "#;
                let _ = webview.eval(js);
            }
        })
        .build()
        .map_err(|e| e.to_string())?;

        tracing::info!("[open_login_window] window built, navigating");
        let login_url: Url = LOGIN_URL.parse().expect("valid URL");
        win.navigate(login_url).map_err(|e| e.to_string())?;
        tracing::info!("[open_login_window] navigate called");

        Ok(())
    }
    #[cfg(not(desktop))]
    {
        // On mobile there are no secondary windows — navigate the main webview to
        // the E-Hentai login page instead. The main webview's on_page_load hook
        // (registered in lib.rs setup) handles cookie extraction and navigation back.
        use tauri::Manager;
        use url::Url;
        const LOGIN_URL: &str = "https://forums.e-hentai.org/index.php?act=Login&CODE=00";
        let webview = app.get_webview_window("main")
            .ok_or_else(|| "main window not found".to_string())?;
        let login_url: Url = LOGIN_URL.parse().expect("valid URL");
        webview.navigate(login_url).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Cancel an in-progress mobile login by navigating the main webview back to the app.
/// On desktop this is a no-op (login uses a separate window, not the main webview).
#[tauri::command]
pub fn cancel_mobile_login(app: AppHandle) -> Result<(), String> {
    #[cfg(not(desktop))]
    {
        use tauri::Manager;
        use url::Url;
        let webview = app.get_webview_window("main")
            .ok_or_else(|| "main window not found".to_string())?;
        let app_url: Url = "tauri://localhost".parse().expect("valid URL");
        webview.navigate(app_url).map_err(|e| e.to_string())?;
    }
    #[cfg(desktop)]
    let _ = app;
    Ok(())
}

#[tauri::command]
pub async fn logout(
    config_state: State<'_, ConfigState>,
    thumb_client: State<'_, ThumbClient>,
) -> Result<LoginResult, String> {
    {
        let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.clear();
    }
    config_state.save()?;
    thumb_client.clear().await;
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

    // Download thumbnails concurrently with adaptive throttling.
    download_thumbs_sequential(&client, &listing.galleries, Arc::clone(&db_state), &thumb_cache, None).await;

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

/// Download thumbnails concurrently (up to THUMB_CONCURRENCY in flight).
/// If `app` is provided, emits "thumbnail-ready" events as each completes.
/// Returns the number of thumbnails successfully downloaded.
async fn download_thumbs_sequential(
    client: &reqwest::Client,
    galleries: &[Gallery],
    db_state: Arc<DbState>,
    thumb_cache: &ThumbCache,
    app: Option<&AppHandle>,
) -> usize {
    use crate::models::ThumbnailReadyEvent;
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    const THUMB_CONCURRENCY: usize = 6;

    let to_download: Vec<(i64, String)> = galleries
        .iter()
        .filter(|g| !g.thumb_url.is_empty() && !thumb_cache.exists_valid(g.gid))
        .map(|g| (g.gid, g.thumb_url.clone()))
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

    let semaphore = Arc::new(Semaphore::new(THUMB_CONCURRENCY));
    // When set, the launch loop pauses before starting new downloads.
    let pause_flag = Arc::new(AtomicBool::new(false));
    // Shared consecutive-failure counter across all tasks.
    let consec_failures: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let downloaded: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let rejected: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let client = client.clone();
    let thumb_cache = thumb_cache.clone();
    let app = app.cloned();

    let mut set: JoinSet<()> = JoinSet::new();

    for (gid, thumb_url) in to_download {
        // Wait while the CDN is rate-limited (backoff pause).
        while pause_flag.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let thumb_cache = thumb_cache.clone();
        let db = Arc::clone(&db_state);
        let app = app.clone();
        let pause_flag = pause_flag.clone();
        let consec_failures = consec_failures.clone();
        let downloaded = downloaded.clone();
        let rejected = rejected.clone();

        set.spawn(async move {
            let _permit = permit; // released when this task ends

            tracing::info!("THUMB_DOWNLOAD: gid={}, url={}", gid, thumb_url);

            let result = tokio::time::timeout(
                std::time::Duration::from_secs(20),
                client.get(thumb_url.as_str()).send(),
            )
            .await;

            let outcome: Option<bool> = match result {
                Err(_) => {
                    tracing::warn!("THUMB_TIMEOUT: gid={}, url={}", gid, thumb_url);
                    let do_backoff = {
                        let mut f = consec_failures.lock().unwrap();
                        *f += 1;
                        if *f >= 3 { tracing::warn!("THUMB_BACKOFF: pausing 30s after {} consecutive failures", *f); *f = 0; true } else { false }
                    };
                    if do_backoff {
                        pause_flag.store(true, Ordering::Relaxed);
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        pause_flag.store(false, Ordering::Relaxed);
                    }
                    Some(false)
                }
                Ok(Err(e)) => {
                    tracing::warn!("THUMB_FAIL: gid={}, error=network: {}", gid, e);
                    let do_backoff = {
                        let mut f = consec_failures.lock().unwrap();
                        *f += 1;
                        if *f >= 3 { tracing::warn!("THUMB_BACKOFF: pausing 30s after {} consecutive failures", *f); *f = 0; true } else { false }
                    };
                    if do_backoff {
                        pause_flag.store(true, Ordering::Relaxed);
                        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                        pause_flag.store(false, Ordering::Relaxed);
                    }
                    Some(false)
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
                            "THUMB_FAIL: gid={}, status={}, body_preview={}",
                            gid, status, body_preview
                        );
                        let is_rate_limit = matches!(status.as_u16(), 429 | 503 | 509);
                        if is_rate_limit {
                            tracing::warn!("THUMB_RATE_LIMITED: gid={}, status={}", gid, status);
                        }
                        let (backoff, pause_secs) = {
                            let mut f = consec_failures.lock().unwrap();
                            *f += 1;
                            let b = *f >= 3;
                            if b { tracing::warn!("THUMB_BACKOFF: pausing 30s after {} consecutive failures", *f); *f = 0; }
                            let secs = if is_rate_limit { 10u64 } else { 30u64 };
                            (b, if is_rate_limit || b { secs } else { 0 })
                        };
                        if pause_secs > 0 {
                            let _ = backoff; // suppress unused warning
                            pause_flag.store(true, Ordering::Relaxed);
                            tokio::time::sleep(std::time::Duration::from_secs(pause_secs)).await;
                            pause_flag.store(false, Ordering::Relaxed);
                        }
                        return;
                    }

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
                        *consec_failures.lock().unwrap() += 1;
                        return;
                    }

                    match response.bytes().await {
                        Ok(bytes) => {
                            tracing::info!("THUMB_SUCCESS: gid={}, size={}", gid, bytes.len());
                            *consec_failures.lock().unwrap() = 0;
                            match thumb_cache.save(gid, &bytes) {
                                Ok(path) => {
                                    let _ = db.set_thumb_path(gid, &path);
                                    if let Some(ref app) = app {
                                        let _ = app.emit(
                                            "thumbnail-ready",
                                            ThumbnailReadyEvent { gid, path },
                                        );
                                    }
                                    Some(true)
                                }
                                Err(e) => {
                                    tracing::warn!("THUMB_SAVE_REJECTED: gid={}, error={}", gid, e);
                                    Some(false)
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("THUMB_FAIL: gid={}, error=read_bytes: {}", gid, e);
                            *consec_failures.lock().unwrap() += 1;
                            Some(false)
                        }
                    }
                }
            };

            match outcome {
                Some(true) => *downloaded.lock().unwrap() += 1,
                Some(false) => *rejected.lock().unwrap() += 1,
                None => {}
            }
        });
    }

    while set.join_next().await.is_some() {}

    let dl = *downloaded.lock().unwrap();
    let rj = *rejected.lock().unwrap();
    tracing::info!("THUMB_BATCH_DONE: downloaded={}, rejected={}", dl, rj);
    dl
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

    // Download thumbnails concurrently with adaptive throttling.
    let thumbs_downloaded = download_thumbs_sequential(
        &client,
        &thumbs_needed,
        Arc::clone(&db_state),
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
    db_state: State<'_, Arc<DbState>>,
    thumb_cache: State<'_, ThumbCache>,
    thumb_client: State<'_, ThumbClient>,
) -> Result<usize, String> {
    if gids.is_empty() {
        return Ok(0);
    }

    let client = thumb_client.get_or_fallback().await;

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
        download_thumbs_sequential(&client, &bg_galleries, bg_db, &bg_thumb, Some(&bg_app)).await;
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
    page_thumb_cache: State<'_, PageThumbCache>,
    cancellation: State<'_, PageThumbCancellation>,
    thumb_client: State<'_, ThumbClient>,
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

    let client = thumb_client.get_or_fallback().await;

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
    is_local: bool,
    db_state: State<'_, Arc<DbState>>,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<(), String> {
    if is_local {
        lib_db.update_read_progress(&progress)
    } else {
        db_state.update_read_progress(&progress)
    }
}

#[tauri::command]
pub async fn get_read_progress(
    gid: i64,
    is_local: bool,
    db_state: State<'_, Arc<DbState>>,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<Option<ReadProgress>, String> {
    if is_local {
        lib_db.get_read_progress(gid)
    } else {
        db_state.get_read_progress(gid)
    }
}

#[tauri::command]
pub async fn get_read_progress_batch(
    gids: Vec<i64>,
    is_local: bool,
    db_state: State<'_, Arc<DbState>>,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<Vec<ReadProgress>, String> {
    if is_local {
        lib_db.get_read_progress_batch(&gids)
    } else {
        db_state.get_read_progress_batch(&gids)
    }
}

// ── Reading session commands ──────────────────────────────────────────────

#[tauri::command]
pub async fn start_reading_session(
    gid: i64,
    opened_at: i64,
    is_local: bool,
    db_state: State<'_, Arc<DbState>>,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<i64, String> {
    if is_local {
        lib_db.start_reading_session(gid, opened_at)
    } else {
        db_state.start_reading_session(gid, opened_at)
    }
}

#[tauri::command]
pub async fn end_reading_session(
    session_id: i64,
    closed_at: i64,
    pages_read: i32,
    is_local: bool,
    db_state: State<'_, Arc<DbState>>,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<(), String> {
    if is_local {
        lib_db.end_reading_session(session_id, closed_at, pages_read)
    } else {
        db_state.end_reading_session(session_id, closed_at, pages_read)
    }
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

/// Get the current library directory path.
/// Returns the custom path from config if set, otherwise the platform default.
#[tauri::command]
pub async fn get_library_dir(
    config_state: State<'_, ConfigState>,
    data_local_dir: State<'_, DataLocalDir>,
) -> Result<String, String> {
    let config = config_state.config.lock().map_err(|e| e.to_string())?;
    let path = crate::library::library_dir(&config, &data_local_dir.path);
    Ok(path.to_string_lossy().to_string())
}

/// Set a custom library directory. Pass empty string to reset to default.
/// Does NOT move existing gallery files.
#[tauri::command]
pub async fn set_library_dir(
    path: String,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    {
        let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
        if path.is_empty() {
            config.storage.library_dir = None;
        } else {
            let p = std::path::Path::new(&path);
            if !p.exists() {
                std::fs::create_dir_all(p).map_err(|e| format!("Cannot create directory: {}", e))?;
            }
            config.storage.library_dir = Some(path);
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

// ── Favorites commands ────────────────────────────────────────────────────

/// Get the local favorite status for a gallery (fast, no network).
#[tauri::command]
pub async fn get_favorite_status(
    gid: i64,
    db_state: State<'_, Arc<DbState>>,
) -> Result<FavoriteStatus, String> {
    match db_state.get_cloud_favorite(gid)? {
        Some(fav) => Ok(FavoriteStatus {
            gid,
            favcat: Some(fav.favcat),
            favnote: fav.favnote,
        }),
        None => Ok(FavoriteStatus {
            gid,
            favcat: None,
            favnote: String::new(),
        }),
    }
}

/// Add or move a gallery to a favorite folder (cloud + local DB).
/// `favcat`: 0–9 folder index.
/// `favnote`: personal note (may be empty).
#[tauri::command]
pub async fn add_favorite(
    gid: i64,
    token: String,
    favcat: u8,
    favnote: String,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
) -> Result<(), String> {
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    http::submit_favorite(&client, &rate_limiter, gid, &token, Some(favcat), &favnote).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    db_state.upsert_cloud_favorite(&CloudFavorite {
        gid,
        token,
        favcat,
        favnote,
        added_at: now,
    })?;

    Ok(())
}

/// Remove a gallery from favorites (cloud + local DB).
#[tauri::command]
pub async fn remove_favorite(
    gid: i64,
    token: String,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
) -> Result<(), String> {
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    http::submit_favorite(&client, &rate_limiter, gid, &token, None, "").await?;

    db_state.remove_cloud_favorite(gid)?;

    Ok(())
}

/// Fetch the favorites page from ExHentai (cloud browse).
/// `favcat`: None = all folders; Some(0–9) = specific folder.
/// `next_url`: cursor URL from a previous result for pagination.
/// Returns galleries + folder metadata + pagination cursor.
#[tauri::command]
pub async fn fetch_favorites(
    favcat: Option<u8>,
    next_url: Option<String>,
    config_state: State<'_, ConfigState>,
    db_state: State<'_, Arc<DbState>>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
) -> Result<FavoritesResult, String> {
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    let url = match next_url {
        Some(ref u) => u.clone(),
        None => http::build_favorites_url(favcat),
    };

    let (listing, folders) = http::fetch_favorites_page(&client, &rate_limiter, &url).await?;

    let has_more = listing.next_url.is_some();
    let result_next_url = listing.next_url;

    // Upsert galleries to DB (browse source) and persist favorite status.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for gallery in &listing.galleries {
        db_state.upsert_gallery_browse(gallery)?;
        // Store as favorites with folder 0 if not already known (folder index
        // not determinable from listing HTML alone without sprite parsing).
        // Only insert if not already tracked to avoid overwriting known folder index.
        if db_state.get_cloud_favorite(gallery.gid)?.is_none() {
            db_state.upsert_cloud_favorite(&CloudFavorite {
                gid: gallery.gid,
                token: gallery.token.clone(),
                favcat: favcat.unwrap_or(0),
                favnote: String::new(),
                added_at: now,
            })?;
        }
    }

    // Persist folder metadata if we got it.
    for folder in &folders {
        db_state.upsert_favorite_folder(folder)?;
    }

    // Re-read from DB with thumb_paths.
    let gids: Vec<i64> = listing.galleries.iter().map(|g| g.gid).collect();
    let result_galleries = if gids.is_empty() {
        Vec::new()
    } else {
        db_state.get_galleries_by_gids(&gids)?
    };

    // Merge stored folders with freshly fetched ones; fall back to DB if none returned.
    let result_folders = if folders.is_empty() {
        db_state.get_favorite_folders()?
    } else {
        folders
    };

    Ok(FavoritesResult {
        galleries: result_galleries,
        folders: result_folders,
        has_more,
        next_url: result_next_url,
    })
}

/// Get cached favorite folders from the local DB.
#[tauri::command]
pub async fn get_favorite_folders(
    db_state: State<'_, Arc<DbState>>,
) -> Result<Vec<FavoriteFolder>, String> {
    db_state.get_favorite_folders()
}

// ── Read cache management ─────────────────────────────────────────────────

/// Get current read cache stats (used_bytes, max_bytes, file_count).
#[tauri::command]
pub async fn get_read_cache_stats(
    db_state: State<'_, Arc<DbState>>,
    config_state: State<'_, ConfigState>,
) -> Result<ReadCacheStats, String> {
    let (used_bytes, file_count) = db_state.read_cache_stats()?;
    let max_bytes = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        (config.storage.read_cache_max_mb.clamp(128, 4096) * 1024 * 1024) as i64
    };
    Ok(ReadCacheStats { used_bytes, max_bytes, file_count })
}

/// Set the maximum read cache size in megabytes (128–4096).
#[tauri::command]
pub async fn set_read_cache_max_mb(
    max_mb: u64,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    let clamped = max_mb.clamp(128, 4096);
    {
        let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.storage.read_cache_max_mb = clamped;
    }
    config_state.save()?;
    Ok(())
}

/// Clear the originals read cache (files + DB index + gallery_pages image_path entries).
#[tauri::command]
pub async fn clear_read_cache(
    db_state: State<'_, Arc<DbState>>,
) -> Result<i64, String> {
    let (paths, bytes_freed) = db_state.read_cache_clear_and_unlink()?;
    for path in paths {
        let _ = std::fs::remove_file(&path);
    }
    tracing::info!("clear_read_cache: freed {} bytes", bytes_freed);
    Ok(bytes_freed)
}

// ── History settings ──────────────────────────────────────────────────────

/// Get the reading history retention period in days (0 = keep forever).
#[tauri::command]
pub async fn get_history_retention_days(
    config_state: State<'_, ConfigState>,
) -> Result<i64, String> {
    let config = config_state.config.lock().map_err(|e| e.to_string())?;
    Ok(config.history.retention_days)
}

/// Set the reading history retention period in days (0 = keep forever, max 365).
#[tauri::command]
pub async fn set_history_retention_days(
    days: i64,
    config_state: State<'_, ConfigState>,
) -> Result<(), String> {
    let clamped = days.clamp(0, 365);
    {
        let mut config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.history.retention_days = clamped;
    }
    config_state.save()?;
    Ok(())
}

// ── Local gallery commands ────────────────────────────────────────────────

/// Get a page of locally-imported galleries.
#[tauri::command]
pub async fn get_local_galleries(
    offset: i64,
    limit: i64,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<GalleryPage, String> {
    lib_db.get_local_galleries(offset, limit)
}

/// Get all pages for a local gallery.
#[tauri::command]
pub async fn get_local_gallery_pages(
    gid: i64,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<Vec<LocalPage>, String> {
    lib_db.get_local_gallery_pages(gid)
}

/// Update metadata fields for a gallery. Only provided (non-null) fields are changed.
/// Also rewrites metadata.json in the gallery's local folder.
#[tauri::command]
pub async fn update_gallery_metadata(
    gid: i64,
    patch: GalleryMetadataPatch,
    lib_db: State<'_, Arc<LibraryDbState>>,
    config_state: State<'_, ConfigState>,
    data_local_dir: State<'_, DataLocalDir>,
) -> Result<(), String> {
    lib_db.update_gallery_metadata(gid, &patch)?;

    // Rewrite metadata.json if this is a local gallery.
    if let Some(gallery) = lib_db.get_gallery_by_gid(gid)? {
        let config = config_state.config.lock().map_err(|e| e.to_string())?.clone();
        let local_pages = lib_db.get_local_gallery_pages(gid)?;
        let meta = gallery_to_meta(&gallery, &local_pages);
        // Try to write to the stored local_folder if available.
        let folder = lib_db.get_local_folder(gid)?;
        let folder_path = match folder {
            Some(ref f) if !f.is_empty() => std::path::PathBuf::from(f),
            _ => crate::library::gallery_folder(&config, &data_local_dir.path, gid),
        };
        let _ = crate::library::write_metadata_json(&folder_path, &meta);
    }
    Ok(())
}

/// Reorder pages of a local gallery. new_order is a list of current page_index values in new order.
/// Also rewrites metadata.json.
#[tauri::command]
pub async fn reorder_local_pages(
    gid: i64,
    new_order: Vec<i32>,
    lib_db: State<'_, Arc<LibraryDbState>>,
    config_state: State<'_, ConfigState>,
    data_local_dir: State<'_, DataLocalDir>,
) -> Result<(), String> {
    lib_db.reorder_local_pages(gid, &new_order)?;
    rewrite_local_metadata(gid, &lib_db, &config_state, &data_local_dir).await?;
    Ok(())
}

/// Insert new image files into a local gallery after a given page index.
/// Reads image dimensions using the image crate.
/// Copies files to the gallery folder and inserts rows.
#[tauri::command]
pub async fn insert_local_pages(
    gid: i64,
    file_paths: Vec<String>,
    insert_after_index: i32,
    lib_db: State<'_, Arc<LibraryDbState>>,
    config_state: State<'_, ConfigState>,
    data_local_dir: State<'_, DataLocalDir>,
) -> Result<Vec<LocalPage>, String> {
    // Get gallery folder.
    let _gallery = lib_db.get_gallery_by_gid(gid)?
        .ok_or_else(|| format!("Gallery {} not found", gid))?;

    let config = config_state.config.lock().map_err(|e| e.to_string())?.clone();
    let folder = crate::library::gallery_folder(&config, &data_local_dir.path, gid);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;

    let mut page_data: Vec<(String, Option<String>, Option<i32>, Option<i32>)> = Vec::new();

    for src_path in &file_paths {
        let src = std::path::Path::new(src_path);
        let filename = src.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid file path: {}", src_path))?;

        let dest = folder.join(filename);

        // Copy file if it's not already in the gallery folder.
        if src != dest {
            std::fs::copy(src, &dest).map_err(|e| format!("Failed to copy {}: {}", src_path, e))?;
        }

        // Read image dimensions.
        let (width, height) = read_image_dimensions(&dest);
        page_data.push((
            dest.to_string_lossy().to_string(),
            None, // source_url
            width,
            height,
        ));
    }

    let inserted = lib_db.insert_local_pages(gid, &page_data, insert_after_index)?;

    // Update file_count.
    let new_count = lib_db.get_local_gallery_pages(gid)?.len() as i32;
    lib_db.set_file_count(gid, new_count)?;

    rewrite_local_metadata(gid, &lib_db, &config_state, &data_local_dir).await?;
    Ok(inserted)
}

/// Remove a page from a local gallery. Optionally deletes the file.
#[tauri::command]
pub async fn remove_local_page(
    gid: i64,
    page_index: i32,
    delete_file: bool,
    lib_db: State<'_, Arc<LibraryDbState>>,
    config_state: State<'_, ConfigState>,
    data_local_dir: State<'_, DataLocalDir>,
) -> Result<(), String> {
    let file_path = lib_db.remove_local_page(gid, page_index)?;

    if delete_file {
        if let Some(path) = file_path {
            let _ = std::fs::remove_file(&path);
        }
    }

    // Update file_count.
    let new_count = lib_db.get_local_gallery_pages(gid)?.len() as i32;
    lib_db.set_file_count(gid, new_count)?;

    rewrite_local_metadata(gid, &lib_db, &config_state, &data_local_dir).await?;
    Ok(())
}

/// Set the cover image for a local gallery.
/// Copies the file to gallery_folder/cover.{ext}, generates a thumbnail, updates galleries.thumb_path.
#[tauri::command]
pub async fn set_local_gallery_cover(
    gid: i64,
    file_path: String,
    lib_db: State<'_, Arc<LibraryDbState>>,
    config_state: State<'_, ConfigState>,
    data_local_dir: State<'_, DataLocalDir>,
) -> Result<String, String> {
    let _gallery = lib_db.get_gallery_by_gid(gid)?
        .ok_or_else(|| format!("Gallery {} not found", gid))?;

    let config = config_state.config.lock().map_err(|e| e.to_string())?.clone();
    let folder = crate::library::gallery_folder(&config, &data_local_dir.path, gid);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;

    let src = std::path::Path::new(&file_path);
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let dest = folder.join(format!("cover.{}", ext));

    if src != dest.as_path() {
        std::fs::copy(src, &dest).map_err(|e| format!("Failed to copy cover: {}", e))?;
    }

    let thumb_path = dest.to_string_lossy().to_string();
    lib_db.set_thumb_path(gid, &thumb_path)?;

    Ok(thumb_path)
}

// ── Import commands ───────────────────────────────────────────────────────

/// Scan a folder and return an import preview (without importing).
#[tauri::command]
pub async fn import_local_folder(
    folder_path: String,
) -> Result<ImportPreview, String> {
    let folder = std::path::Path::new(&folder_path);
    if !folder.exists() || !folder.is_dir() {
        return Err(format!("Folder does not exist: {}", folder_path));
    }

    // Check for metadata.json.
    let meta = crate::library::read_metadata_json(folder)?;

    // Scan image files.
    let images = crate::library::scan_image_files(folder)?;
    let page_count = images.len();

    let sample_filenames: Vec<String> = images.iter()
        .take(5)
        .map(|(name, _)| name.clone())
        .collect();

    let detected_title = meta.as_ref()
        .map(|m| m.title.clone())
        .unwrap_or_else(|| {
            folder.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

    let detected_gid = meta.as_ref().and_then(|m| m.gid);
    let detected_token = meta.as_ref().and_then(|m| m.token.clone());
    let metadata_found = meta.is_some();

    Ok(ImportPreview {
        detected_title,
        detected_gid,
        detected_token,
        metadata_found,
        page_count,
        sample_filenames,
    })
}

/// Confirm import of a local folder as a gallery.
/// Copies image files to the library folder, upserts gallery in DB, generates thumb, writes metadata.json.
#[tauri::command]
pub async fn confirm_import_local_folder(
    folder_path: String,
    gid: i64,
    token: String,
    title: String,
    category: String,
    lib_db: State<'_, Arc<LibraryDbState>>,
    config_state: State<'_, ConfigState>,
    data_local_dir: State<'_, DataLocalDir>,
) -> Result<Gallery, String> {
    let src_folder = std::path::Path::new(&folder_path);
    if !src_folder.exists() || !src_folder.is_dir() {
        return Err(format!("Source folder does not exist: {}", folder_path));
    }

    let config = config_state.config.lock().map_err(|e| e.to_string())?.clone();
    let dest_folder = crate::library::gallery_folder(&config, &data_local_dir.path, gid);
    std::fs::create_dir_all(&dest_folder).map_err(|e| e.to_string())?;

    // Read existing metadata.json (if any).
    let existing_meta = crate::library::read_metadata_json(src_folder)?;

    // Scan image files.
    let images = crate::library::scan_image_files(src_folder)?;
    let file_count = images.len() as i32;

    // Copy images to dest folder and collect page data.
    let mut page_data: Vec<(String, Option<String>, Option<i32>, Option<i32>)> = Vec::new();
    let mut page_metas: Vec<crate::library::LocalPageMeta> = Vec::new();

    for (i, (filename, src_path)) in images.iter().enumerate() {
        let dest_path = dest_folder.join(filename);
        if src_path != &dest_path {
            std::fs::copy(src_path, &dest_path)
                .map_err(|e| format!("Failed to copy {}: {}", filename, e))?;
        }
        let (width, height) = read_image_dimensions(&dest_path);
        let dest_str = dest_path.to_string_lossy().to_string();
        page_data.push((dest_str.clone(), None, width, height));
        page_metas.push(crate::library::LocalPageMeta {
            index: i,
            filename: filename.clone(),
            source_url: None,
            width: width.map(|w| w as u32),
            height: height.map(|h| h as u32),
        });
    }

    // Use first page file in the gallery folder as the thumbnail — fully local, no cache.
    let thumb_path = page_data.first().map(|(p, _, _, _)| p.clone());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Build gallery to upsert.
    let tags: Vec<Tag> = existing_meta.as_ref()
        .map(|m| m.tags.iter().map(|t| Tag::parse(t)).collect())
        .unwrap_or_default();

    let description = existing_meta.as_ref().and_then(|m| m.description.clone());

    let gallery = Gallery {
        gid,
        token: token.clone(),
        title: title.clone(),
        title_jpn: existing_meta.as_ref().and_then(|m| m.title_jpn.clone()),
        category: category.clone(),
        thumb_url: String::new(),
        thumb_path: thumb_path.clone(),
        uploader: existing_meta.as_ref().and_then(|m| m.uploader.clone()),
        posted: now,
        rating: 0.0,
        file_count,
        file_size: None,
        tags: tags.clone(),
        is_local: Some(1),
        description: description.clone(),
        // Preserve origin/remote_gid from existing metadata.json if present.
        origin: existing_meta.as_ref().and_then(|m| m.origin.clone()),
        remote_gid: existing_meta.as_ref().and_then(|m| m.remote_gid),
    };

    let folder_str = dest_folder.to_string_lossy().to_string();
    lib_db.upsert_local_gallery(&gallery, &folder_str, description.as_deref())?;

    // Insert pages starting at index -1 (so they begin at 0).
    // Clear any existing local pages first.
    {
        let conn = lib_db.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM local_gallery_pages WHERE gid = ?1", rusqlite::params![gid])
            .map_err(|e| e.to_string())?;
    }
    lib_db.insert_local_pages(gid, &page_data, -1)?;

    // Write metadata.json to dest folder.
    let meta = crate::library::LocalGalleryMeta {
        gid: Some(gid),
        token: Some(token),
        title: title.clone(),
        title_jpn: gallery.title_jpn.clone(),
        category,
        uploader: gallery.uploader.clone(),
        description,
        origin: existing_meta.as_ref().and_then(|m| m.origin.clone()),
        remote_gid: existing_meta.as_ref().and_then(|m| m.remote_gid),
        tags: tags.iter().map(|t| t.full_name()).collect(),
        pages: page_metas,
    };
    let _ = crate::library::write_metadata_json(&dest_folder, &meta);

    // Re-read from DB to return consistent state.
    lib_db.get_gallery_by_gid(gid)?
        .ok_or_else(|| "Gallery not found in DB after import".to_string())
}

// ── Queue download commands ───────────────────────────────────────────────

/// Parse a JSON string of download queue entries.
/// Input: JSON array of objects with "gid" (number or string) and optional "token" fields.
#[tauri::command]
pub async fn parse_download_queue_json(
    json_text: String,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<Vec<crate::models::QueueEntry>, String> {
    let values: Vec<serde_json::Value> = serde_json::from_str(&json_text)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    let mut entries = Vec::new();
    for val in values {
        let gid: i64 = match val.get("gid") {
            Some(serde_json::Value::Number(n)) => n.as_i64().ok_or("gid not i64")?,
            Some(serde_json::Value::String(s)) => s.parse::<i64>().map_err(|e| e.to_string())?,
            _ => return Err("Each entry must have a numeric gid field".into()),
        };
        let token = val.get("token").and_then(|v| v.as_str()).map(|s| s.to_string());
        let title = val.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());

        let already_local = lib_db.is_local(gid)?;

        entries.push(crate::models::QueueEntry {
            gid,
            token,
            title,
            already_local,
        });
    }
    Ok(entries)
}

/// Resolve the token for a gallery gid by fetching the canonical URL from ExHentai.
#[tauri::command]
pub async fn resolve_gallery_token(
    gid: i64,
    config_state: State<'_, ConfigState>,
    rate_limiter: State<'_, Arc<RateLimiter>>,
) -> Result<ResolvedGallery, String> {
    let cookies = {
        let config = config_state.config.lock().map_err(|e| e.to_string())?;
        config.auth.to_cookies().ok_or_else(|| "Not logged in.".to_string())?
    };
    let client = http::build_client(&cookies)?;

    // Use rate limiter before fetching.
    rate_limiter.wait().await;

    let url = format!("https://exhentai.org/g/{}/", gid);
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Ok(ResolvedGallery {
            gid,
            token: None,
            title: None,
            error: Some(format!("HTTP {}", response.status())),
        });
    }

    let html = response.text().await.map_err(|e| e.to_string())?;

    // Parse canonical link: <link rel="canonical" href="https://exhentai.org/g/{gid}/{token}/">
    let token = parse_canonical_token(&html, gid);
    let title = parse_gallery_title_from_html(&html);

    Ok(ResolvedGallery {
        gid,
        token,
        title,
        error: None,
    })
}

fn parse_canonical_token(html: &str, gid: i64) -> Option<String> {
    let prefix = format!("https://exhentai.org/g/{}/", gid);
    // Find <link rel="canonical" href="...">
    for line in html.lines() {
        if line.contains("rel=\"canonical\"") {
            if let Some(start) = line.find(&prefix) {
                let rest = &line[start + prefix.len()..];
                let token: String = rest.chars().take_while(|c| c.is_alphanumeric()).collect();
                if !token.is_empty() {
                    return Some(token);
                }
            }
        }
    }
    None
}

fn parse_gallery_title_from_html(html: &str) -> Option<String> {
    // Look for <title> tag.
    if let Some(start) = html.find("<title>") {
        let rest = &html[start + 7..];
        if let Some(end) = rest.find("</title>") {
            let raw = &rest[..end];
            // Strip " - ExHentai.org" suffix if present.
            let title = raw.trim().trim_end_matches(" - ExHentai.org").trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    None
}

/// Submit a batch of gid/token pairs to the local download queue.
#[tauri::command]
pub async fn submit_download_queue(
    entries: Vec<SubmitEntry>,
    lib_db: State<'_, Arc<LibraryDbState>>,
    local_queue: State<'_, crate::download::local_queue::LocalDownloadQueue>,
) -> Result<SubmitResult, String> {
    let mut queued = 0i64;
    let mut skipped = 0i64;

    for entry in entries {
        let already_local = lib_db.is_local(entry.gid)?;

        if already_local {
            skipped += 1;
        } else {
            local_queue.enqueue(entry.gid, entry.token, None);
            queued += 1;
        }
    }

    Ok(SubmitResult {
        queued,
        skipped_already_local: skipped,
    })
}

/// Get the current status of the local download queue.
#[tauri::command]
pub async fn get_download_queue_status(
    local_queue: State<'_, crate::download::local_queue::LocalDownloadQueue>,
) -> Result<DownloadQueueStatus, String> {
    Ok(local_queue.status())
}

/// Pause the local download queue.
#[tauri::command]
pub async fn pause_download_queue(
    local_queue: State<'_, crate::download::local_queue::LocalDownloadQueue>,
) -> Result<(), String> {
    local_queue.pause();
    Ok(())
}

/// Resume the local download queue.
#[tauri::command]
pub async fn resume_download_queue(
    local_queue: State<'_, crate::download::local_queue::LocalDownloadQueue>,
) -> Result<(), String> {
    local_queue.resume();
    Ok(())
}

/// Cancel queued downloads. If gid is None, cancel all pending.
#[tauri::command]
pub async fn cancel_download_queue(
    gid: Option<i64>,
    local_queue: State<'_, crate::download::local_queue::LocalDownloadQueue>,
) -> Result<(), String> {
    local_queue.cancel(gid);
    Ok(())
}

// ── Shared state for data_local_dir ──────────────────────────────────────

/// Stores the platform data_local_dir for use by library functions.
pub struct DataLocalDir {
    pub path: std::path::PathBuf,
}

// ── Private helpers ───────────────────────────────────────────────────────

/// Read image dimensions using the image crate. Returns (width, height) or (None, None) on failure.
fn read_image_dimensions(path: &std::path::Path) -> (Option<i32>, Option<i32>) {
    match image::image_dimensions(path) {
        Ok((w, h)) => (Some(w as i32), Some(h as i32)),
        Err(_) => (None, None),
    }
}

/// Helper to rewrite metadata.json for a local gallery.
async fn rewrite_local_metadata(
    gid: i64,
    lib_db: &Arc<LibraryDbState>,
    config_state: &State<'_, ConfigState>,
    data_local_dir: &State<'_, DataLocalDir>,
) -> Result<(), String> {
    if let Some(gallery) = lib_db.get_gallery_by_gid(gid)? {
        let config = config_state.config.lock().map_err(|e| e.to_string())?.clone();
        let local_pages = lib_db.get_local_gallery_pages(gid)?;
        let meta = gallery_to_meta(&gallery, &local_pages);

        let folder_str = lib_db.get_local_folder(gid)?;

        let folder_path = match folder_str {
            Some(ref f) if !f.is_empty() => std::path::PathBuf::from(f),
            _ => crate::library::gallery_folder(&config, &data_local_dir.path, gid),
        };

        let _ = crate::library::write_metadata_json(&folder_path, &meta);
    }
    Ok(())
}

/// Delete a local gallery from the DB and its folder from disk.
#[tauri::command]
pub async fn delete_local_gallery(
    gid: i64,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<(), String> {
    // Get the local_folder path before deleting.
    let local_folder = lib_db.get_local_folder(gid)?;

    // Delete gallery from DB (cascades to local_gallery_pages and related tables).
    lib_db.delete_gallery(gid)?;

    // Delete folder from disk if it exists.
    if let Some(folder) = local_folder {
        let path = std::path::Path::new(&folder);
        if path.exists() {
            std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Convert a Gallery + its local pages to a LocalGalleryMeta for metadata.json.
fn gallery_to_meta(gallery: &Gallery, pages: &[LocalPage]) -> crate::library::LocalGalleryMeta {
    let page_metas: Vec<crate::library::LocalPageMeta> = pages.iter().map(|p| {
        let filename = std::path::Path::new(&p.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        crate::library::LocalPageMeta {
            index: p.page_index as usize,
            filename,
            source_url: p.source_url.clone(),
            width: p.width.map(|w| w as u32),
            height: p.height.map(|h| h as u32),
        }
    }).collect();

    crate::library::LocalGalleryMeta {
        gid: Some(gallery.gid),
        token: Some(gallery.token.clone()),
        title: gallery.title.clone(),
        title_jpn: gallery.title_jpn.clone(),
        category: gallery.category.clone(),
        uploader: gallery.uploader.clone(),
        description: gallery.description.clone(),
        origin: gallery.origin.clone(),
        remote_gid: gallery.remote_gid,
        tags: gallery.tags.iter().map(|t| t.full_name()).collect(),
        pages: page_metas,
    }
}

/// Placeholder: sync a local gallery from its origin site.
/// Only works for galleries that have an origin and remote_gid set.
/// Currently a no-op — sync logic will be added per origin site in a future phase.
#[tauri::command]
pub async fn sync_local_gallery(
    gid: i64,
    lib_db: State<'_, Arc<LibraryDbState>>,
) -> Result<(), String> {
    let gallery = lib_db.get_gallery_by_gid(gid)?
        .ok_or_else(|| format!("Gallery {} not found", gid))?;

    if gallery.origin.is_none() || gallery.remote_gid.is_none() {
        return Err("Gallery has no origin — cannot sync".to_string());
    }

    // TODO: dispatch to origin-specific sync handler (Phase 5+).
    Ok(())
}
