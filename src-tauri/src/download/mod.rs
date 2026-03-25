// Image download queue with concurrency control, rate limiting, and nl retry.
// Max 3 concurrent image downloads with 500ms between starts.
// On 509 (bandwidth exceeded): pause all downloads with exponential backoff.
// On failure: retry with nl key for alternate server.
pub mod local_queue;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex, Semaphore};

use crate::db::DbState;
use crate::http::{self, RateLimiter};
use crate::images::OriginalsCache;
use crate::models::ImageDownloadProgressEvent;

/// A request to download a single image page.
pub struct DownloadRequest {
    pub gid: i64,
    pub page_index: i32,
    pub page_url: String,
    /// Optional showpage API params for fast URL resolution.
    /// If present, the queue tries the showpage API first before falling back to HTML.
    pub showpage_params: Option<ShowPageParams>,
    /// Optional cancellation flag. If set to true, the download is skipped.
    pub cancel_flag: Option<Arc<AtomicBool>>,
    pub respond: oneshot::Sender<Result<String, String>>,
}

/// Parameters needed for the showpage API shortcut.
pub struct ShowPageParams {
    pub imgkey: String,
    pub showkey: String,
}

/// Status of the download queue, shared between the queue and callers.
pub struct QueueStatus {
    /// Whether the queue is currently paused due to rate limiting.
    pub rate_limited: bool,
    /// When the rate limit pause ends.
    pub rate_limited_until: Option<Instant>,
    /// Current backoff duration for rate limiting.
    pub backoff_secs: u64,
}

/// The download queue. Holds an mpsc sender to submit requests.
pub struct ImageDownloadQueue {
    sender: mpsc::UnboundedSender<DownloadRequest>,
    #[allow(dead_code)]
    status: Arc<TokioMutex<QueueStatus>>,
}

impl ImageDownloadQueue {
    /// Create a new download queue and spawn the background worker.
    pub fn new(
        app: AppHandle,
        client: Client,
        rate_limiter: Arc<RateLimiter>,
        db_state: Arc<DbState>,
        originals_cache: Arc<OriginalsCache>,
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let status = Arc::new(TokioMutex::new(QueueStatus {
            rate_limited: false,
            rate_limited_until: None,
            backoff_secs: 5,
        }));

        let worker_status = status.clone();

        tauri::async_runtime::spawn(queue_worker(
            receiver,
            app,
            client,
            rate_limiter,
            db_state,
            originals_cache,
            worker_status,
        ));

        Self { sender, status }
    }

