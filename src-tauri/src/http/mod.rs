pub mod api;
pub mod parser;

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::{cookie::Jar, Client, Url};
use tokio::sync::Mutex as TokioMutex;

use crate::models::{AdvancedSearchOptions, ExhCookies};

pub use parser::{GalleryDetailPage, ImagePageResult, ListingPage};

const EXH_BASE: &str = "https://exhentai.org";

/// Rate limiter: enforces a minimum delay between requests to ExHentai.
/// Also implements a circuit breaker: after 3 consecutive failures the next
/// wait() blocks for 30 seconds before allowing any further request through,
/// regardless of how fast the frontend calls. This is the fundamental protection
/// against hammering ExHentai on error loops.
pub struct RateLimiter {
    state: TokioMutex<RateLimiterState>,
    min_delay: Duration,
}

struct RateLimiterState {
    last_request: Option<Instant>,
    consecutive_failures: u32,
    /// If set, no request is allowed until this instant.
    blocked_until: Option<Instant>,
}

/// Circuit breaker: how many consecutive failures trigger a long pause.
const FAILURE_THRESHOLD: u32 = 3;
/// How long to block all requests after hitting the failure threshold.
const CIRCUIT_BREAKER_SECS: u64 = 30;

impl RateLimiter {
    pub fn new(min_delay_ms: u64) -> Self {
        Self {
            state: TokioMutex::new(RateLimiterState {
                last_request: None,
                consecutive_failures: 0,
                blocked_until: None,
            }),
            min_delay: Duration::from_millis(min_delay_ms),
        }
    }

    /// Wait until the rate limit allows the next request.
    /// Enforces the minimum inter-request delay AND any circuit-breaker block.
    pub async fn wait(&self) {
        // Read block deadline and last-request time under the lock, then sleep outside it.
        let (block_remaining, min_delay_remaining) = {
            let state = self.state.lock().await;
            let block_remaining = state.blocked_until
                .and_then(|u| u.checked_duration_since(Instant::now()));
            let min_delay_remaining = state.last_request
                .and_then(|prev| self.min_delay.checked_sub(prev.elapsed()));
            (block_remaining, min_delay_remaining)
        };

        // Sleep for circuit-breaker block first (may be longer than min_delay).
        if let Some(remaining) = block_remaining {
            tracing::warn!(
                secs = remaining.as_secs(),
                "RateLimiter circuit breaker active — blocking request"
            );
            tokio::time::sleep(remaining).await;
        } else if let Some(remaining) = min_delay_remaining {
            tokio::time::sleep(remaining).await;
        }

        // Record this request time.
        let mut state = self.state.lock().await;
        // Clear block if it has expired.
        if let Some(u) = state.blocked_until {
            if Instant::now() >= u {
                state.blocked_until = None;
                tracing::info!("RateLimiter circuit breaker cleared");
            }
        }
        state.last_request = Some(Instant::now());
    }

    /// Report that the last request succeeded. Resets the failure counter.
    pub async fn report_success(&self) {
        let mut state = self.state.lock().await;
        state.consecutive_failures = 0;
    }

    /// Report that the last request failed (any error — network, parse, bad status).
    /// After FAILURE_THRESHOLD consecutive failures, blocks all requests for
    /// CIRCUIT_BREAKER_SECS seconds.
    pub async fn report_failure(&self) {
        let mut state = self.state.lock().await;
        state.consecutive_failures += 1;
        tracing::warn!(
            consecutive_failures = state.consecutive_failures,
            threshold = FAILURE_THRESHOLD,
            "RateLimiter: request failure reported"
        );
        if state.consecutive_failures >= FAILURE_THRESHOLD {
            let until = Instant::now() + Duration::from_secs(CIRCUIT_BREAKER_SECS);
            state.blocked_until = Some(until);
            state.consecutive_failures = 0; // reset so next batch of failures restarts the count
            tracing::error!(
                secs = CIRCUIT_BREAKER_SECS,
                "RateLimiter circuit breaker tripped — blocking all ExHentai requests"
            );
        }
    }
}

/// Burst-style rate limiter for the gdata API.
/// Allows `burst_size` requests, then enforces a cooldown before the next burst.
pub struct GdataRateLimiter {
    state: TokioMutex<GdataBurstState>,
    burst_size: u32,
    cooldown: Duration,
}

struct GdataBurstState {
    requests_in_burst: u32,
    burst_started: Option<Instant>,
}

impl GdataRateLimiter {
    pub fn new(burst_size: u32, cooldown_secs: u64) -> Self {
        Self {
            state: TokioMutex::new(GdataBurstState {
                requests_in_burst: 0,
                burst_started: None,
            }),
            burst_size,
            cooldown: Duration::from_secs(cooldown_secs),
        }
    }

