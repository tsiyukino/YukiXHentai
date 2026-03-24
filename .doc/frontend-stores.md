# Frontend Stores
> Last updated: 2026-03-23 (sort stores added) | Affects: src/lib/stores/

## Auth stores (stores/auth.ts)

### isLoggedIn / authLoading / authMessage
- Standard auth state. See Phase 1.

## Gallery stores (stores/galleries.ts)

### galleries / totalCount / syncing / syncMessage / browsePage
- Standard gallery browse state. In home mode, `galleries` is populated directly from `sync_next_page` return values — each call returns a batch of Gallery objects which are appended to the array. No DB reload, no streaming events. Thumbnails and enrichment update individual entries in-place via `thumbnail-ready` and `gallery-enriched` event listeners.

### syncProgress
- **Shape:** `Writable<SyncProgress | null>`
- **Used by:** `GalleryGrid.svelte`

### activeFilter / activeSort / searchActive
- Search/filter state. See Phase 3.

### quickFilter
- **Shape:** `Writable<string>`
- **Default:** `""`
- **Used by:** `GalleryGrid.svelte`
- **Notes:** Text-based hide-filter term for the home gallery grid. Pure view-layer exclusion applied before `homeFilter` in the `filteredGalleries` derivation. Excludes galleries matching the term (case-insensitive substring on title, title_jpn, category, uploader, tags). Zero effect on loading, pagination, or thumbnail fetching. Cleared when entering search mode. Persisted in store across nav switches within session.

### homeFilter
- **Shape:** `Writable<HomeFilterState>`
- **Type:** `{ tagsInclude: TagFilter[], tagsExclude: TagFilter[], categories: string[], ratingMin: number | null, pagesMin: number | null, pagesMax: number | null, language: string, uploader: string }`
- **Default:** all empty / null (via `emptyHomeFilter()`)
- **Used by:** `GalleryGrid.svelte`, `FilterPanel.svelte`
- **Notes:** Criteria-based hide filter for the home gallery grid. Applied after `quickFilter` in the `filteredGalleries` derivation. Pure view-layer: no IPC, no DB queries. `isHomeFilterActive(f)` returns true if any field is set. Applied only in home mode (not search mode). Persisted across nav switches within session. `FilterPanel` reads initial state from this store on mount and writes back on Apply/Clear.

### sortScope
- **Shape:** `Writable<SortScope>`
- **Type:** `{ mode: 'count' | 'days', count: number, days: number, field: SortField, dir: 'asc' | 'desc' }`
- **Default:** `{ mode: 'count', count: 0, days: 30, field: 'posted', dir: 'desc' }`
- **Used by:** `FilterPanel.svelte`, `GalleryGrid.svelte`
- **Notes:** Written by FilterPanel on Sort click. Count=0 means use current `$galleries.length`. GalleryGrid reads this to drive fetch loop and sort.

### sortFetchProgress
- **Shape:** `Writable<SortFetchProgress>`
- **Type:** `{ fetching: bool, loaded: number, target: number, estimatedSeconds: number, cancelled: bool }`
- **Default:** all zero/false
- **Used by:** `GalleryGrid.svelte`
- **Notes:** While `fetching` is true, GalleryGrid shows the progress overlay. Updated after each batch during the pre-sort fetch phase. Reset to default when fetch completes or is cancelled.

### sortActive
- **Shape:** `Writable<boolean>`
- **Default:** `false`
- **Used by:** `GalleryGrid.svelte`
- **Notes:** When true, VirtualGrid/VirtualList renders `sortedGalleries` instead of `filteredGalleries`. `onScrollNearEnd` is a no-op while true (infinite scroll paused). Sort banner shown above grid.

### sortedGalleries
- **Shape:** `Writable<Gallery[]>`
- **Default:** `[]`
- **Used by:** `GalleryGrid.svelte`
- **Notes:** Final sorted snapshot rendered in sorted view. Client-side `homeFilter`+`quickFilter` applied first, then sorted by `sortScope.field`/`dir`. Set atomically after fetch phase. Never mutated incrementally.

## Reader stores (stores/reader.ts)

### readerGallery
- **Shape:** `Writable<GalleryPages | null>`
- **Used by:** `GalleryDetail.svelte`, `GalleryReader.svelte`, `+page.svelte`

