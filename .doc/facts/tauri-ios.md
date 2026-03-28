# Tauri 2 iOS — Build & Runtime Facts
> These are third-party facts about Tauri 2's iOS support. Do not modify without user permission.

## Minimum iOS Version
- Default deployment target: **iOS 14.0** (raised from 13.0 in Tauri PR #13997)
- Controlled via `bundle.iOS.minimumSystemVersion` in `tauri.conf.json`
- Maps to `IPHONEOS_DEPLOYMENT_TARGET` in Xcode project

## Build Requirements
- **Must build on macOS** with Xcode installed — no Linux/Windows cross-compilation possible
- Apple SDK headers (`TargetConditionals.h`) and linker are macOS-only
- `rusqlite` bundled feature requires Xcode toolchain; compiles via `cc` crate against iPhone SDK

## Rust Targets
```bash
rustup target add aarch64-apple-ios          # physical device (ARM64)
rustup target add aarch64-apple-ios-sim      # simulator on Apple Silicon
rustup target add x86_64-apple-ios           # simulator on Intel Mac
```
- `tauri ios build` / `tauri ios dev` selects correct target automatically
- No manual `.cargo/config.toml` linker overrides needed; Tauri CLI uses Xcode toolchain via `xcrun`

## Project Scaffolding (`tauri ios init`)
- Run once on macOS to generate Xcode project: `cargo tauri ios init`
- Generates `src-tauri/gen/apple/` — **commit this folder** (contains `.gitignore` for build artifacts)
- Structure:
  ```
  src-tauri/gen/apple/
  ├── <AppName>.xcodeproj/
  ├── project.yml          ← XcodeGen spec
  ├── Assets.xcassets/     ← App icons
  ├── Sources/<AppName>/main.swift
  ├── Info.plist
  └── ExportOptions.plist
  ```
- Re-run after changing `identifier` in `tauri.conf.json`
- IPA output: `src-tauri/gen/apple/build/arm64/<AppName>.ipa`
- iOS-specific plist overrides: place `Info.ios.plist` in `src-tauri/`

## tauri.conf.json iOS Config
```json
{
  "bundle": {
    "iOS": {
      "minimumSystemVersion": "14.0",
      "bundleVersion": "1"
    }
  }
}
```
- Platform-specific overrides can also go in `tauri.ios.conf.json` alongside `tauri.conf.json`
- `CFBundleShortVersionString` is derived from top-level `version` (must be `M.m.p` format)
- Known bug (#9851): bundle identifier may not update in Xcode project without re-running `tauri ios init`

## App Directory Paths on iOS
- iOS apps run in a private sandbox: `/private/var/mobile/Containers/Data/Application/<UUID>/`
- The `dirs` crate applies macOS conventions on iOS (not officially documented for iOS):
  - `dirs::data_local_dir()` → `<sandbox>/Library/Application Support`
  - `dirs::cache_dir()` → `<sandbox>/Library/Caches`
  - `dirs::config_dir()` → same as `data_local_dir()` (no separate config dir on iOS)
- These are valid paths within the sandbox but must be created with `std::fs::create_dir_all()` first
- Known bug (#12552): `app_data_dir()` may fail with permission-denied if parent dirs don't exist
- **Preferred approach:** Use `app_handle.path().app_data_dir()` / `app_cache_dir()` from Tauri `Manager` trait instead of calling `dirs` directly — these are mobile-aware and bundle-id-scoped

## HTTP / TLS
- `reqwest` with `rustls-tls`: works on iOS (pure Rust TLS, no native dependency)
- `reqwest` with `native-tls`: **does not compile** for iOS targets (links SecureTransport/OpenSSL)
- ATS (App Transport Security) enforces HTTPS at OS level — ExHentai uses HTTPS so no issue
- No `Info.plist` ATS exception needed for ExHentai

## WKWebView & Asset Protocol
- Tauri uses `WKURLSchemeHandler` on iOS (same as macOS) — custom scheme `tauri://`
- URL format on iOS: `tauri://localhost/...` (same as macOS; differs from Windows `http://tauri.localhost`)
- Local assets served via this scheme bypass ATS
- No iOS-specific `assetProtocol` config needed beyond standard Tauri setup
- CSP must include `tauri:` scheme: `default-src 'self' tauri: asset:; img-src 'self' asset: tauri: blob: data:`

## tauri-plugin-dialog on iOS
- `open()` for file picking: **buggy** — defaults to photo picker unless MIME types/known extensions specified
- Document picker (Files app): unreliable; custom extensions may appear but cannot always be selected
- `message()` / `ask()` / `confirm()`: work (native UIAlertController)
- Security-scoped resource access: incomplete — no persistent access after picker closes
- Folder picker: no native iOS support
- `PickerMode` parameter added (PR #3034) to force document vs media mode — check plugin version

## SQLite (rusqlite bundled)
- Works on iOS with `features = ["bundled"]`
- WAL mode supported on iOS
- Requires macOS host + Xcode for compilation — not cross-compilable from Linux/Windows

## Codemagic CI for iOS
- Use `mac_mini_m2` instance (macOS required)
- 500 free minutes/month on free plan; macOS minutes cost more than Linux
- `tauri ios init` must be run as part of build or `gen/apple/` committed to repo
- IPA signing requires Apple Developer account credentials (API key or certificate + provisioning profile)
- See Codemagic iOS code signing docs for `keychain` + `app-store-connect` tooling