    /// Wait until the rate limit allows the next request.
    pub async fn wait(&self) {
        let mut state = self.state.lock().await;

        if state.requests_in_burst >= self.burst_size {
            // Burst exhausted — wait for cooldown from when the burst started.
            if let Some(started) = state.burst_started {
                let elapsed = started.elapsed();
                if elapsed < self.cooldown {
                    let wait_time = self.cooldown - elapsed;
                    // Drop lock while sleeping so we don't block other checks.
                    drop(state);
                    tokio::time::sleep(wait_time).await;
                    state = self.state.lock().await;
                }
            }
            // Reset burst.
            state.requests_in_burst = 0;
            state.burst_started = None;
        }

        if state.burst_started.is_none() {
            state.burst_started = Some(Instant::now());
        }
        state.requests_in_burst += 1;
    }
}

/// Build a reqwest client with the given ExHentai cookies baked in.
pub fn build_client(cookies: &ExhCookies) -> Result<Client, String> {
    let jar = Arc::new(Jar::default());
    let url: Url = EXH_BASE.parse().map_err(|e: url::ParseError| e.to_string())?;

    jar.add_cookie_str(
        &format!("ipb_member_id={}; Domain=exhentai.org; Path=/", cookies.ipb_member_id),
        &url,
    );
    jar.add_cookie_str(
        &format!("ipb_pass_hash={}; Domain=exhentai.org; Path=/", cookies.ipb_pass_hash),
        &url,
    );
    jar.add_cookie_str(
        &format!("igneous={}; Domain=exhentai.org; Path=/", cookies.igneous),
        &url,
    );

    #[cfg(not(target_os = "ios"))]
    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
    #[cfg(target_os = "ios")]
    let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

    Client::builder()
        .cookie_provider(jar)
        .user_agent(ua)
        .build()
        .map_err(|e| e.to_string())
}

/// Validate cookies by making a test request to exhentai.org.
pub async fn validate_cookies(cookies: &ExhCookies) -> Result<(), String> {
    let client = build_client(cookies)?;

    let response = client
        .get(EXH_BASE)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Server returned status {}", status));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if body.len() < 200 || !body.contains("<html") {
        return Err("Authentication failed — received sadpanda page. Check your cookies.".into());
    }

    Ok(())
}

