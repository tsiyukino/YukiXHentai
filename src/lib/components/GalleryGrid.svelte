<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { t } from "$lib/i18n";
  import { syncNextPage, resetSyncCursor, searchGalleries, getGalleries, onThumbnailReady, startEnrichment, onGalleryEnriched, downloadThumbnailsForGids } from "$lib/api/galleries";
  import type { Gallery } from "$lib/api/galleries";
  import { getReadProgressBatch } from "$lib/api/reader";
  import type { ReadProgress } from "$lib/api/reader";
  import {
    galleries, totalCount, syncing,
    activeFilter, activeSort, searchActive,
    quickFilter,
    homeFilter, isHomeFilterActive,
    sortScope, sortFetchProgress, sortActive, sortedGalleries,
    emptySortFetchProgress,
  } from "$lib/stores/galleries";
  import type { HomeFilterState, SortField } from "$lib/stores/galleries";
  import { detailGallery } from "$lib/stores/detail";
  import { viewMode, cardSize } from "$lib/stores/ui";
  import GalleryCard from "./GalleryCard.svelte";
  import GalleryListItem from "./GalleryListItem.svelte";
  import FilterPanel from "./FilterPanel.svelte";
  import VirtualGrid from "./VirtualGrid.svelte";
  import VirtualList from "./VirtualList.svelte";

  const PAGE_SIZE = 25;
  const SKELETON_INITIAL = 16;
  const OFFLINE_TIMEOUT_MS = 3000;

  let initialLoading = $state(true);
  let loadingMore = $state(false);
  let syncingMore = $state(false);
  // True while any syncNextPage call is in-flight (prevents concurrent calls).
  let syncInFlight = $state(false);
  let hasMoreRemote = $state(true);
  let isOffline = $state(false);
  let filterOpen = $state(false);
  let progressMap = $state<Map<number, ReadProgress>>(new Map());
  // Incremented after sync/load completes to trigger scroll-proximity recheck
  // in VirtualGrid/VirtualList.
  let recheckTrigger = $state(0);

  // Search mode uses DB pagination — track whether more DB results exist.
  let hasMoreSearchResults = $state(true);

  // Sort fetch cancellation token — incremented on cancel to abort the loop.
  let sortCancelToken = $state(0);

  // Sorted-but-unfiltered snapshot — kept so homeFilter/quickFilter changes can
  // re-derive $sortedGalleries (step 3) without re-sorting or re-fetching.
  let sortedUnfiltered = $state<Gallery[]>([]);

  // Quick visibility filter: debounced term applied client-side.
  let quickFilterInput = $state($quickFilter);
  let debouncedFilter = $state($quickFilter);
  let filterDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Sync quickFilterInput back to store for persistence across nav.
  $effect(() => { $quickFilter = debouncedFilter; });

  function onQuickFilterInput(value: string) {
    quickFilterInput = value;
    if (filterDebounceTimer) clearTimeout(filterDebounceTimer);
    filterDebounceTimer = setTimeout(() => {
      debouncedFilter = value;
    }, 250);
  }

  /** Check if a gallery matches the hide-filter term (should be hidden). */
  function matchesHideFilter(gallery: Gallery, term: string): boolean {
    const lower = term.toLowerCase();
    if (gallery.title.toLowerCase().includes(lower)) return true;
    if (gallery.title_jpn?.toLowerCase().includes(lower)) return true;
    if (gallery.category.toLowerCase().includes(lower)) return true;
    if (gallery.uploader?.toLowerCase().includes(lower)) return true;
    for (const tag of gallery.tags) {
      if (tag.name.toLowerCase().includes(lower)) return true;
      if (tag.namespace.toLowerCase().includes(lower)) return true;
      const full = `${tag.namespace}:${tag.name}`.toLowerCase();
      if (full.includes(lower)) return true;
    }
    return false;
  }

  /** Returns true if gallery should be HIDDEN by homeFilter. */
  function hiddenByHomeFilter(gallery: Gallery, f: HomeFilterState): boolean {
    // Tags include: must have ALL
    for (const tag of f.tagsInclude) {
      const has = gallery.tags.some(t => t.namespace === tag.namespace && t.name === tag.name);
      if (!has) return true;
    }
    // Tags exclude: must not have ANY
    for (const tag of f.tagsExclude) {
      const has = gallery.tags.some(t => t.namespace === tag.namespace && t.name === tag.name);
      if (has) return true;
    }
    // Categories: if any selected, gallery.category must be in the set
    if (f.categories.length > 0 && !f.categories.includes(gallery.category)) return true;
    // Min rating
    if (f.ratingMin !== null && gallery.rating < f.ratingMin) return true;
    // Page count range
    if (f.pagesMin !== null && gallery.file_count < f.pagesMin) return true;
    if (f.pagesMax !== null && gallery.file_count > f.pagesMax) return true;
    // Language: match gallery tags where namespace="language"
    if (f.language.trim()) {
      const lang = f.language.trim().toLowerCase();
      const hasLang = gallery.tags.some(
        t => t.namespace === "language" && t.name.toLowerCase().includes(lang)
      );
      if (!hasLang) return true;
    }
    // Uploader: case-insensitive substring
    if (f.uploader.trim()) {
      const up = f.uploader.trim().toLowerCase();
      if (!gallery.uploader?.toLowerCase().includes(up)) return true;
    }
    return false;
  }

  /** Visible galleries: master list minus hidden ones. Pure view-layer transform. */
  let filteredGalleries = $derived.by(() => {
    const term = debouncedFilter.trim();
    const hf = $homeFilter;
    const hfActive = !$searchActive && isHomeFilterActive(hf);
    let list = !term ? $galleries : $galleries.filter(g => !matchesHideFilter(g, term));
    if (hfActive) list = list.filter(g => !hiddenByHomeFilter(g, hf));
    return list;
  });

  let unlisteners: (() => void)[] = [];

  onMount(async () => {
    // Listen for thumbnail-ready events — update the specific gallery's thumb_path.
    const unlistenThumb = await onThumbnailReady((event) => {
      const idx = $galleries.findIndex(g => g.gid === event.gid);
      if (idx >= 0) {
        const updated = [...$galleries];
        updated[idx] = { ...updated[idx], thumb_path: event.path };
        $galleries = updated;
      }
      // Also patch sortedUnfiltered so sorted view stays up-to-date.
      const sidx = sortedUnfiltered.findIndex(g => g.gid === event.gid);
      if (sidx >= 0) {
        const su = [...sortedUnfiltered];
        su[sidx] = { ...su[sidx], thumb_path: event.path };
        sortedUnfiltered = su;
      }
    });
    unlisteners.push(unlistenThumb);

    // Listen for enrichment events — update gallery metadata in-place when enriched.
    const unlistenEnriched = await onGalleryEnriched((event) => {
      const idx = $galleries.findIndex(g => g.gid === event.gallery.gid);
      if (idx >= 0) {
        const updated = [...$galleries];
        updated[idx] = { ...event.gallery, thumb_path: $galleries[idx].thumb_path ?? event.gallery.thumb_path };
        $galleries = updated;
      }
      // Also patch sortedUnfiltered so sorted view stays up-to-date.
      const sidx = sortedUnfiltered.findIndex(g => g.gid === event.gallery.gid);
      if (sidx >= 0) {
        const su = [...sortedUnfiltered];
        su[sidx] = { ...event.gallery, thumb_path: sortedUnfiltered[sidx].thumb_path ?? event.gallery.thumb_path };
        sortedUnfiltered = su;
      }
    });
    unlisteners.push(unlistenEnriched);

    if ($searchActive) {
      await loadSearchResults();
      initialLoading = false;
    } else {
      $galleries = [];
      await startFreshSync();
    }
  });

  onDestroy(() => {
    for (const unlisten of unlisteners) unlisten();
    if (filterDebounceTimer) clearTimeout(filterDebounceTimer);
    if (thumbDebounceTimer) clearTimeout(thumbDebounceTimer);
  });


  /** Home page startup: reset cursor, sync page 1 from ExHentai. */
  async function startFreshSync() {
    await resetSyncCursor();
    hasMoreRemote = true;
    isOffline = false;
    syncInFlight = true;

    const offlineTimer = setTimeout(async () => {
      if (initialLoading && $galleries.length === 0) {
        isOffline = true;
        try {
          const page = await getGalleries(0, PAGE_SIZE);
          if (page.galleries.length > 0) {
            $galleries = page.galleries;
            $totalCount = page.total_count;
            await loadProgress(page.galleries);
          }
        } catch { /* ignore */ }
        initialLoading = false;
      }
    }, OFFLINE_TIMEOUT_MS);

    try {
      const result = await syncNextPage();
      clearTimeout(offlineTimer);
      $galleries = result.galleries;
      hasMoreRemote = result.has_more;
      await loadProgress(result.galleries);
      initialLoading = false;
      startEnrichment().catch(() => {});
    } catch {
      clearTimeout(offlineTimer);
      isOffline = true;
      try {
        const page = await getGalleries(0, PAGE_SIZE);
        if (page.galleries.length > 0) {
          $galleries = page.galleries;
          $totalCount = page.total_count;
          await loadProgress(page.galleries);
        }
      } catch { /* ignore */ }
      initialLoading = false;
    } finally {
      syncInFlight = false;
      lastSyncEndTime = Date.now();
      recheckTrigger++;
    }
  }

  let consecutiveErrors = $state(0);
  const MAX_CONSECUTIVE_ERRORS = 5;
  // Minimum interval (ms) between auto-sync calls to prevent runaway fetch loops.
  const AUTO_SYNC_COOLDOWN_MS = 2000;
  let lastSyncEndTime = 0;

  /** Fetch a single batch from ExHentai. Appends returned galleries to the master list. */
  async function fetchOneBatch() {
    const result = await syncNextPage();
    $galleries = [...$galleries, ...result.galleries];
    hasMoreRemote = result.has_more;
    isOffline = false;
    consecutiveErrors = 0;
    await loadProgress(result.galleries);
    startEnrichment().catch(() => {});
  }

  /** Sync the next page when scrolling past current content. */
  async function autoSync() {
    if (syncingMore || syncInFlight || $syncing || isOffline) return;
    // Enforce cooldown to prevent runaway fetch loops from reactive rechecks.
    if (Date.now() - lastSyncEndTime < AUTO_SYNC_COOLDOWN_MS) return;
    syncingMore = true;
    syncInFlight = true;
    try {
      await fetchOneBatch();
    } catch (err) {
      console.error("[GalleryGrid] autoSync error:", err);
      consecutiveErrors++;
      if (consecutiveErrors >= MAX_CONSECUTIVE_ERRORS) {
        isOffline = true;
        hasMoreRemote = false;
      }
    } finally {
      syncingMore = false;
      syncInFlight = false;
      lastSyncEndTime = Date.now();
      recheckTrigger++;
      // The user was near the bottom when autoSync was triggered. After new
      // items are appended the spacer grows and no scroll event fires if the
      // user is stationary. Schedule a deferred recheck after the cooldown so
      // the VirtualGrid/VirtualList $effect fires again and can re-measure.
      if (hasMoreRemote && !isOffline) {
        setTimeout(() => {
          recheckTrigger++;
        }, AUTO_SYNC_COOLDOWN_MS + 50);
      }
    }
  }


  /** Search mode: load results from local DB with pagination. */
  async function loadSearchResults() {
    try {
      const page = await searchGalleries($activeFilter, $activeSort, 0, PAGE_SIZE);
      $galleries = page.galleries;
      $totalCount = page.total_count;
      hasMoreSearchResults = page.galleries.length >= PAGE_SIZE;
      await loadProgress(page.galleries);
    } catch (err) {
      console.error("Failed to load search results:", err);
    }
  }

  /** Search mode: load more results from DB. */
  async function loadMoreSearch() {
    if (loadingMore || !hasMoreSearchResults) return;
    loadingMore = true;
    try {
      const page = await searchGalleries($activeFilter, $activeSort, $galleries.length, PAGE_SIZE);
      $galleries = [...$galleries, ...page.galleries];
      $totalCount = page.total_count;
      hasMoreSearchResults = page.galleries.length >= PAGE_SIZE;
      await loadProgress(page.galleries);
    } catch (err) {
      console.error("Failed to load more search results:", err);
    } finally {
      loadingMore = false;
      recheckTrigger++;
    }
  }

  async function loadProgress(galleryList: Gallery[]) {
    if (galleryList.length === 0) return;
    try {
      const gids = galleryList.map(g => g.gid);
      const progresses = await getReadProgressBatch(gids);
      const newMap = new Map(progressMap);
      for (const p of progresses) {
        newMap.set(p.gid, p);
      }
      progressMap = newMap;
    } catch { /* ignore */ }
  }

  function handleScrollEnd() {
    // While sorted view is active, infinite scroll is paused.
    if ($sortActive) return;
    if ($searchActive) {
      loadMoreSearch();
    } else if (hasMoreRemote && !syncingMore && !syncInFlight && !$syncing) {
      if (isOffline && consecutiveErrors < MAX_CONSECUTIVE_ERRORS) {
        isOffline = false;
      }
      autoSync();
    }
  }

  /** Apply sort to a gallery list and return sorted copy. */
  function applySortToList(list: Gallery[], field: SortField, dir: "asc" | "desc"): Gallery[] {
    const sorted = [...list];
    sorted.sort((a, b) => {
      let cmp = 0;
      if (field === "rating") cmp = a.rating - b.rating;
      else if (field === "posted") cmp = a.posted - b.posted;
      else if (field === "pages") cmp = a.file_count - b.file_count;
      else if (field === "title") cmp = a.title.localeCompare(b.title);
      return dir === "asc" ? cmp : -cmp;
    });
    return sorted;
  }

  /** Filter galleries by "last N days" date range. */
  function filterByDays(list: Gallery[], days: number): Gallery[] {
    const cutoff = Date.now() / 1000 - days * 86400;
    return list.filter(g => g.posted >= cutoff);
  }

  /** Called by FilterPanel's Sort button via onSort prop. */
  async function handleSort() {
    const scope = $sortScope;
    const myToken = ++sortCancelToken;

    // Determine target count / whether we're in date mode
    const isDateMode = scope.mode === "days";
    const targetCount = isDateMode ? Infinity : scope.count;
    const currentLoaded = $galleries.length;

    if (!isDateMode && currentLoaded >= targetCount) {
      // Enough already loaded — step 2: sort full master list, step 3: filter.
      // $effect watching sortedUnfiltered + $sortActive will write $sortedGalleries.
      sortedUnfiltered = applySortToList($galleries.slice(0, targetCount), scope.field, scope.dir);
      $sortActive = true;
      return;
    }

    // Need to fetch more. Show progress overlay.
    const secsPerPage = 2;
    const galleriesPerPage = 25;

    function calcEta(loaded: number): number {
      if (isDateMode) return 0; // unknown in date mode
      const remaining = Math.max(0, targetCount - loaded);
      const pages = Math.ceil(remaining / galleriesPerPage);
      return pages * secsPerPage;
    }

    try {
      // $sortFetchProgress inside try so finally always clears it.
      $sortFetchProgress = {
        fetching: true,
        loaded: currentLoaded,
        target: isDateMode ? 0 : targetCount,
        estimatedSeconds: calcEta(currentLoaded),
        cancelled: false,
      };

      // Fetch loop
      while (true) {
        // Check cancel
        if (sortCancelToken !== myToken) return;

        const currentList = get(galleries);
        const loaded = currentList.length;

        // Check stop condition
        if (!isDateMode && loaded >= targetCount) break;
        if (!hasMoreRemote) break;

        // In date mode, check if oldest gallery in the last batch is past cutoff
        if (isDateMode && loaded > 0) {
          const cutoff = Date.now() / 1000 - scope.days * 86400;
          const oldest = currentList[currentList.length - 1];
          if (oldest && oldest.posted < cutoff) break;
        }

        try {
          await fetchOneBatch();
        } catch (err) {
          console.error("[GalleryGrid] sort fetch error:", err);
          break;
        }

        if (sortCancelToken !== myToken) return;

        // Update progress
        const nowLoaded = get(galleries).length;
        $sortFetchProgress = {
          fetching: true,
          loaded: nowLoaded,
          target: isDateMode ? 0 : targetCount,
          estimatedSeconds: calcEta(nowLoaded),
          cancelled: false,
        };
      }

      if (sortCancelToken !== myToken) return;

      // Step 2: sort the FULL master list (or date-trimmed slice of it).
      // Use get(galleries) to guarantee we read the current store value after all awaits.
      let master = get(galleries);
      if (isDateMode) {
        master = filterByDays(master, scope.days);
      } else {
        master = master.slice(0, targetCount);
      }
      let sorted = applySortToList(master, scope.field, scope.dir);

      // Step 3: $effect watching sortedUnfiltered + $sortActive writes $sortedGalleries.
      sortedUnfiltered = sorted;
      $sortActive = true;
    } finally {
      $sortFetchProgress = emptySortFetchProgress();
    }
  }

  function cancelSortFetch() {
    sortCancelToken++; // invalidates any running sort loop — finally block resets $sortFetchProgress
    // Sort whatever we have so far: step 2 (sort master list) then step 3 (filter).
    const scope = $sortScope;
    const isDateMode = scope.mode === "days";
    let master = get(galleries);
    if (isDateMode) {
      master = filterByDays(master, scope.days);
    } else {
      master = master.slice(0, scope.count);
    }
    sortedUnfiltered = applySortToList(master, scope.field, scope.dir);
    $sortActive = true;
  }

  function clearSort() {
    sortCancelToken++;
    $sortActive = false;
    $sortedGalleries = [];
    sortedUnfiltered = [];
    $sortFetchProgress = emptySortFetchProgress();
  }

  function handleFilterClear() {
    $searchActive = false;
    filterOpen = false;
    initialLoading = true;
    $galleries = [];
    startFreshSync();
  }

  function handleOpenGallery(gallery: Gallery) {
    $detailGallery = gallery;
  }

  // When detail closes, refresh progress.
  let prevDetailGallery: Gallery | null = null;
  $effect(() => {
    const current = $detailGallery;
    const wasOpen = prevDetailGallery !== null;
    prevDetailGallery = current;
    if (current === null && wasOpen && $galleries.length > 0) {
      loadProgress($galleries);
    }
  });

  // When sort is active and the homeFilter or quickFilter changes, re-apply step 3
  // (filter only — no re-sort, no re-fetch) on the stored sorted-but-unfiltered array.
  $effect(() => {
    if (!$sortActive || sortedUnfiltered.length === 0) return;
    const term = debouncedFilter.trim();
    const hf = $homeFilter;
    let list: Gallery[] = sortedUnfiltered;
    if (term) list = list.filter(g => !matchesHideFilter(g, term));
    if (isHomeFilterActive(hf)) list = list.filter(g => !hiddenByHomeFilter(g, hf));
    $sortedGalleries = list;
  });

  // Auto-fill: when a filter is active and filteredGalleries shrinks (or filter
  // is first applied), prod VirtualGrid/VirtualList to recheck scroll proximity.
  // If the visible content is shorter than the container, VirtualGrid's $effect
  // will detect distFromBottom <= threshold and call onScrollNearEnd → autoSync.
  // This self-perpetuates via the deferred recheckTrigger++ already scheduled
  // inside autoSync — no separate loop needed.
  let prevFilteredLength = filteredGalleries.length;
  $effect(() => {
    const len = filteredGalleries.length;
    const filterActive = !$searchActive && (debouncedFilter.trim() !== "" || isHomeFilterActive($homeFilter));
    if (filterActive && len !== prevFilteredLength && hasMoreRemote && !initialLoading) {
      prevFilteredLength = len;
      recheckTrigger++;
    } else {
      prevFilteredLength = len;
    }
  });

  // ── Viewport-driven thumbnail downloading ───────────────────────────────
  // Only download thumbnails for galleries visible in the virtual scroll viewport.
  // Debounced to avoid spamming IPC during fast scrolling.
  let thumbDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingThumbGids = new Set<number>();
  let requestedThumbGids = new Set<number>();

  function handleVisibleRangeChanged(startIdx: number, endIdx: number) {
    // Collect gids of visible galleries that need thumbnails.
    const source = $sortActive
      ? $sortedGalleries
      : (!$searchActive && (debouncedFilter.trim() || isHomeFilterActive($homeFilter))
        ? filteredGalleries
        : $galleries);
    const newGids: number[] = [];
    for (let i = startIdx; i < endIdx && i < source.length; i++) {
      const g = source[i];
      if (!g.thumb_path && g.thumb_url && !requestedThumbGids.has(g.gid)) {
        newGids.push(g.gid);
        pendingThumbGids.add(g.gid);
      }
    }

    if (newGids.length === 0) return;

    // Debounce: wait 150ms after last scroll before firing the IPC call.
    if (thumbDebounceTimer) clearTimeout(thumbDebounceTimer);
    thumbDebounceTimer = setTimeout(() => {
      const gids = [...pendingThumbGids];
      pendingThumbGids.clear();
      if (gids.length === 0) return;
      // Mark as requested so we don't re-request on subsequent scrolls.
      for (const gid of gids) requestedThumbGids.add(gid);
      downloadThumbnailsForGids(gids).catch((err) => {
        console.error("[GalleryGrid] thumbnail download error:", err);
        // On error, allow retry by removing from requested set.
        for (const gid of gids) requestedThumbGids.delete(gid);
      });
    }, 150);
  }

  function toggleViewMode() {
    $viewMode = $viewMode === "cards" ? "list" : "cards";
  }
