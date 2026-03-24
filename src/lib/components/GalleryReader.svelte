<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { slide } from "svelte/transition";
  import Slider from "./Slider.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { t } from "$lib/i18n";
  import {
    getGalleryImage,
    getGalleryPagesBatch,
    updateReadProgress,
    endReadingSession,
    onImageDownloadProgress,
    cancelImageDownloads,
    registerDownloadSession,
    setActiveDetailGallery,
  } from "$lib/api/reader";
  import { enqueuePageThumb, resetPageThumbs, setThumbReadyCallback } from "$lib/stores/pageThumbs";
  import type { GalleryPages, ReadProgress, ImageDownloadProgressEvent } from "$lib/api/reader";
  import { readerGallery, readerPage, readerMode, readerSessionId, readerSourceGallery } from "$lib/stores/reader";
  import { detailGallery, detailPageThumbs, detailBatchState } from "$lib/stores/detail";
  import { get } from "svelte/store";

  let gallery = $state<GalleryPages | null>(null);
  let currentPage = $state(0);
  let mode = $state<"page" | "scroll">("page");
  let sessionId = $state<number | null>(null);
  let pagesViewed = $state(new Set<number>());
  let showControls = $state(false);

  let loadedImages = $state<Record<number, string>>({});
  let loadingPages = $state<Record<number, true>>({});
  let errorPages = $state<Record<number, true>>({});
  let downloadStatus = $state<Record<number, string>>({}); // page_index -> status text
  let downloadProgress = $state<Record<number, number>>({}); // page_index -> 0-100 percent

  let scrollContainer: HTMLDivElement | undefined = $state(undefined);
  let stripEl: HTMLDivElement | undefined = $state(undefined);

  // Thumbnail strip state
  let thumbPaths = $state<Record<number, string>>({});

  // Throttle strip thumbnail requests
  let thumbRequestTimer: ReturnType<typeof setTimeout> | null = null;

  // Per-batch in-flight promises for strip batch fetches.
  // Allows loadImage to await an already-in-progress batch fetch rather than
  // duplicating the IPC call.
  let stripFetchInFlight = new Map<number, Promise<void>>();

  // Track the current gallery GID to detect gallery changes and clear stale image state.
  let activeGid = $state<number | null>(null);

  // Incremented each time a gallery is opened (even the same gallery re-opened).
  // Used as a dependency in the strip sentinel setup $effect so the effect always
  // re-runs on every open, even when showControls and activeGid are unchanged.
  let readerOpenCount = $state(0);

  // Track whether component is still alive (not destroyed).
  let alive = true;

  let totalPages = $derived(gallery ? gallery.total_pages : 0);

  // --- Batch loading state for the preview strip ---
  // Mirrors the detail page's batch mechanism so the reader can continue fetching
  // preview page entries (thumb_url, page_url, imgkey) for not-yet-loaded batches.
  let batchObservers: IntersectionObserver[] = [];
  // Local cache of which detail pages we are currently fetching (avoids double-fetch).
  let fetchingBatches = new Set<number>();

  const unsubs: (() => void)[] = [];
  onMount(async () => {
    unsubs.push(readerGallery.subscribe((g) => { gallery = g; }));
    unsubs.push(readerPage.subscribe((p) => { currentPage = p; }));
    unsubs.push(readerMode.subscribe((m) => { mode = m; }));
    unsubs.push(readerSessionId.subscribe((s) => { sessionId = s; }));

    // Listen for download progress events.
    const unlistenProgress = await onImageDownloadProgress((event) => {
      if (!alive || !gallery || event.gid !== gallery.gid) return;
      if (event.status === "done") {
        delete downloadStatus[event.page_index];
        delete downloadProgress[event.page_index];
        // Update loaded image if we have a path.
        if (event.path) {
          loadedImages[event.page_index] = convertFileSrc(event.path);
          delete loadingPages[event.page_index];
        }
      } else if (event.status === "error") {
        delete downloadStatus[event.page_index];
        delete downloadProgress[event.page_index];
      } else if (event.status === "rate_limited") {
        downloadStatus[event.page_index] = event.error ?? "Rate limited";
      } else if (event.status === "downloading") {
        downloadStatus[event.page_index] = "Downloading...";
      }
    });
    unsubs.push(unlistenProgress);

    window.addEventListener("keydown", handleKeydown);
  });

  onDestroy(() => {
    alive = false;
    unsubs.forEach((u) => u());
    setThumbReadyCallback(null);
    window.removeEventListener("keydown", handleKeydown);
    // Disconnect strip batch observers.
    for (const obs of batchObservers) obs.disconnect();
    batchObservers = [];
    // Cancel any in-progress image downloads for the current gallery.
    if (activeGid !== null) {
      tracing_log(`[GalleryReader] onDestroy: cancelling downloads for gid=${activeGid}`);
      cancelImageDownloads(activeGid).catch(() => {});
    }
  });

  function tracing_log(msg: string) {
    console.log(msg);
  }

  // Clear all image state when the gallery changes (prevents stale images from previous gallery).
  $effect(() => {
    const newGid = gallery ? gallery.gid : null;
    if (newGid !== activeGid) {
      // Cancel full-size image downloads for the previous gallery.
      if (activeGid !== null) {
        tracing_log(`[GalleryReader] gallery changed: cancelling downloads for gid=${activeGid}`);
        cancelImageDownloads(activeGid).catch(() => {});
      }
      // Disconnect all strip batch observers.
      for (const obs of batchObservers) obs.disconnect();
      batchObservers = [];
      fetchingBatches = new Set();
      stripFetchInFlight = new Map();
      loadedImages = {};
      loadingPages = {};
      errorPages = {};
      downloadStatus = {};
      downloadProgress = {};
      pagesViewed = new Set();
      thumbPaths = {};
      activeGid = newGid;
      // Register a new download session for the new gallery.
      if (newGid !== null) {
        const pageCount = gallery ? gallery.total_pages : 0;
        tracing_log(`[GalleryReader] GALLERY_PAGES_PARSED: gid=${newGid} total_pages=${pageCount} pages_in_pages_array=${gallery ? gallery.pages.length : 0}`);
        tracing_log(`[GalleryReader] registering download session for gid=${newGid}`);
        registerDownloadSession(newGid).catch(() => {});
        // Re-enable PageThumbCancellation so fetchStripBatch and getPageThumbnail work.
        // The detail's g=null branch calls setActiveDetailGallery(null) when it closes;
        // the reader must restore it here so strip thumb downloads aren't cancelled.
        setActiveDetailGallery(newGid).catch(() => {});
        // Reset the shared thumbnail service for this gallery and register our callback.
        resetPageThumbs(newGid);
        setThumbReadyCallback((pageIdx, rawPath) => {
          if (activeGid === newGid && alive) {
            thumbPaths[pageIdx] = convertFileSrc(rawPath);
          }
        });
        // Increment open counter so the strip sentinel $effect always re-runs on every
        // open, even if showControls is already true and activeGid hasn't changed.
        readerOpenCount++;
        // NOTE: do NOT call setupStripBatchObservers here — the strip is only rendered
        // when showControls=true. Observers are set up by the showControls $effect below.
      } else {
        // Gallery closed — release the callback so stale results don't write to thumbPaths.
        setThumbReadyCallback(null);
      }
    }
  });

  // Auto-scroll the preview strip to keep the current page centered.
  $effect(() => {
    if (!stripEl || !showControls) return;
    const THUMB_W = 52; // px — matches .thumb-item width in CSS
    const GAP = 4;
    const stride = THUMB_W + GAP;
    const target = currentPage * stride - stripEl.clientWidth / 2 + THUMB_W / 2;
    stripEl.scrollTo({ left: Math.max(0, target), behavior: "smooth" });
  });

  // Preload strategy: current (urgent), +1..+3 (preload), -1 (behind)
  $effect(() => {
    if (gallery && mode === "page" && alive) {
      // Urgent: current page.
      loadImage(currentPage);
      // Preload ahead: next 3 pages.
      for (let i = 1; i <= 3; i++) {
        if (currentPage + i < totalPages) loadImage(currentPage + i);
      }
      // Preload behind: previous page.
      if (currentPage - 1 >= 0) loadImage(currentPage - 1);
    }
  });

  $effect(() => {
    if (gallery) {
      pagesViewed.add(currentPage);
    }
  });

  async function loadImage(pageIdx: number) {
    if (!alive || !gallery) return;
    if (pageIdx in loadedImages || pageIdx in loadingPages) return;
    let entry = gallery.pages[pageIdx];

    // If entry is a stub (no page_url), try to get it from shared batch state.
    if (!entry || !entry.page_url) {
      const bs = get(detailBatchState);
      if (bs && bs.gid === gallery.gid && bs.pageEntries[pageIdx]?.page_url) {
        entry = bs.pageEntries[pageIdx];
        gallery.pages[pageIdx] = entry;
      }
    }

    // Still no page_url — the batch for this page hasn't been fetched yet.
    // Trigger and await fetchStripBatch so we have page_url before downloading.
    // fetchStripBatch registers itself in stripFetchInFlight, so concurrent
    // callers (IntersectionObserver + loadImage) share the same promise.
    if (!entry || !entry.page_url) {
      const bs = get(detailBatchState);
      const pagesPerBatch = bs?.pagesPerBatch ?? 20;
      const dp = Math.floor(pageIdx / pagesPerBatch);
      await fetchStripBatch(gallery.gid, dp);
      if (!alive || !gallery) return;
      // Re-read entry after batch fetch populated gallery.pages.
      entry = gallery.pages[pageIdx];
      if (!entry || !entry.page_url) return;
    }

    if (entry.image_path) {
      loadedImages[pageIdx] = convertFileSrc(entry.image_path);
      return;
    }

    loadingPages[pageIdx] = true;
    try {
      // Pass showpage API params (imgkey + showkey) for fast URL resolution.
      const path = await getGalleryImage(
        gallery.gid,
        pageIdx,
        entry.page_url,
        entry.imgkey,
        gallery.showkey,
      );
      if (!alive) return;
      tracing_log(`[GalleryReader] READER_IMAGE_LOAD: gid=${gallery.gid} page=${pageIdx} status=SUCCESS`);
      loadedImages[pageIdx] = convertFileSrc(path);
      gallery.pages[pageIdx].image_path = path;
      delete errorPages[pageIdx];
    } catch (err) {
      if (!alive) return;
      tracing_log(`[GalleryReader] READER_IMAGE_LOAD: gid=${gallery.gid} page=${pageIdx} status=FAIL error=${err}`);
      console.error(`Failed to load page ${pageIdx}:`, err);
      errorPages[pageIdx] = true;
    } finally {
      delete loadingPages[pageIdx];
    }
  }

  // Enqueue a page thumbnail via the shared pageThumbs service.
  // Skips if already displayed or no thumb_url available yet (stub).
  function enqueueThumb(pageIdx: number) {
    if (!alive || !gallery) return;
    if (pageIdx in thumbPaths) return;

    // Check shared store first — no IPC needed if already cached.
    const shared = get(detailPageThumbs);
    if (shared && shared.gid === gallery.gid && pageIdx in shared.paths) {
      thumbPaths[pageIdx] = convertFileSrc(shared.paths[pageIdx]);
      return;
    }

    // Also check the batch state's page entries in case a path was downloaded there.
    const bs = get(detailBatchState);
    if (bs && bs.gid === gallery.gid && bs.pageEntries[pageIdx]?.page_url) {
      // Merge entry into gallery.pages so it's available for loadImage too.
      // Merge if the existing slot lacks page_url OR thumb_url — the DB may have stored
      // a page entry with page_url but null thumb_url, leaving the thumb unresolvable
      // without the richer entry from detailBatchState.
      if (!gallery.pages[pageIdx]?.page_url || !gallery.pages[pageIdx]?.thumb_url) {
        gallery.pages[pageIdx] = bs.pageEntries[pageIdx];
      }
    }

    const entry = gallery.pages[pageIdx];
    if (!entry || !entry.thumb_url) return; // Stub — batch observer will deliver the entry.

    enqueuePageThumb(gallery.gid, pageIdx, entry.thumb_url);
  }

  // --- Strip batch loading (mirrors detail page IntersectionObserver mechanism) ---

  /** Set up IntersectionObservers on the strip for each unfetched batch boundary.
   *  Must only be called after the strip DOM has rendered (showControls=true) and
   *  stripEl is bound, so root: stripEl is valid for correct scroll-relative intersection. */
  function setupStripBatchObservers(gid: number) {
    if (!alive || !gallery || !stripEl) return;
    // Disconnect any existing observers.
    for (const obs of batchObservers) obs.disconnect();
    batchObservers = [];

    const bs = get(detailBatchState);
    const pagesPerBatch = bs?.pagesPerBatch ?? 20;
    const totalDetailPages = Math.ceil(totalPages / pagesPerBatch);
    const fetchedSet = bs?.fetchedDetailPages ?? new Set<number>();

    let sentinelsFound = 0;
    let alreadyFetched = 0;

    for (let dp = 0; dp < totalDetailPages; dp++) {
      if (fetchedSet.has(dp) || fetchingBatches.has(dp)) {
        alreadyFetched++;
        continue;
      }

      // Observe every thumbnail in the batch — any one becoming visible triggers the fetch.
      // This ensures the batch loads even if the user scrolls into the middle of an
      // unfetched range without ever passing the first thumbnail of the batch.
      const start = dp * pagesPerBatch;
      const end = Math.min(start + pagesPerBatch, totalPages);

      const observer = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            if (entry.isIntersecting && alive && activeGid === gid) {
              tracing_log(`[GalleryReader] STRIP_SENTINEL_VISIBLE: gid=${gid} detail_page=${dp}`);
              observer.disconnect();
              fetchStripBatch(gid, dp);
            }
          }
        },
        // root: stripEl so intersection is checked within the strip's scroll area.
        // rootMargin pre-loads one strip-width ahead of current scroll position.
        { root: stripEl, rootMargin: "0px 600px 0px 600px", threshold: 0 }
      );

      let observed = 0;
      for (let i = start; i < end; i++) {
        const el = stripEl.querySelector(`[data-strip-sentinel="${i}"]`) as HTMLElement | null;
        if (el) { observer.observe(el); observed++; }
      }
      if (observed === 0) { observer.disconnect(); continue; }

      sentinelsFound++;
      batchObservers.push(observer);
    }

    tracing_log(`[GalleryReader] STRIP_OBSERVERS_SETUP: gid=${gid} sentinels_found=${sentinelsFound} already_fetched=${alreadyFetched}`);
  }

  /** Fetch one detail-page batch of page entries and merge into gallery.pages.
   *  Registers the in-flight promise in stripFetchInFlight so loadImage can
   *  await it rather than firing a redundant call. */
  function fetchStripBatch(gid: number, detailPage: number): Promise<void> {
    // If there's already an in-flight fetch for this batch, return it.
    const existing = stripFetchInFlight.get(detailPage);
    if (existing) return existing;

    const p = _fetchStripBatchImpl(gid, detailPage);
    stripFetchInFlight.set(detailPage, p);
    p.finally(() => stripFetchInFlight.delete(detailPage));
    return p;
  }

  async function _fetchStripBatchImpl(gid: number, detailPage: number) {
    if (!alive || !gallery || activeGid !== gid) return;

    const bs = get(detailBatchState);
    // Don't fetch if already fetched (could be fetched by detail page in the background).
    if (bs?.fetchedDetailPages.has(detailPage)) {
      tracing_log(`[GalleryReader] STRIP_FETCH_BATCH: gid=${gid} detail_page=${detailPage} already_fetched=true`);
      // Entries may already be in pageEntries — sync into gallery.pages.
      syncBatchStateToGallery(gid);
      // Enqueue thumbnails for this batch's pages — entries are present in pageEntries
      // but enqueueThumb was never called for them since we skipped the IPC fetch.
      if (bs) {
        const ppb = bs.pagesPerBatch ?? 20;
        const start = detailPage * ppb;
        const end = Math.min(start + ppb, totalPages);
        for (let i = start; i < end; i++) {
          enqueueThumb(i);
        }
      }
      return;
    }

    tracing_log(`[GalleryReader] STRIP_FETCH_BATCH: gid=${gid} detail_page=${detailPage} already_fetched=false`);

    if (fetchingBatches.has(detailPage)) return;
    fetchingBatches.add(detailPage);

    const token = bs?.token ?? gallery.token;
    const pagesPerBatch = bs?.pagesPerBatch ?? 20;

    try {
      const ppb = detailPage === 0 ? undefined : pagesPerBatch;
      const result = await getGalleryPagesBatch(gid, token, detailPage, ppb);
      if (!alive || activeGid !== gid) return;

      // Merge entries into gallery.pages (mutate in place — Svelte 5 proxy tracks writes).
      for (const entry of result.pages) {
        gallery!.pages[entry.page_index] = entry;
      }

      // Update the shared batch state so the detail page sees the new entries.
      const cur = get(detailBatchState);
      if (cur && cur.gid === gid) {
        for (const entry of result.pages) {
          cur.pageEntries[entry.page_index] = entry;
        }
        cur.fetchedDetailPages.add(detailPage);
        // Update gallery showkey so reader can use fast path for image fetches.
        if (result.showkey && !gallery!.showkey) {
          gallery!.showkey = result.showkey;
        }
        // Update scalar fields that may have changed.
        // NOTE: totalPageCount is intentionally NOT updated here — it is authoritative
        // only in GalleryDetail.fetchBatch (set from HTML parse). Never overwrite it
        // from the reader side to prevent stale spreads from clobbering the real value.
        const newPagesPerBatch = (detailPage === 0 && result.pages.length > 0)
          ? result.pages.length
          : cur.pagesPerBatch;
        const newShowkey = result.showkey ?? cur.showkey;
        if (newPagesPerBatch !== cur.pagesPerBatch || newShowkey !== cur.showkey) {
          detailBatchState.set({ ...cur, pagesPerBatch: newPagesPerBatch, showkey: newShowkey });
        }
      }

      tracing_log(`[GalleryReader] STRIP_FETCH_DONE: gid=${gid} detail_page=${detailPage} new_pages=${result.pages.length}`);

      // Enqueue thumbnails for this batch.
      for (const entry of result.pages) {
        enqueueThumb(entry.page_index);
      }

      // Re-setup observers for any remaining unfetched batches.
      requestAnimationFrame(() => {
        if (alive && activeGid === gid) setupStripBatchObservers(gid);
      });
    } catch (err) {
      if (alive && activeGid === gid) {
        console.error(`[GalleryReader] fetchStripBatch p=${detailPage} failed:`, err);
      }
    } finally {
      fetchingBatches.delete(detailPage);
    }
  }

  /** Sync any entries from detailBatchState.pageEntries into gallery.pages. */
  function syncBatchStateToGallery(gid: number) {
    if (!gallery || activeGid !== gid) return;
    const bs = get(detailBatchState);
    if (!bs || bs.gid !== gid) return;
    for (const [idxStr, entry] of Object.entries(bs.pageEntries)) {
      const idx = Number(idxStr);
      const existing = gallery.pages[idx];
      if (!existing || !existing.page_url) {
        gallery.pages[idx] = entry;
      }
    }
  }

  function handleStripWheel(e: WheelEvent) {
    e.preventDefault();
    (e.currentTarget as HTMLElement).scrollLeft += e.deltaY;
  }

  // Attach wheel listener as non-passive so preventDefault() works.
  function stripWheelAction(node: HTMLElement) {
    node.addEventListener("wheel", handleStripWheel, { passive: false });
    return { destroy() { node.removeEventListener("wheel", handleStripWheel); } };
  }

  function handleStripScroll() {
    if (!stripEl || !gallery) return;
    // Debounce at 300ms so rapid scrolling doesn't fire dozens of requests.
    if (thumbRequestTimer !== null) clearTimeout(thumbRequestTimer);
    thumbRequestTimer = setTimeout(() => {
      if (!stripEl || !gallery || !activeGid) return;
      const THUMB_W = 52;
      const GAP = 4;
      const stride = THUMB_W + GAP;
      const left = stripEl.scrollLeft;
      const right = left + stripEl.clientWidth;
      // Load a 2-thumb buffer outside the visible window.
      const startIdx = Math.max(0, Math.floor(left / stride) - 2);
      const endIdx = Math.min(totalPages - 1, Math.ceil(right / stride) + 2);
      for (let i = startIdx; i <= endIdx; i++) {
        enqueueThumb(i);
      }
      // Re-setup batch observers in case new sentinels are now in range.
      setupStripBatchObservers(activeGid);
    }, 300);
  }

  // Sync any newly-available shared store paths into local thumbPaths reactively.
  $effect(() => {
    const shared = $detailPageThumbs;
    if (!shared || !gallery || shared.gid !== gallery.gid) return;
    for (const [idxStr, rawPath] of Object.entries(shared.paths)) {
      const idx = Number(idxStr);
      if (!(idx in thumbPaths)) {
        thumbPaths[idx] = convertFileSrc(rawPath);
      }
    }
  });

  // Load thumbnails for the visible strip area whenever strip becomes visible.
  // Limited to ±5 pages (11 total max) to avoid burst requests.
  // Also (re-)setup batch sentinels so unfetched batches trigger a fetch.
  // readerOpenCount is read here as a dependency so this effect re-runs on every
  // gallery open, even when showControls is already true and activeGid is unchanged.
  $effect(() => {
    void readerOpenCount; // dependency — forces re-run on every open
    if (!showControls || !gallery || mode !== "page" || !activeGid) return;
    const gid = activeGid;
    const half = 5;
    const start = Math.max(0, currentPage - half);
    const end = Math.min(totalPages - 1, currentPage + half);
    for (let i = start; i <= end; i++) {
      enqueueThumb(i);
    }
    // Setup observers after a tick so the strip DOM has rendered.
    requestAnimationFrame(() => {
      if (alive && activeGid === gid) setupStripBatchObservers(gid);
    });
  });

  function goToPage(idx: number) {
    if (idx >= 0 && idx < totalPages) {
      $readerPage = idx;
      saveProgress(idx);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!gallery) return;
    if (mode === "page") {
      if (e.key === "ArrowRight" || e.key === " ") {
        e.preventDefault();
        goToPage(currentPage + 1);
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        goToPage(currentPage - 1);
      } else if (e.key === "Escape") {
        handleClose();
      }
    } else if (mode === "scroll") {
      if (e.key === "Escape") {
        handleClose();
      }
    }
  }

  function handlePageViewClick(e: MouseEvent) {
    if (!gallery || mode !== "page") return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const third = rect.width / 3;
    if (x < third) {
      goToPage(currentPage - 1);
    } else if (x > third * 2) {
      goToPage(currentPage + 1);
    } else {
      showControls = !showControls;
    }
  }

  async function saveProgress(pageIdx: number) {
    if (!gallery) return;
    const isCompleted = pageIdx >= totalPages - 1;
    const progress: ReadProgress = {
      gid: gallery.gid,
      last_page_read: pageIdx,
      total_pages: totalPages,
      last_read_at: Math.floor(Date.now() / 1000),
      is_completed: isCompleted,
    };
    try {
      await updateReadProgress(progress);
    } catch { /* not critical */ }
  }

  async function handleClose() {
    // Disconnect strip batch observers before cancelling anything.
    for (const obs of batchObservers) obs.disconnect();
    batchObservers = [];
    // Cancel full-size image downloads immediately.
    if (activeGid !== null) {
      tracing_log(`[GalleryReader] handleClose: cancelling downloads for gid=${activeGid}`);
      cancelImageDownloads(activeGid).catch(() => {});
    }
    if (sessionId !== null) {
      const now = Math.floor(Date.now() / 1000);
      try {
        await endReadingSession(sessionId, now, pagesViewed.size);
      } catch { /* ignore */ }
    }
    await saveProgress(currentPage);
    const source = $readerSourceGallery;
    $readerGallery = null;
    $readerPage = 0;
    $readerSessionId = null;
    $readerSourceGallery = null;
    if (source !== null) {
      // Returning to the detail page — preserve detailBatchState so the detail
      // page can continue batch fetching without re-fetching already-loaded batches.
      $detailGallery = source;
    } else {
      // Navigating away entirely — cancel all pending thumbnail downloads.
      setActiveDetailGallery(null).catch(() => {});
      detailBatchState.set(null);
    }
  }

  function toggleMode() {
    $readerMode = mode === "page" ? "scroll" : "page";
    if (mode === "scroll") {
      // Load a small initial batch for scroll mode.
      for (let i = 0; i < Math.min(5, totalPages); i++) {
        loadImage(i);
      }
    }
  }

  function handleScrollImageVisible(pageIdx: number) {
    if (!alive) return;
    loadImage(pageIdx);
    // Also preload the next 2 images for smooth scrolling.
    for (let i = 1; i <= 2; i++) {
      if (pageIdx + i < totalPages) loadImage(pageIdx + i);
    }
    pagesViewed.add(pageIdx);
    if (pageIdx > currentPage) {
      $readerPage = pageIdx;
      saveProgress(pageIdx);
    }
  }

  function scrollImageAction(node: HTMLElement, pageIdx: number) {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            handleScrollImageVisible(pageIdx);
          }
        }
      },
      { root: scrollContainer, rootMargin: "200px" }
    );
    observer.observe(node);
    return {
      destroy() {
        observer.disconnect();
      },
    };
  }
