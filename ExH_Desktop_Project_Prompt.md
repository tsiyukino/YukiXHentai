# PROJECT PROMPT — ExH Desktop

**Third-Party ExHentai Client**
*Cross-platform · Tauri 2 + Rust + Svelte · SQLite*

---

## 1. Project overview

You are building a third-party desktop and mobile client for ExHentai (and its public mirror E-Hentai). The application is a local-first gallery manager and reader that syncs metadata and images from the site, stores them locally, and provides a browsing, searching, filtering, and reading experience that goes far beyond what the website offers.

The app is not a scraper or a bulk downloader. It is a fully featured client application with its own UI, data model, and feature set. The website is the data source; the app is the product.

---

## 2. Why this stack

### The decision: Rust + Tauri 2 + Svelte + SQLite

This stack was chosen after evaluating C/C++, C#/.NET, Dart/Flutter, and Rust/Tauri against the project's specific requirements: image-heavy UI, low resource usage, local data management with custom queries, authenticated web scraping, and cross-platform targets including mobile.

**Why Rust over C/C++:**
Both compile to native code with equivalent runtime performance. The image pipeline, SQLite access, and file I/O are effectively identical in speed — Rust's `image` crate calls into the same C libraries (libjpeg-turbo, libpng, libwebp) under the hood, and `rusqlite` is a thin binding over SQLite's C engine. Where Rust wins is the web-facing half of this project. HTTP session management with cookies, async download queues, HTML parsing for metadata extraction, and JSON serialization are significantly less code and fewer bugs in Rust (`reqwest`, `scraper`, `serde`) compared to the C equivalents (libcurl + manual buffer management, libxml2 + DOM pointer traversal). The async runtime (`tokio`) handles concurrent downloads, rate limiting, and background tasks with structured concurrency that would require manual thread pool management in C/C++. Rust also eliminates the entire class of memory safety bugs (use-after-free, buffer overflows, dangling pointers) at compile time, which matters for a long-running application managing thousands of cached files and network connections.

**Why Rust over C#/.NET:**
The .NET runtime adds 30–50 MB of baseline memory before the application does any work. This undermines the core reason for choosing Tauri over Electron — minimal resource footprint. Tauri + Rust idles at roughly 30–40 MB total. Adding a .NET runtime on top would double that. C#'s library ecosystem (ImageSharp, AngleSharp, HttpClient, EF Core) is excellent for this type of project, but the runtime overhead is a permanent tax on every platform.

**Why Tauri 2 over Qt (C++) or .NET MAUI:**
Cross-platform mobile from a single codebase. Tauri 2 compiles to Windows, Linux, macOS, Android, and iOS using one build system and one project structure. Qt achieves similar reach but carries GPL or commercial licensing constraints and a heavier framework footprint. .NET MAUI supports Windows, Android, iOS, and macOS but has no Linux target and adds runtime overhead. Tauri's approach — native OS WebView for the UI, Rust for the backend — produces the smallest binaries (typically under 10 MB) and lowest idle memory of any cross-platform framework.

**Why Svelte over React/Vue:**
Svelte compiles away at build time, producing vanilla JavaScript with no framework runtime in the bundle. For an image-heavy grid that needs fast DOM updates during scrolling, less JavaScript overhead means smoother performance in the WebView. React and Vue would also work; Svelte is preferred, not required.

**Why SQLite:**
The app stores metadata for potentially tens of thousands of galleries with complex filtering, full-text tag search, and custom sort orders. SQLite handles this locally with zero server overhead, supports FTS5 for fast tag queries, and is the most battle-tested embedded database available. It runs identically on every target platform. Its single-file design simplifies backup, migration, and portability of user data.

---

## 3. Architecture and technology stack

### 3.1 Framework

**Framework: Tauri 2 (stable release).**

- Rust backend for all core logic: networking, image processing, data management, file I/O.
- Web-based frontend rendered in the OS native WebView (WebView2 on Windows, WebKitGTK on Linux, WKWebView on macOS/iOS, Android WebView on Android).
- Target platforms in order of priority: Windows first, then Linux, then Android, then iOS.
- All platform targets come from a single codebase. Platform-specific code is only permitted inside Tauri plugins using Swift (iOS) or Kotlin (Android) when absolutely necessary.

### 3.2 Frontend

**UI framework: Svelte (latest stable, with TypeScript).**

- Use a virtual scroll / windowed list approach for all gallery grid views. Only visible thumbnails plus a small buffer should exist in the DOM at any time.
- Lazy load images using IntersectionObserver. Thumbnails load as they scroll into view; full images load on demand in the reader.
- CSS should be component-scoped (Svelte default). Use CSS custom properties for theming. Support light and dark themes from day one.
- All communication with the Rust backend happens through Tauri IPC commands. The frontend never makes HTTP requests directly.