</script>

{#snippet scrollFooter()}
  {#if syncingMore || loadingMore}
    <div class="scroll-footer">
      <div class="loading-dots">
        <span class="dot"></span><span class="dot"></span><span class="dot"></span>
      </div>
      <span class="loading-text">{$t("gallery.loading")}</span>
    </div>
  {:else if !$searchActive && !hasMoreRemote}
    <div class="scroll-footer end">
      {$t("gallery.end_of_content")}
    </div>
  {:else if $searchActive && !hasMoreSearchResults}
    <div class="scroll-footer end">
      {$t("gallery.end_of_content")}
    </div>
  {/if}
{/snippet}

<!-- Filter sidebar overlay -->
{#if filterOpen}
  <div class="filter-overlay" onclick={() => filterOpen = false} role="presentation"></div>
{/if}

<div class="filter-sidebar" class:open={filterOpen}>
  <div class="sidebar-header">
    <h3>{$t("filters.title")}</h3>
    <button class="close-btn" onclick={() => filterOpen = false}>
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
        <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
    </button>
  </div>
  <FilterPanel onClose={() => filterOpen = false} onSort={handleSort} />
</div>

<div class="gallery-grid-wrapper">
  <div class="toolbar">
    <div class="toolbar-left">
      {#if !$searchActive && (debouncedFilter.trim() || isHomeFilterActive($homeFilter))}
        <span class="count">{$t("gallery.hiding_count", { hidden: $galleries.length - filteredGalleries.length, total: $galleries.length })}</span>
      {:else}
        <span class="count">{$t("gallery.count", { count: $searchActive ? $totalCount : $galleries.length })}</span>
      {/if}
      {#if $searchActive}
        <span class="search-badge">{$t("gallery.filtered")}</span>
      {/if}
    </div>
    <div class="toolbar-right">
      {#if !$searchActive}
        <div class="quick-filter-wrap">
          <svg class="quick-filter-icon" width="14" height="14" viewBox="0 0 16 16" fill="none">
            <path d="M2 3h12L9 8.5V12l-2 1V8.5L2 3z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
          </svg>
          <input
            class="quick-filter-input"
            type="text"
            placeholder={$t("gallery.hide_filter_placeholder")}
            value={quickFilterInput}
            oninput={(e) => onQuickFilterInput(e.currentTarget.value)}
          />
          {#if quickFilterInput}
            <button class="quick-filter-clear" onclick={() => { onQuickFilterInput(""); }}>
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
              </svg>
            </button>
          {/if}
        </div>
      {/if}
      <button class="tool-btn icon-only" onclick={toggleViewMode} title={$viewMode === "cards" ? $t("settings.list") : $t("settings.cards")}>
        {#if $viewMode === "cards"}
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none"><rect x="1" y="1" width="6" height="6" rx="1" stroke="currentColor" stroke-width="1.3"/><rect x="9" y="1" width="6" height="6" rx="1" stroke="currentColor" stroke-width="1.3"/><rect x="1" y="9" width="6" height="6" rx="1" stroke="currentColor" stroke-width="1.3"/><rect x="9" y="9" width="6" height="6" rx="1" stroke="currentColor" stroke-width="1.3"/></svg>
        {:else}
          <svg width="15" height="15" viewBox="0 0 16 16" fill="none"><path d="M2 4h12M2 8h12M2 12h12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
        {/if}
      </button>
      <button class="tool-btn" class:active={filterOpen || (!$searchActive && isHomeFilterActive($homeFilter))} onclick={() => filterOpen = !filterOpen}>
        <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
          <path d="M2 4h12M4 8h8M6 12h4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
        {$t("filters.title")}
      </button>
    </div>
  </div>

  {#if isOffline && $galleries.length > 0}
    <div class="offline-banner">
      {$t("gallery.offline_cached")}
    </div>
  {/if}

  <!-- Sort fetch progress overlay -->
  {#if $sortFetchProgress.fetching}
    <div class="sort-progress-overlay">
      <div class="sort-progress-content">
        <div class="sort-progress-label">
          Fetching galleries… {$sortFetchProgress.loaded}{$sortFetchProgress.target > 0 ? `/${$sortFetchProgress.target}` : ""}
          {#if $sortFetchProgress.estimatedSeconds > 0}
            · ~{$sortFetchProgress.estimatedSeconds}s remaining
          {/if}
        </div>
        <div class="sort-progress-bar">
          <div
            class="sort-progress-fill"
            style="width:{$sortFetchProgress.target > 0 ? Math.min(100, Math.round($sortFetchProgress.loaded / $sortFetchProgress.target * 100)) : 0}%"
          ></div>
        </div>
        <button class="sort-progress-cancel" onclick={cancelSortFetch}>Cancel</button>
      </div>
    </div>
  {/if}

  <!-- Sort active banner -->
  {#if $sortActive && !$sortFetchProgress.fetching}
    <div class="sort-banner">
      <span class="sort-banner-text">
        Sorted by {$sortScope.field === "posted" ? "date" : $sortScope.field}
        · {$sortedGalleries.length} galleries
      </span>
      <button class="sort-banner-clear" onclick={clearSort}>Clear sort</button>
    </div>
  {/if}

  {#if initialLoading}
    <!-- Skeleton loading: initial page load -->
    <div class="grid-scroll">
      {#if $viewMode === "cards"}
        <div class="grid" style="--card-min:{$cardSize}px">
          {#each Array(SKELETON_INITIAL) as _}
            <div class="skeleton-card">
              <div class="skeleton-thumb skeleton-pulse"></div>
              <div class="skeleton-info">
                <div class="skeleton-line skeleton-pulse" style="width:60%"></div>
                <div class="skeleton-line skeleton-pulse" style="width:90%"></div>
                <div class="skeleton-line skeleton-pulse" style="width:40%"></div>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <div class="list">
          {#each Array(SKELETON_INITIAL) as _}
            <div class="skeleton-list-item">
              <div class="skeleton-list-thumb skeleton-pulse"></div>
              <div class="skeleton-list-middle">
                <div class="skeleton-line skeleton-pulse" style="width:70%"></div>
                <div class="skeleton-line skeleton-pulse" style="width:40%"></div>
              </div>
              <div class="skeleton-list-right">
                <div class="skeleton-line skeleton-pulse" style="width:50px"></div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {:else if $galleries.length === 0 && !syncingMore}
    <div class="empty">
      {#if $searchActive}
        <p>{$t("gallery.no_match")}</p>
        <button class="tool-btn" onclick={() => { handleFilterClear(); }}>{$t("gallery.clear_filters")}</button>
      {:else}
        <div class="empty-icon">
          <svg width="48" height="48" viewBox="0 0 48 48" fill="none">
            <rect x="6" y="10" width="36" height="28" rx="4" stroke="currentColor" stroke-width="2"/>
            <path d="M6 20h36M18 10v28" stroke="currentColor" stroke-width="2" opacity="0.3"/>
          </svg>
        </div>
        <p>{$t("gallery.empty_title")}</p>
        <p class="sub">{$t("gallery.empty_hint")}</p>
      {/if}
    </div>
  {:else if filteredGalleries.length === 0 && (debouncedFilter.trim() || isHomeFilterActive($homeFilter))}
    <div class="empty">
      <p>{$t("gallery.all_hidden")}</p>
      <button class="tool-btn" onclick={() => { onQuickFilterInput(""); filterOpen = false; }}>{$t("gallery.clear_filters")}</button>
    </div>
  {:else}
    {#if $viewMode === "cards"}
      <VirtualGrid
        items={$sortActive ? $sortedGalleries : filteredGalleries}
        rowHeight={Math.round($cardSize * (4/3) + 138)}
        columnMinWidth={$cardSize}
        gap={16}
        buffer={8}
        onScrollNearEnd={handleScrollEnd}
        onVisibleRangeChanged={handleVisibleRangeChanged}
        {recheckTrigger}
      >
        {#snippet children(gallery, _index)}
          <GalleryCard
            {gallery}
            progress={progressMap.get(gallery.gid) ?? null}
            onOpen={handleOpenGallery}
          />
        {/snippet}
        {#snippet footer()}
          {#if !$sortActive}
            {@render scrollFooter()}
          {/if}
        {/snippet}
      </VirtualGrid>
    {:else}
      <VirtualList
        items={$sortActive ? $sortedGalleries : filteredGalleries}
        rowHeight={72}
        gap={5}
        buffer={20}
        onScrollNearEnd={handleScrollEnd}
        onVisibleRangeChanged={handleVisibleRangeChanged}
        {recheckTrigger}
      >
        {#snippet children(gallery, _index)}
          <GalleryListItem
            {gallery}
            progress={progressMap.get(gallery.gid) ?? null}
            onOpen={handleOpenGallery}
          />
        {/snippet}
        {#snippet footer()}
          {#if !$sortActive}
            {@render scrollFooter()}
          {/if}
        {/snippet}
      </VirtualList>
    {/if}
  {/if}
</div>

<style>
  .gallery-grid-wrapper {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    position: relative;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.85rem 1.5rem;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
    background: var(--bg-primary);
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .count {
    font-size: 0.8rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .search-badge {
    font-size: 0.68rem;
    font-weight: 600;
    background: var(--accent);
    color: #fff;
    padding: 3px 10px;
    border-radius: 20px;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .tool-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.45rem 0.9rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-primary);
    color: var(--text-secondary);
    font-size: 0.78rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .tool-btn.icon-only {
    padding: 0.45rem 0.6rem;
  }

  .tool-btn:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .tool-btn.active {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .tool-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .quick-filter-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  .quick-filter-icon {
    position: absolute;
    left: 0.55rem;
    color: var(--text-muted);
    pointer-events: none;
  }

  .quick-filter-input {
    padding: 0.4rem 1.8rem 0.4rem 1.75rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    font-size: 0.75rem;
    background: var(--bg-secondary);
    color: var(--text-primary);
    outline: none;
    width: 180px;
    transition: border-color 0.15s, box-shadow 0.15s, width 0.2s;
  }

  .quick-filter-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
    width: 220px;
  }

  .quick-filter-input::placeholder {
    color: var(--text-muted);
    font-size: 0.72rem;
  }

  .quick-filter-clear {
    position: absolute;
    right: 0.3rem;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.15rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 2px;
    transition: color 0.15s;
  }

  .quick-filter-clear:hover {
    color: var(--text-primary);
  }

  .offline-banner {
    padding: 0.4rem 1rem;
    background: var(--bg-tertiary);
    border-bottom: 1px solid var(--border);
    font-size: 0.72rem;
    color: var(--text-muted);
    text-align: center;
    flex-shrink: 0;
  }

  /* ── Sort progress overlay ──────────────────────────────────── */

  .sort-progress-overlay {
    position: absolute;
    inset: 0;
    background: var(--overlay-bg);
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: all;
  }

  .sort-progress-content {
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    padding: 1.5rem 2rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.85rem;
    min-width: 280px;
  }

  .sort-progress-label {
    font-size: 0.8rem;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .sort-progress-bar {
    width: 100%;
    height: 6px;
    background: var(--bg-tertiary);
    border-radius: 3px;
    overflow: hidden;
  }

  .sort-progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.3s;
  }

  .sort-progress-cancel {
    padding: 0.35rem 1rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .sort-progress-cancel:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  /* ── Sort banner ────────────────────────────────────────────── */

  .sort-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.4rem 1.5rem;
    background: var(--accent-subtle);
    border-bottom: 1px solid var(--accent);
    flex-shrink: 0;
  }

  .sort-banner-text {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--accent);
  }

  .sort-banner-clear {
    font-size: 0.72rem;
    font-weight: 500;
    color: var(--accent);
    background: none;
    border: 1px solid var(--accent);
    border-radius: var(--radius-sm);
    padding: 0.2rem 0.6rem;
    cursor: pointer;
    transition: background 0.1s;
  }

  .sort-banner-clear:hover {
    background: var(--accent);
    color: #fff;
  }

  .filter-overlay {
    position: fixed;
    inset: 0;
    background: var(--overlay-bg);
    z-index: 200;
  }

  .filter-sidebar {
    position: fixed;
    top: 0;
    right: 0;
    width: 340px;
    max-width: 90vw;
    height: 100vh;
    background: var(--bg-primary);
    border-left: 1px solid var(--border);
    box-shadow: -4px 0 24px var(--overlay-bg);
    z-index: 210;
    transform: translateX(100%);
    transition: transform 0.2s ease;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  .filter-sidebar.open {
    transform: translateX(0);
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .sidebar-header h3 {
    margin: 0;
    font-size: 0.85rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-primary);
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.25rem;
    border-radius: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.15s;
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 0.75rem;
    color: var(--text-muted);
  }

  .empty-icon {
    opacity: 0.15;
    margin-bottom: 0.5rem;
  }

  .empty p {
    margin: 0;
    font-size: 0.85rem;
  }

  .empty .sub {
    font-size: 0.78rem;
    opacity: 0.6;
  }

  .grid-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-min, 165px), 1fr));
    gap: 16px;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  /* ── Skeleton styles ──────────────────────────────────────────── */

  @keyframes skeleton-pulse {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 0.7; }
  }

  .skeleton-pulse {
    animation: skeleton-pulse 1.8s ease-in-out infinite;
  }

  .skeleton-card {
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: var(--card-bg);
    border: 1px solid var(--card-border);
    box-shadow: var(--shadow-sm);
  }

  .skeleton-thumb {
    width: 100%;
    aspect-ratio: 3 / 4;
    background: var(--bg-tertiary);
  }

  .skeleton-info {
    padding: 0.6rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    height: 8.5rem;
  }

  .skeleton-line {
    height: 0.6rem;
    border-radius: 2px;
    background: var(--bg-tertiary);
  }

  .skeleton-list-item {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    padding: 0.65rem 1rem;
    border-radius: var(--radius-md);
    background: var(--card-bg);
    border: 1px solid var(--card-border);
    box-shadow: var(--shadow-sm);
  }

  .skeleton-list-thumb {
    width: 52px;
    height: 70px;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    background: var(--bg-tertiary);
  }

  .skeleton-list-middle {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .skeleton-list-right {
    flex-shrink: 0;
  }

  /* ── Scroll footer / loading indicator ─────────────────────── */

  .scroll-footer {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 1.5rem 1rem;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .scroll-footer.end {
    padding: 1.25rem 1rem;
  }

  .loading-dots {
    display: inline-flex;
    gap: 4px;
  }

  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--text-muted);
    animation: dot-bounce 1.2s ease-in-out infinite;
  }

  .dot:nth-child(2) {
    animation-delay: 0.15s;
  }

  .dot:nth-child(3) {
    animation-delay: 0.3s;
  }

  @keyframes dot-bounce {
    0%, 60%, 100% { opacity: 0.25; transform: translateY(0); }
    30% { opacity: 1; transform: translateY(-3px); }
  }

  .loading-text {
    font-weight: 500;
  }
</style>
