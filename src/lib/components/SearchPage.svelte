<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { t } from "$lib/i18n";
  import {
    searchExhentai, onThumbnailReady, downloadThumbnailsForGids,
    startEnrichment, onGalleryEnriched, getSearchHistory, clearSearchHistory,
  } from "$lib/api/galleries";
  import type { Gallery, AdvancedSearchOptions, SearchHistoryEntry, TagSuggestion } from "$lib/api/galleries";
  import { getReadProgressBatch } from "$lib/api/reader";
  import type { ReadProgress } from "$lib/api/reader";
  import {
    searchResults, searchQuery, searchNextUrl, searchHasMore,
    searchLoading, searchCategoryMask, searchAdvancedOptions, searchHideFilter,
    searchIncludeTags, searchExcludeTags,
    searchSortScope, searchSortFetchProgress, searchSortActive, searchSortedGalleries,
    emptySearchSortFetchProgress,
  } from "$lib/stores/search";
  import type { SearchSortField } from "$lib/stores/search";
  import { detailGallery, detailOpenedAsLocal } from "$lib/stores/detail";
  import { viewMode, cardSize, isIos } from "$lib/stores/ui";
  import GalleryCard from "./GalleryCard.svelte";
  import GalleryListItem from "./GalleryListItem.svelte";
  import VirtualGrid from "./VirtualGrid.svelte";
  import VirtualList from "./VirtualList.svelte";
  import TagInputAutocomplete from "./TagInputAutocomplete.svelte";

  // ── Category definitions ─────────────────────────────────────────────
  const CATEGORIES = [
    { name: "Doujinshi", bit: 2 },
    { name: "Manga", bit: 4 },
    { name: "Artist CG", bit: 8 },
    { name: "Game CG", bit: 16 },
    { name: "Western", bit: 512 },
    { name: "Non-H", bit: 256 },
    { name: "Image Set", bit: 32 },
    { name: "Cosplay", bit: 64 },
    { name: "Asian Porn", bit: 128 },
    { name: "Misc", bit: 1 },
  ];

  // ── Local state ──────────────────────────────────────────────────────
  let inputValue = $state($searchQuery);
  let showAdvanced = $state(false);
  let showHistory = $state(false);
  let showFilterPanel = $state(false);
  let historyEntries = $state<SearchHistoryEntry[]>([]);
  let progressMap = $state<Map<number, ReadProgress>>(new Map());
  let recheckTrigger = $state(0);
  let loadingMore = $state(false);
  let hasSearched = $state($searchResults.length > 0 || $searchQuery !== "");
  /** The last full combined f_search string sent to search_exhentai. Used for pagination. */
  let lastCombinedQuery = "";

  // Cooldown to prevent runaway fetch loops from reactive rechecks.
  const SEARCH_COOLDOWN_MS = 2000;
  let lastSearchEndTime = 0;

  // ── Tag include/exclude ──────────────────────────────────────────────
  function handleTagInputFocus() {
    showHistory = false;
  }

  /** Build the combined f_search string from free text + include/exclude tags. */
  function buildSearchQuery(freeText: string): string {
    const parts: string[] = [];
    if (freeText.trim()) parts.push(freeText.trim());
    for (const tag of $searchIncludeTags) {
      const val = tag.name.includes(" ") ? `"${tag.name}"` : tag.name;
      parts.push(`${tag.namespace}:${val}`);
    }
    for (const tag of $searchExcludeTags) {
      const val = tag.name.includes(" ") ? `"${tag.name}"` : tag.name;
      parts.push(`-${tag.namespace}:${val}`);
    }
    return parts.join(" ");
  }

  /**
   * Parse a full f_search string (e.g. from history) back into its components.
   * Handles: namespace:tag, namespace:"multi word", namespace:"exact$", and minus-prefixed exclusions.
   */
  function parseSearchString(raw: string): { freeText: string; includeTags: TagSuggestion[]; excludeTags: TagSuggestion[] } {
    const includeTags: TagSuggestion[] = [];
    const excludeTags: TagSuggestion[] = [];
    // Match optional leading minus, then namespace:tag or namespace:"quoted"
    const tagRe = /(-?)(\w+):("(?:[^"\\]|\\.)*"|\S+)/g;
    const consumed = new Set<string>();
    let match: RegExpExecArray | null;
    while ((match = tagRe.exec(raw)) !== null) {
      const [full, minus, namespace, rawName] = match;
      // Strip surrounding quotes and trailing $ (exact match marker)
      const name = rawName.startsWith('"')
        ? rawName.slice(1, -1).replace(/\$$/, "")
        : rawName.replace(/\$$/, "");
      const tag: TagSuggestion = { namespace, name };
      if (minus === "-") {
        excludeTags.push(tag);
      } else {
        includeTags.push(tag);
      }
      consumed.add(full);
    }
    // Remove consumed tag tokens from the string to get free text
    let freeText = raw;
    for (const token of consumed) {
      freeText = freeText.replace(token, "");
    }
    freeText = freeText.replace(/\s+/g, " ").trim();
    return { freeText, includeTags, excludeTags };
  }

  // Hide filter
  let hideFilterInput = $state($searchHideFilter);
  let debouncedHideFilter = $state($searchHideFilter);
  let hideFilterTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => { $searchHideFilter = debouncedHideFilter; });

  function onHideFilterInput(value: string) {
    hideFilterInput = value;
    if (hideFilterTimer) clearTimeout(hideFilterTimer);
    hideFilterTimer = setTimeout(() => { debouncedHideFilter = value; }, 250);
  }

  function matchesHideFilter(gallery: Gallery, term: string): boolean {
    const lower = term.toLowerCase();
    if (gallery.title.toLowerCase().includes(lower)) return true;
    if (gallery.title_jpn?.toLowerCase().includes(lower)) return true;
    if (gallery.category.toLowerCase().includes(lower)) return true;
    if (gallery.uploader?.toLowerCase().includes(lower)) return true;
    for (const tag of gallery.tags) {
      if (tag.name.toLowerCase().includes(lower)) return true;
      if (`${tag.namespace}:${tag.name}`.toLowerCase().includes(lower)) return true;
    }
    return false;
  }

  let filteredResults = $derived.by(() => {
    const term = debouncedHideFilter.trim();
    return !term ? $searchResults : $searchResults.filter(g => !matchesHideFilter(g, term));
  });

  // ── Category toggle ──────────────────────────────────────────────────
  function isCategoryEnabled(bit: number): boolean {
    return ($searchCategoryMask & bit) === 0;
  }

  function toggleCategory(bit: number) {
    if (isCategoryEnabled(bit)) {
      $searchCategoryMask = $searchCategoryMask | bit;
    } else {
      $searchCategoryMask = $searchCategoryMask & ~bit;
    }
  }

  // ── Advanced options ─────────────────────────────────────────────────
  function toggleAdvOption(key: keyof AdvancedSearchOptions) {
    $searchAdvancedOptions = {
      ...$searchAdvancedOptions,
      [key]: !$searchAdvancedOptions[key],
    };
  }

  function setAdvRating(value: number | null) {
    $searchAdvancedOptions = { ...$searchAdvancedOptions, minimum_rating: value };
  }

  function setAdvMinPages(value: number | null) {
    $searchAdvancedOptions = { ...$searchAdvancedOptions, min_pages: value };
  }

  function setAdvMaxPages(value: number | null) {
    $searchAdvancedOptions = { ...$searchAdvancedOptions, max_pages: value };
  }

  // ── Search execution ─────────────────────────────────────────────────
  async function executeSearch(query?: string) {
    const freeText = query ?? inputValue;
    const effectiveQ = buildSearchQuery(freeText);
    if (!effectiveQ.trim()) return;

    $searchQuery = freeText.trim();
    lastCombinedQuery = effectiveQ;
    inputValue = freeText.trim();
    $searchNextUrl = null;
    $searchLoading = true;
    hasSearched = true;
    showHistory = false;
    // Clear any active sort — new search invalidates the snapshot.
    sortCancelToken++;
    $searchSortActive = false;
    $searchSortedGalleries = [];
    sortedUnfiltered = [];
    $searchSortFetchProgress = emptySearchSortFetchProgress();

    try {
      const result = await searchExhentai(effectiveQ, null, $searchCategoryMask, $searchAdvancedOptions);
      $searchResults = result.galleries;
      $searchHasMore = result.has_more;
      $searchNextUrl = result.next_url;
      await loadProgress(result.galleries);
      startEnrichment().catch(() => {});
    } catch (err) {
      console.error("[SearchPage] search error:", err);
      $searchResults = [];
      $searchHasMore = false;
      $searchNextUrl = null;
    } finally {
      $searchLoading = false;
      lastSearchEndTime = Date.now();
      recheckTrigger++;
      // Schedule a deferred recheck so if results don't fill the viewport,
      // the next page loads automatically.
      if ($searchHasMore) {
        setTimeout(() => {
          recheckTrigger++;
        }, SEARCH_COOLDOWN_MS + 50);
      }
    }
  }

  async function loadNextPage() {
    if (loadingMore || !$searchHasMore || !$searchNextUrl || $searchLoading) return;
    // Enforce cooldown to prevent runaway fetch loops from reactive rechecks.
    if (Date.now() - lastSearchEndTime < SEARCH_COOLDOWN_MS) return;
    loadingMore = true;

    try {
      const result = await searchExhentai(
        lastCombinedQuery, $searchNextUrl, $searchCategoryMask, $searchAdvancedOptions
      );
      // Deduplicate by gid before appending.
      const existingGids = new Set($searchResults.map(g => g.gid));
      const newGalleries = result.galleries.filter(g => !existingGids.has(g.gid));
      $searchResults = [...$searchResults, ...newGalleries];
      $searchHasMore = result.has_more;
      $searchNextUrl = result.next_url;
      await loadProgress(newGalleries);
      startEnrichment().catch(() => {});
    } catch (err) {
      console.error("[SearchPage] load more error:", err);
    } finally {
      loadingMore = false;
      lastSearchEndTime = Date.now();
      recheckTrigger++;
      // Schedule a deferred recheck after cooldown so VirtualGrid/VirtualList
      // can re-measure if the user is still near the bottom and stopped scrolling.
      if ($searchHasMore) {
        setTimeout(() => {
          recheckTrigger++;
        }, SEARCH_COOLDOWN_MS + 50);
      }
    }
  }

  async function loadProgress(galleryList: Gallery[]) {
    if (galleryList.length === 0) return;
    try {
      const gids = galleryList.map(g => g.gid);
      const progresses = await getReadProgressBatch(gids);
      const newMap = new Map(progressMap);
      for (const p of progresses) newMap.set(p.gid, p);
      progressMap = newMap;
    } catch { /* ignore */ }
  }

  function handleScrollEnd() {
    if ($searchSortActive) return;
    loadNextPage();
  }

  // ── Sort ─────────────────────────────────────────────────────────────
  // Local sort form state (not committed to store until Sort is clicked).
  let sortCount = $state<string>(
    $searchSortScope.count > 0 ? String($searchSortScope.count) : "100"
  );
  let sortField = $state<SearchSortField>($searchSortScope.field);
  let sortDir = $state<"asc" | "desc">($searchSortScope.dir);

  // Sort fetch cancellation token.
  let sortCancelToken = $state(0);

  // Sorted-but-unfiltered snapshot for reactive re-filter on hide filter changes.
  let sortedUnfiltered = $state<Gallery[]>([]);

  // When sort is active and hide filter changes, re-apply filter without re-sorting.
  $effect(() => {
    if (!$searchSortActive || sortedUnfiltered.length === 0) return;
    const term = debouncedHideFilter.trim();
    let list: Gallery[] = sortedUnfiltered;
    if (term) list = list.filter(g => !matchesHideFilter(g, term));
    $searchSortedGalleries = list;
  });

  function applySortToList(list: Gallery[], field: SearchSortField, dir: "asc" | "desc"): Gallery[] {
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

  async function handleSearchSort() {
    const count = parseInt(sortCount) || 100;
    const myToken = ++sortCancelToken;

    // Commit scope to store.
    $searchSortScope = { count, field: sortField, dir: sortDir };

    const targetCount = count;
    const currentLoaded = $searchResults.length;

    if (currentLoaded >= targetCount) {
      // Enough loaded — sort and render immediately.
      sortedUnfiltered = applySortToList(get(searchResults).slice(0, targetCount), sortField, sortDir);
      $searchSortActive = true;
      return;
    }

    // Need to fetch more.
    const secsPerPage = 2;
    const galleriesPerPage = 25;

    function calcEta(loaded: number): number {
      const remaining = Math.max(0, targetCount - loaded);
      return Math.ceil(remaining / galleriesPerPage) * secsPerPage;
    }

    try {
      $searchSortFetchProgress = {
        fetching: true,
        loaded: currentLoaded,
        target: targetCount,
        estimatedSeconds: calcEta(currentLoaded),
      };

      while (true) {
        if (sortCancelToken !== myToken) return;

        const nowLoaded = get(searchResults).length;
        if (nowLoaded >= targetCount) break;
        if (!$searchHasMore || !$searchNextUrl) break;

        try {
          const result = await searchExhentai(
            lastCombinedQuery, $searchNextUrl, $searchCategoryMask, $searchAdvancedOptions
          );
          const existingGids = new Set(get(searchResults).map(g => g.gid));
          const newGalleries = result.galleries.filter(g => !existingGids.has(g.gid));
          $searchResults = [...get(searchResults), ...newGalleries];
          $searchHasMore = result.has_more;
          $searchNextUrl = result.next_url;
          await loadProgress(newGalleries);
          startEnrichment().catch(() => {});
        } catch (err) {
          console.error("[SearchPage] sort fetch error:", err);
          break;
        }

        if (sortCancelToken !== myToken) return;

        $searchSortFetchProgress = {
          fetching: true,
          loaded: get(searchResults).length,
          target: targetCount,
          estimatedSeconds: calcEta(get(searchResults).length),
        };
      }

      if (sortCancelToken !== myToken) return;

      sortedUnfiltered = applySortToList(get(searchResults).slice(0, targetCount), sortField, sortDir);
      $searchSortActive = true;
    } finally {
      $searchSortFetchProgress = emptySearchSortFetchProgress();
    }
  }

  function cancelSearchSortFetch() {
    sortCancelToken++;
    sortedUnfiltered = applySortToList(get(searchResults).slice(0, $searchSortScope.count || 100), sortField, sortDir);
    $searchSortActive = true;
  }

  function clearSearchSort() {
    sortCancelToken++;
    $searchSortActive = false;
    $searchSortedGalleries = [];
    sortedUnfiltered = [];
    $searchSortFetchProgress = emptySearchSortFetchProgress();
  }

  function handleOpenGallery(gallery: Gallery) {
    $detailOpenedAsLocal = false;
    $detailGallery = gallery;
  }

  // ── Search history ───────────────────────────────────────────────────
  async function loadHistory() {
    try {
      historyEntries = await getSearchHistory(20);
    } catch { historyEntries = []; }
  }

  function handleHistoryClick(query: string) {
    const { freeText, includeTags, excludeTags } = parseSearchString(query);
    $searchIncludeTags = includeTags;
    $searchExcludeTags = excludeTags;
    inputValue = freeText;
    showHistory = false;
    executeSearch(freeText);
  }

  async function handleClearHistory() {
    await clearSearchHistory();
    historyEntries = [];
  }

  function handleInputFocus() {
    loadHistory();
    showHistory = true;
    showTagDropdown = false;
  }

  function handleInputBlur() {
    // Delay to allow click on history item
    setTimeout(() => { showHistory = false; }, 200);
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      executeSearch();
    } else if (e.key === "Escape") {
      showHistory = false;
    }
  }

  // ── Viewport-driven thumbnail downloading ────────────────────────────
  let thumbDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingThumbGids = new Set<number>();
  let requestedThumbGids = new Set<number>();

  function handleVisibleRangeChanged(startIdx: number, endIdx: number) {
    const source = $searchSortActive
      ? $searchSortedGalleries
      : (debouncedHideFilter.trim() ? filteredResults : $searchResults);
    const newGids: number[] = [];
    for (let i = startIdx; i < endIdx && i < source.length; i++) {
      const g = source[i];
      if (!g.thumb_path && g.thumb_url && !requestedThumbGids.has(g.gid)) {
        newGids.push(g.gid);
        pendingThumbGids.add(g.gid);
      }
    }
    if (newGids.length === 0) return;

    if (thumbDebounceTimer) clearTimeout(thumbDebounceTimer);
    thumbDebounceTimer = setTimeout(() => {
      const gids = [...pendingThumbGids];
      pendingThumbGids.clear();
      if (gids.length === 0) return;
      for (const gid of gids) requestedThumbGids.add(gid);
      downloadThumbnailsForGids(gids).catch((err) => {
        console.error("[SearchPage] thumbnail download error:", err);
        for (const gid of gids) requestedThumbGids.delete(gid);
      });
    }, 150);
  }

  // ── Lifecycle ────────────────────────────────────────────────────────
  let unlisteners: (() => void)[] = [];

  onMount(async () => {
    const unlistenThumb = await onThumbnailReady((event) => {
      const idx = $searchResults.findIndex(g => g.gid === event.gid);
      if (idx >= 0) {
        const updated = [...$searchResults];
        updated[idx] = { ...updated[idx], thumb_path: event.path };
        $searchResults = updated;
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

    const unlistenEnriched = await onGalleryEnriched((event) => {
      const idx = $searchResults.findIndex(g => g.gid === event.gallery.gid);
      if (idx >= 0) {
        const updated = [...$searchResults];
        updated[idx] = { ...event.gallery, thumb_path: $searchResults[idx].thumb_path ?? event.gallery.thumb_path };
        $searchResults = updated;
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

    // If there's a pending query (e.g., from sidebar or tag click), execute it
    if ($searchQuery && $searchResults.length === 0) {
      inputValue = $searchQuery;
      executeSearch($searchQuery);
    }
  });

  onDestroy(() => {
    for (const unlisten of unlisteners) unlisten();
    if (hideFilterTimer) clearTimeout(hideFilterTimer);
    if (thumbDebounceTimer) clearTimeout(thumbDebounceTimer);
  });
</script>

{#snippet scrollFooter()}
  {#if loadingMore}
    <div class="scroll-footer">
      <div class="loading-dots">
        <span class="dot"></span><span class="dot"></span><span class="dot"></span>
      </div>
      <span class="loading-text">{$t("gallery.loading")}</span>
    </div>
  {:else if hasSearched && !$searchHasMore && $searchResults.length > 0}
    <div class="scroll-footer end">
      {$t("gallery.end_of_content")}
    </div>
  {/if}
{/snippet}

<div class="search-page">
  <!-- Search header -->
  <div class="search-header">
    <!-- Combined input row: search bar + tag input + search button -->
    <div class="search-input-row" class:ios-search-row={$isIos}>
      <!-- Search bar (left, ~60%) -->
      <div class="input-field-wrap search-field-wrap">
        <svg class="search-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        <input
          class="search-input"
          type="text"
          placeholder={$t("search_page.placeholder")}
          bind:value={inputValue}
          onkeydown={handleSearchKeydown}
          onfocus={handleInputFocus}
          onblur={handleInputBlur}
        />
        {#if inputValue}
          <button class="input-clear" onclick={() => { inputValue = ""; }}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
          </button>
        {/if}

        <!-- History dropdown — spans full combined width of both inputs -->
        {#if showHistory && historyEntries.length > 0}
          <div class="history-dropdown">
            <div class="history-header">
              <span class="history-label">{$t("search_page.recent")}</span>
              <button class="history-clear" onclick={handleClearHistory}>{$t("search_page.clear_history")}</button>
            </div>
            {#each historyEntries as entry}
              <button class="history-item" onmousedown={() => handleHistoryClick(entry.query)}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                <span>{entry.query}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      {#if !$isIos}
        <!-- Tag input (right, ~40%) — desktop/tablet only -->
        <div class="tag-field-wrap-outer">
          <TagInputAutocomplete
            bind:includeTags={$searchIncludeTags}
            bind:excludeTags={$searchExcludeTags}
            onFocus={handleTagInputFocus}
          />
        </div>
      {/if}

      <button class="search-btn" onclick={() => executeSearch()} disabled={!inputValue.trim() && $searchIncludeTags.length === 0 && $searchExcludeTags.length === 0}>
        {$t("search_page.search_btn")}
      </button>

      {#if $isIos}
        <!-- iOS: filter panel toggle button -->
        <button
          class="ios-filter-btn"
          class:ios-filter-btn--active={showFilterPanel || $searchIncludeTags.length > 0 || $searchExcludeTags.length > 0}
          onclick={() => showFilterPanel = !showFilterPanel}
          aria-label="Filters"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="4" y1="6" x2="20" y2="6"/>
            <line x1="8" y1="12" x2="16" y2="12"/>
            <line x1="11" y1="18" x2="13" y2="18"/>
          </svg>
          {#if $searchIncludeTags.length + $searchExcludeTags.length > 0}
            <span class="ios-filter-badge">{$searchIncludeTags.length + $searchExcludeTags.length}</span>
          {/if}
        </button>
      {/if}
    </div>

    {#if !$isIos}
      <!-- Category chips — desktop/tablet only -->
      <div class="category-chips">
        {#each CATEGORIES as cat}
          <button
            class="cat-chip"
            class:off={!isCategoryEnabled(cat.bit)}
            onclick={() => toggleCategory(cat.bit)}
          >
            {cat.name}
          </button>
        {/each}
      </div>

      <!-- Advanced toggle — desktop/tablet only -->
      <button class="advanced-toggle" onclick={() => showAdvanced = !showAdvanced}>
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" class:rotated={showAdvanced}>
          <path d="M6 4l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        {$t("search_page.advanced")}
      </button>

      {#if showAdvanced}
        <div class="advanced-options">
          <label class="adv-toggle">
            <input type="checkbox" checked={$searchAdvancedOptions.search_name} onchange={() => toggleAdvOption("search_name")} />
            {$t("search_page.search_names")}
          </label>
          <label class="adv-toggle">
            <input type="checkbox" checked={$searchAdvancedOptions.search_tags} onchange={() => toggleAdvOption("search_tags")} />
            {$t("search_page.search_tags")}
          </label>
          <label class="adv-toggle">
            <input type="checkbox" checked={$searchAdvancedOptions.search_description} onchange={() => toggleAdvOption("search_description")} />
            {$t("search_page.search_desc")}
          </label>
          <label class="adv-toggle">
            <input type="checkbox" checked={$searchAdvancedOptions.show_expunged} onchange={() => toggleAdvOption("show_expunged")} />
            {$t("search_page.show_expunged")}
          </label>
          <label class="adv-toggle">
            <input type="checkbox" checked={$searchAdvancedOptions.search_torrent_filenames} onchange={() => toggleAdvOption("search_torrent_filenames")} />
            Search torrent filenames
          </label>
          <label class="adv-toggle">
            <input type="checkbox" checked={$searchAdvancedOptions.only_with_torrents} onchange={() => toggleAdvOption("only_with_torrents")} />
            Only with torrents
          </label>
          <label class="adv-toggle">
            <input type="checkbox" checked={$searchAdvancedOptions.search_low_power_tags} onchange={() => toggleAdvOption("search_low_power_tags")} />
            Search low-power tags
          </label>
          <label class="adv-toggle">
            <input type="checkbox" checked={$searchAdvancedOptions.search_downvoted_tags} onchange={() => toggleAdvOption("search_downvoted_tags")} />
            Search downvoted tags
          </label>
          <div class="adv-row">
            <label class="adv-label" for="adv-rating">Minimum rating</label>
            <select
              id="adv-rating"
              class="adv-select"
              value={$searchAdvancedOptions.minimum_rating ?? ""}
              onchange={(e) => {
                const v = e.currentTarget.value;
                setAdvRating(v === "" ? null : parseInt(v));
              }}
            >
              <option value="">Any</option>
              <option value="2">2★</option>
              <option value="3">3★</option>
              <option value="4">4★</option>
              <option value="5">5★</option>
            </select>
          </div>
          <div class="adv-row">
            <label class="adv-label">Pages between</label>
            <input
              class="adv-pages-input"
              type="number"
              min="0"
              placeholder="min"
              value={$searchAdvancedOptions.min_pages ?? ""}
              oninput={(e) => {
                const v = e.currentTarget.value;
                setAdvMinPages(v === "" ? null : parseInt(v));
              }}
            />
            <span class="adv-pages-sep">and</span>
            <input
              class="adv-pages-input"
              type="number"
              min="0"
              placeholder="max"
              value={$searchAdvancedOptions.max_pages ?? ""}
              oninput={(e) => {
                const v = e.currentTarget.value;
                setAdvMaxPages(v === "" ? null : parseInt(v));
              }}
            />
          </div>
        </div>
      {/if}
    {/if}
  </div>

  <!-- iOS filter panel (right slide-in) -->
  {#if $isIos && showFilterPanel}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="ios-panel-backdrop" onclick={() => showFilterPanel = false}></div>
    <div class="ios-filter-panel">
      <div class="ios-panel-header">
        <span class="ios-panel-title">Filters & Sort</span>
        <button class="ios-panel-close" onclick={() => showFilterPanel = false}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="6" x2="6" y2="18"/>
            <line x1="6" y1="6" x2="18" y2="18"/>
          </svg>
        </button>
      </div>

      <div class="ios-panel-body">
        <!-- Tag input -->
        <div class="ios-panel-section">
          <span class="ios-panel-section-label">Tags</span>
          <TagInputAutocomplete
            bind:includeTags={$searchIncludeTags}
            bind:excludeTags={$searchExcludeTags}
            onFocus={handleTagInputFocus}
          />
        </div>

        <!-- Category chips -->
        <div class="ios-panel-section">
          <span class="ios-panel-section-label">Categories</span>
          <div class="category-chips">
            {#each CATEGORIES as cat}
              <button
                class="cat-chip"
                class:off={!isCategoryEnabled(cat.bit)}
                onclick={() => toggleCategory(cat.bit)}
              >
                {cat.name}
              </button>
            {/each}
          </div>
        </div>

        <!-- Advanced options -->
        <div class="ios-panel-section">
          <button class="advanced-toggle" onclick={() => showAdvanced = !showAdvanced}>
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" class:rotated={showAdvanced}>
              <path d="M6 4l4 4-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            {$t("search_page.advanced")}
          </button>
          {#if showAdvanced}
            <div class="advanced-options">
              <label class="adv-toggle">
                <input type="checkbox" checked={$searchAdvancedOptions.search_name} onchange={() => toggleAdvOption("search_name")} />
                {$t("search_page.search_names")}
              </label>
              <label class="adv-toggle">
                <input type="checkbox" checked={$searchAdvancedOptions.search_tags} onchange={() => toggleAdvOption("search_tags")} />
                {$t("search_page.search_tags")}
              </label>
              <label class="adv-toggle">
                <input type="checkbox" checked={$searchAdvancedOptions.search_description} onchange={() => toggleAdvOption("search_description")} />
                {$t("search_page.search_desc")}
              </label>
              <label class="adv-toggle">
                <input type="checkbox" checked={$searchAdvancedOptions.show_expunged} onchange={() => toggleAdvOption("show_expunged")} />
                {$t("search_page.show_expunged")}
              </label>
              <label class="adv-toggle">
                <input type="checkbox" checked={$searchAdvancedOptions.search_torrent_filenames} onchange={() => toggleAdvOption("search_torrent_filenames")} />
                Search torrent filenames
              </label>
              <label class="adv-toggle">
                <input type="checkbox" checked={$searchAdvancedOptions.only_with_torrents} onchange={() => toggleAdvOption("only_with_torrents")} />
                Only with torrents
              </label>
              <label class="adv-toggle">
                <input type="checkbox" checked={$searchAdvancedOptions.search_low_power_tags} onchange={() => toggleAdvOption("search_low_power_tags")} />
                Search low-power tags
              </label>
              <label class="adv-toggle">
                <input type="checkbox" checked={$searchAdvancedOptions.search_downvoted_tags} onchange={() => toggleAdvOption("search_downvoted_tags")} />
                Search downvoted tags
              </label>
              <div class="adv-row">
                <label class="adv-label" for="adv-rating-ios">Minimum rating</label>
                <select
                  id="adv-rating-ios"
                  class="adv-select"
                  value={$searchAdvancedOptions.minimum_rating ?? ""}
                  onchange={(e) => {
                    const v = e.currentTarget.value;
                    setAdvRating(v === "" ? null : parseInt(v));
                  }}
                >
                  <option value="">Any</option>
                  <option value="2">2★</option>
                  <option value="3">3★</option>
                  <option value="4">4★</option>
                  <option value="5">5★</option>
                </select>
              </div>
              <div class="adv-row">
                <label class="adv-label">Pages between</label>
                <input
                  class="adv-pages-input"
                  type="number"
                  min="0"
                  placeholder="min"
                  value={$searchAdvancedOptions.min_pages ?? ""}
                  oninput={(e) => {
                    const v = e.currentTarget.value;
                    setAdvMinPages(v === "" ? null : parseInt(v));
                  }}
                />
                <span class="adv-pages-sep">and</span>
                <input
                  class="adv-pages-input"
                  type="number"
                  min="0"
                  placeholder="max"
                  value={$searchAdvancedOptions.max_pages ?? ""}
                  oninput={(e) => {
                    const v = e.currentTarget.value;
                    setAdvMaxPages(v === "" ? null : parseInt(v));
                  }}
                />
              </div>
            </div>
          {/if}
        </div>

        <!-- Hide filter -->
        {#if hasSearched}
          <div class="ios-panel-section">
            <span class="ios-panel-section-label">Hide matching</span>
            <div class="quick-filter-wrap">
              <svg class="quick-filter-icon" width="14" height="14" viewBox="0 0 16 16" fill="none">
                <path d="M2 3h12L9 8.5V12l-2 1V8.5L2 3z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
              </svg>
              <input
                class="quick-filter-input ios-quick-filter"
                type="text"
                placeholder={$t("gallery.hide_filter_placeholder")}
                value={hideFilterInput}
                oninput={(e) => onHideFilterInput(e.currentTarget.value)}
              />
              {#if hideFilterInput}
                <button class="quick-filter-clear" onclick={() => onHideFilterInput("")}>
                  <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                    <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                  </svg>
                </button>
              {/if}
            </div>
          </div>

          <!-- Sort -->
          {#if !$searchSortActive}
            <div class="ios-panel-section">
              <span class="ios-panel-section-label">Sort</span>
              <div class="ios-sort-row">
                <select class="sort-select" bind:value={sortField}>
                  <option value="posted">Date posted</option>
                  <option value="rating">Rating</option>
                  <option value="pages">Page count</option>
                  <option value="title">Title</option>
                </select>
                <button
                  class="sort-dir-btn"
                  class:active={sortDir === "desc"}
                  onclick={() => sortDir = sortDir === "desc" ? "asc" : "desc"}
                >
                  {#if sortDir === "desc"}
                    <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                      <path d="M8 3v10M4 9l4 4 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    Desc
                  {:else}
                    <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                      <path d="M8 13V3M4 7l4-4 4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                    Asc
                  {/if}
                </button>
              </div>
              <div class="ios-sort-count-row">
                <div class="sort-preset-row">
                  {#each [100, 250, 500, 1000] as p}
                    <button
                      class="sort-preset-btn"
                      class:active={sortCount === String(p)}
                      onclick={() => sortCount = String(p)}
                    >{p}</button>
                  {/each}
                </div>
                <input
                  class="sort-count-input"
                  type="number"
                  min="1"
                  placeholder="100"
                  value={sortCount}
                  oninput={(e) => sortCount = e.currentTarget.value}
                />
                <span class="sort-bar-label">galleries</span>
                <button class="sort-go-btn" onclick={() => { handleSearchSort(); showFilterPanel = false; }} disabled={$searchResults.length === 0}>
                  Sort
                </button>
              </div>
            </div>
          {/if}
        {/if}
      </div>
    </div>
  {/if}

  <!-- Toolbar (non-iOS only) -->
  {#if hasSearched && !$isIos}
    <div class="toolbar">
      <div class="toolbar-left">
        {#if debouncedHideFilter.trim()}
          <span class="count">{$t("gallery.hiding_count", { hidden: $searchResults.length - filteredResults.length, total: $searchResults.length })}</span>
        {:else}
          <span class="count">{$searchResults.length} {$t("search_page.results_loaded")}</span>
        {/if}
      </div>
      <div class="toolbar-right">
        <div class="quick-filter-wrap">
          <svg class="quick-filter-icon" width="14" height="14" viewBox="0 0 16 16" fill="none">
            <path d="M2 3h12L9 8.5V12l-2 1V8.5L2 3z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
          </svg>
          <input
            class="quick-filter-input"
            type="text"
            placeholder={$t("gallery.hide_filter_placeholder")}
            value={hideFilterInput}
            oninput={(e) => onHideFilterInput(e.currentTarget.value)}
          />
          {#if hideFilterInput}
            <button class="quick-filter-clear" onclick={() => onHideFilterInput("")}>
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
              </svg>
            </button>
          {/if}
        </div>
      </div>
    </div>

    <!-- Sort bar (non-iOS only) -->
    {#if !$searchSortActive}
      <div class="sort-bar">
        <select class="sort-select" bind:value={sortField}>
          <option value="posted">Date posted</option>
          <option value="rating">Rating</option>
          <option value="pages">Page count</option>
          <option value="title">Title</option>
        </select>
        <button
          class="sort-dir-btn"
          class:active={sortDir === "desc"}
          onclick={() => sortDir = sortDir === "desc" ? "asc" : "desc"}
          title={sortDir === "desc" ? "Descending" : "Ascending"}
        >
          {#if sortDir === "desc"}
            <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
              <path d="M8 3v10M4 9l4 4 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            Desc
          {:else}
            <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
              <path d="M8 13V3M4 7l4-4 4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            Asc
          {/if}
        </button>
        <span class="sort-bar-sep"></span>
        <div class="sort-preset-row">
          {#each [100, 250, 500, 1000] as p}
            <button
              class="sort-preset-btn"
              class:active={sortCount === String(p)}
              onclick={() => sortCount = String(p)}
            >{p}</button>
          {/each}
        </div>
        <input
          class="sort-count-input"
          type="number"
          min="1"
          placeholder="100"
          value={sortCount}
          oninput={(e) => sortCount = e.currentTarget.value}
        />
        <span class="sort-bar-label">galleries</span>
        <button class="sort-go-btn" onclick={handleSearchSort} disabled={$searchResults.length === 0}>
          Sort
        </button>
      </div>
    {/if}
  {/if}

  <!-- iOS: slim toolbar showing count only -->
  {#if hasSearched && $isIos}
    <div class="toolbar ios-toolbar">
      <div class="toolbar-left">
        {#if debouncedHideFilter.trim()}
          <span class="count">{$t("gallery.hiding_count", { hidden: $searchResults.length - filteredResults.length, total: $searchResults.length })}</span>
        {:else}
          <span class="count">{$searchResults.length} {$t("search_page.results_loaded")}</span>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Sort fetch progress overlay -->
  {#if $searchSortFetchProgress.fetching}
    <div class="sort-progress-overlay">
      <div class="sort-progress-content">
        <div class="sort-progress-label">
          Fetching results… {$searchSortFetchProgress.loaded}/{$searchSortFetchProgress.target}
          {#if $searchSortFetchProgress.estimatedSeconds > 0}
            · ~{$searchSortFetchProgress.estimatedSeconds}s remaining
          {/if}
        </div>
        <div class="sort-progress-bar">
          <div
            class="sort-progress-fill"
            style="width:{$searchSortFetchProgress.target > 0 ? Math.min(100, Math.round($searchSortFetchProgress.loaded / $searchSortFetchProgress.target * 100)) : 0}%"
          ></div>
        </div>
        <button class="sort-progress-cancel" onclick={cancelSearchSortFetch}>Cancel</button>
      </div>
    </div>
  {/if}

  <!-- Sort active banner -->
  {#if $searchSortActive && !$searchSortFetchProgress.fetching}
    <div class="sort-banner">
      <span class="sort-banner-text">
        Sorted by {$searchSortScope.field === "posted" ? "date" : $searchSortScope.field}
        · {$searchSortedGalleries.length} results
      </span>
      <button class="sort-banner-clear" onclick={clearSearchSort}>Clear sort</button>
    </div>
  {/if}

  <!-- Results area -->
  {#if $searchLoading && $searchResults.length === 0}
    <!-- Loading skeleton -->
    <div class="grid-scroll">
      <div class="grid" style="--card-min:{$cardSize}px">
        {#each Array(12) as _}
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
    </div>
  {:else if hasSearched && $searchResults.length === 0 && !$searchLoading}
    <div class="empty">
      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.15">
        <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
      </svg>
      <p>{$t("search_page.no_results")}</p>
    </div>
  {:else if filteredResults.length === 0 && debouncedHideFilter.trim()}
    <div class="empty">
      <p>{$t("gallery.all_hidden")}</p>
      <button class="tool-btn" onclick={() => onHideFilterInput("")}>{$t("gallery.clear_filters")}</button>
    </div>
  {:else if !hasSearched}
    <div class="empty">
      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.15">
        <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
      </svg>
      <p>{$t("search_page.hint")}</p>
    </div>
  {:else}
    {#if $viewMode === "cards"}
      <VirtualGrid
        items={$searchSortActive ? $searchSortedGalleries : filteredResults}
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
          {#if !$searchSortActive}
            {@render scrollFooter()}
          {/if}
        {/snippet}
      </VirtualGrid>
    {:else}
      <VirtualList
        items={$searchSortActive ? $searchSortedGalleries : filteredResults}
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
          {#if !$searchSortActive}
            {@render scrollFooter()}
          {/if}
        {/snippet}
      </VirtualList>
    {/if}
  {/if}
</div>

<style>
  .search-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    position: relative;
  }

  /* ── Search header ────────────────────────────────────────── */

  .search-header {
    padding: 1rem 1.5rem 0.75rem;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
    background: var(--bg-primary);
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    position: relative;
    z-index: 10;
  }

  /* Combined row: search bar + tag input + button */
  .search-input-row {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .input-field-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  /* Search bar takes ~60% of available space */
  .search-field-wrap {
    flex: 3;
    min-width: 0;
  }

  /* Tag input wrapper takes ~40% of available space */
  .tag-field-wrap-outer {
    flex: 2;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .search-icon {
    position: absolute;
    left: 12px;
    color: var(--text-muted);
    pointer-events: none;
    z-index: 1;
  }

  .search-input {
    width: 100%;
    padding: 0.6rem 2.2rem 0.6rem 2.5rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.88rem;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .search-input::placeholder {
    color: var(--text-muted);
  }

  .search-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  .input-clear {
    position: absolute;
    right: 0.5rem;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.2rem;
    display: flex;
    align-items: center;
    border-radius: 2px;
    transition: color 0.15s;
  }

  .input-clear:hover {
    color: var(--text-primary);
  }

  .search-btn {
    padding: 0.6rem 1.2rem;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    flex-shrink: 0;
    align-self: flex-start;
  }

  .search-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .search-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ── History dropdown ─────────────────────────────────────── */

  .history-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    /* Extend right to cover both the search bar and tag input + gap between them.
       Tag input is flex:2 vs search bar flex:3, so it's ~40% of combined area.
       We use a calc based on the tag input's proportion + 0.5rem gap. */
    right: calc(-66.6667% - 0.5rem);
    margin-top: 4px;
    background: var(--bg-primary);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    z-index: 50;
    max-height: 300px;
    overflow-y: auto;
  }

  .history-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border);
  }

  .history-label {
    font-size: 0.68rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .history-clear {
    font-size: 0.7rem;
    color: var(--text-muted);
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    transition: color 0.15s;
  }

  .history-clear:hover {
    color: var(--red);
  }

  .history-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 0.5rem 0.75rem;
    background: none;
    border: none;
    color: var(--text-secondary);
    font-size: 0.82rem;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s, color 0.1s;
  }

  .history-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  /* ── Category chips ───────────────────────────────────────── */

  .category-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .cat-chip {
    padding: 4px 10px;
    border-radius: 20px;
    border: 1px solid var(--border-strong);
    background: var(--accent-subtle);
    color: var(--accent);
    font-size: 0.72rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }

  .cat-chip:hover {
    border-color: var(--accent);
  }

  .cat-chip.off {
    background: transparent;
    color: var(--text-muted);
    border-color: var(--border);
    opacity: 0.45;
  }

  .cat-chip.off:hover {
    border-color: var(--border-strong);
    opacity: 0.7;
  }

  /* ── Advanced options ─────────────────────────────────────── */

  .advanced-toggle {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    padding: 0;
    transition: color 0.15s;
    align-self: flex-start;
  }

  .advanced-toggle:hover {
    color: var(--text-primary);
  }

  .advanced-toggle svg {
    transition: transform 0.2s;
  }

  .advanced-toggle svg.rotated {
    transform: rotate(90deg);
  }

  /* ── Advanced options ─────────────────────────────────────── */

  .advanced-options {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem 1.5rem;
    padding: 0.25rem 0 0.25rem;
    align-items: center;
  }

  .adv-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.78rem;
    color: var(--text-secondary);
    cursor: pointer;
    user-select: none;
  }

  .adv-toggle input {
    accent-color: var(--accent);
    width: 14px;
    height: 14px;
  }

  .adv-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.78rem;
    color: var(--text-secondary);
  }

  .adv-label {
    white-space: nowrap;
  }

  .adv-select {
    padding: 2px 6px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.78rem;
    outline: none;
    cursor: pointer;
  }

  .adv-pages-input {
    width: 64px;
    padding: 2px 6px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.78rem;
    outline: none;
  }

  .adv-pages-sep {
    color: var(--text-muted);
    font-size: 0.75rem;
  }

  /* ── Toolbar ──────────────────────────────────────────────── */

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.65rem 1.5rem;
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
    border-radius: 2px;
    transition: color 0.15s;
  }

  .quick-filter-clear:hover {
    color: var(--text-primary);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 0.75rem;
    color: var(--text-muted);
  }

  .empty p {
    margin: 0;
    font-size: 0.85rem;
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
    transition: background 0.15s, color 0.15s;
  }

  .tool-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
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

  /* ── Skeleton ─────────────────────────────────────────────── */

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

  /* ── Scroll footer ────────────────────────────────────────── */

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

  .dot:nth-child(2) { animation-delay: 0.15s; }
  .dot:nth-child(3) { animation-delay: 0.3s; }

  @keyframes dot-bounce {
    0%, 60%, 100% { opacity: 0.25; transform: translateY(0); }
    30% { opacity: 1; transform: translateY(-3px); }
  }

  .loading-text {
    font-weight: 500;
  }

  /* ── Sort bar ──────────────────────────────────────────────── */

  .sort-bar {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.5rem 1.5rem;
    border-bottom: 1px solid var(--border);
    background: var(--bg-primary);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .sort-select {
    padding: 0.3rem 0.55rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.78rem;
    outline: none;
    cursor: pointer;
    transition: border-color 0.15s;
  }

  .sort-select:focus {
    border-color: var(--accent);
  }

  .sort-dir-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.73rem;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.1s, color 0.1s;
  }

  .sort-dir-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .sort-dir-btn.active {
    background: var(--accent-subtle);
    color: var(--accent);
    border-color: var(--accent);
  }

  .sort-bar-sep {
    width: 1px;
    height: 18px;
    background: var(--border-strong);
    margin: 0 0.2rem;
    flex-shrink: 0;
  }

  .sort-preset-row {
    display: flex;
    gap: 0.25rem;
  }

  .sort-preset-btn {
    padding: 0.28rem 0.6rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.72rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }

  .sort-preset-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .sort-preset-btn.active {
    background: var(--accent-subtle);
    color: var(--accent);
    border-color: var(--accent);
  }

  .sort-count-input {
    width: 60px;
    padding: 0.3rem 0.45rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.78rem;
    outline: none;
    text-align: center;
    transition: border-color 0.15s;
  }

  .sort-count-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  .sort-bar-label {
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .sort-go-btn {
    padding: 0.3rem 0.85rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--accent);
    background: transparent;
    color: var(--accent);
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
    margin-left: auto;
  }

  .sort-go-btn:hover:not(:disabled) {
    background: var(--accent-subtle);
  }

  .sort-go-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ── Sort progress overlay ─────────────────────────────────── */

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

  /* ── Sort banner ───────────────────────────────────────────── */

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

  /* ── iOS filter button ────────────────────────────────────── */

  .ios-filter-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    flex-shrink: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
    -webkit-tap-highlight-color: transparent;
  }

  .ios-filter-btn--active {
    background: var(--accent-subtle);
    color: var(--accent);
    border-color: var(--accent);
  }

  .ios-filter-badge {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    font-size: 0.55rem;
    font-weight: 700;
    display: flex;
    align-items: center;
    justify-content: center;
    line-height: 1;
  }

  /* ── iOS filter panel ────────────────────────────────────── */

  .ios-panel-backdrop {
    position: fixed;
    inset: 0;
    background: var(--overlay-bg);
    z-index: 299;
  }

  .ios-filter-panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(85vw, 340px);
    background: var(--bg-primary);
    border-left: 1px solid var(--border-strong);
    z-index: 300;
    display: flex;
    flex-direction: column;
    box-shadow: -4px 0 20px rgba(0, 0, 0, 0.15);
    padding-bottom: env(safe-area-inset-bottom, 0px);
  }

  .ios-panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 16px 12px;
    padding-top: calc(16px + env(safe-area-inset-top, 0px));
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .ios-panel-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .ios-panel-close {
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
  }

  .ios-panel-body {
    flex: 1;
    overflow-y: auto;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    -webkit-overflow-scrolling: touch;
  }

  .ios-panel-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .ios-panel-section-label {
    font-size: 0.68rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .ios-toolbar {
    border-bottom: 1px solid var(--border);
  }

  .ios-sort-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .ios-sort-count-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .ios-quick-filter {
    width: 100% !important;
  }

  /* On iOS, history dropdown stays within search bar bounds */
  .ios-search-row .history-dropdown {
    right: 0;
  }
</style>
