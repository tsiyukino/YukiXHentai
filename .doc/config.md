# Configuration
> Last updated: 2026-03-21 (storage section) | Affects: src-tauri/src/config/

Config file: `{platform_config_dir}/yukixhentai/config.toml`
- Windows: `%APPDATA%/yukixhentai/config.toml`
- Linux: `~/.config/yukixhentai/config.toml`
- macOS: `~/Library/Application Support/yukixhentai/config.toml`

## [auth] section

### auth.ipb_member_id
- **Type:** `Option<String>`
- **Default:** `None`
- **Used by:** `config/`, `commands/login`, `commands/get_auth_status`
- **Notes:** ExHentai session cookie. Cleared on logout.

### auth.ipb_pass_hash
- **Type:** `Option<String>`
- **Default:** `None`
- **Used by:** `config/`, `commands/login`, `commands/get_auth_status`
- **Notes:** ExHentai session cookie. Cleared on logout.

### auth.igneous
- **Type:** `Option<String>`
- **Default:** `None`
- **Used by:** `config/`, `commands/login`, `commands/get_auth_status`
- **Notes:** ExHentai session cookie. Cleared on logout.

## ConfigState (Tauri managed state)
- **Signature:** `struct ConfigState { config: Mutex<AppConfig>, path: PathBuf }`
- **Used by:** all IPC commands via `State<'_, ConfigState>`
- **Notes:** Loaded once at app startup from disk. Written on every mutation (login/logout, UI settings).

## [ui] section

### ui.detail_preview_size
- **Type:** `u32`
- **Default:** `120`
- **Range:** 80–200
- **Used by:** `commands/get_detail_preview_size`, `commands/set_detail_preview_size`
- **Notes:** Detail panel page preview thumbnail size in px. Persisted via `set_detail_preview_size` IPC.

### ui.theme
- **Type:** `String`
- **Default:** `"light"`
- **Values:** `"light"` | `"dark"`
- **Used by:** `commands/get_theme`, `commands/set_theme`
- **Notes:** Color theme. Applied via `data-theme` attribute on `<html>`. Persisted via `set_theme` IPC. Invalid values fall back to `"light"`.

## [storage] section

### storage.cache_dir
- **Type:** `Option<String>`
- **Default:** `None` (uses platform default: `{cache_dir}/yukixhentai/`)
- **Used by:** `commands/get_cache_dir`, `commands/set_cache_dir`
- **Notes:** Custom cache directory for thumbnails, page thumbnails, and originals. None = platform default. Requires restart to take effect (caches are initialized at startup). Set via `set_cache_dir` IPC.