### 3.3 Backend (Rust core)

The Rust backend is the brain of the application. It handles everything the frontend cannot and should not do:

- **HTTP client:** Use `reqwest` with a persistent cookie jar. ExHentai requires authenticated sessions via cookies (`ipb_member_id`, `ipb_pass_hash`, `igneous`). The client must manage these cookies across sessions, handle login/re-authentication, and respect rate limits to avoid bans.
- **Image pipeline:** Download images, decode them (JPEG, PNG, WebP, GIF), generate thumbnails at configurable sizes, and write both originals and thumbnails to the disk cache. Use the `image` crate for decoding and resizing. Run this on a background thread pool via `tokio` so it never blocks the UI.
- **Download queue:** Async download manager built on `tokio`. Support concurrent downloads with configurable concurrency limits, per-domain rate limiting, pause/resume, and resumable downloads where the server supports range requests.
- **Data manager:** All reads and writes to the SQLite database go through this module. It owns the schema, runs migrations, and exposes typed query functions to the rest of the backend.
- **Search and filter engine:** Translate the frontend's filter parameters into optimized SQL queries. Use SQLite FTS5 for full-text tag search. Support compound filters, saved filter presets, and sort orders that do not exist on the website.

### 3.4 Storage

**Database: SQLite (single file, WAL mode enabled).**

- Store all gallery metadata: title (both Japanese and romanized), tags (with namespace: artist, group, parody, character, female, male, mixed, other), category, uploader, posted date, rating, page count, file size, thumbnail URLs, gallery token, and any ExHentai-specific identifiers.
- Store user-generated data: favorites, custom collections, read status, last page read, reading history with timestamps, user notes, custom tags.
- Store download state: queue entries, progress, completed/failed status.
- Use FTS5 virtual tables for fast full-text search across titles and tags.
- Use compound indexes on commonly filtered columns (category, rating, date, language).
- Store image file *paths* in the database, not the image bytes themselves.

**Filesystem: Content-addressable image cache.**

- Organize cached images in a sharded directory structure to avoid OS-level directory listing performance issues: `cache/ab/cd/abcdef1234.jpg` (first 2 bytes of hash as nested directories).
- Store originals and thumbnails separately: `cache/originals/...` and `cache/thumbs/...`.
- Thumbnail dimensions should be configurable. Default: 300px on the long edge.
- The cache directory location should be configurable and default to a platform-appropriate path (e.g., `%LOCALAPPDATA%` on Windows, `~/.cache` on Linux).

**Config: TOML file** for user-facing settings. Stored alongside the database.

---

## 4. Core principles

These principles apply to every decision made during development:

1. **Local-first.** The app must be fully usable when offline (reading cached content, filtering, organizing). Network is only needed for syncing new data from the site.
2. **Low resource usage.** Tauri was chosen specifically because it does not bundle Chromium. Respect that choice. Keep memory usage low, avoid unnecessary background processing, release image buffers after thumbnailing, use streaming where possible.
3. **The frontend is a thin view layer.** It renders data and collects user input. All logic — HTTP, image processing, SQL queries, file I/O — lives in the Rust backend and is exposed via Tauri IPC commands. The frontend never touches the filesystem or network directly.
4. **The database is the source of truth.** Once metadata is synced, the app queries the local database, not the website. This is what makes custom filtering, sorting, and features possible.
5. **Do not get banned.** Respect ExHentai's rate limits. Use configurable delays between requests. Never make parallel requests to the same endpoint. Implement exponential backoff on errors. Cache aggressively so repeated views never hit the network.
6. **Incremental development.** Build the minimum working version of each feature first, then iterate. Do not over-engineer abstractions before the feature works end-to-end.

---

## 5. Documentation standard — the `.doc/` folder

### 5.1 Purpose

The `.doc/` folder is the single source of truth for all contracts, interfaces, and conventions in this project. It exists so that any agent session or developer can look up exactly how something works without reading through scattered source files.

### 5.2 Rules — these are mandatory

1. **Before writing any code**, check `.doc/` for existing contracts that affect what you are building. If a relevant doc exists, follow it. Do not invent a new pattern.
2. **After changing any interface** (IPC command signature, database schema, shared type, public function signature, API contract), update the corresponding doc in `.doc/` in the same commit. Code and docs must never be out of sync.
3. **Before using a shared function or type**, check `.doc/` to confirm its current signature and behavior. Do not guess from memory.
4. **When creating something new** that other modules will depend on (a new IPC command, a new database table, a new shared utility), create or update the corresponding doc first, then write the code.

### 5.3 Folder structure

Keep docs short and focused. One file per topic. Never combine unrelated things into one file. Long files cause context overflow and mistakes.

