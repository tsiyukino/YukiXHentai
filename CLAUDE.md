# ExH Desktop — Agent Instructions

## What is this
Third-party ExHentai client. Tauri 2 + Rust backend + Svelte frontend + SQLite.
Project spec: `.doc/SPEC.md` — read it if you need project context.

## Build & Run
```bash
npm install                  # install frontend deps
npm run build                # build frontend (vite)
npm run tauri dev            # dev mode (frontend + backend)
npm run tauri build          # production build
cargo build                  # build Rust backend only (from src-tauri/)
cargo test                   # run Rust tests (from src-tauri/)
```

## Before you write any code
1. Read `.doc/` files relevant to your task.
2. If a contract exists (IPC command, DB table, shared type), follow it exactly.
3. If no contract exists for what you're building, create the doc entry first.

## After you change any interface
Update the matching `.doc/` file in the same commit. No exceptions.
Interfaces = IPC commands, DB schema, Rust public types, Svelte stores, config keys, error types.

## Architecture rules
- Frontend (Svelte) = thin view layer. No fetch(), no fs, no SQL.
- All logic in Rust. Exposed via Tauri IPC commands.
- SQLite is source of truth. Images on filesystem, paths in DB.
- Rate limit all HTTP. Configurable delays. Exponential backoff.
- Image cache is content-addressable on filesystem (paths in DB, not bytes).
- Performance and fluency are non-negotiable. Every interaction must feel seamless, instant, and frictionless. No visible lag on scroll, click, or navigation. Images must load promptly. Animations and transitions must be smooth. If a change introduces any perceptible jank or delay, it is a bug. Always batch IPC calls, throttle scroll handlers, use file paths instead of base64 for images, and keep reactive store updates minimal.

## Conventions
- Rust modules: commands/, http/, db/, images/, download/, models/, config/
- Frontend: src/lib/{components,stores,api,utils}/, src/routes/
- All IPC commands defined in commands/ module, documented in `.doc/ipc-commands.md`
- Rate limiting is mandatory on all HTTP requests to ExHentai
- Use configurable delays between requests; never parallel requests to same endpoint
- Thumbnails: 300px on long edge default, stored in sharded content-addressable cache

## ⚠️ IPC serialization: snake_case fields, camelCase params — NOT the same thing
This is the single most common source of bugs. Know the difference:

| What | Case | Example |
|---|---|---|
| IPC command **param names** in `invoke()` | camelCase | `invoke("get_foo", { myParam: x })` |
| Rust **struct field names** serialized in return values | snake_case | `{ page_index: 0, file_path: "..." }` |

Tauri auto-converts **param names** from camelCase → snake_case at the boundary.
Tauri does **NOT** convert **struct field names** — they serialize exactly as written in Rust.

**Rules:**
- Frontend `invoke("cmd", { camelCaseParams })` → Rust receives `snake_case_params`. ✓
- Rust returns `struct { snake_case_field }` → frontend receives `{ snake_case_field }`. ✓
- TypeScript interfaces for return types **must use snake_case** to match Rust struct fields.
- TypeScript `invoke()` argument objects **must use camelCase** for param names.
- NEVER write `interface LocalPage { pageIndex: number }` when Rust has `page_index: i32`. That produces `undefined` for every field and causes downstream errors (duplicate keys, broken images, etc.).

## .doc/ quick reference
| Need to know... | Read |
|---|---|
| Project spec & context | `.doc/SPEC.md` |
| IPC command signatures | `.doc/ipc-commands.md` |
| Database tables/columns | `.doc/db-schema.md` |
| Query patterns | `.doc/db-queries.md` |
| Rust shared types | `.doc/models.md` |
| HTTP client implementation | `.doc/http-client.md` |
| Cache file layout | `.doc/image-pipeline.md` |
| Svelte stores | `.doc/frontend-stores.md` |
| Component props/events | `.doc/frontend-components.md` |
| Config keys | `.doc/config.md` |
| Error types | `.doc/error-codes.md` |
| Naming/style rules | `.doc/conventions.md` |
| ExHentai URL patterns | `.doc/facts/exhentai-urls.md` |
| ExHentai JSON API (gdata, showpage) | `.doc/facts/exhentai-api.md` |
| ExHentai HTML selectors & page structure | `.doc/facts/exhentai-html-structure.md` |
| ExHentai cookies & authentication | `.doc/facts/exhentai-auth.md` |
| All third-party facts (README) | `.doc/facts/README.md` |
| iOS build strategy & known issues | `.doc/ios-build.md` |

## Doc file rules
- One topic per file. Never combine unrelated things.
- Max ~150 lines per file. If longer, split it.
- Entries are terse: signature, used-by, notes. No prose.

## Do NOT
- Make HTTP requests from the frontend
- Store image bytes in SQLite
- Skip .doc/ updates when changing interfaces
- Over-engineer; build minimum working version first
- Ignore rate limits
- Change or modify any third-party API endpoints, URL patterns, request formats, response parsing, cookie handling, or protocol details for ExHentai/E-Hentai. These are external facts about how the third-party site works, not our design choices. They must match exactly what the real site expects and returns. This includes gallery URLs, image URLs, API JSON structure, page HTML structure, login/cookie requirements, content server patterns, and any other external interface. If something is not working, the fix is to make our code match the real site behavior, never to change what we think the site behavior is. The only exception is if we discover through testing or documentation that our current understanding of the third-party API is actually wrong, in which case we update both the code and `.doc/facts/` to match the real behavior.
- Modify any file inside `.doc/facts/` without explicitly asking the user for permission first. If your code contradicts a file in `.doc/facts/`, your code is wrong, not the doc. Adding new facts is allowed but changing or deleting existing facts requires user approval.

## Current phase
Phase 4 — Reader + UI restructure (complete). Phases 1–4 done, major UI restructure applied.
Update this line as phases complete.