/// Fetch one page of the gallery listing from ExHentai.
/// `next_url` is the full URL for the next page (from #unext href).
/// Pass None for the first page (fetches the base listing URL).
pub async fn fetch_gallery_listing(
    client: &Client,
    rate_limiter: &RateLimiter,
    next_url: Option<&str>,
) -> Result<ListingPage, String> {
    rate_limiter.wait().await;

    let url = match next_url {
        Some(u) => u.to_string(),
        None => format!("{}/", EXH_BASE),
    };

    tracing::info!("[fetch_gallery_listing] requesting URL: {}", url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| { let e = format!("Network error: {}", e); e })?;

    let status = response.status();
    if !status.is_success() {
        rate_limiter.report_failure().await;
        return Err(format!("Server returned status {}", status));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    match parser::parse_gallery_listing(&body) {
        Ok(listing) => {
            rate_limiter.report_success().await;
            tracing::info!(
                "[fetch_gallery_listing] parsed: {} galleries, next_url={:?}, first_3_gids={:?}",
                listing.galleries.len(),
                listing.next_url,
                listing.galleries.iter().take(3).map(|g| g.gid).collect::<Vec<_>>()
            );
            Ok(listing)
        }
        Err(e) => {
            rate_limiter.report_failure().await;
            Err(e)
        }
    }
}

/// Fetch one page of a gallery detail page (thumbnail grid listing image page URLs).
/// `page` is 0-based detail page number (for galleries with many images).
pub async fn fetch_gallery_detail(
    client: &Client,
    rate_limiter: &RateLimiter,
    gid: i64,
    token: &str,
    page: Option<u32>,
) -> Result<GalleryDetailPage, String> {
    rate_limiter.wait().await;

    let mut url = format!("{}/g/{}/{}/", EXH_BASE, gid, token);
    if let Some(p) = page {
        if p > 0 {
            url = format!("{}?p={}", url, p);
        }
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        rate_limiter.report_failure().await;
        return Err(format!("Server returned status {}", response.status()));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    match parser::parse_gallery_detail(&body) {
        Ok(detail) => {
            rate_limiter.report_success().await;
            Ok(detail)
        }
        Err(e) => {
            rate_limiter.report_failure().await;
            Err(e)
        }
    }
}

/// Fetch an image viewer page and extract the actual image URL + nl key.
pub async fn fetch_image_url(
    client: &Client,
    rate_limiter: &RateLimiter,
    page_url: &str,
) -> Result<ImagePageResult, String> {
    fetch_image_url_inner(client, rate_limiter, page_url, None).await
}

/// Fetch an image viewer page with an nl key for alternate server fallback.
pub async fn fetch_image_url_with_nl(
    client: &Client,
    rate_limiter: &RateLimiter,
    page_url: &str,
    nl_key: &str,
) -> Result<ImagePageResult, String> {
    fetch_image_url_inner(client, rate_limiter, page_url, Some(nl_key)).await
}

async fn fetch_image_url_inner(
    client: &Client,
    rate_limiter: &RateLimiter,
    page_url: &str,
    nl_key: Option<&str>,
) -> Result<ImagePageResult, String> {
    rate_limiter.wait().await;

    // The page_url might be relative or absolute.
    let mut url = if page_url.starts_with("http") {
        page_url.to_string()
    } else {
        format!("{}{}", EXH_BASE, page_url)
    };

    // Append nl key for alternate server.
    if let Some(nl) = nl_key {
        let separator = if url.contains('?') { "&" } else { "?" };
        url = format!("{}{}nl={}", url, separator, nl);
    }

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if status.as_u16() == 509 {
        rate_limiter.report_failure().await;
        return Err("509_RATE_LIMITED".into());
    }
    if !status.is_success() {
        rate_limiter.report_failure().await;
        return Err(format!("Server returned status {}", status));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    match parser::parse_image_page(&body) {
        Ok(result) => {
            rate_limiter.report_success().await;
            Ok(result)
        }
        Err(e) => {
            rate_limiter.report_failure().await;
            Err(e)
        }
    }
}

/// Download a full-size image and return its bytes.
pub async fn download_image(
    client: &Client,
    rate_limiter: &RateLimiter,
    url: &str,
) -> Result<Vec<u8>, String> {
    rate_limiter.wait().await;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download image: {}", e))?;

    let status = response.status();
    if status.as_u16() == 509 {
        rate_limiter.report_failure().await;
        return Err("509_RATE_LIMITED".into());
    }
    if !status.is_success() {
        rate_limiter.report_failure().await;
        return Err(format!("Image download returned status {}", status));
    }

    let bytes = response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Failed to read image bytes: {}", e))?;

    // Check for 509 error image (small GIF).
    if bytes.len() < 2000 {
        if bytes.starts_with(b"GIF") {
            rate_limiter.report_failure().await;
            return Err("509_RATE_LIMITED".into());
        }
    }

    rate_limiter.report_success().await;
    Ok(bytes)
}

/// Build the initial search URL from query parameters.
/// Used for the first page of a new search.
pub fn build_search_url(
    query: &str,
    category_mask: u32,
    adv: &AdvancedSearchOptions,
) -> String {
    let mut url = format!(
        "{}/?f_search={}",
        EXH_BASE,
        urlencoding::encode(query)
    );

    if category_mask > 0 {
        url.push_str(&format!("&f_cats={}", category_mask));
    }

    let search_name = adv.search_name.unwrap_or(true);
    let search_tags = adv.search_tags.unwrap_or(true);
    let search_description = adv.search_description.unwrap_or(false);
    let show_expunged = adv.show_expunged.unwrap_or(false);
    let search_torrent_filenames = adv.search_torrent_filenames.unwrap_or(false);
    let only_with_torrents = adv.only_with_torrents.unwrap_or(false);
    let search_low_power_tags = adv.search_low_power_tags.unwrap_or(false);
    let search_downvoted_tags = adv.search_downvoted_tags.unwrap_or(false);

    let has_advanced = search_name
        || search_tags
        || search_description
        || show_expunged
        || search_torrent_filenames
        || only_with_torrents
        || search_low_power_tags
        || search_downvoted_tags
        || adv.minimum_rating.is_some()
        || adv.min_pages.is_some()
        || adv.max_pages.is_some();

    if has_advanced {
        url.push_str("&advsearch=1");
        if search_name       { url.push_str("&f_sname=on"); }
        if search_tags       { url.push_str("&f_stags=on"); }
        if search_description { url.push_str("&f_sdesc=on"); }
        if show_expunged     { url.push_str("&f_sh=on"); }
        if search_torrent_filenames { url.push_str("&f_storr=on"); }
        if only_with_torrents { url.push_str("&f_sto=on"); }
        if search_low_power_tags { url.push_str("&f_sdt1=on"); }
        if search_downvoted_tags { url.push_str("&f_sdt2=on"); }

        if let Some(rating) = adv.minimum_rating {
            url.push_str(&format!("&f_sr=on&f_srdd={}", rating));
        }

        if adv.min_pages.is_some() || adv.max_pages.is_some() {
            url.push_str("&f_sp=on");
            match adv.min_pages {
                Some(n) => url.push_str(&format!("&f_spf={}", n)),
                None    => url.push_str("&f_spf="),
            }
            match adv.max_pages {
                Some(n) => url.push_str(&format!("&f_spt={}", n)),
                None    => url.push_str("&f_spt="),
            }
        }
    }

    url
}

/// Fetch search results from ExHentai using server-side search.
///
/// `url` is either a freshly built search URL (first page) or the #unext href
/// from a previous search result (cursor-based pagination for subsequent pages).
///
/// Returns parsed listing using the same gallery listing parser.
pub async fn fetch_search_results(
    client: &Client,
    rate_limiter: &RateLimiter,
    url: &str,
) -> Result<ListingPage, String> {
    rate_limiter.wait().await;

    tracing::info!("[fetch_search_results] requesting URL: {}", url);

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        rate_limiter.report_failure().await;
        return Err(format!("Server returned status {}", status));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    match parser::parse_gallery_listing(&body) {
        Ok(listing) => {
            rate_limiter.report_success().await;
            tracing::info!(
                "[fetch_search_results] parsed: {} galleries, has_next={}, next_url={:?}",
                listing.galleries.len(),
                listing.next_url.is_some(),
                listing.next_url
            );
            Ok(listing)
        }
        Err(e) => {
            rate_limiter.report_failure().await;
            Err(e)
        }
    }
}

/// Download a single image (thumbnail) and return its bytes.
/// Thumbnails are served from CDN (ehgt.org), not ExHentai — no rate limit needed.
pub async fn download_thumbnail(
    client: &Client,
    url: &str,
) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to download thumbnail: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        // Read body for diagnosis.
        let body_preview = response.text().await
            .map(|t| t.chars().take(200).collect::<String>())
            .unwrap_or_else(|_| "<unreadable>".to_string());
        return Err(format!("Thumbnail download returned status {}, body: {}", status, body_preview));
    }

    // Validate content-type is actually an image.
    let content_type = response.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !content_type.is_empty() && !content_type.starts_with("image/") {
        let body_preview = response.text().await
            .map(|t| t.chars().take(200).collect::<String>())
            .unwrap_or_else(|_| "<unreadable>".to_string());
        return Err(format!("Thumbnail response is not an image (content-type: {}), body: {}", content_type, body_preview));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Failed to read thumbnail bytes: {}", e))
}

// ── Favorites ─────────────────────────────────────────────────────────────

/// Fetch the favorites page. Returns parsed listing + folder metadata.
/// `url` is either the base favorites URL or a cursor-based next URL.
pub async fn fetch_favorites_page(
    client: &Client,
    rate_limiter: &RateLimiter,
    url: &str,
) -> Result<(ListingPage, Vec<crate::models::FavoriteFolder>), String> {
    rate_limiter.wait().await;

    tracing::info!("[fetch_favorites_page] requesting URL: {}", url);

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        rate_limiter.report_failure().await;
        return Err(format!("Server returned status {}", status));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    match (parser::parse_gallery_listing(&body), parser::parse_favorite_folders(&body)) {
        (Ok(listing), folders) => {
            rate_limiter.report_success().await;
            Ok((listing, folders.unwrap_or_default()))
        }
        (Err(e), _) => {
            rate_limiter.report_failure().await;
            Err(e)
        }
    }
}

/// Submit an add/move/remove favorite action.
/// `favcat`: Some(0–9) = add/move to folder; None = remove.
/// `favnote`: personal note (empty string is fine).
pub async fn submit_favorite(
    client: &Client,
    rate_limiter: &RateLimiter,
    gid: i64,
    token: &str,
    favcat: Option<u8>,
    favnote: &str,
) -> Result<(), String> {
    rate_limiter.wait().await;

    let url = format!(
        "{}/gallerypopups.php?gid={}&t={}&act=addfav",
        EXH_BASE, gid, token
    );

    let favcat_str = match favcat {
        Some(idx) => idx.to_string(),
        None => "favdel".to_string(),
    };
    let note = if favcat.is_none() { "" } else { favnote };

    tracing::info!(
        "[submit_favorite] gid={} token={} favcat={} note_len={}",
        gid, token, favcat_str, note.len()
    );

    let params = [
        ("favcat", favcat_str.as_str()),
        ("favnote", note),
        ("apply", "Apply Changes"),
        ("update", "1"),
    ];

    let response = client
        .post(&url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        rate_limiter.report_failure().await;
        return Err(format!("Server returned status {}", status));
    }

    rate_limiter.report_success().await;
    Ok(())
}

/// Build the base favorites page URL for a given folder (or all).
pub fn build_favorites_url(favcat: Option<u8>) -> String {
    match favcat {
        Some(idx) => format!("{}/favorites.php?favcat={}", EXH_BASE, idx),
        None => format!("{}/favorites.php", EXH_BASE),
    }
}
