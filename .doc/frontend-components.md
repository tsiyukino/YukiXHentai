# Frontend Components
> Last updated: 2026-03-24 (fix Bug A: detailBatchState not wiped on g=null close; fix Bug B: setActiveDetailGallery(null) guarded so reader slot not overwritten) | Affects: src/lib/components/GalleryDetail.svelte

## Design System
- **Theming:** CSS custom properties on `:root` with `data-theme` attribute (`"light"` | `"dark"`). All colors use `var(--*)` — no hardcoded colors in components.
- **Light palette:** `--bg-primary: #ffffff`, `--bg-secondary: #f8f8fa`, `--accent: #7c3aed` (purple).
- **Dark palette:** `--bg-primary: #111111`, `--bg-secondary: #0f0f0f`, `--accent: #8b5cf6` (lighter purple).
- **Semantic vars:** `--danger-border`, `--danger-bg`, `--success-bg`, `--overlay-bg`, `--scrollbar-thumb`, `--scrollbar-thumb-hover`.
- **Radii:** `--radius-sm: 8px`, `--radius-md: 10px`, `--radius-lg: 12px`.
- **Shadows:** `--shadow-sm`, `--shadow-md` (lighter in light theme, darker in dark theme).
- **Titlebar:** 32px custom titlebar at top of `.app` (replaces native decorations). `--bg-primary` bg, bottom border, `-webkit-app-region: drag`. Contains `WindowControls` (right-aligned). All screens (loading, login, main) sit below it.
- **Hover:** Cards use shadow lift (sm→md), not background tint. Nav items use subtle accent-subtle bg.
- **Active nav:** `background: var(--accent-subtle); color: var(--accent)` — not solid colored block.
- **Pills/badges:** `border-radius: 6px` (tags) or `20px` (category, search badge). No 2px radii.
- **Inputs:** Focus ring uses `box-shadow: 0 0 0 3px var(--accent-subtle)` + `border-color: var(--accent)`.
- **Typography:** Section headers are small uppercase muted. Body text medium weight dark gray.
- **Theme toggle:** Sidebar bottom (moon/sun icon), Settings page theme section. Both call `setTheme` IPC to persist.

## Sidebar
- **Props:** none
- **Events:** none (uses navigation stores)
- **Used by:** `+page.svelte`
- **Notes:** Two modes: collapsed (56px icon strip) and expanded (220px). Expanded has: brand name + collapse toggle, rounded search bar with `/` shortcut badge, nav items grouped under uppercase muted labels ("NAVIGATION", "LIBRARY"). Bottom section: theme toggle (moon/sun icon) + settings, separated by border. Active state: `accent-subtle` bg + `accent` color. Uses `--bg-primary` background.

## WindowControls
- **Props:** none
- **Events:** none
- **Used by:** `+page.svelte` (always rendered, all screens)
- **Notes:** Custom window control buttons (minimize, maximize, close) for frameless window. Inline in titlebar (right-aligned via parent `justify-content: flex-end`). Uses `@tauri-apps/api/window` for `minimize()`, `toggleMaximize()`, `close()`. Close button turns red on hover. All buttons use `--text-secondary` color, `--bg-hover` on hover. 46x32px buttons. `app-region: no-drag` to stay clickable inside drag titlebar.

## LoginForm
- **Props:** none
- **Events:** none (uses auth stores)
- **Used by:** `+page.svelte`
- **Notes:** Three-field cookie form. Calls `login()` IPC. Centered card with `--bg-primary` bg, `--shadow-md` shadow, `--radius-lg` corners. Input focus uses accent ring. Uses i18n. Wrapper uses `flex: 1` to fill space below titlebar.

## AuthStatus
- **Props:** none
- **Events:** none (uses auth stores)
- **Used by:** `+page.svelte` header
- **Notes:** Green dot + "Connected" + logout button. Uses i18n.

## GalleryCard
- **Props:** `{ gallery: Gallery, progress?: ReadProgress, onOpen?: (gallery) => void }`
- **Events:** none (calls onOpen callback)
- **Used by:** `GalleryGrid.svelte`
- **Notes:** Card view. Preview image (3:4 aspect, ~2/3 of card). Category pill + completed badge overlaid on image top. Bottom: info section with FIXED height (8.5rem, overflow hidden) — required for stable virtual scroll positioning. Row 1 = title (2-line clamp, 0.875rem, bold 700). Row 2 = star rating (left), lang badge + page count (right). Row 3 = tag pills (overflow hidden, auto-pushed to bottom, max-height 2.5rem). All single-line fields use `text-overflow: ellipsis` + `overflow: hidden` + `white-space: nowrap`. Tags use `max-width: 120px` with ellipsis. CSS `contain: layout style paint` + `content-visibility: auto`. Uses `--card-bg`/`--card-border` vars. Flat hover: border color shift + background tint only, no shadows or transforms. Skeleton placeholder visible until img onload fires. Image retry on error (3 attempts with backoff, uses reactive state not DOM manipulation). Shows skeleton when no local thumb_path (remote URLs not used — they require cookies).