</script>

{#if gallery}
  <div class="reader-overlay">
    <!-- Top bar -->
    {#if showControls}
      <div class="reader-bar top-bar" transition:slide={{ duration: 200, axis: 'y' }}>
        <button class="bar-btn" onclick={handleClose}>
          <svg width="18" height="18" viewBox="0 0 16 16" fill="none">
            <path d="M10 4L6 8l4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <span class="reader-title" title={gallery.title}>{gallery.title}</span>
        <span class="page-counter">
          {$t("reader.page_of", { current: currentPage + 1, total: totalPages })}
        </span>
        <button class="bar-btn" onclick={toggleMode}>
          {mode === "page" ? $t("reader.scroll_mode") : $t("reader.page_mode")}
        </button>
      </div>
    {/if}

    {#if mode === "page"}
      <div class="page-view" onclick={handlePageViewClick} role="button" tabindex="-1">
        {#if currentPage in loadedImages}
          <img
            src={loadedImages[currentPage]}
            alt="Page {currentPage + 1}"
            class="page-image"
          />
        {:else if currentPage in errorPages}
          <div class="page-status error">
            <span>{$t("reader.failed_page", { page: currentPage + 1 })}</span>
            <button class="retry-btn" onclick={(e) => { e.stopPropagation(); delete errorPages[currentPage]; loadImage(currentPage); }}>
              {$t("reader.retry")}
            </button>
          </div>
        {:else}
          <div class="page-spinner">
            <svg class="arc-spinner" viewBox="0 0 44 44" width="44" height="44">
              <circle class="arc-track" cx="22" cy="22" r="18" />
              <circle class="arc-fill" cx="22" cy="22" r="18" />
            </svg>
          </div>
        {/if}
      </div>

      {#if showControls}
        <div class="reader-bar bottom-bar" transition:slide={{ duration: 200, axis: 'y' }}>
          <!-- Preview strip: iterates ALL pages (totalPages), not just loaded entries.
               Each batch boundary has a data-strip-sentinel attribute so the
               IntersectionObserver can trigger the next batch fetch. -->
          <div class="thumb-strip" bind:this={stripEl} onscroll={handleStripScroll} use:stripWheelAction role="listbox" aria-label="Page previews">
            {#each { length: totalPages } as _, idx (idx)}
              {@const isActive = idx === currentPage}
              <button
                class="thumb-item"
                class:active={isActive}
                onclick={() => goToPage(idx)}
                aria-label="Go to page {idx + 1}"
                aria-selected={isActive}
                role="option"
                data-strip-sentinel={idx}
              >
                {#if idx in thumbPaths}
                  <img src={thumbPaths[idx]} alt="" class="thumb-img" />
                {:else}
                  <div class="thumb-skeleton"></div>
                {/if}
              </button>
            {/each}
          </div>
          <div class="slider-row">
            <div class="page-slider-wrap">
              <Slider
                min={1}
                max={totalPages}
                value={currentPage + 1}
                onChange={(v) => goToPage(v - 1)}
              />
            </div>
            <span class="pct-label">{Math.round((currentPage + 1) / totalPages * 100)}%</span>
          </div>
          <div class="page-label">{currentPage + 1} / {totalPages}</div>
        </div>
      {/if}
    {:else}
      <div class="scroll-view" bind:this={scrollContainer}>
        {#each { length: totalPages } as _, idx (idx)}
          <div class="scroll-page" use:scrollImageAction={idx}>
            {#if idx in loadedImages}
              <img src={loadedImages[idx]} alt="Page {idx + 1}" />
            {:else if idx in loadingPages}
              <div class="scroll-skeleton">
                <div class="skeleton-rect tall"></div>
              </div>
            {:else if idx in errorPages}
              <div class="scroll-placeholder error">
                {$t("reader.failed_page", { page: idx + 1 })}
                <button class="retry-btn" onclick={() => { delete errorPages[idx]; loadImage(idx); }}>
                  {$t("reader.retry")}
                </button>
              </div>
            {:else}
              <div class="scroll-skeleton">
                <div class="skeleton-rect tall"></div>
                <span class="page-num">{idx + 1}</span>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .reader-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: #000;
    display: flex;
    flex-direction: column;
    color: #fff;
  }

  .reader-bar {
    background: rgba(0, 0, 0, 0.85);
    flex-shrink: 0;
    z-index: 10;
  }

  .top-bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1.25rem;
  }

  .bottom-bar {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    display: flex;
    flex-direction: column;
    padding: 0.875rem 1.25rem;
    padding-bottom: max(0.875rem, env(safe-area-inset-bottom));
    gap: 0.375rem;
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .page-slider-wrap {
    flex: 1;
    min-width: 0;
  }

  .pct-label {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.45);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    min-width: 2.75rem;
    text-align: right;
  }

  .page-label {
    text-align: center;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.4);
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .bar-btn {
    padding: 0.35rem 0.7rem;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    font-size: 0.75rem;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    transition: background 0.15s;
  }

  .bar-btn:hover {
    background: rgba(255, 255, 255, 0.16);
  }

  .reader-title {
    flex: 1;
    font-size: 0.85rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    opacity: 0.7;
    font-weight: 500;
  }

  .page-counter {
    font-size: 0.85rem;
    opacity: 0.6;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
    font-weight: 500;
  }

  .page-view {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    cursor: pointer;
    user-select: none;
  }

  .page-image {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    will-change: transform;
    animation: fade-in 0.3s ease;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  /* Arc spinner for page loading */
  .page-spinner {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }

  .arc-spinner {
    animation: arc-rotate 1s linear infinite;
    overflow: visible;
  }

  .arc-track {
    fill: none;
    stroke: rgba(255, 255, 255, 0.1);
    stroke-width: 4;
  }

  .arc-fill {
    fill: none;
    stroke: var(--accent, #7c3aed);
    stroke-width: 4;
    stroke-linecap: round;
    /* circumference of r=18 is ~113.1; 270° arc = 84.8, gap = 28.3 */
    stroke-dasharray: 84.8 28.3;
    stroke-dashoffset: 0;
  }

  @keyframes arc-rotate {
    to { transform: rotate(360deg); transform-origin: 22px 22px; }
  }

  /* Scroll mode skeleton */
  .skeleton-rect.tall {
    width: 100%;
    min-height: 500px;
    aspect-ratio: auto;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 2px;
    animation: skeleton-pulse 1.8s ease-in-out infinite;
  }

  @keyframes skeleton-pulse {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 0.8; }
  }

  .page-status {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    opacity: 0.4;
    font-size: 0.85rem;
  }

  .page-status.error {
    color: var(--red, #f87171);
    opacity: 1;
  }

  .retry-btn {
    margin-top: 0.25rem;
    padding: 0.35rem 0.85rem;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: transparent;
    color: #fff;
    cursor: pointer;
    font-size: 0.75rem;
  }

  .retry-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }


  .scroll-view {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .scroll-page {
    width: 100%;
    max-width: 900px;
    display: flex;
    justify-content: center;
  }

  .scroll-page img {
    width: 100%;
    height: auto;
    display: block;
    animation: fade-in 0.3s ease;
  }

  .scroll-skeleton {
    width: 100%;
    min-height: 500px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    position: relative;
  }

  .scroll-placeholder {
    width: 100%;
    min-height: 400px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    font-size: 0.85rem;
    opacity: 0.3;
  }

  .scroll-placeholder.error {
    color: var(--red, #f87171);
    opacity: 1;
  }

  .page-num {
    font-size: 1.8rem;
    font-weight: 200;
    opacity: 0.3;
    position: absolute;
    font-variant-numeric: tabular-nums;
  }

  /* Preview thumbnail strip */
  .thumb-strip {
    display: flex;
    flex-direction: row;
    gap: 4px;
    overflow-x: auto;
    overflow-y: hidden;
    height: 74px;
    align-items: center;
    padding: 0 2px;
    /* Hide scrollbar but keep scroll functionality */
    scrollbar-width: none;
  }
  .thumb-strip::-webkit-scrollbar {
    display: none;
  }

  .thumb-item {
    flex-shrink: 0;
    width: 52px;
    height: 66px;
    border-radius: 3px;
    overflow: hidden;
    border: 1.5px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.06);
    padding: 0;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0.45;
    transition: opacity 0.12s, border-color 0.12s, transform 0.12s;
    position: relative;
  }

  .thumb-item:hover {
    opacity: 0.75;
  }

  .thumb-item.active {
    opacity: 1;
    border-color: var(--accent, #8b5cf6);
    transform: scale(1.08);
    z-index: 1;
  }

  .thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .thumb-skeleton {
    width: 100%;
    height: 100%;
    background: rgba(255, 255, 255, 0.07);
    animation: skeleton-pulse 1.8s ease-in-out infinite;
  }

</style>
