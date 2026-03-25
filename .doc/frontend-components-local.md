# Frontend Components — Local Library
> Last updated: 2026-03-24 (file dialogs, local reader, delete button) | Affects: src/lib/components/Local*.svelte, src/lib/components/QueueDownloadPage.svelte, src/lib/components/ImportPreviewDialog.svelte, src/lib/components/GalleryDetail.svelte

## LocalPage.svelte
- **Props:** none
- **Nav target:** `"downloads"` page (sidebar label: "Local" / 本地库 / ローカル)
- **Layout:** header toolbar + optional download banner + gallery grid
- **Toolbar:** gallery count label, quick filter input (client-side title/category/uploader/tags match), view toggle (cards/list), three-dot menu
- **Three-dot menu:** "Import folder", "Import from JSON", "Queue download" — opens respective dialogs
- **Import folder dialog:** in-app text-input dialog (FavoriteDialog pattern, z-600/601) — user pastes folder path, no native file picker
- **Gallery grid:** `get_local_galleries` (offset/limit 50), infinite scroll, `detailGallery` store on card click
- **Download banner:** shows when `DownloadQueueStatus.queued > 0 || downloading > 0`; listens to `local-download-progress` event; pause/resume/cancel controls
- **Events listened:** `local-download-progress`
- **i18n:** `local.title`, `local.gallery_count`, `local.filter_placeholder`, etc.

## ImportPreviewDialog.svelte
- **Props:** `folderPath: string`, `preview: ImportPreview`, `onConfirm: (g: Gallery) => void`, `onClose: () => void`
- **z-index:** 600 (same as FavoriteDialog)
- **Content:** editable title/titleJpn/category/uploader/description; read-only page count + sample filenames; metadata_found badge; confirm/cancel buttons
- **On confirm:** calls `confirmImportLocalFolder(folderPath, meta)` → on success calls `onConfirm(gallery)` + closes
- **Error handling:** shows inline error message

## LocalMetadataEditor.svelte
- **Props:** `gallery: Gallery`, `onClose: () => void`
- **z-index:** 1000 (full-screen overlay, same as GalleryReader)
- **Trigger:** "Edit metadata" button in GalleryDetail action row (only when `gallery.isLocal === 1`)
- **Sections:**
  - Basic fields: title, titleJpn, category (select), uploader, description (textarea)
  - Cover: thumbnail preview + "Change cover" button → native file dialog via `@tauri-apps/plugin-dialog` `open()` → `setLocalGalleryCover`
  - Tags: chips grouped by namespace; all removable via `updateGalleryMetadata({ tagsRemove })`; "Add tag" inline form with namespace datalist + value input
  - Pages: horizontal-scroll grid (~80×110px), drag-to-reorder (pointer events), × remove, "Insert pages" button → native multi-file dialog via `@tauri-apps/plugin-dialog` `open({ multiple: true })`
- **Save:** calls `updateGalleryMetadata` with changed scalar fields + tag ops
- **Unsaved changes:** prompts on close if dirty
- **Page ops:** `getLocalGalleryPages`, `reorderLocalPages`, `insertLocalPages`, `removeLocalPage`
- **File dialogs:** use `open` from `@tauri-apps/plugin-dialog` (requires `tauri-plugin-dialog` registered in lib.rs)

## QueueDownloadPage.svelte
- **Props:** `onClose: () => void`
- **z-index:** 1000 (full-screen overlay)
- **Trigger:** "Queue download" in LocalPage three-dot menu
- **Tabs:** Manual (textarea) | JSON file (pick file → `parseDownloadQueueJson`)
- **Parse (manual, frontend-only):** per line: full URL → extract gid/token; `gid:token` → direct; integer → gid only
- **Preview list:** one card per entry; status: Pending/Resolving/Ready/Already local/Failed
- **Resolve all:** sequentially calls `resolveGalleryToken` (one at a time) for token-less entries
- **Queue all ready:** calls `submitDownloadQueue` with all Ready entries
- **Options:** download originals toggle, subfolder input
- **Summary bar:** "{ready} ready · {alreadyLocal} already local · {failed} failed · {resolving} resolving"
- **Reorder:** drag handle on each card (pointer events)

## GalleryDetail.svelte (updated)
- Added "Edit metadata" button in action row; visible only when `gallery.is_local === 1`
- Opens `LocalMetadataEditor` as a full-screen overlay (z-1000)
- Download button replaced by "Delete" button (danger style) when `gallery.is_local === 1`
  - Delete: calls `deleteLocalGallery(gid)`, removes from DB + disk, closes detail panel
- Read button: for local galleries (`is_local === 1`) loads pages from `getLocalGalleryPages` (with `image_path` set) instead of fetching from ExHentai. Reader uses `image_path` directly (no network).
- `buildLocalReaderPages()`: maps `LocalPage[]` to `GalleryPageEntry[]` with `image_path=filePath`, `page_url=""`

## SettingsPage.svelte (updated — storage section)
- Added read cache slider (min=128, max=4096, step=64, unit MB) bound to `readCacheMaxMb`
- Added usage bar: `<progress>` + "{X MB / Y MB}" text
- Added "Clear read cache" button → `clearReadCache()` → shows freed bytes
- Distinct from existing "Clear all caches" button (which clears thumbs + originals)
- Initialized from `getReadCacheStats()` on mount