### readerPage / readerMode / readerSessionId
- Reader navigation state.

## Navigation stores (stores/navigation.ts)

### currentPage
- **Shape:** `Writable<NavPage>`
- **Type:** `"home" | "search" | "popular" | "favorites" | "watched" | "history" | "downloads" | "settings"`
- **Used by:** `Sidebar.svelte`, `+page.svelte`

### sidebarCollapsed
- **Shape:** `Writable<boolean>`
- **Used by:** `Sidebar.svelte`

## Page thumbnail service (stores/pageThumbs.ts)

### enqueuePageThumb / resetPageThumbs / setThumbReadyCallback
- **Exports:** `enqueuePageThumb(gid, pageIdx, thumbUrl)`, `resetPageThumbs(gid)`, `setThumbReadyCallback(cb | null)`
- **Used by:** `GalleryDetail.svelte`, `GalleryReader.svelte`
- **Notes:** Singleton service owning the concurrent page-thumbnail download queue (up to 6 in flight — matches JHentai/EhViewer safe concurrency for ehgt.org). Replaces the duplicate `processDownloadQueue`/`downloadThumb` logic that previously existed in both components. On success, writes raw path to `detailPageThumbs` store in-place, then invokes the registered `onReady(pageIdx, rawPath)` callback. Only one callback active at a time (last `setThumbReadyCallback` call wins — Detail registers on gallery open, Reader registers on gallery open, both clear on close/destroy). `resetPageThumbs(gid)` sets the active gid and drops all queued (not yet started) downloads; in-flight downloads for the old gid resolve but are discarded via gid mismatch check. `enqueuePageThumb` deduplicates against the in-flight set and queue.

## Detail stores (stores/detail.ts)

### detailGallery
- **Shape:** `Writable<Gallery | null>`
- **Used by:** `GalleryGrid.svelte`, `GalleryDetail.svelte`, `+page.svelte`
- **Notes:** Non-null when detail panel is open.