    /// Submit a download request. Returns a oneshot receiver for the result.
    pub fn submit(
        &self,
        gid: i64,
        page_index: i32,
        page_url: String,
        showpage_params: Option<ShowPageParams>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> oneshot::Receiver<Result<String, String>> {
        let (tx, rx) = oneshot::channel();
        let _ = self.sender.send(DownloadRequest {
            gid,
            page_index,
            page_url,
            showpage_params,
            cancel_flag,
            respond: tx,
        });
        rx
    }
}

/// Background worker that processes download requests.
async fn queue_worker(
    mut receiver: mpsc::UnboundedReceiver<DownloadRequest>,
    app: AppHandle,
    client: Client,
    rate_limiter: Arc<RateLimiter>,
    db_state: Arc<DbState>,
    originals_cache: Arc<OriginalsCache>,
    status: Arc<TokioMutex<QueueStatus>>,
) {
    let semaphore = Arc::new(Semaphore::new(3)); // Max 3 concurrent downloads.
    let mut interval = tokio::time::interval(Duration::from_millis(300)); // 300ms between starts.

    while let Some(request) = receiver.recv().await {
        // Check cancellation before processing.
        if let Some(ref flag) = request.cancel_flag {
            if flag.load(Ordering::Relaxed) {
                let _ = request.respond.send(Err("Download cancelled".into()));
                continue;
            }
        }

        // Wait for the interval (300ms spacing between download starts).
        interval.tick().await;

        // Re-check cancellation after waiting.
        if let Some(ref flag) = request.cancel_flag {
            if flag.load(Ordering::Relaxed) {
                let _ = request.respond.send(Err("Download cancelled".into()));
                continue;
            }
        }

        // Check if rate-limited; wait if so.
        loop {
            let is_limited = {
                let s = status.lock().await;
                if s.rate_limited {
                    s.rate_limited_until
                        .map(|u| Instant::now() < u)
                        .unwrap_or(false)
                } else {
                    false
                }
            };

            if is_limited {
                // Emit rate-limited event.
                let remaining = {
                    let s = status.lock().await;
                    s.rate_limited_until
                        .map(|u| u.saturating_duration_since(Instant::now()).as_secs())
                        .unwrap_or(0)
                };
                let _ = app.emit(
                    "image-download-progress",
                    ImageDownloadProgressEvent {
                        gid: request.gid,
                        page_index: request.page_index,
                        status: "rate_limited".into(),
                        path: None,
                        error: Some(format!("Rate limited — waiting {}s", remaining)),
                    },
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            } else {
                // Clear rate limit flag if time has passed.
                let mut s = status.lock().await;
                if s.rate_limited {
                    s.rate_limited = false;
                }
                break;
            }
        }

        // Emit downloading status.
        let _ = app.emit(
            "image-download-progress",
            ImageDownloadProgressEvent {
                gid: request.gid,
                page_index: request.page_index,
                status: "downloading".into(),
                path: None,
                error: None,
            },
        );

        // Acquire semaphore permit.
        let permit = semaphore.clone().acquire_owned().await.unwrap();

        let client = client.clone();
        let rate_limiter = rate_limiter.clone();
        let db_state = db_state.clone();
        let originals_cache = originals_cache.clone();
        let app_clone = app.clone();
        let status_clone = status.clone();

        tokio::spawn(async move {
            let _permit = permit;
            let result = download_single_image(
                &client,
                &rate_limiter,
                &db_state,
                &originals_cache,
                &app_clone,
                &status_clone,
                request.gid,
                request.page_index,
                &request.page_url,
                request.showpage_params.as_ref(),
            )
            .await;

            // Emit result event.
            match &result {
                Ok(path) => {
                    let _ = app_clone.emit(
                        "image-download-progress",
                        ImageDownloadProgressEvent {
                            gid: request.gid,
                            page_index: request.page_index,
                            status: "done".into(),
                            path: Some(path.clone()),
                            error: None,
                        },
                    );
                }
                Err(e) => {
                    let _ = app_clone.emit(
                        "image-download-progress",
                        ImageDownloadProgressEvent {
                            gid: request.gid,
                            page_index: request.page_index,
                            status: "error".into(),
                            path: None,
                            error: Some(e.clone()),
                        },
                    );
                }
            }

            if let Err(_) = request.respond.send(result) {
                tracing::warn!(
                    gid = request.gid,
                    page_index = request.page_index,
                    "Download result receiver dropped — caller likely cancelled"
                );
            }
        });
    }
}

/// Timeout for the entire image download pipeline (fetch page + download image).
const IMAGE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15);

/// Download a single image with nl retry on failure.
/// Wrapped with a 15s timeout to prevent hangs.
async fn download_single_image(
    client: &Client,
    rate_limiter: &RateLimiter,
    db_state: &DbState,
    originals_cache: &OriginalsCache,
    app: &AppHandle,
    status: &TokioMutex<QueueStatus>,
    gid: i64,
    page_index: i32,
    page_url: &str,
    showpage_params: Option<&ShowPageParams>,
) -> Result<String, String> {
    match tokio::time::timeout(
        IMAGE_DOWNLOAD_TIMEOUT,
        download_single_image_inner(client, rate_limiter, db_state, originals_cache, app, status, gid, page_index, page_url, showpage_params),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(format!("Download timed out after {}s", IMAGE_DOWNLOAD_TIMEOUT.as_secs())),
    }
}

async fn download_single_image_inner(
    client: &Client,
    rate_limiter: &RateLimiter,
    db_state: &DbState,
    originals_cache: &OriginalsCache,
    app: &AppHandle,
    status: &TokioMutex<QueueStatus>,
    gid: i64,
    page_index: i32,
    page_url: &str,
    showpage_params: Option<&ShowPageParams>,
) -> Result<String, String> {
    // Check cache first (might have been downloaded by another request).
    if let Some(path) = originals_cache.find_cached(gid, page_index) {
        return Ok(path);
    }

    // Step 1: Resolve image URL — try showpage API first, fall back to HTML scraping.
    let page_result = if let Some(sp) = showpage_params {
        // showpage API uses 1-indexed pages.
        match http::api::api_show_page(client, gid, page_index + 1, &sp.imgkey, &sp.showkey).await {
            Ok(r) => {
                tracing::debug!(gid, page_index, "Resolved image URL via showpage API");
                r
            }
            Err(e) if e == "509_RATE_LIMITED" => {
                trigger_rate_limit(status, app, gid, page_index).await;
                return Err("Rate limited by ExHentai".into());
            }
            Err(e) => {
                tracing::info!(gid, page_index, error = %e, "showpage API failed, falling back to HTML");
                // Fallback to HTML scraping.
                match http::fetch_image_url(client, rate_limiter, page_url).await {
                    Ok(r) => r,
                    Err(e2) if e2 == "509_RATE_LIMITED" => {
                        trigger_rate_limit(status, app, gid, page_index).await;
                        return Err("Rate limited by ExHentai".into());
                    }
                    Err(e2) => return Err(e2),
                }
            }
        }
    } else {
        // No showpage params — use HTML scraping directly.
        match http::fetch_image_url(client, rate_limiter, page_url).await {
            Ok(r) => r,
            Err(e) if e == "509_RATE_LIMITED" => {
                trigger_rate_limit(status, app, gid, page_index).await;
                return Err("Rate limited by ExHentai".into());
            }
            Err(e) => return Err(e),
        }
    };

    // Step 2: Download the image bytes.
    let bytes = match http::download_image(client, rate_limiter, &page_result.image_url).await {
        Ok(b) => b,
        Err(e) if e == "509_RATE_LIMITED" => {
            trigger_rate_limit(status, app, gid, page_index).await;
            return Err("Rate limited by ExHentai".into());
        }
        Err(e) => {
            // Try nl retry if we have a reload key.
            if let Some(ref nl_key) = page_result.nl_key {
                tracing::info!(gid, page_index, "Retrying with nl key: {}", nl_key);
                match retry_with_nl(client, rate_limiter, page_url, nl_key).await {
                    Ok(bytes) => bytes,
                    Err(retry_err) => {
                        return Err(format!(
                            "Download failed: {}. Retry failed: {}",
                            e, retry_err
                        ))
                    }
                }
            } else {
                return Err(e);
            }
        }
    };

    // Step 3: Determine file extension from URL.
    let ext = extract_extension(&page_result.image_url);

    // Step 4: Save to cache.
    let path = originals_cache.save(gid, page_index, &bytes, &ext)?;

    // Step 5: Update DB with cached path.
    let _ = db_state.set_page_image_path(gid, page_index, &path);

    Ok(path)
}

/// Retry image download using the nl key for an alternate server.
async fn retry_with_nl(
    client: &Client,
    rate_limiter: &RateLimiter,
    page_url: &str,
    nl_key: &str,
) -> Result<Vec<u8>, String> {
    let page_result =
        http::fetch_image_url_with_nl(client, rate_limiter, page_url, nl_key).await?;
    http::download_image(client, rate_limiter, &page_result.image_url).await
}

/// Trigger rate-limit pause with exponential backoff.
async fn trigger_rate_limit(
    status: &TokioMutex<QueueStatus>,
    app: &AppHandle,
    gid: i64,
    page_index: i32,
) {
    let mut s = status.lock().await;
    let backoff = s.backoff_secs;
    s.rate_limited = true;
    s.rate_limited_until = Some(Instant::now() + Duration::from_secs(backoff));
    // Exponential backoff: 5 -> 10 -> 20 -> 30 (cap).
    s.backoff_secs = (backoff * 2).min(30);

    tracing::warn!(
        gid,
        page_index,
        backoff_secs = backoff,
        "Rate limited by ExHentai, pausing downloads"
    );

    let _ = app.emit(
        "image-download-progress",
        ImageDownloadProgressEvent {
            gid,
            page_index,
            status: "rate_limited".into(),
            path: None,
            error: Some(format!("Rate limited — pausing for {}s", backoff)),
        },
    );
}

/// Extract file extension from an image URL.
fn extract_extension(url: &str) -> String {
    url.rsplit('.')
        .next()
        .and_then(|e| {
            let e = e.split('?').next().unwrap_or(e).to_lowercase();
            match e.as_str() {
                "jpg" | "jpeg" | "png" | "webp" | "gif" => Some(e),
                _ => None,
            }
        })
        .unwrap_or_else(|| "jpg".to_string())
}
