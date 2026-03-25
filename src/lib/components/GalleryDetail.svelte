<script lang="ts">
  import { t } from "$lib/i18n";
  import Slider from "./Slider.svelte";
  import FavoriteDialog from "./FavoriteDialog.svelte";
  import LocalMetadataEditor from "./LocalMetadataEditor.svelte";

  // When true, renders as a full-page inline element (no fixed overlay, no animation).
  // When false (default), renders as fixed side-panel overlay.
  let { fullPage = false }: { fullPage?: boolean } = $props();
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { detailGallery, detailPageThumbs, detailBatchState, detailOpenedAsLocal } from "$lib/stores/detail";
  import { readerGallery, readerPage, readerSessionId, readerSourceGallery } from "$lib/stores/reader";
  import { detailExpanded, detailPreviewSize, libraryRefreshTick } from "$lib/stores/ui";
  import { thumbSrc } from "$lib/utils/thumb";
  import { categoryColor } from "$lib/utils/category";
  import { onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { getFavoriteStatus, folderColor } from "$lib/api/favorites";
  import { submitDownloadQueue, getLocalGalleryPages, deleteLocalGallery, syncLocalGallery } from "$lib/api/library";
  import {
    getReadProgress,
    startReadingSession,
    fetchGalleryMetadata,
    setActiveDetailGallery,
    getDetailPreviewSize,
    setDetailPreviewSize,
    getGalleryPagesBatch,
  } from "$lib/api/reader";
  import { enqueuePageThumb, resetPageThumbs, setThumbReadyCallback } from "$lib/stores/pageThumbs";
  import type { GalleryPageEntry } from "$lib/api/reader";
  import type { Gallery } from "$lib/api/galleries";

  let gallery = $state<Gallery | null>(null);
  let loading = $state(false);
  let loadingMetadata = $state(false);
  // Snapshot of detailOpenedAsLocal at the moment the gallery opened.
  // Determines whether to go fully offline (no network calls).
  let openedAsLocal = $state(false);

  // Page entries indexed by page_index. Sparse — only populated for fetched batches.
  let pageEntries = $state<Record<number, GalleryPageEntry>>({});
  // Total number of pages in the gallery (from first batch response).
  let totalPageCount = $state(0);

  // page_index -> local thumbnail path (downloaded via Rust).
  let pageThumbPaths = $state<Record<number, string>>({});

  // Track which gallery we opened to avoid stale updates.
  let currentGid = $state<number | null>(null);

  // Showkey from the gallery pages response.
  let showkey = $state<string | null>(null);

  // Expand/collapse state.
  let expanded = $state(false);

  // Preview thumbnail size — synced with store/config.
  let previewSize = $state(120);

  // --- Batch loading state ---
  // Which detail pages (p=0, p=1, ...) have been fetched or are being fetched.
  let fetchedDetailPages = new Set<number>();
  let fetchingDetailPages = new Set<number>();
  // How many thumbnails ExHentai returns per detail page (20 or 40 depending on site settings).
  // Learned from the p=0 response and passed to all subsequent batch fetches.
  let pagesPerBatch = 20;

  // Thumbnail download tracking — queue/concurrency managed by pageThumbs service.

  // IntersectionObservers for batch sentinels.
  let batchObservers: IntersectionObserver[] = [];

  // Track whether component is still alive (not destroyed).
  let alive = true;

  // Download to local library state.
  let downloading = $state(false);
  let downloadMessage = $state("");

  onDestroy(() => {
    alive = false;
    setThumbReadyCallback(null);
    cleanupAll();
    // Cancel pending page thumbnail downloads.
    setActiveDetailGallery(null).catch(() => {});
    // Wipe shared state only if the reader isn't open (it may be holding a reference).
    if (!get(readerGallery)) {
      detailBatchState.set(null);
    }
  });

  // Load saved preview size on mount.
  $effect(() => {
    getDetailPreviewSize().then((size) => {
      previewSize = size;
      $detailPreviewSize = size;
    }).catch(() => {});
  });

  // Keep local expanded state in sync with store.
  $effect(() => {
    expanded = $detailExpanded;
  });

  // Subscribe to store — load on open.
  $effect(() => {
    const g = $detailGallery;
    imgLoaded = false;
    loadingMetadata = false;
    showkey = null;
    favcat = null;
    favnote = "";
    showFavDialog = false;
    downloading = false;
    downloadMessage = "";

    // Detect same-gallery re-open (e.g. returning from the reader).
    // We use detailBatchState.gid instead of currentGid because currentGid gets
    // nulled when the detail closes to open the reader (g=null branch below), so
    // checking currentGid would always see a new gallery after reader exit.
    const existingBs = get(detailBatchState);
    const sameGallery = g !== null && existingBs !== null && existingBs.gid === g.gid;

    if (!sameGallery) {
      totalPageCount = 0;
      fetchedDetailPages = new Set();
      fetchingDetailPages = new Set();
      pagesPerBatch = 20;
      cleanupObservers();
      // Only wipe detailBatchState here when opening a NEW gallery (g !== null).
      // When g=null (detail closing to open reader), the batch state must survive
      // so the reader can use it and sameGallery detection works on return.
      // The g=null branch below handles wiping when the reader is not open.
      if (g !== null) {
        detailBatchState.set(null);
      }
    } else {
      // Restore local state from shared store so the detail page renders instantly.
      fetchedDetailPages = existingBs!.fetchedDetailPages;
      pageEntries = existingBs!.pageEntries;
      pagesPerBatch = existingBs!.pagesPerBatch;
      showkey = existingBs!.showkey;
      if (existingBs!.totalPageCount > 0) {
        totalPageCount = existingBs!.totalPageCount;
        console.log(`[GalleryDetail] TOTAL_PAGES_SET: gid=${g!.gid} totalPages=${totalPageCount} source=RESTORED_FROM_BATCH_STATE`);
      }
    }

    if (g) {
      gallery = g;
      currentGid = g.gid;
      // Always re-capture openedAsLocal when any gallery opens (including same-gallery
      // re-opens from reader) so a page switch (home → library) always reflects the
      // correct context. get() avoids making detailOpenedAsLocal a reactive dependency
      // of this effect (which would cause spurious re-runs).
      openedAsLocal = get(detailOpenedAsLocal);
      // Reset service queue for this gallery and register our write-back callback.
      resetPageThumbs(g.gid);
      setThumbReadyCallback((pageIdx, rawPath) => {
        if (currentGid === g.gid && alive) {
          pageThumbPaths[pageIdx] = rawPath;
        }
      });
      // Recover any thumbnail paths already cached in the shared store (e.g. loaded
      // by the reader while it was open for this same gallery) so we don't re-fetch.
      // Use get() (non-reactive) so reading the store here doesn't make it a
      // dependency of this effect.
      const existingThumbs = get(detailPageThumbs);
      if (existingThumbs && existingThumbs.gid === g.gid) {
        pageThumbPaths = { ...existingThumbs.paths };
      } else {
        pageThumbPaths = {};
        detailPageThumbs.set({ gid: g.gid, paths: {} });
      }
      if (!sameGallery) {
        pageEntries = {};
      }
      // Register active gallery for cancellation.
      setActiveDetailGallery(g.gid).catch(() => {});
      // Initialise the shared batch state for a new gallery open.
      // On same-gallery re-open the existing store is kept as-is (already restored above).
      if (!sameGallery) {
        detailBatchState.set({
          gid: g.gid,
          token: g.token,
          showkey: null,
          pagesPerBatch: 20,
          totalPageCount: 0,
          fetchedDetailPages,   // the freshly-reset Set
          pageEntries,          // the freshly-reset Record
        });
      }
      if (openedAsLocal) {
        // Opened from local library: fully offline. Load pages from DB only.
        loadLocalPages(g.gid);
      } else {
        // Opened from home/search/favorites: fetch metadata and pages from ExHentai.
        loadMetadata(g.gid, g.token);
        // Fetch first batch of pages (p=0) — fetchBatch is a no-op if already fetched.
        fetchBatch(g.gid, g.token, 0);
      }
      // Load favorite status from local DB (fast, no network).
      loadFavoriteStatus(g.gid);
    } else {
      gallery = null;
      currentGid = null;
      setThumbReadyCallback(null);
      // Don't reset pageEntries/totalPageCount here — the reader may still be open
      // and holding references into the same objects. They'll be reset on next gallery open.
      pageThumbPaths = {};
      // Only wipe the shared caches if the reader is not currently open.
      // When the detail closes because the reader opened, we must preserve both
      // detailPageThumbs and detailBatchState so the reader can use them immediately.
      // Use get() (non-reactive) to avoid making readerGallery a dependency
      // of this effect, which would re-trigger it when the reader opens/closes.
      if (!get(readerGallery)) {
        detailPageThumbs.set(null);
        detailBatchState.set(null);
        // Cancel pending page thumbnail downloads only when the reader is not
        // about to take ownership of the active gallery slot. If the reader is
        // opening it will call setActiveDetailGallery(gid) and must not be
        // overwritten by a trailing null from here.
        setActiveDetailGallery(null).catch(() => {});
      }
      cleanupAll();
    }
  });

  function cleanupObservers() {
    for (const obs of batchObservers) {
      obs.disconnect();
    }
    batchObservers = [];
  }

  function cleanupAll() {
    cleanupObservers();
    fetchingDetailPages.clear();
  }

  async function loadMetadata(gid: number, token: string) {
    loadingMetadata = true;
    try {
      const enriched = await fetchGalleryMetadata(gid, token);
      if (currentGid === gid) {
        gallery = enriched;
      }
    } catch (err) {
      console.error("Failed to fetch gallery metadata:", err);
    } finally {
      if (currentGid === gid) {
        loadingMetadata = false;
      }
    }
  }

  async function loadFavoriteStatus(gid: number) {
    try {
      const status = await getFavoriteStatus(gid);
      if (currentGid === gid) {
        favcat = status.favcat;
        favnote = status.favnote;
      }
    } catch {
      // Non-critical — ignore
    }
  }

  /** Load pages for a local gallery from DB and populate thumbnails from local file paths. */
  async function loadLocalPages(gid: number) {
    try {
      const localPages = await getLocalGalleryPages(gid);
      if (!alive || currentGid !== gid) return;
      totalPageCount = localPages.length;
      // Populate pageEntries with local image paths as thumb — no network needed.
      for (const p of localPages) {
        pageEntries[p.page_index] = {
          page_index: p.page_index,
          page_url: "",
          image_path: p.file_path,
          thumb_url: null,
          imgkey: null,
        };
        // Use local file path directly as the page thumbnail.
        pageThumbPaths[p.page_index] = p.file_path;
      }
      // Update shared batch state so reader can use the page data.
      const bs = get(detailBatchState);
      if (bs && bs.gid === gid) {
        detailBatchState.set({ ...bs, totalPageCount: localPages.length });
      }
    } catch (err) {
      console.error("Failed to load local pages:", err);
    }
  }

  function handleFavoriteClick() {
    showFavDialog = true;
  }

  async function handleDownload() {
    if (!gallery || downloading) return;
    downloading = true;
    downloadMessage = "";
    try {
      const result = await submitDownloadQueue(
        [{ gid: gallery.gid, token: gallery.token }],
        true,
        undefined,
      );
      if (result.skippedAlreadyLocal > 0) {
        downloadMessage = $t("detail.download_already_local");
      } else {
        downloadMessage = $t("detail.download_queued");
      }
    } catch (err) {
      downloadMessage = String(err);
    } finally {
      downloading = false;
      // Clear message after 3 seconds.
      setTimeout(() => { downloadMessage = ""; }, 3000);
    }
  }

  function handleFavDialogClose() {
    showFavDialog = false;
  }

  function handleFavUpdated(newFavcat: number | null, newNote: string) {
    favcat = newFavcat;
    favnote = newNote;
  }

  async function fetchBatch(gid: number, token: string, detailPage: number) {
    if (!alive || currentGid !== gid) return;
    if (fetchedDetailPages.has(detailPage) || fetchingDetailPages.has(detailPage)) {
      // Already fetched — re-enqueue any thumbnails not yet downloaded (e.g. on
      // return from the reader). Deferred so this runs outside the $effect's reactive
      // tracking window (pageEntries/$state reads must not become tracked dependencies).
      // enqueuePageThumb deduplicates internally — safe to call unconditionally.
      const start = detailPage * pagesPerBatch;
      const end = start + pagesPerBatch;
      setTimeout(() => {
        if (!alive || currentGid !== gid) return;
        for (let i = start; i < end; i++) {
          if (i in pageEntries && !(i in pageThumbPaths)) {
            enqueueThumbDownload(i);
          }
        }
        setupSentinelObservers(gid, token);
      }, 0);
      return;
    }

    fetchingDetailPages.add(detailPage);

    try {
      // Pass pagesPerBatch for p=1+ so backend can compute the correct base_index.
      const ppb = detailPage === 0 ? undefined : pagesPerBatch;
      const result = await getGalleryPagesBatch(gid, token, detailPage, ppb);
      if (!alive || currentGid !== gid) return;

      // After p=0, record how many pages ExHentai returned per detail page.
      if (detailPage === 0 && result.pages.length > 0) {
        pagesPerBatch = result.pages.length;
        console.log(`[GalleryDetail] GALLERY_PAGES_PARSED: gid=${gid} pages_per_batch=${pagesPerBatch} total_pages=${result.total_pages}`);
      }

      // Store results.
      for (const entry of result.pages) {
        pageEntries[entry.page_index] = entry;
      }
      if (result.showkey && !showkey) {
        showkey = result.showkey;
      }
      if (result.total_pages > totalPageCount) {
        totalPageCount = result.total_pages;
        console.log(`[GalleryDetail] TOTAL_PAGES_SET: gid=${gid} totalPages=${totalPageCount} source=FIRST_BATCH`);
      }
      fetchedDetailPages.add(detailPage);

      // Mirror scalar updates into the shared batch state so GalleryReader sees them.
      // The Set and Record are the SAME objects (mutated above in-place).
      const bs = get(detailBatchState);
      if (bs && bs.gid === gid) {
        const needsUpdate =
          bs.pagesPerBatch !== pagesPerBatch ||
          bs.totalPageCount !== totalPageCount ||
          (result.showkey && !bs.showkey);
        if (needsUpdate) {
          detailBatchState.set({ ...bs, pagesPerBatch, totalPageCount, showkey: showkey ?? bs.showkey });
        }
      }

      // Start downloading thumbnails for this batch.
      for (const entry of result.pages) {
        enqueueThumbDownload(entry.page_index);
      }

      // Set up observers for all not-yet-fetched batches after a short tick
      // so the DOM has rendered the new placeholder slots.
      requestAnimationFrame(() => {
        if (alive && currentGid === gid) {
          setupSentinelObservers(gid, token);
        }
      });
    } catch (err) {
      // Cancelled or failed — ignore if navigated away.
      if (currentGid === gid) {
        console.error(`Failed to fetch batch p=${detailPage}:`, err);
      }
    } finally {
      fetchingDetailPages.delete(detailPage);
    }
  }

  function setupSentinelObservers(gid: number, token: string) {
    // Clean up old observers.
    cleanupObservers();

    // For each unfetched detail page, observe every thumbnail in the batch.
    // Any one becoming visible triggers the fetch — ensures the batch loads even
    // if the user scrolls into the middle of an unfetched range.
    const totalDetailPages = Math.ceil(totalPageCount / pagesPerBatch);
    for (let dp = 0; dp < totalDetailPages; dp++) {
      if (fetchedDetailPages.has(dp) || fetchingDetailPages.has(dp)) continue;

      const start = dp * pagesPerBatch;
      const end = Math.min(start + pagesPerBatch, totalPageCount);

      const observer = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            if (entry.isIntersecting && alive && currentGid === gid) {
              observer.disconnect();
              fetchBatch(gid, token, dp);
            }
          }
        },
        { root: null, rootMargin: "400px", threshold: 0 }
      );

      let observed = 0;
      for (let i = start; i < end; i++) {
        const el = document.querySelector(`[data-page-sentinel="${i}"]`) as HTMLElement | null;
        if (el) { observer.observe(el); observed++; }
      }
      if (observed === 0) { observer.disconnect(); continue; }

      batchObservers.push(observer);
    }
  }

  function enqueueThumbDownload(pageIdx: number) {
    if (pageIdx in pageThumbPaths || currentGid === null) return;
    const entry = pageEntries[pageIdx];
    if (!entry?.thumb_url) return;
    enqueuePageThumb(currentGid, pageIdx, entry.thumb_url);
  }

  // Favorite state
  let favcat = $state<number | null>(null);
  let favnote = $state("");
  let showFavDialog = $state(false);

  // Local metadata editor
  let showMetadataEditor = $state(false);

  let imgLoaded = $state(false);
  let imgSrc = $derived(
    !gallery ? "" :
    (openedAsLocal && !gallery.thumb_path)
      ? (pageEntries[0]?.image_path ? convertFileSrc(pageEntries[0].image_path)
          : pageThumbPaths[0] ? convertFileSrc(pageThumbPaths[0])
          : thumbSrc(gallery.thumb_path, gallery.thumb_url))
      : thumbSrc(gallery.thumb_path, gallery.thumb_url)
  );
  let catColor = $derived(gallery ? categoryColor(gallery.category) : "#9e9e9e");
  let langTag = $derived(gallery?.tags.find(t => t.namespace === "language")?.name ?? null);
  let date = $derived(
    gallery && gallery.posted > 0
      ? new Date(gallery.posted * 1000).toLocaleDateString()
      : ""
  );
  let stars = $derived(gallery ? "\u2605".repeat(Math.round(gallery.rating)) : "");

  // Generate page index array for rendering (0..totalPageCount-1).
  let pageIndices = $derived(
    totalPageCount > 0 ? Array.from({ length: totalPageCount }, (_, i) => i) : []
  );

  // Group tags by namespace.
  let tagGroups = $derived(() => {
    if (!gallery) return [];
    const groups = new Map<string, string[]>();
    for (const tag of gallery.tags) {
      const arr = groups.get(tag.namespace) || [];
      arr.push(tag.name);
      groups.set(tag.namespace, arr);
    }
    return Array.from(groups.entries()).map(([ns, names]) => ({ namespace: ns, names }));
  });

  function handleClose() {
    $detailGallery = null;
  }

  function toggleExpanded() {
    expanded = !expanded;
    $detailExpanded = expanded;
  }

  function handleCollapse() {
    $detailGallery = null;
  }

  function handlePreviewSizeChange(val: number) {
    previewSize = val;
    $detailPreviewSize = val;
    setDetailPreviewSize(val).catch(() => {});
  }

  /** Build a GalleryPages object for the reader.
   *  Always uses totalPageCount as total_pages (the truth from HTML).
   *  Fills loaded entries from pageEntries; unloaded slots get empty stubs so the
   *  reader knows every page exists and can fetch their batch on demand. */
  function buildReaderPages(startPage: number) {
    if (!gallery) return null;
    // totalPageCount from the HTML parse is authoritative.
    // Prefer detailBatchState first (most up-to-date, survives same-galaxy re-open),
    // then local variable (in case store was just set), then file_count fallback.
    const bs = get(detailBatchState);
    const bsCount = (bs?.gid === gallery.gid ? bs.totalPageCount : 0) ?? 0;
    const count = bsCount || totalPageCount || gallery.file_count || 0;
    console.log(`[GalleryDetail] TOTAL_PAGES_SET: gid=${gallery.gid} totalPages=${count} source=BUILD_READER_PAGES (local=${totalPageCount} bs=${bsCount} file_count=${gallery.file_count})`);
    const dense: GalleryPageEntry[] = [];
    for (let i = 0; i < count; i++) {
      dense.push(pageEntries[i] ?? {
        page_index: i,
        page_url: "",
        image_path: null,
        thumb_url: null,
        imgkey: null,
      });
    }
    return {
      gid: gallery.gid,
      token: gallery.token,
      title: gallery.title,
      pages: dense,
      total_pages: count,
      showkey,
    };
  }

  async function buildLocalReaderPages() {
    if (!gallery) return null;
    const localPages = await getLocalGalleryPages(gallery.gid);
    const dense: GalleryPageEntry[] = localPages.map((p) => ({
      page_index: p.page_index,
      page_url: "",
      image_path: p.file_path,
      thumb_url: null,
      imgkey: null,
    }));
    return {
      gid: gallery.gid,
      token: gallery.token,
      title: gallery.title,
      pages: dense,
      total_pages: dense.length,
      showkey: null,
    };
  }

  async function handleRead() {
    if (!gallery || loading) return;
    loading = true;
    try {
      let startPage = 0;
      const progress = await getReadProgress(gallery.gid);
      if (progress && !progress.is_completed) {
        startPage = progress.last_page_read;
      }
      const pages = openedAsLocal
        ? await buildLocalReaderPages()
        : buildReaderPages(startPage);
      if (!pages) return;
      const now = Math.floor(Date.now() / 1000);
      const sessionId = await startReadingSession(gallery.gid, now);
      $readerSessionId = sessionId;
      $readerPage = startPage;
      $readerSourceGallery = gallery;
      $readerGallery = pages;
      $detailGallery = null;
    } catch (err) {
      console.error("Failed to open reader:", err);
    } finally {
      loading = false;
    }
  }

  async function handleOpenPage(pageIdx: number) {
    if (!gallery || loading) return;
    loading = true;
    try {
      const pages = openedAsLocal
        ? await buildLocalReaderPages()
        : buildReaderPages(pageIdx);
      if (!pages) return;
      const now = Math.floor(Date.now() / 1000);
      const sessionId = await startReadingSession(gallery.gid, now);
      $readerSessionId = sessionId;
      $readerPage = pageIdx;
      $readerSourceGallery = gallery;
      $readerGallery = pages;
      $detailGallery = null;
    } catch (err) {
      console.error("Failed to open reader at page:", err);
    } finally {
      loading = false;
    }
  }

  async function handleDeleteLocal() {
    if (!gallery) return;
    const confirmed = confirm($t("detail.delete_local_confirm"));
    if (!confirmed) return;
    downloading = true;
    downloadMessage = "";
    try {
      await deleteLocalGallery(gallery.gid);
      // Signal library page to refresh before closing.
      $libraryRefreshTick += 1;
      $detailGallery = null;
    } catch (err) {
      downloadMessage = String(err);
      downloading = false;
    }
  }

  async function handleSync() {
    if (!gallery || downloading) return;
    if (!gallery.origin || !gallery.remote_gid) {
      alert($t("detail.sync_local_no_origin"));
      return;
    }
    downloading = true;
    downloadMessage = "";
    try {
      await syncLocalGallery(gallery.gid);
      downloadMessage = $t("detail.sync_local");
    } catch (err) {
      downloadMessage = String(err);
    } finally {
      downloading = false;
      setTimeout(() => { downloadMessage = ""; }, 3000);
    }
  }

  function pageThumbSrc(pageIdx: number): string | null {
    const localPath = pageThumbPaths[pageIdx];
    if (localPath) return convertFileSrc(localPath);
    return null;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && gallery && !showMetadataEditor) {
      if (fullPage) {
        handleCollapse();
      } else {
        handleClose();
      }
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if showFavDialog && gallery}
  <FavoriteDialog
    gid={gallery.gid}
    token={gallery.token}
    currentFavcat={favcat}
    currentNote={favnote}
    onClose={handleFavDialogClose}
    onUpdated={handleFavUpdated}
  />
{/if}

