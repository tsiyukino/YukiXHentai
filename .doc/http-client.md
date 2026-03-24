# HTTP Client
> Last updated: 2026-03-24 (circuit breaker added to RateLimiter) | Affects: src-tauri/src/http/

Third-party API facts (URLs, endpoints, HTML structure, auth) are in `.doc/facts/`. This file documents our implementation only.

## Rate Limiting
- **Implementation:** `RateLimiter` with async mutex, Tauri managed state.
- **Default delay:** 1000ms between requests.
- **Used by:** all ExHentai HTML page fetches (listing, detail, image viewer, search).
- **Not used by:** thumbnail downloads (from CDN, parallel OK), gdata/showpage JSON API.

## Circuit Breaker (inside RateLimiter)
- **Trigger:** 3 consecutive failures reported via `report_failure()` on any rate-limited fetch.
- **Effect:** all subsequent `wait()` calls block for 30 seconds before proceeding.
- **Reset:** `blocked_until` clears after the 30s window; failure counter resets to 0 on trip or on any `report_success()`.
- **Failure conditions:** HTTP non-2xx status, HTTP 509, 509.gif detection, parse error.
- **Purpose:** fundamental protection against ban loops — frontend can retry as fast as it wants, the backend enforces the pause on every request regardless.

### GdataRateLimiter
- **Implementation:** `GdataRateLimiter` with burst tracking, Tauri managed state.
- **Burst size:** 4 requests.
- **Cooldown:** 5 seconds after burst is exhausted.
- **Used by:** `commands::start_enrichment` (background metadata enrichment).
- **Notes:** Separate from the main `RateLimiter`. The gdata API allows ~4-5 requests before rate limiting; we use 4 to be safe. After 4 requests, waits 5 seconds before the next burst.

## build_client
- **Signature:** `fn build_client(cookies: &ExhCookies) -> Result<Client, String>`
- **Notes:** reqwest::Client with cookie jar, rustls-tls, Chrome User-Agent.

## validate_cookies
- **Signature:** `async fn validate_cookies(cookies: &ExhCookies) -> Result<(), String>`
- **Notes:** GETs exhentai.org. Uses sadpanda detection (see `.doc/facts/exhentai-html-structure.md`).

## fetch_gallery_listing
- **Signature:** `async fn fetch_gallery_listing(client, rate_limiter, next_url?) -> Result<ListingPage, String>`
- **Notes:** Uses cursor-based pagination (see `.doc/facts/exhentai-urls.md`). Rate-limited.

## fetch_gallery_detail
- **Signature:** `async fn fetch_gallery_detail(client, rate_limiter, gid, token, page?) -> Result<GalleryDetailPage, String>`
- **Used by:** `commands::get_gallery_pages`
- **Notes:** Rate-limited.

## fetch_image_url
- **Signature:** `async fn fetch_image_url(client, rate_limiter, page_url) -> Result<String, String>`
- **Used by:** `commands::get_gallery_image`
- **Notes:** GETs image viewer page. Rate-limited. Parses actual image URL.

## download_image
- **Signature:** `async fn download_image(client, rate_limiter, url) -> Result<Vec<u8>, String>`
- **Used by:** `commands::get_gallery_image`
- **Notes:** Downloads full-size image bytes. Rate-limited.

## download_thumbnail
- **Signature:** `async fn download_thumbnail(client, url) -> Result<Vec<u8>, String>`
- **Notes:** No rate limiting — thumbnails served from CDN. Both `sync_gallery_page` and `sync_next_page` use `download_thumbs_parallel` for concurrent thumbnail downloads.

## api_show_page (http/api.rs)
- **Signature:** `async fn api_show_page(client, gid, page, imgkey, showkey) -> Result<ImagePageResult, String>`
- **Used by:** `download/mod.rs` (fast path for image downloads)
- **Notes:** Uses showpage API (see `.doc/facts/exhentai-api.md`). Falls back to `fetch_image_url` on failure.

## api_gallery_metadata (http/api.rs)
- **Signature:** `async fn api_gallery_metadata(client, gids_tokens: &[(i64, String)]) -> Result<Vec<Gallery>, String>`
- **Used by:** `commands::fetch_gallery_metadata`, `commands::start_enrichment`
- **Notes:** Uses gdata API (see `.doc/facts/exhentai-api.md`). Max 25 per request. Rate limited via `GdataRateLimiter` when called from enrichment.

## ImagePageResult
- **Signature:** `struct ImagePageResult { image_url: String, nl_key: Option<String> }`
- **Used by:** `fetch_image_url`, `download/mod.rs`
- **Notes:** `nl_key` is the reload key for alternate server fallback (see `.doc/facts/exhentai-api.md`).

## HTML Parser (http/parser.rs)
All ExHentai HTML parsing logic isolated here. Selectors and structure documented in `.doc/facts/exhentai-html-structure.md`.

### Gallery listing parser
- **Output:** `ListingPage { galleries, next_url, prev_url }`

### Gallery detail parser (`parse_gallery_detail`)
- **Input:** Gallery detail page HTML
- **Output:** `GalleryDetailPage { page_urls, thumb_urls, imgkeys, total_pages, next_detail_page, showkey }`
- **Notes:** Handles three thumbnail formats (see `.doc/facts/exhentai-html-structure.md`).
- **Sprite encoding:** `sprite:{url}:{offsetX}:{offsetY}:{width}:{height}` — offset values are absolute (positive), converted from negative CSS values.

### Image page parser (`parse_image_page`)
- **Input:** Image viewer page HTML
- **Output:** `ImagePageResult { image_url, nl_key }`

### Showpage response parser (`parse_showpage_response`)
- **Input:** `i3` HTML fragment from showpage API response
- **Output:** `ImagePageResult { image_url, nl_key }`

### extract_showkey
- **Input:** Gallery detail page HTML
- **Output:** `Option<String>`

### extract_imgkey_from_page_url
- **Input:** Page URL string (`/s/{imgkey}/{gid}-{page}`)
- **Output:** `Option<String>`
