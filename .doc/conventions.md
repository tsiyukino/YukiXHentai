# Conventions
> Last updated: 2026-03-21 | Affects: all modules

## IPC param naming
- **Rule:** Rust commands use `snake_case` params. Tauri auto-converts from frontend `camelCase`. Frontend `invoke()` calls use `camelCase` keys.
- **Used by:** all IPC commands and `src/lib/api/` wrappers
- **Notes:** e.g. Rust `ipb_member_id` ↔ frontend `ipbMemberId`

## API wrappers
- **Rule:** All `invoke()` calls go through typed wrappers in `src/lib/api/`. Components never call `invoke()` directly.
- **Used by:** `src/lib/api/`, all components
- **Notes:** One file per domain: `auth.ts`, `galleries.ts`.

## Config persistence
- **Rule:** Config loaded once at startup into `ConfigState`. Every mutation calls `config_state.save()`.
- **Used by:** `src-tauri/src/config/`, `src-tauri/src/commands/`
- **Notes:** TOML format. Platform-appropriate directory via `dirs::config_dir()`.

## Error handling in IPC
- **Rule:** Expected failures return a result type with `success: false` + message. Unexpected errors use `Err()`.
- **Used by:** `src-tauri/src/commands/`

## Tauri managed state
- **Rule:** Shared state (ConfigState, DbState, RateLimiter, ThumbCache) is registered via `tauri::Builder::manage()` and accessed in commands via `State<'_, T>`.
- **Used by:** `lib.rs`, all commands
- **Notes:** Each state type uses `Mutex` (std or tokio) for thread safety.

## HTML parser isolation
- **Rule:** All ExHentai HTML parsing logic lives in `http/parser.rs`. No HTML selectors elsewhere.
- **Used by:** `http/parser.rs`
- **Notes:** If the site's HTML changes, only this file needs updating.

## Thumbnail caching
- **Rule:** Thumbnails stored in content-addressable cache: `{cache_dir}/yukixhentai/thumbs/{ab}/{cd}/{hex_gid}.jpg`
- **Used by:** `images/`, `commands/`
- **Notes:** Frontend uses `convertFileSrc()` to display local files.

## Internationalization (i18n)
- **Rule:** All user-facing strings must use i18n keys. Never hardcode display text in components.
- **Used by:** all frontend components
- **Notes:** Locale files in `src/lib/i18n/{en,zh,ja}.json`. Import `{ t }` from `$lib/i18n`. Use `$t("key")` in templates. Interpolation: `$t("key", { param: value })`. Language selector in Settings > Preference. Locale stored in `locale` writable store.
- **Adding strings:** Add key to all 3 locale files (en.json, zh.json, ja.json) in same commit.

## Navigation
- **Rule:** App uses sidebar + content area layout. Navigation state in `stores/navigation.ts`.
- **Used by:** `Sidebar.svelte`, `+page.svelte`
- **Notes:** Pages: home, search, popular, favorites, watched, history, downloads, settings. Gallery clicks open detail panel (not reader directly). Reader opened from detail panel's Read button.