```
.doc/
├── OVERVIEW.md              # One-paragraph project summary + pointer to this prompt
├── ipc-commands.md          # All Tauri IPC commands: name, params, return type, which module handles it
├── db-schema.md             # All tables, columns, types, indexes, FTS5 setup
├── db-queries.md            # Named query patterns used across the app (filter builder, search, etc.)
├── models.md                # All shared Rust structs/enums: name, fields, where used
├── http-client.md           # Endpoints hit, request format, rate limit rules, cookie handling
├── image-pipeline.md        # Cache paths, thumbnail sizes, hash scheme, file naming convention
├── frontend-stores.md       # Svelte stores: name, shape, what updates them
├── frontend-components.md   # Reusable components: props, events, usage
├── config.md                # All config keys, types, defaults, where read
├── error-codes.md           # Error types, codes, how they propagate from Rust to frontend
└── conventions.md           # Naming conventions, file organization rules, code style decisions
```

### 5.4 Document format

Every doc follows this structure:

```markdown
# [Topic name]
> Last updated: [date] | Affects: [list of modules/files]

## [Item name]
- **Signature / Schema / Shape:** (the actual contract)
- **Used by:** (which files or modules depend on this)
- **Notes:** (edge cases, constraints, gotchas)
```

Keep entries terse. No prose explanations — just the contract. If someone needs to understand *why*, they read this project prompt. The `.doc/` files answer *what* and *how*.

### 5.5 When to update

| Event | Action |
|---|---|
| Add a new IPC command | Add entry to `ipc-commands.md` |
| Change a command's params or return type | Update `ipc-commands.md` |
| Add or alter a database table | Update `db-schema.md` |
| Add a new shared Rust type | Update `models.md` |
| Add a new Svelte store | Update `frontend-stores.md` |
| Change a config key | Update `config.md` |
| Add a new reusable component | Update `frontend-components.md` |
| Add a new error variant | Update `error-codes.md` |
| Change any convention | Update `conventions.md` |

If you are unsure whether a change needs a doc update, it does.

---

## 6. Features — what the app does beyond the website

The entire point of this client is to offer capabilities the website does not. These are the custom features that differentiate the app:

### 6.1 Advanced filtering and sorting

The website offers basic tag search and a few sort options. This app offers:

- **Compound filters:** Combine any number of conditions with AND/OR logic. Filter by tag (include/exclude, with namespace), category, rating range, page count range, date range, language, uploader, file size range, and any user-defined metadata.
- **Custom sort orders:** Sort by any metadata field ascending or descending. Support multi-level sort (e.g., sort by rating descending, then by date descending within the same rating).
- **Saved filter presets:** Users can name and save any combination of filters for quick reuse. Presets appear in a sidebar or dropdown for one-click access.
- **Smart collections:** Auto-updating collections defined by filter rules (like smart playlists). For example: "All English, rating > 4, artist is X, added in the last 30 days."

### 6.2 Reading and tracking

- **Built-in reader** with page-by-page and continuous scroll modes.
- **Read progress tracking:** Automatically saves the last page read per gallery. Shows completion percentage in the grid view.
- **Reading history:** Timestamped log of when each gallery was opened and how far the user read.
- **Bookmarks within galleries:** Mark specific pages for quick return.

### 6.3 Organization

- **Custom collections:** User-created folders/groups for organizing galleries manually. A gallery can belong to multiple collections.
- **Custom tags:** Users can add their own tags to any gallery, independent of the site's tag system.
- **User notes:** Free-text notes attachable to any gallery.
- **Batch operations:** Select multiple galleries and apply actions in bulk — add to collection, tag, mark as read, download, delete from cache.

### 6.4 Duplicate detection

- **Metadata-based:** Flag galleries with matching titles, overlapping tags, or same artist+title combination.
- **Thumbnail hash comparison (optional, advanced):** Perceptual hashing of cover thumbnails to detect visual duplicates even with different metadata.

### 6.5 Statistics

- **Library stats:** Total galleries, storage used, tag frequency distribution, category breakdown.
- **Reading stats:** Galleries read over time, average reading time, most-read categories/artists.
- **Optional dashboard view** presenting these as simple charts or summary cards.

---

## 7. Project structure

Use the standard Tauri 2 project layout:

```
exh-desktop/
├── .doc/                  # Documentation (see section 5)
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── main.rs        # Tauri entry point
│   │   ├── lib.rs         # Module declarations
│   │   ├── commands/      # Tauri IPC command handlers
│   │   ├── http/          # HTTP client, cookie management, rate limiter
│   │   ├── db/            # SQLite schema, migrations, query functions
│   │   ├── images/        # Image download, decode, thumbnail, cache
│   │   ├── download/      # Download queue manager
│   │   ├── models/        # Shared data types (Gallery, Tag, Filter, etc.)
│   │   └── config/        # App configuration
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                   # Svelte frontend
│   ├── lib/
│   │   ├── components/    # Reusable UI components
│   │   ├── stores/        # Svelte stores for state management
│   │   ├── api/           # Typed wrappers around Tauri invoke() calls
│   │   └── utils/         # Frontend helpers
│   ├── routes/            # Pages / views
│   └── app.html
├── static/                # Static assets
├── package.json
├── svelte.config.js
├── vite.config.ts
└── tsconfig.json
```

---

## 8. Key Rust crates

| Purpose | Crate |
|---|---|
| Async runtime | `tokio` (full features) |
| HTTP client | `reqwest` (with cookies, rustls-tls) |
| HTML parsing | `scraper` |
| Image processing | `image` |
| Perceptual hashing | `img_hash` |
| SQLite | `rusqlite` (with bundled feature, FTS5) |
| Serialization | `serde`, `serde_json` |
| Config files | `toml` |
| Logging | `tracing`, `tracing-subscriber` |
| Error handling | `thiserror`, `anyhow` |

---

## 9. Key frontend libraries

| Purpose | Library |
|---|---|
| Virtual scroll | `svelte-virtual-list` or custom IntersectionObserver-based solution |
| Routing | SvelteKit file-based routing or `svelte-spa-router` |
| Icons | `lucide-svelte` or similar SVG icon set |
| Charts (stats) | `chart.js` with Svelte wrapper, or `layercake` |
| Date formatting | `date-fns` |

---

## 10. Data flow summary

1. **User browses or searches** → frontend sends filter parameters via Tauri `invoke()` → Rust backend builds SQL query → queries local SQLite → returns typed results → frontend renders virtual-scrolled grid with lazy-loaded thumbnails.
2. **User syncs from site** → frontend triggers sync command → Rust HTTP client fetches gallery listing pages → parses HTML → upserts metadata into SQLite → downloads thumbnails to disk cache → frontend refreshes view from database.
3. **User reads a gallery** → frontend requests page URLs from backend → backend checks cache, returns cached paths or fetches from site → frontend displays images → backend updates read progress in database.
4. **User applies custom action** (tag, collect, note, batch op) → frontend sends command → backend writes to SQLite → frontend re-queries and updates view.

---

## 11. What NOT to do

- **Do not build a web scraper that dumps files.** This is a client application with a UI and a data model.
- **Do not use Electron.** The framework choice is Tauri 2 for its low resource footprint.
- **Do not make the frontend do backend work.** No `fetch()` calls from Svelte. No filesystem access from JavaScript. No SQL from the frontend.
- **Do not store images in the database.** Store file paths. Images live on the filesystem in the content-addressable cache.
- **Do not ignore rate limiting.** Build it into the HTTP layer from day one, not as an afterthought.
- **Do not over-abstract early.** Write the straightforward implementation first. Refactor when patterns emerge from real usage.
- **Do not hardcode ExHentai-specific HTML parsing assumptions** without marking them clearly. The site's HTML structure can change. Isolate all parsing logic in dedicated modules so it can be updated without touching the rest of the codebase.
- **Do not skip doc updates.** If you change an interface, update `.doc/` in the same commit. No exceptions.
- **Do not write long doc files.** Each `.doc/` file covers one topic. If a file exceeds ~150 lines, split it. Long files cause context overflow and lead to mistakes.

---

## 12. Starting point — build in this order

**Phase 1 — Skeleton and authentication**
Set up the Tauri 2 project with Svelte frontend. Create the `.doc/` folder with initial docs. Implement the login flow: user enters cookies or credentials, Rust backend validates them against ExHentai, persists them. Display a confirmation in the UI.

**Phase 2 — Metadata sync and basic browsing**
Implement gallery list fetching and HTML parsing. Store parsed metadata in SQLite. Display a basic grid of galleries with titles and thumbnails. Implement pagination or infinite scroll.

**Phase 3 — Search and filtering**
Build the filter panel UI. Implement SQL query building from filter parameters. Add FTS5 for tag search. Add custom sort orders.

**Phase 4 — Reader**
Build the image viewer. Implement page-by-page and scroll modes. Track read progress. Cache viewed images to disk.

**Phase 5 — Organization features**
Collections, custom tags, notes, batch operations. Saved filter presets and smart collections.

**Phase 6 — Advanced features**
Duplicate detection, statistics dashboard, download queue management UI.

**Phase 7 — Mobile targets**
Adapt the UI for touch and smaller screens. Build for Android and iOS using Tauri 2 mobile support. Test and adjust platform-specific behaviors.
