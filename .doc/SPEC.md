# YukiXHentai — Project Spec Summary

## What
Third-party ExHentai/E-Hentai desktop client. Local-first gallery manager + reader.
Syncs metadata & images from the site, stores locally, provides advanced browse/search/filter/read.
NOT a scraper or bulk downloader — it's a full client application.

## Stack
- **Backend:** Rust (Tauri 2) — all logic lives here
- **Frontend:** Svelte + TypeScript — thin view layer only, no direct HTTP/filesystem/SQL
- **Database:** SQLite (WAL mode, FTS5 for tag search)
- **Config:** TOML file
- **Image cache:** Content-addressable filesystem (`cache/{originals,thumbs}/ab/cd/abcdef1234.jpg`)
- **Targets:** Windows > Linux > Android > iOS

## Core Principles
1. Local-first (fully usable offline with cached content)
2. Low resource usage (no Chromium, release image buffers, stream where possible)
3. Frontend = thin view layer (all logic in Rust, exposed via Tauri IPC)
4. Database = source of truth (query local DB, not website)
5. Don't get banned (rate limits, configurable delays, exponential backoff, cache aggressively)
6. Incremental development (minimum working version first, refactor when patterns emerge)

## Architecture
- **HTTP:** `reqwest` with persistent cookie jar (ipb_member_id, ipb_pass_hash, igneous)
- **Image pipeline:** download → decode (JPEG/PNG/WebP/GIF) → thumbnail (300px default) → disk cache
- **Download queue:** tokio async, configurable concurrency, rate limiting, pause/resume
- **Data manager:** owns SQLite schema, migrations, typed queries
- **Search engine:** filter params → SQL, FTS5 for tags, compound filters, saved presets

## Features
- Advanced filtering (compound AND/OR, multi-field, saved presets, smart collections)
- Custom sort orders (multi-level, any metadata field)
- Built-in reader (page-by-page + continuous scroll, progress tracking)
- Reading history with timestamps
- Bookmarks within galleries
- Custom collections, custom tags, user notes
- Batch operations
- Duplicate detection (metadata-based + optional perceptual hashing)
- Statistics dashboard

## Build Phases
1. Skeleton + authentication (login flow, cookie validation)
2. Metadata sync + basic browsing (HTML parsing, gallery grid, thumbnails)
3. Search + filtering (filter panel, SQL query builder, FTS5, sort orders)
4. Reader (image viewer, page/scroll modes, read progress, image caching)
5. Organization (collections, custom tags, notes, batch ops, saved filters)
6. Advanced (duplicate detection w/ img_hash, statistics, download queue UI)
7. Mobile (touch UI, Android/iOS via Tauri 2 mobile)

## Key Crates
tokio, reqwest, scraper, image, rusqlite, serde/serde_json, toml, tracing/tracing-subscriber, thiserror, anyhow
(img_hash deferred to Phase 6)

## Frontend Libraries
svelte-virtual-list, svelte-spa-router, lucide-svelte, chart.js, date-fns

## What NOT To Do
- No web scraper / bulk downloader behavior
- No Electron
- No frontend HTTP/filesystem/SQL (all via IPC)
- No images in database (store file paths only)
- No ignoring rate limits
- No over-engineering / premature abstraction
- No hardcoded HTML parsing without isolation
- No skipping .doc/ updates when interfaces change
- No long doc files (split at ~150 lines)