### detailPageThumbs
- **Shape:** `Writable<{ gid: number; paths: Record<number, string> } | null>`
- **Used by:** `GalleryDetail.svelte`, `GalleryReader.svelte`
- **Notes:** Raw filesystem paths (not convertFileSrc'd) for page thumbnails. Shared between detail and reader so neither re-fetches the other's work. NOT wiped when detail closes due to reader opening.

### detailBatchState
- **Shape:** `Writable<{ gid, token, showkey, pagesPerBatch, totalPageCount, fetchedDetailPages: Set<number>, pageEntries: Record<number, GalleryPageEntry> } | null>`
- **Used by:** `GalleryDetail.svelte`, `GalleryReader.svelte`
- **Notes:** Shared batch-loading state. `fetchedDetailPages` and `pageEntries` are the SAME object references as the detail's local variables — mutated in-place by both sides. Scalar fields (`pagesPerBatch`, `showkey`, `totalPageCount`) updated via `detailBatchState.set({...bs, ...})`. `totalPageCount` is set once from the first batch HTML parse — never recalculated from loaded entries. **`totalPageCount` is write-authoritative only in `GalleryDetail.fetchBatch` — `GalleryReader.fetchStripBatch` must never write it.** Used to detect same-gallery re-open (gid match) and restore `totalPageCount` without re-fetching. NOT wiped when detail closes due to reader opening; wiped when both close (no reader open). Reset on new gallery open.

## UI stores (stores/ui.ts)

### viewMode
- **Shape:** `Writable<"cards" | "list">`
- **Used by:** `GalleryGrid.svelte`, `SettingsPage.svelte`

### cardSize
- **Shape:** `Writable<number>`
- **Default:** 165
- **Used by:** `GalleryGrid.svelte`, `SettingsPage.svelte`

### detailExpanded
- **Shape:** `Writable<boolean>`
- **Default:** false
- **Used by:** `GalleryDetail.svelte`, `+page.svelte`
- **Notes:** Controls whether detail panel is in collapsed (side panel) or expanded (full-page) mode. When true, `+page.svelte` hides page content and passes `fullPage=true` to `GalleryDetail`, which renders inline filling the entire content area.

### detailPreviewSize
- **Shape:** `Writable<number>`
- **Default:** 120
- **Range:** 80–200
- **Used by:** `GalleryDetail.svelte`, `SettingsPage.svelte`
- **Notes:** Preview thumbnail size in px. Persisted to config via `set_detail_preview_size` IPC.

### theme
- **Shape:** `Writable<Theme>`
- **Type:** `"light" | "dark"`
- **Default:** `"light"`
- **Used by:** `+page.svelte`, `Sidebar.svelte`, `SettingsPage.svelte`
- **Notes:** Color theme. Loaded from config on mount. Synced to `data-theme` attribute on `<html>` via `$effect`. Persisted via `set_theme` IPC.

## Search stores (stores/search.ts)

### searchResults
- **Shape:** `Writable<Gallery[]>`
- **Default:** `[]`
- **Used by:** `SearchPage.svelte`
- **Notes:** Galleries from ExHentai server-side search. Completely independent from `galleries` store (home page).

### searchQuery
- **Shape:** `Writable<string>`
- **Default:** `""`
- **Used by:** `SearchPage.svelte`, `Sidebar.svelte`, `GalleryCard.svelte`, `GalleryListItem.svelte`
- **Notes:** **Free text only.** Never contains tag syntax (namespace:tag). Set by sidebar search input or tag click. SearchPage reads on mount to populate the text input. The full combined f_search string (free text + tag chips) is built at search execution time and never stored back here. `searchIncludeTags` and `searchExcludeTags` are always separate.

### searchNextUrl
- **Shape:** `Writable<string | null>`
- **Default:** `null`
- **Used by:** `SearchPage.svelte`
- **Notes:** Full URL from #unext href for cursor-based pagination. Null on first search or when no more pages. Passed back to `search_exhentai` IPC for subsequent page fetches.

### searchHasMore / searchLoading
- Search pagination state. `searchHasMore` derived from presence of #unext in last result.

### searchCategoryMask
- **Shape:** `Writable<number>`
- **Default:** `0` (show all)
- **Notes:** ExHentai f_cats bitmask. Each bit excludes a category.

### searchAdvancedOptions
- **Shape:** `Writable<AdvancedSearchOptions>`
- **Default:** `{ search_name: true, search_tags: true, search_description: false, show_expunged: false, search_torrent_filenames: false, only_with_torrents: false, search_low_power_tags: false, search_downvoted_tags: false, minimum_rating: null, min_pages: null, max_pages: null }`

### searchIncludeTags
- **Shape:** `Writable<TagSuggestion[]>` where `TagSuggestion = { namespace: string, name: string }`
- **Default:** `[]`
- **Notes:** Tags to include in search. Appended to f_search as `namespace:"name"` (space-joined). Set by tag chip UI and by GalleryCard tag clicks.

### searchExcludeTags
- **Shape:** `Writable<TagSuggestion[]>`
- **Default:** `[]`
- **Notes:** Tags to exclude from search. Appended to f_search as `-namespace:"name"` (space-joined).

### searchHideFilter
- **Shape:** `Writable<string>`
- **Default:** `""`
- **Notes:** Client-side hide filter for search results. Same pattern as quickFilter for home page.

### searchSortScope
- **Shape:** `Writable<SearchSortScope>`
- **Type:** `{ count: number, field: SearchSortField, dir: 'asc' | 'desc' }`
- **Default:** `{ count: 100, field: 'posted', dir: 'desc' }`
- **Used by:** `SearchPage.svelte`
- **Notes:** Committed to store on Sort click. Count defaults to 100. No date-range mode.

### searchSortFetchProgress
- **Shape:** `Writable<SearchSortFetchProgress>`
- **Type:** `{ fetching: bool, loaded: number, target: number, estimatedSeconds: number }`
- **Default:** all zero/false
- **Used by:** `SearchPage.svelte`

### searchSortActive
- **Shape:** `Writable<boolean>`
- **Default:** `false`
- **Used by:** `SearchPage.svelte`
- **Notes:** When true, VirtualGrid/VirtualList renders `searchSortedGalleries`. Infinite scroll paused. Sort bar hidden; sort banner shown. Auto-cleared on new search.

### searchSortedGalleries
- **Shape:** `Writable<Gallery[]>`
- **Default:** `[]`
- **Used by:** `SearchPage.svelte`
- **Notes:** Written exclusively by `$effect` watching `searchSortActive` + local `sortedUnfiltered` + `debouncedHideFilter`. Never written directly by sort handler.
