<script lang="ts">
  import { onDestroy } from "svelte";
  import { slide } from "svelte/transition";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { t } from "$lib/i18n";
  import Slider from "./Slider.svelte";
  import { localReaderGallery, localReaderPage, localReaderMode, localDetailGallery, localReaderSourceGallery } from "$lib/stores/localLibrary";
  import { updateLocalReadProgress } from "$lib/api/reader";

  let gallery = $derived($localReaderGallery);
  let currentPage = $derived($localReaderPage);
  let mode = $derived($localReaderMode);
  let totalPages = $derived(gallery ? gallery.total_pages : 0);

  let showControls = $state(false);

  // Loaded images: populated lazily from local file paths.
  let loadedImages = $state<Record<number, string>>({});

  // Thumbnail strip: populated lazily from local file paths.
  let thumbPaths = $state<Record<number, string>>({});

  let scrollContainer: HTMLDivElement | undefined = $state(undefined);
  let stripEl: HTMLDivElement | undefined = $state(undefined);

  let pagesViewed = $state(new Set<number>());

  let alive = true;
  onDestroy(() => { alive = false; });

  // Clear state when gallery changes.
  let activeGid = $state<number | null>(null);
  $effect(() => {
    const newGid = gallery ? gallery.gid : null;
    if (newGid !== activeGid) {
      loadedImages = {};
      thumbPaths = {};
      pagesViewed = new Set();
      activeGid = newGid;
    }
  });

  // Preload current page + neighbours.
  $effect(() => {
    if (gallery && mode === "page" && alive) {
      loadImage(currentPage);
      for (let i = 1; i <= 3; i++) {
        if (currentPage + i < totalPages) loadImage(currentPage + i);
      }
      if (currentPage - 1 >= 0) loadImage(currentPage - 1);
    }
  });

  // Track viewed pages.
  $effect(() => {
    if (gallery) pagesViewed.add(currentPage);
  });

  // Auto-scroll strip to keep current page centred.
  $effect(() => {
    if (!stripEl || !showControls) return;
    const THUMB_W = 52;
    const GAP = 4;
    const stride = THUMB_W + GAP;
    const target = currentPage * stride - stripEl.clientWidth / 2 + THUMB_W / 2;
    stripEl.scrollTo({ left: Math.max(0, target), behavior: "smooth" });
  });

  // Load strip thumbnails when controls become visible.
  $effect(() => {
    if (!showControls || !gallery || mode !== "page") return;
    const half = 5;
    const start = Math.max(0, currentPage - half);
    const end = Math.min(totalPages - 1, currentPage + half);
    for (let i = start; i <= end; i++) enqueueThumb(i);
  });

  function loadImage(pageIdx: number) {
    if (!alive || !gallery) return;
    if (pageIdx in loadedImages) return;
    const page = gallery.pages[pageIdx];
    if (!page || !page.file_path) return;
    loadedImages[pageIdx] = convertFileSrc(page.file_path);
  }

  function enqueueThumb(pageIdx: number) {
    if (!alive || !gallery) return;
    if (pageIdx in thumbPaths) return;
    const page = gallery.pages[pageIdx];
    if (!page || !page.file_path) return;
    thumbPaths[pageIdx] = convertFileSrc(page.file_path);
  }

  function goToPage(idx: number) {
    if (idx >= 0 && idx < totalPages) {
      $localReaderPage = idx;
      saveProgress(idx);
    }
  }

  async function saveProgress(pageIdx: number) {
    if (!gallery) return;
    try {
      await updateLocalReadProgress({
        gid: gallery.gid,
        last_page_read: pageIdx,
        total_pages: totalPages,
        last_read_at: Math.floor(Date.now() / 1000),
        is_completed: pageIdx >= totalPages - 1,
      });
    } catch { /* not critical */ }
  }

  async function handleClose() {
    if (!gallery) return;
    alive = false;
    await saveProgress(currentPage);
    const src = $localReaderSourceGallery;
    $localReaderGallery = null;
    $localReaderPage = 0;
    if (src) {
      $localReaderSourceGallery = null;
      $localDetailGallery = src;
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

  function handleScrollImageVisible(pageIdx: number) {
    if (!alive) return;
    loadImage(pageIdx);
    for (let i = 1; i <= 2; i++) {
      if (pageIdx + i < totalPages) loadImage(pageIdx + i);
    }
    pagesViewed.add(pageIdx);
    if (pageIdx > currentPage) {
      $localReaderPage = pageIdx;
      saveProgress(pageIdx);
    }
  }

  function scrollImageAction(node: HTMLElement, pageIdx: number) {
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) handleScrollImageVisible(pageIdx);
        }
      },
      { root: scrollContainer, rootMargin: "200px" }
    );
    observer.observe(node);
    return { destroy() { observer.disconnect(); } };
  }

  function handleStripScroll() {
    if (!stripEl || !gallery) return;
    const THUMB_W = 52;
    const GAP = 4;
    const stride = THUMB_W + GAP;
    const left = stripEl.scrollLeft;
    const right = left + stripEl.clientWidth;
    const startIdx = Math.max(0, Math.floor(left / stride) - 2);
    const endIdx = Math.min(totalPages - 1, Math.ceil(right / stride) + 2);
    for (let i = startIdx; i <= endIdx; i++) enqueueThumb(i);
  }

  function handleStripWheel(e: WheelEvent) {
    e.preventDefault();
    (e.currentTarget as HTMLElement).scrollLeft += e.deltaY;
  }

  function stripWheelAction(node: HTMLElement) {
    node.addEventListener("wheel", handleStripWheel, { passive: false });
    return { destroy() { node.removeEventListener("wheel", handleStripWheel); } };
  }

  function toggleMode() {
    $localReaderMode = mode === "page" ? "scroll" : "page";
    if (mode === "scroll") {
      for (let i = 0; i < Math.min(5, totalPages); i++) loadImage(i);
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
      if (e.key === "Escape") handleClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

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
    padding-top: max(0.75rem, env(safe-area-inset-top));
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
    stroke-dasharray: 84.8 28.3;
    stroke-dashoffset: 0;
  }

  @keyframes arc-rotate {
    to { transform: rotate(360deg); transform-origin: 22px 22px; }
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

  .skeleton-rect.tall {
    width: 100%;
    min-height: 500px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 2px;
    animation: skeleton-pulse 1.8s ease-in-out infinite;
  }

  @keyframes skeleton-pulse {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 0.8; }
  }

  .page-num {
    font-size: 1.8rem;
    font-weight: 200;
    opacity: 0.3;
    position: absolute;
    font-variant-numeric: tabular-nums;
  }

  .thumb-strip {
    display: flex;
    flex-direction: row;
    gap: 4px;
    overflow-x: auto;
    overflow-y: hidden;
    height: 74px;
    align-items: center;
    padding: 0 2px;
    scrollbar-width: none;
  }

  .thumb-strip::-webkit-scrollbar { display: none; }

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
