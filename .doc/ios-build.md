# iOS Build — Codemagic CI + AltStore Sideloading

## Overview
Build unsigned IPA on Codemagic (free tier, mac_mini_m2), install via AltStore/AltServer on Windows.
No paid Apple Developer account required. AltStore re-signs with your free Apple ID (7-day cert).

## Build config
File: `codemagic.yaml` (repo root)
Artifact: `output/YukiXHentai.ipa`
Notification: tsiyukino@gmail.com on success/failure

## Install flow
1. Push to master → Codemagic auto-triggers
2. Download `.ipa` from Artifacts (~25 min build)
3. AltServer (Windows tray) → Sideload .ipa → select iPhone
4. iPhone: Settings → General → VPN & Device Management → trust cert
5. Re-sign every 7 days via AltStore (auto if on same WiFi as AltServer)

## Key decisions

### No signing cert in CI
Codemagic config has no `ios_signing` group. Signing is done by AltServer at install time.
The IPA is unsigned — that is intentional.

### Manual IPA packaging
`cargo tauri ios build` always fails at `exportArchive` with "No Team Found" when there
is no signing cert. This is expected. We let it fail and manually package the IPA from
the `.xcarchive` that was already created:

```bash
ARCHIVE=$(find src-tauri/gen/apple/build -name "*.xcarchive" | head -1)
cp -r "$ARCHIVE/Products/Applications" /tmp/Payload
cd /tmp && zip -r YukiXHentai.ipa Payload
```

An IPA is just a zip with `Payload/<App>.app` inside.

## Problems encountered (and fixes)

### `npm install` fails: EBADPLATFORM for rollup-win32-x64-msvc
**Cause:** package-lock.json had Windows-only rollup packages without `"optional": true`.
**Fix:** Mark them optional in package-lock.json. Also use `npm install --force` in CI to
bypass platform checks on the macOS build machine.

### CocoaPods fails: target `yukixhentai_macOS` not found
**Cause:** Generated `Podfile` had both `yukixhentai_iOS` and `yukixhentai_macOS` targets,
but the Xcode project only has `yukixhentai_iOS`.
**Fix:** Remove the `yukixhentai_macOS` target block from `src-tauri/gen/apple/Podfile`.

### `cargo tauri ios build` fails: unexpected argument `CODE_SIGN_IDENTITY=`
**Cause:** xcodebuild flags were passed directly to tauri, not forwarded to xcodebuild.
**Fix:** Pass them after `--`: `cargo tauri ios build -- CODE_SIGN_IDENTITY="" ...`
(Eventually superseded by the manual IPA packaging approach which avoids this entirely.)

### xcodebuild fails: "Signing requires a development team"
**Cause:** Xcode project had `CODE_SIGN_IDENTITY = "iPhone Developer"` but no
`CODE_SIGNING_ALLOWED = NO` / `CODE_SIGNING_REQUIRED = NO`.
**Fix:** Added both to build settings in `project.pbxproj` for both debug and release configs.

### exportArchive fails: "No Team Found in Archive"
**Cause:** `cargo tauri ios build --export-method debugging` calls `xcodebuild -exportArchive`
which requires a team ID even for debugging exports. No workaround via flags.
**Fix:** Let tauri build fail at export (`|| true`), then find the `.xcarchive` at
`src-tauri/gen/apple/build/*_iOS.xcarchive` and manually zip `Products/Applications`
into an IPA. The `.xcarchive` is created before the export step.

### IPA artifact was empty (0 bytes Payload)
**Cause:** We were searching DerivedData intermediates for the `.app`, but that path is
cleaned up after the failed export. The `.xcarchive` is what persists.
**Fix:** Search `src-tauri/gen/apple/build/` for `*.xcarchive`, not DerivedData.

### `ad-hoc` is not a valid --export-method value
**Cause:** Tauri only accepts: `app-store-connect`, `release-testing`, `debugging`.
**Fix:** Use `--export-method debugging`. The export will fail anyway (no team), so the
value doesn't matter much — we just need the build to proceed past compilation.

### `app_handle()` compile error: "private field, not a method"
**Cause:** `tauri::Manager` trait is not in scope. On iOS (`#[cfg(not(desktop))]`), the
`use tauri::{webview::PageLoadEvent, Emitter}` import was missing `Manager`.
`app_handle()` is provided by the `Manager` trait and silently fails to resolve without it.
**Fix:** Add `Manager` to the import: `use tauri::{webview::PageLoadEvent, Emitter, Manager};`

### WKWebView does not fill the full physical screen (large black bar at bottom)
**Cause:** wry initializes WKWebView with a zero frame and sets it as `UIWindow.contentView`
without explicit layout constraints. iOS constrains it to the safe area layout guide by default,
leaving a native black gap at the bottom that no CSS (`100vh`, `100dvh`, `-webkit-fill-available`,
`viewport-fit=cover`) can fix — the webview frame itself is too small.
**Fix:** Add `tauri-plugin-edge-to-edge` (v0.3) to `src-tauri/Cargo.toml` and register it first
in `lib.rs`:
```rust
.plugin(tauri_plugin_edge_to_edge::init())
```
This Swift plugin patches the WKWebView at the native level to fill the full physical screen.
It also injects `--safe-area-inset-top` / `--safe-area-inset-bottom` CSS variables for
positioning content away from the Dynamic Island and home indicator.

### Build phase script panics: missing server addr file
**Cause:** Tried to call `xcodebuild` directly (bypassing `cargo tauri ios build`).
The Rust build phase script is a tauri-cli wrapper that expects a dev server addr file
created by tauri before invoking xcodebuild.
**Fix:** Always use `cargo tauri ios build` to drive the build. Never call xcodebuild
directly for this project.