{#if showMetadataEditor && gallery}
  <LocalMetadataEditor
    gallery={gallery}
    onClose={() => { showMetadataEditor = false; }}
  />
{/if}

{#if gallery}
  {#if !fullPage}
    <div class="detail-overlay" onclick={handleClose} role="presentation"></div>
  {/if}
  <div class="detail-panel" class:expanded class:full-page={fullPage}>
    <!-- Top bar -->
    <div class="detail-header">
      <button class="back-btn" onclick={fullPage ? handleCollapse : handleClose}>
        <svg width="18" height="18" viewBox="0 0 16 16" fill="none">
          <path d="M10 4L6 8l4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        {$t("common.back")}
      </button>

      <div class="header-controls">
        <!-- Preview size slider -->
        <div class="size-control" title={$t("settings.detail_preview_size")}>
          <Slider min={80} max={200} bind:value={previewSize} onChange={handlePreviewSizeChange} />
        </div>

        <!-- Expand/collapse toggle -->
        <button class="expand-btn" onclick={toggleExpanded} title={expanded ? "Collapse" : "Expand"}>
          {#if expanded}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 14 10 14 10 20"/><polyline points="20 10 14 10 14 4"/><line x1="14" y1="10" x2="21" y2="3"/><line x1="3" y1="21" x2="10" y2="14"/></svg>
          {:else}
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" y1="3" x2="14" y2="10"/><line x1="3" y1="21" x2="10" y2="14"/></svg>
          {/if}
        </button>
      </div>
    </div>

    <div class="detail-body">
      <!-- Top section: cover + title (instant, from cached data) -->
      <div class="top-section">
        <div class="preview">
          <div class="preview-skeleton" class:hidden={imgLoaded}></div>
          {#if imgSrc}
            <img src={imgSrc} alt={gallery.title} class:loaded={imgLoaded} onload={() => imgLoaded = true} />
          {/if}
        </div>
        <div class="top-info">
          <div class="title-row">
            <h2>{gallery.title}</h2>
            <span class="category-badge" style="background:{catColor}">{gallery.category}</span>
          </div>
          {#if gallery.title_jpn}
            <p class="title-jpn">{gallery.title_jpn}</p>
          {/if}
          {#if gallery.uploader}
            <p class="uploader">{$t("detail.uploader")}: {gallery.uploader}</p>
          {:else if loadingMetadata}
            <div class="skeleton-line" style="width:120px"></div>
          {/if}
        </div>
      </div>

      <!-- Metadata row: shows skeleton until API metadata arrives -->
      <div class="meta-grid">
        {#if langTag}
          <div class="meta-item">
            <span class="meta-label">{$t("detail.language")}</span>
            <span class="meta-value">{langTag}</span>
          </div>
        {/if}
        <div class="meta-item">
          <span class="meta-label">{$t("detail.page_count")}</span>
          <span class="meta-value">{gallery.file_count}</span>
        </div>
        {#if gallery.file_size}
          <div class="meta-item">
            <span class="meta-label">{$t("detail.file_size")}</span>
            <span class="meta-value">{(gallery.file_size / 1024 / 1024).toFixed(1)} MB</span>
          </div>
        {:else if loadingMetadata}
          <div class="meta-item">
            <span class="meta-label">{$t("detail.file_size")}</span>
            <div class="skeleton-line" style="width:50px"></div>
          </div>
        {/if}
        <div class="meta-item">
          <span class="meta-label">{$t("detail.rating_count")}</span>
          <span class="meta-value">{stars} {gallery.rating.toFixed(1)}</span>
        </div>
        {#if date}
          <div class="meta-item">
            <span class="meta-label">{$t("detail.uploaded")}</span>
            <span class="meta-value">{date}</span>
          </div>
        {:else if loadingMetadata}
          <div class="meta-item">
            <span class="meta-label">{$t("detail.uploaded")}</span>
            <div class="skeleton-line" style="width:80px"></div>
          </div>
        {/if}
      </div>

      <!-- Action buttons -->
      <div class="actions">
        <button class="action-btn primary" onclick={handleRead} disabled={loading}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h6a4 4 0 014 4v14a3 3 0 00-3-3H2z"/><path d="M22 3h-6a4 4 0 00-4 4v14a3 3 0 013-3h7z"/></svg>
          {$t("detail.read")}
        </button>
        {#if openedAsLocal}
          <button
            class="action-btn danger"
            onclick={handleDeleteLocal}
            disabled={downloading}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 011-1h4a1 1 0 011 1v2"/></svg>
            {downloading ? $t("common.loading") : $t("detail.delete_local")}
          </button>
        {:else}
          <button
            class="action-btn"
            onclick={handleDownload}
            disabled={downloading}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
            {downloading ? $t("common.loading") : $t("detail.download")}
          </button>
        {/if}
        <button
          class="action-btn"
          class:favorited={favcat !== null}
          onclick={handleFavoriteClick}
          style={favcat !== null ? `--fav-color: ${folderColor(favcat)}` : ""}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill={favcat !== null ? "var(--fav-color, currentColor)" : "none"} stroke={favcat !== null ? "var(--fav-color, currentColor)" : "currentColor"} stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"/></svg>
          {$t("detail.favorite")}
        </button>
        <button class="action-btn" disabled>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2L15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2z"/></svg>
          {$t("detail.rate")}
        </button>
        <button class="action-btn" disabled>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
          {$t("detail.search_similar")}
        </button>
        {#if openedAsLocal}
          <button class="action-btn" onclick={() => showMetadataEditor = true}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
            {$t("local.edit_metadata")}
          </button>
          {#if gallery.origin && gallery.remote_gid}
            <button class="action-btn" onclick={handleSync} disabled={downloading}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/></svg>
              {$t("detail.sync_local")}
            </button>
          {/if}
        {/if}
      </div>
      {#if downloadMessage}
        <p class="download-msg">{downloadMessage}</p>
      {/if}

      <!-- Tags section -->
      {#if tagGroups().length > 0}
        <div class="tags-section">
          <h3>{$t("detail.tags")}</h3>
          <div class="tag-groups">
            {#each tagGroups() as group}
              <div class="tag-group">
                <span class="tag-namespace">{group.namespace}:</span>
                <div class="tag-names">
                  {#each group.names as name}
                    <span class="tag-chip">{name}</span>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Page preview thumbnails — lazy loaded by batch via IntersectionObserver -->
      {#if pageIndices.length > 0}
        <div class="pages-section">
          <h3>{$t("detail.page_count")} ({totalPageCount})</h3>
          <div
            class="pages-grid"
            style="grid-template-columns: repeat(auto-fill, minmax({previewSize}px, 1fr));"
          >
            {#each pageIndices as idx (idx)}
              <button
                class="page-thumb-wrapper"
                data-page-sentinel={idx}
                onclick={() => handleOpenPage(idx)}
              >
                <div class="page-thumb" style="height:{Math.round(previewSize * 1.5)}px;">
                  {#if pageThumbSrc(idx)}
                    <img src={pageThumbSrc(idx)} alt="Page {idx + 1}" loading="lazy" />
                  {:else if idx in pageEntries}
                    <div class="page-thumb-skeleton">
                      <span class="page-thumb-skeleton-num">{idx + 1}</span>
                    </div>
                  {:else}
                    <div class="page-placeholder">{idx + 1}</div>
                  {/if}
                </div>
                <span class="page-num-label">{idx + 1}</span>
              </button>
            {/each}
          </div>
        </div>
      {:else if gallery}
        <div class="pages-section">
          <h3>{$t("detail.page_count")}</h3>
          <div
            class="pages-grid"
            style="grid-template-columns: repeat(auto-fill, minmax({previewSize}px, 1fr));"
          >
            {#each Array(gallery?.file_count || 10) as _, idx}
              <div class="page-thumb-skeleton-item" style="height:{Math.round(previewSize * 1.5)}px;">
                <span class="page-thumb-skeleton-num">{idx + 1}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .detail-overlay {
    position: fixed;
    inset: 0;
    background: var(--overlay-bg);
    z-index: 500;
  }

  .detail-panel {
    position: fixed;
    top: 0;
    right: 0;
    width: 520px;
    max-width: 95vw;
    height: 100vh;
    background: var(--bg-primary);
    border-left: 1px solid var(--border);
    box-shadow: -4px 0 24px var(--overlay-bg);
    z-index: 510;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: slideIn 0.2s ease;
    will-change: transform;
    transition: width 0.2s ease, left 0.2s ease, border-left 0.2s ease;
  }

  .detail-panel.expanded {
    width: calc(100vw - var(--sidebar-width, 56px));
    left: var(--sidebar-width, 56px);
    border-left: none;
  }

  /* Full-page mode: renders as normal flow inside <main>, fills entire content area */
  .detail-panel.full-page {
    position: static;
    width: 100%;
    max-width: none;
    height: auto;
    flex: 1;
    min-height: 0;
    box-shadow: none;
    border-left: none;
    animation: none;
    will-change: auto;
    transition: none;
    z-index: auto;
  }

  @keyframes slideIn {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }

  .detail-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--bg-primary);
  }

  .header-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .size-control {
    width: 80px;
  }

  .expand-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-primary);
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s;
  }

  .expand-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.3rem 0.6rem;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .back-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .detail-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .top-section {
    display: flex;
    gap: 1rem;
  }

  .preview {
    width: 180px;
    flex-shrink: 0;
    border-radius: var(--radius-md);
    overflow: hidden;
    background: var(--bg-tertiary);
    position: relative;
    min-height: 270px;
  }

  .preview img {
    width: 100%;
    height: auto;
    display: block;
    opacity: 0;
    transition: opacity 0.3s ease;
    position: relative;
  }

  .preview img.loaded {
    opacity: 1;
  }

  .preview-skeleton {
    width: 100%;
    aspect-ratio: 2 / 3;
    background: var(--bg-tertiary);
    border-radius: 2px;
    animation: skeleton-pulse 1.8s ease-in-out infinite;
    position: absolute;
    inset: 0;
  }

  .preview-skeleton.hidden {
    animation: none;
    opacity: 0;
    transition: opacity 0.3s ease;
  }

  @keyframes skeleton-pulse {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 0.8; }
  }

  .top-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .title-row {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .title-row h2 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 700;
    line-height: 1.4;
    color: var(--text-primary);
    flex: 1;
  }

  .category-badge {
    font-size: 0.6rem;
    font-weight: 700;
    color: #fff;
    padding: 3px 10px;
    border-radius: 20px;
    text-transform: uppercase;
    white-space: nowrap;
    flex-shrink: 0;
    margin-top: 2px;
  }

  .title-jpn {
    margin: 0;
    font-size: 0.78rem;
    color: var(--text-muted);
    line-height: 1.35;
  }

  .uploader {
    margin: 0;
    font-size: 0.75rem;
    color: var(--text-secondary);
  }

  .skeleton-line {
    height: 14px;
    background: var(--bg-tertiary);
    border-radius: 2px;
    animation: skeleton-pulse 1.8s ease-in-out infinite;
  }

  .meta-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem 1.25rem;
    padding: 0.75rem 0;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .meta-item {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .meta-label {
    font-size: 0.68rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }

  .meta-value {
    font-size: 0.85rem;
    color: var(--text-primary);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.5rem 0.85rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-primary);
    color: var(--text-secondary);
    font-size: 0.78rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, box-shadow 0.15s;
    white-space: nowrap;
  }

  .action-btn:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
    box-shadow: var(--shadow-sm);
  }

  .action-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .action-btn.primary {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .action-btn.primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .action-btn.favorited {
    color: var(--fav-color, var(--accent));
    border-color: var(--fav-color, var(--border-strong));
  }

  .action-btn.active-dl {
    color: var(--green);
    border-color: var(--green);
    opacity: 0.7;
  }

  .action-btn.danger {
    color: var(--red, #e05252);
    border-color: var(--red, #e05252);
  }

  .action-btn.danger:hover:not(:disabled) {
    background: var(--red, #e05252);
    color: #fff;
  }

  .download-msg {
    margin: 4px 0 0;
    font-size: 0.72rem;
    color: var(--green);
    padding: 0 2px;
  }

  .tags-section h3 {
    margin: 0 0 0.5rem;
    font-size: 0.78rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }

  .tag-groups {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .tag-group {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
  }

  .tag-namespace {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-muted);
    min-width: 65px;
    padding-top: 2px;
    flex-shrink: 0;
  }

  .tag-names {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .tag-chip {
    font-size: 0.78rem;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    padding: 4px 10px;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .tag-chip:hover {
    background: var(--accent);
    color: #fff;
  }

  /* -- Page preview thumbnails -- */

  .pages-section h3 {
    margin: 0 0 0.5rem;
    font-size: 0.78rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }

  .pages-grid {
    display: grid;
    gap: 0.5rem;
  }

  .page-thumb-wrapper {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    cursor: pointer;
    border: none;
    padding: 0;
    background: transparent;
  }

  .page-thumb {
    width: 100%;
    overflow: hidden;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-tertiary);
    transition: border-color 0.15s;
  }

  .page-thumb-wrapper:hover .page-thumb {
    border-color: var(--accent);
  }

  .page-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    animation: fadeIn 0.2s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .page-thumb-skeleton {
    width: 100%;
    height: 100%;
    background: var(--bg-tertiary);
    animation: skeleton-pulse 1.5s ease-in-out infinite;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .page-thumb-skeleton-num {
    font-size: 0.8rem;
    color: var(--text-muted);
    opacity: 0.5;
    font-variant-numeric: tabular-nums;
  }

  .page-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .page-num-label {
    font-size: 0.72rem;
    color: var(--text-muted);
    text-align: center;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }

  .page-thumb-skeleton-item {
    background: var(--bg-tertiary);
    border-radius: 2px;
    animation: skeleton-pulse 1.8s ease-in-out infinite;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