## GalleryListItem
- **Props:** `{ gallery: Gallery, progress?: ReadProgress, onOpen?: (gallery) => void }`
- **Events:** none (calls onOpen callback)
- **Used by:** `GalleryGrid.svelte`
- **Notes:** List view. Thumbnail (left, 52x70), middle: title + tag pills, right: category pill + lang badge + page count. CSS `contain: layout style paint` + `content-visibility: auto`. Uses `--card-bg`/`--card-border` vars. Flat hover: background tint + border shift only. Skeleton placeholder + crossfade on img load. Image retry on error (3 attempts with backoff, uses reactive state).

## VirtualGrid
- **Props:** `{ items: any[], rowHeight: number, columnMinWidth: number, gap?: number (default 0), buffer?: number (default 8), onScrollNearEnd?: () => void, scrollEndThreshold?: number (default 200), recheckTrigger?: number (default 0), children: snippet(item, index), footer?: snippet }`
- **Used by:** `GalleryGrid.svelte` (card mode)
- **Notes:** Virtualized CSS grid. Footer is an extra virtual row with fixed small height (`FOOTER_HEIGHT = 64px`), not the full `rowStride`. `dataRows = ceil(items.length / columns)`, `totalRows = dataRows + 1` when footer exists. Spacer height = `dataHeight + footerH + 64` where `dataHeight = dataRows * rowStride - gap`, `footerH = FOOTER_HEIGHT + gap`. Footer rendered as `position: absolute` div with `transform: translateY(footerY)`, height = `FOOTER_HEIGHT`. Footer only rendered when its row index falls within the visible range (`footerVisible` derived). Uses `ResizeObserver` for container width/height. Computes column count from `columnMinWidth` using gap-aware formula. Only renders visible rows ± buffer. Visible items container is `position: absolute` with `will-change: transform`. Scroll container uses `min-height: 0` (required for flex overflow). Scroll handler throttled via `requestAnimationFrame`. `recheckTrigger` prop: increment to force scroll-proximity recheck. `$effect` re-checks scroll proximity on `items.length` or `recheckTrigger` change — snapshots distance-from-bottom synchronously before rAF to handle the case where appended items grow the spacer before measurement. Uses `Math.ceil` on distance calculation to handle subpixel rounding. Uses `item.gid` as {#each} key.

## VirtualList
- **Props:** `{ items: any[], rowHeight: number, gap?: number (default 0), buffer?: number (default 20), onScrollNearEnd?: () => void, scrollEndThreshold?: number (default 200), recheckTrigger?: number (default 0), children: snippet(item, index), footer?: snippet }`
- **Used by:** `GalleryGrid.svelte` (list mode)
- **Notes:** Virtualized single-column list. Same footer pattern as VirtualGrid but simpler (1 column). `totalCount = dataCount + 1` when footer exists. Spacer height = `dataHeight + footerH + 64` where `dataHeight = dataCount * rowStride - gap`, `footerH = FOOTER_HEIGHT + gap` (`FOOTER_HEIGHT = 64px`). Footer rendered with `transform: translateY(footerY)`, height = `FOOTER_HEIGHT`. Footer only rendered when within visible range. Flexbox column layout. All heights are `$derived`, recalculate on every items change. Scroll container uses `min-height: 0`. `recheckTrigger` prop for post-sync recheck. Same snapshot-based scroll proximity recheck as VirtualGrid. Uses `item.gid` as {#each} key.

## GalleryGrid
- **Props:** none
- **Events:** none (uses gallery stores + thumbnail-ready/gallery-enriched event listeners)
- **Used by:** `+page.svelte` (home page)
- **Notes:** Supports card/list toggle via `viewMode` store. Card size configurable via `cardSize` store. Toolbar: gallery count, hide filter input (funnel icon), view toggle, filter button. Click opens GalleryDetail. Filter sidebar slide-in (no blur). On startup: shows skeleton grid, resets sync cursor, calls `sync_next_page` which returns the full gallery array directly. Galleries are appended to the master list from the command's return value — no streaming events, no DB queries for the gallery list. `syncInFlight` flag prevents concurrent `syncNextPage` calls (blocks `autoSync` during `startFreshSync`). Thumbnails update in-place via `thumbnail-ready` events; enrichment updates via `gallery-enriched` events. Both handlers update both `$galleries` (master list) and `sortedUnfiltered` so sorted view stays current. Progress refresh only on detail close. Infinite scroll: `recheckTrigger` counter signals VirtualGrid/VirtualList to re-check scroll proximity after `syncingMore` resets. After a successful batch, a deferred `recheckTrigger++` is scheduled after `AUTO_SYNC_COOLDOWN_MS + 50ms` so the VirtualGrid snapshot-based recheck can detect if the user is still near the old bottom and trigger the next fetch automatically. Up to 5 consecutive errors before giving up. Offline fallback after 3s timeout (only case where DB is queried for gallery list). Search mode uses DB pagination. Card gap: 16px. Uses i18n. Loading indicator (animated dots + text) shown inside virtual scroll footer while fetching next page. Footer states: loading dots while fetching; "No more galleries" only when `hasMoreRemote` is false.
- **Hide filter (content blocker):** Toolbar input with funnel icon (home mode only, hidden in search mode). 250ms debounce. **Excludes** galleries matching the term — the opposite of search. Matches title, title_jpn, category, uploader, tags (case-insensitive substring). Pure view-layer transform.
- **Sort feature:** FilterPanel "Sort" button writes `sortScope` and calls `handleSort`. Default sort count is **100** (first preset); restores to previous `$sortScope.count` if a sort was already committed. `handleSort` follows a strict sequence: **(1) Fetch** — if target count > `$galleries.length`, show progress overlay and call `fetchOneBatch` sequentially until `get(galleries).length >= target` or `hasMoreRemote` false; in date mode, keep fetching until oldest gallery in latest batch is older than the cutoff. Skip fetch if enough already loaded. **(2) Sort** — read master list with `get(galleries)` (not `$galleries` — avoids stale closure across `await` boundaries), slice/date-filter to scope, sort with `applySortToList`, store result in `sortedUnfiltered`. **(3) Render** — set `sortActive=true`. A single `$effect` watching `$sortActive`+`sortedUnfiltered`+`$homeFilter`+`debouncedFilter` applies the hide-filter and writes `$sortedGalleries` — this is the only writer of `$sortedGalleries`. No direct write in `handleSort` or `cancelSortFetch`. **(4)** When `homeFilter`/`quickFilter` changes while sort is active, the same `$effect` re-runs step 3 on `sortedUnfiltered` without re-sorting or re-fetching. `$sortFetchProgress = { fetching: true }` is set inside the `try` block so `finally` is guaranteed to clear it. `clearSort` increments cancel token + resets `sortActive`, `sortedGalleries`, `sortedUnfiltered`. Progress ETA: each `sync_next_page` ≈ 2s, returns 25 galleries. Sort banner shown: "Sorted by {field} · {count} galleries" + "Clear sort" button. `onScrollNearEnd` is no-op while `sortActive`.
- **Sort progress overlay:** Absolute-positioned overlay over the grid area (z-index 50, `pointer-events: all`). Shows fetch progress bar + label + Cancel button. Disappears when fetch phase ends.
- **filteredGalleries derivation:** `$derived.by` chain: `$galleries` → exclude quickFilter text matches → exclude `homeFilter` criteria → `filteredGalleries`. homeFilter applied only in home mode (not search mode). All galleries stay in the store; non-matching ones are hidden from view. No IPC, no DB queries. Count display shows "Hiding X of Y" when quickFilter or homeFilter is active. Filter button shows active state when homeFilter has any criteria set (even when panel is closed). If all loaded galleries are hidden, shows empty state with "Clear" button.
- **Auto-fill loop:** When a filter is active and `filteredGalleries.length` changes, a `$effect` increments `recheckTrigger`. VirtualGrid/VirtualList's own recheck `$effect` then measures `distFromBottom` — if the visible content is shorter than the container (or user is already at the bottom), it calls `onScrollNearEnd` → `autoSync`. `autoSync` schedules a further deferred `recheckTrigger++` after `AUTO_SYNC_COOLDOWN_MS + 50ms`, which repeats the cycle. The loop self-terminates when either: (a) enough matching galleries are loaded that the content overflows the viewport and the user is no longer at the bottom, or (b) `hasMoreRemote` becomes false. All fetches go through the same `autoSync` path with `syncInFlight` guard and cooldown — no runaway fetching. Matching galleries render immediately as each batch arrives; the loading indicator stays visible in the footer throughout.

## GalleryDetail
- **Props:** `{ fullPage?: boolean (default false) }`
- **Events:** none
- **Used by:** `+page.svelte`
- **Notes:** Two modes controlled by `detailExpanded` store + `fullPage` prop:
  - **Collapsed** (default, `fullPage=false`): Slide-in fixed side panel from right (520px). Renders with backdrop overlay outside `<main>`.
  - **Expanded** (`fullPage=true`): Full-page inline view. Page content in `<main>` is hidden via `.page-content.hidden`; detail fills entire content area as a flex child. No overlay, no fixed positioning, no animation. Back/Escape collapses back to panel mode (sets `detailExpanded=false`) rather than closing.
  - `+page.svelte` passes `fullPage={$detailExpanded && !!$detailGallery}`. Single component instance always rendered inside `<main>` — no mount/unmount on expand/collapse.
  - Preview thumbnail size configurable (80–200px, default 120) via `detailPreviewSize` store.
  - **Lazy batch loading:** On open, calls `getGalleryPagesBatch(gid, token, 0)` to fetch first 20 page entries + total_pages. Renders placeholder slots for all pages immediately based on total count. Each batch gets one IntersectionObserver (rootMargin 400px) that watches **all** thumbnails in that batch range — any thumbnail entering view triggers `getGalleryPagesBatch`, not just the first one. Never fetches ahead of scroll position. Page thumb paths stored in local `pageThumbPaths: Record<number, string>` (raw paths) and mirrored to the `detailPageThumbs` store on every write so GalleryReader can read them.
  - **Shared batch state (detail↔reader):** On open, sets `detailBatchState` store to `{ gid, token, showkey, pagesPerBatch, totalPageCount, fetchedDetailPages: Set, pageEntries: Record }`. The Set and Record are the SAME object references as the detail's local variables — mutated in-place by both sides. `fetchBatch` updates scalar fields (`pagesPerBatch`, `showkey`, `totalPageCount`) via `detailBatchState.set({...bs, ...})`. On close due to reader open, `detailBatchState` is NOT wiped so reader inherits the state immediately.
  - **Same-gallery re-open guard:** The open `$effect` detects same-gallery by checking `detailBatchState.gid === g.gid` (NOT `currentGid` — that gets nulled when detail closes for reader). When same-gallery: restores `fetchedDetailPages`, `pageEntries`, `pagesPerBatch`, `showkey`, `totalPageCount` from the store; skips all resets; `fetchBatch(0)` is a no-op; renders instantly. `currentGid` alone cannot be used because it's set to null in the `g=null` branch before the reader sets `$detailGallery = source`.
  - **`totalPageCount` authority:** Set once in `fetchBatch` when `result.total_pages > totalPageCount` (from HTML parse). Stored in `detailBatchState.totalPageCount`. Restored from store on same-gallery re-open. `buildReaderPages` prefers `detailBatchState.totalPageCount` first (most up-to-date after round-trips), then local `totalPageCount`, then `gallery.file_count`. Never recalculated from loaded entry count. `GalleryReader.fetchStripBatch` must never write `totalPageCount` into `detailBatchState`.
  - **Reader page list (no fetch-all):** `handleRead`/`handleOpenPage` call `buildReaderPages()` which builds a dense `pages[]` of length `totalPageCount`, filling unloaded slots with stubs `{page_url:"", ...}`. `total_pages` is always `totalPageCount` (from HTML), never `pageEntries.length`. `get_gallery_pages` (fetch-all) is no longer used.
  - **Thumbnail cache preservation:** When detail closes due to reader open, `detailPageThumbs` and `detailBatchState` are NOT wiped. On return from reader, `detailPageThumbs.gid` check restores `pageThumbPaths` by spreading the FULL store paths record (`{ ...existingThumbs.paths }`), which includes any paths written by the reader's strip during the session — not just the detail's own downloads. This prevents redundant re-fetches of thumbs already loaded by the reader. Both reads use `get()` (non-reactive).
  - **Store update discipline:** `pageThumbs` service updates `detailPageThumbs` in-place on each download. `detailBatchState` scalar fields updated via spread. Both avoid wholesale replacement to prevent spurious re-renders.
  - **Cancellation:** Tracks `alive` flag. On close/destroy: disconnects all IntersectionObservers, clears `setThumbReadyCallback(null)`. `setActiveDetailGallery(null)` and `detailBatchState.set(null)` only called when reader is NOT open — when reader opens, both are preserved so the reader inherits the active slot and can call `setActiveDetailGallery(gid)` without a race condition. In-flight downloads in the `pageThumbs` service are dropped via gid mismatch when the gallery changes.
  - Action buttons: Read, Download*, Favorite*, Rate*, Search Similar* (*disabled placeholders). Escape collapses (full-page) or closes (panel).

## GalleryReader
- **Props:** none (uses reader stores)
- **Events:** none
- **Used by:** `+page.svelte`
- **Notes:** Full-screen overlay (z-1000). Preloads current + next 3 + prev 1 images in page mode (showpage API fast path). In scroll mode, IntersectionObserver (rootMargin 200px) loads current + next 2 as user scrolls — never prefetches entire gallery. Page/scroll mode toggle. Uses i18n. All image state stored as `$state<Record>`. Tracks `alive` flag and `activeGid` — all state cleared on gallery change. Download cancellation: calls `registerDownloadSession` on gallery open, `cancelImageDownloads` on close/destroy/gallery change. `alive` flag prevents any new downloads after destroy.
- **Total page count:** `totalPages = gallery.total_pages` — set from `buildReaderPages` which prefers `detailBatchState.totalPageCount` (set once from HTML parse, never clobbered by reader). Never recalculated from loaded entry count. Strip and scroll view both iterate `{ length: totalPages }`, not `gallery.pages`.
- **`readerOpenCount`:** Local `$state` counter incremented each time a gallery is opened (in the gallery-change `$effect`). Read as a dependency in the strip sentinel setup `$effect` to force re-run on every open, even when `showControls` is already true and `activeGid` is unchanged.
- **`setActiveDetailGallery` ownership:** The reader calls `setActiveDetailGallery(gid)` in the gallery-change `$effect` when a new gallery opens. This overrides the null set by the detail's g=null branch (which fires before the reader's effect), ensuring `get_page_thumbnail` and `get_gallery_pages_batch` are not cancelled while the reader is open. On close back to detail, the detail's open effect calls `setActiveDetailGallery(g.gid)` and takes ownership back. On close to home/search, reader's `handleClose` calls `setActiveDetailGallery(null)` explicitly.
- **Strip sentinel `totalPageCount` discipline:** `fetchStripBatch` updates only `pagesPerBatch` and `showkey` in `detailBatchState` — never `totalPageCount`. This prevents stale spreads from clobbering the authoritative count on round-trips.
- **Loading indicator (page mode):** Centered arc spinner shown while current page is loading or not yet queued. SVG `<circle>` with `stroke-dasharray: 84.8 28.3` (~270° filled arc, ~90° gap), `stroke: var(--accent)` for filled arc, faint white track. 44px diameter, 4px stroke-width, `stroke-linecap: round`. Rotates continuously via `@keyframes arc-rotate` (`transform: rotate(360deg)` on the SVG). Disappears instantly when image loads (no fade). Scroll mode still uses pulsing `.skeleton-rect.tall` placeholder.
- **Click zones (page mode):** Viewport divided into 3 horizontal zones covering full viewport height. Left third = prev page, right third = next page, center third = toggle header/toolbar visibility. Zones are invisible (no buttons or borders) — large tap targets. No visible prev/next buttons.
- **Controls visibility:** Default hidden on open (full immersive view). Center-zone click is the only toggle — no mouse-move trigger, no auto-hide timer. Header and toolbar always show/hide together.
- **Header/toolbar transitions:** `{#if showControls}` blocks with `transition:slide={{ duration: 200, axis: 'y' }}`. Top bar slides down from top edge; bottom bar slides up from bottom edge.
- **Bottom toolbar layout:** `flex-direction: column`, padding `0.875rem 1.25rem`, `padding-bottom: max(0.875rem, env(safe-area-inset-bottom))`. Inner structure: (1) `.thumb-strip` — horizontal scrollable preview strip (see below). (2) `.slider-row` — flex row with Slider + `.pct-label`. (3) `.page-label` — centered page counter.
- **Preview thumbnail strip (page mode only):** Horizontal scrollable row of page thumbnails sitting above the slider. 74px tall, 52×66px per thumbnail, 4px gap. Scrollbar hidden. Active page: full opacity, accent border, `scale(1.08)`. Others: 45% opacity, dimmed. Auto-scrolls to keep current page centered (`scrollTo` with `behavior: smooth`) on every page change. Click any thumbnail → `goToPage(idx)`. **Iterates ALL totalPages** (not just loaded entries) — renders a button stub for every page. **Batch loading:** IntersectionObserver on `data-strip-sentinel` attributes — queried inside `stripEl` via `stripEl.querySelector(...)` so `root: stripEl` is valid. `rootMargin: "0px 600px 0px 600px"` pre-fetches ~10 pages ahead horizontally. Sentinels only set up when `showControls=true` (strip rendered) AND `stripEl` is bound — never from gallery-change effect (strip not in DOM yet). **Each batch gets one observer that watches all thumbnails in that batch range `[dp*ppb, dp*ppb+ppb)`** — any thumbnail becoming visible triggers the fetch, not just the first one. This prevents missed fetches when the user scrolls into the middle of an unfetched batch. When any thumbnail for batch dp enters view, `fetchStripBatch(gid, dp)` fetches that batch, merges entries into `gallery.pages[]` and `detailBatchState.pageEntries`, marks `fetchedDetailPages.add(dp)`, then enqueues thumbnails for all pages in that batch. Observers re-setup after each batch. If detail page already fetched the batch, `syncBatchStateToGallery` merges entries without IPC **and** immediately enqueues thumbnails for all pages in that batch (same range `[dp*ppb, dp*ppb+ppb)`) — this ensures thumbs load even when the IPC path was skipped. **Thumbnail source:** `detailPageThumbs` + `detailBatchState` shared stores. `enqueueThumb` checks shared paths first, then merges entry from `detailBatchState.pageEntries` into `gallery.pages[pageIdx]` if the slot is missing `page_url` OR `thumb_url` (both conditions checked — `GalleryPageEntry.thumb_url` is nullable, so a DB-sourced entry may have `page_url` but null `thumb_url`; the wider guard ensures `detailBatchState`'s richer entry always fills missing fields), then IPC. **Thumbnail downloads:** delegated to the `pageThumbs` service (`enqueuePageThumb`). Reader registers `setThumbReadyCallback` on gallery open to write results into `thumbPaths` (converted with `convertFileSrc`); clears callback on gallery close and `onDestroy`. Loads ±5 around current page on strip show; re-setup observers on strip scroll (300ms debounce). Unloaded thumbnails show pulsing `.thumb-skeleton`. **Mouse wheel:** non-passive `wheel` listener scrolls strip left/right. **Diagnostic logs:** `STRIP_OBSERVERS_SETUP`, `STRIP_SENTINEL_VISIBLE`, `STRIP_FETCH_BATCH`, `STRIP_FETCH_DONE`.
- **Thumbnail download discipline:** Owned by `pageThumbs` service. Components only call `enqueuePageThumb` and receive results via `setThumbReadyCallback`. Callback is cleared on gallery close and component destroy to prevent stale writes.
- **`stripFetchInFlight` map:** `Map<detailPage, Promise<void>>`. `fetchStripBatch` is a thin wrapper that checks this map first and returns the existing promise if one is already in flight for that batch. `_fetchStripBatchImpl` does the actual IPC work. `loadImage` calls `fetchStripBatch` (not the impl directly) when a page's batch hasn't been fetched yet — this ensures IntersectionObserver-triggered fetches and image-load-triggered fetches share the same promise and never duplicate the IPC call. Map is cleared on gallery change.
- **Stub-page image loading:** If `loadImage` is called for a page whose batch hasn't been fetched yet (`entry.page_url` empty after checking `detailBatchState`), it awaits `fetchStripBatch(gid, dp)` for that page's batch (dp = `Math.floor(pageIdx / pagesPerBatch)`), then re-reads the entry. If `page_url` is still empty after the batch fetch (e.g. cancelled), the load silently returns rather than calling `get_gallery_image` with an empty URL.
- **Navigation flow:** Grid → Detail → Reader → (back/Escape) → Detail → (back/Escape) → Grid. When the reader is opened from GalleryDetail, the source `Gallery` object is saved in `readerSourceGallery` store. On close to detail: `detailBatchState` preserved, `detailGallery` restored, batch observers disconnected. On close to home/search (no source): `setActiveDetailGallery(null)` + `detailBatchState.set(null)` cancel all pending downloads.

## FilterPanel
- **Props:** `{ onClose: () => void, onSort?: () => void }`
- **Used by:** `GalleryGrid.svelte`
- **Notes:** Client-side hide filter + sort scope UI for the home page. Fields: tags include/exclude, categories (checkbox grid), min rating, pages range, language, uploader. "Apply" commits to `homeFilter` store and calls `onClose`. "Clear" resets `homeFilter` and calls `onClose`. "Sort" commits `homeFilter` + writes `sortScope` store, then calls `onSort` (triggers `handleSort` in GalleryGrid) + `onClose`. Form pre-populated from current stores on mount.
- **Sort section (Sorted By):**
  - **Field selector:** dropdown — Date posted / Rating / Page count / Title. Direction toggle (Asc/Desc).
  - **Scope tabs:** "Gallery count" | "Date range" (mutually exclusive).
  - **Count mode:** Preset buttons (100, 250, 500, 1000) + free-form number input "Sort across ___ galleries". Hint shows whether fetch is needed.
  - **Date mode:** "Last ___ days" number input. Fetches until oldest gallery in latest batch is older than cutoff.
- **Sort state local to form:** `sortScopeMode`, `sortCount`, `sortDays`, `sortField`, `sortDir`. Written to `sortScope` store only on Sort click.

## Slider
- **Props:** `{ min?: number (default 0), max?: number (default 100), step?: number (default 1), value: number (bindable), disabled?: boolean, onChange?: (value: number) => void }`
- **Events:** none (mutations via bindable `value` prop + optional `onChange` callback)
- **Used by:** `GalleryDetail.svelte` (preview size), `GalleryReader.svelte` (page navigation), `SettingsPage.svelte` (card size, detail preview size)
- **Notes:** Fully custom range slider — no browser default styling (`-webkit-appearance: none`). Track: 9px tall pill. Filled portion (left of thumb) uses `--accent`; unfilled portion uses `--bg-elevated`. Thumb: 20px circle, `--bg-primary` fill, `--accent` 2.5px border, drop shadow. Focus ring: `--accent-subtle` glow. Hover/active states on thumb. Works in both light and dark themes via existing design tokens. `.slider-wrap` root element has `flex: 1; min-width: 0` — place inside a flex row for correct sizing. For fixed-width context, wrap in a `div` with explicit `width`. Fill percentage computed via `$derived` from `(value - min) / (max - min)` injected as CSS `--fill` custom property on the input element.

## TagInputAutocomplete
- **Props:** `{ includeTags: TagSuggestion[] (bindable), excludeTags: TagSuggestion[] (bindable), onFocus?: () => void }`
- **Events:** none (mutations via bindable props)
- **Used by:** `SearchPage.svelte`, `FilterPanel.svelte`
- **Notes:** Self-contained tag input with autocomplete dropdown + include/exclude chip display. Calls `search_tags_autocomplete` IPC with 200ms debounce. Dropdown shows namespace:name rows; left click = include, right button (−) = exclude. Chips support +/− prefix toggle (include↔exclude) and × remove. Exposes `clear()` method to reset all state. `onFocus` callback fires on input focus (used by SearchPage to close history dropdown). Styles are scoped to the component. Chips have `margin-top: 0.4rem` gap from the input row.

## SettingsPage
- **Props:** none
- **Used by:** `+page.svelte` (settings nav)
- **Notes:** Left nav + content area. Sections: Account, Theme, Preference, Storage, Network, Downloads, About.

## HistoryPage
- **Props:** none
- **Used by:** `+page.svelte` (history nav)
- **Notes:** Uses `get_reading_history` IPC. Load-more pagination. Uses i18n.

## SearchPage
- **Props:** none
- **Used by:** `+page.svelte` (search nav)
- **Stores:** `stores/search.ts` (searchResults, searchQuery, searchNextUrl, searchHasMore, searchLoading, searchCategoryMask, searchAdvancedOptions, searchHideFilter, searchIncludeTags, searchExcludeTags)
- **IPC:** `search_exhentai`, `get_search_history`, `clear_search_history`, `download_thumbnails_for_gids`, `search_tags_autocomplete`
- **Events:** `thumbnail-ready`, `gallery-enriched` (both update master list AND `sortedUnfiltered` so sorted view stays current)
- **Sections (top to bottom):**
  1. Combined input row: search bar (flex:3, ~60%) + `TagInputAutocomplete` (flex:2, ~40%) side-by-side + Search button at far right. History dropdown opens under search bar focus and spans full combined width of both inputs (right offset = `calc(-66.6667% - 0.5rem)`). Tag autocomplete dropdown opens under tag input focus and matches only the tag input width. Only one dropdown is open at a time: focusing search bar closes tag dropdown; focusing tag input (`onFocus` prop) closes history dropdown. Clear (×) buttons are absolutely positioned inside each input wrapper at `right: 0.5rem`.
  2. Tag chips row (inside `TagInputAutocomplete`, shown when tags present): green (+) include chips and red (−) exclude chips; click mode prefix to toggle include↔exclude; "×" to remove
  3. Category exclusion chips (bitmask)
  4. Advanced toggle → expanded panel: search_name, search_tags, search_description, show_expunged, search_torrent_filenames, only_with_torrents, search_low_power_tags, search_downvoted_tags (all checkboxes); minimum_rating dropdown (Any/2★–5★); pages-between range inputs
- **Query building:** `buildSearchQuery(freeText)` combines free text + all include tags as `namespace:"name"` + all exclude tags as `-namespace:"name"`, joined with spaces. This combined string is passed as `query` to `search_exhentai` and saved to search history, but is **never stored back** into `$searchQuery`. `$searchQuery` holds free text only. The combined string is kept in a local `lastCombinedQuery` variable for pagination.
- **History recall:** `parseSearchString(raw)` parses a full history string back into `{ freeText, includeTags, excludeTags }`. Regex handles `namespace:tag`, `namespace:"multi word"`, `namespace:"exact$"`, and minus-prefixed exclusions. Clicking a history entry calls this, sets `$searchIncludeTags`/`$searchExcludeTags` to the parsed chips, puts `freeText` into the search input, and executes the search. The search bar shows only free text; tags appear as chips.
- **Tag click from GalleryCard:** Adds the tag to `searchIncludeTags` (green chip) and navigates to search, instead of setting raw text in the search bar.
- **Notes:** Results in VirtualGrid/VirtualList. Infinite scroll via cursor-based pagination. Cooldown guard (`SEARCH_COOLDOWN_MS = 2000`). Gid deduplication. Client-side hide filter. Viewport-driven thumbnail downloading.
- **Sort feature:** Inline sort bar shown below toolbar when `hasSearched && !$searchSortActive`. Controls: field dropdown (Date/Rating/Pages/Title), direction toggle (Asc/Desc), preset buttons (100/250/500/1000), free-form count input, Sort button. `handleSearchSort()` follows the same 4-step pattern as GalleryGrid: **(1) Fetch** — if `$searchResults.length < count`, fetch via `searchExhentai` with current `lastCombinedQuery`/`$searchNextUrl`/`$searchCategoryMask`/`$searchAdvancedOptions` until count reached or `$searchHasMore` false; uses `get(searchResults)` after each await to avoid stale closure. **(2) Sort** — `applySortToList(get(searchResults).slice(0, count), ...)` → stored in local `sortedUnfiltered`. **(3) Render** — set `searchSortActive=true`; `$effect` watching `searchSortActive`+`sortedUnfiltered`+`debouncedHideFilter` writes `$searchSortedGalleries`. **(4)** Hide filter changes reactively re-filter `sortedUnfiltered`. Progress overlay same as GalleryGrid (position: absolute, inset 0, z-index 50). Sort banner: "Sorted by {field} · {count} results" + Clear button. `onScrollNearEnd` no-op while sort active. Sort state cleared on new search execution. No date-range mode (count-only).

## PlaceholderPage
- **Props:** `{ titleKey: string, messageKey: string, icon?: string }`
- **Used by:** `+page.svelte` (popular, favorites, watched, downloads pages)

## Performance patterns
- **CSS containment:** All cards/list items use `contain: layout style paint` + `content-visibility: auto`.
- **Scroll throttling:** VirtualGrid/VirtualList use `requestAnimationFrame`.
- **Store updates:** New array references on every store update (spread syntax). Svelte 5 reactivity requires distinct references for derived/prop propagation.
- **Flat hover:** Cards use border-color + background shift only — no shadows or transforms.
- **will-change: transform:** Only on virtual scroll containers.
- **Detail close guard:** Progress refreshed only on detail close transition.
- **Fixed card height:** GalleryCard info section uses fixed height 8.5rem. VirtualGrid `rowHeight` must match: `cardSize * (4/3) + 138` (image aspect 3:4 + info section + border).
- **Absolute positioned visible container:** VirtualGrid/VirtualList visible items container is `position: absolute`, preventing layout jumps when items stream in or total height changes.
- **Padding-aware scroll math:** Total height includes +64px for 2rem top+bottom padding. Scroll position adjusted by -32px for top padding. OffsetY includes +32px for top padding. Column count uses `containerWidth - 64` (subtracts horizontal padding). CSS horizontal padding is `2rem`.
- **Virtual scroll buffer:** VirtualGrid ≥8 rows, VirtualList ≥20 items.
- **Gap-aware row stride:** `rowStride = rowHeight + gap`.
- **Stable {#each} keys:** `item.gid` as key.
- **Image retry on error:** GalleryCard/GalleryListItem retry failed image loads up to 3 times with increasing delay. Uses reactive state (not direct DOM manipulation) to avoid conflicts with Svelte's binding.
- **Gap-aware column count:** VirtualGrid column calculation uses `floor((gridWidth + gap) / (minWidth + gap))` to match CSS `auto-fill` behavior.
- **Scroll container sizing:** VirtualGrid/VirtualList scroll containers use `min-height: 0` (required for flex children with overflow-y: auto to actually scroll instead of expanding).
- **Scroll container isolation:** VirtualGrid/VirtualList scroll containers use `contain: strict` (implies size+layout+style+paint) so scrolling never triggers layout recalculation or repaint outside the scroll area. Safe because the container is sized by flex parent, not its children.
- **Header z-index above scroll container:** SearchPage `.search-header` uses `position: relative; z-index: 10` to stack above the `contain: strict` scroll area and allow the history dropdown (position: absolute) to overflow visibly over the results. Do NOT use `contain: layout/paint` on headers that have overflow dropdowns — paint containment clips absolutely-positioned children.
- **Button transitions — no box-shadow:** Toolbar buttons (GalleryGrid, SearchPage) use `transition: background 0.1s, color 0.1s` only. No `box-shadow` in transitions — box-shadow animation triggers repaint every frame.
- **Reactive scroll-end re-check:** VirtualGrid/VirtualList use `$effect` on `items.length` and `recheckTrigger` to re-check `distFromBottom` after items are appended or sync completes. Prevents infinite scroll from stalling when user is already near the bottom and stopped scrolling. Uses double-`requestAnimationFrame` to defer until DOM has laid out new content (prevents runaway fetch loops that occurred with `queueMicrotask`).
- **Footer as virtual row:** Footer is +1 extra row in the virtual item count but uses a small fixed height (`FOOTER_HEIGHT = 64px`) instead of `rowStride`. `totalRows = dataRows + 1` (grid) / `totalCount = dataCount + 1` (list). Spacer height = `dataHeight + footerH + 64` (footer height is independent of row stride). Footer positioned with `transform: translateY(footerY)`, height = `FOOTER_HEIGHT`. `footerVisible` derived checks if footer row falls within visible range. All values are `$derived`.
- **Detail state:** Detail page thumb paths and reader image state use `$state<Record>` with direct property assignment (Svelte 5 proxy detects property writes automatically — no need for explicit reassignment).
