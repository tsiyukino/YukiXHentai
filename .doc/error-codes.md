# Error Codes
> Last updated: 2026-03-21 (509 rate limit) | Affects: src-tauri/src/, src/lib/

## 509_RATE_LIMITED
- **Code:** `"509_RATE_LIMITED"`
- **Used by:** `http/parser.rs`, `http/mod.rs`, `download/mod.rs`
- **Notes:** Returned when ExHentai responds with HTTP 509 status or serves 509.gif (bandwidth exceeded). Download queue pauses all downloads with exponential backoff (5s → 10s → 20s → 30s cap). Frontend receives `"rate_limited"` status via `image-download-progress` event.
