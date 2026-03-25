<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { t } from "$lib/i18n";
  import { fetchFavorites, folderColor } from "$lib/api/favorites";
  import type { FavoriteFolder } from "$lib/api/favorites";
  import { downloadThumbnailsForGids, onThumbnailReady, type Gallery } from "$lib/api/galleries";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { detailGallery, detailOpenedAsLocal } from "$lib/stores/detail";
  import { categoryColor } from "$lib/utils/category";

  let galleries = $state<Gallery[]>([]);
  let folders = $state<FavoriteFolder[]>([]);
  let selectedFavcat = $state<number | null>(null);
  let nextUrl = $state<string | null>(null);
  let hasMore = $state(false);
  let loading = $state(false);
  let loadingMore = $state(false);
  let error = $state<string | null>(null);

  // ── Viewport-driven thumbnail loading ───────────────────────────────────
  let thumbDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingThumbGids = new Set<number>();
  let requestedThumbGids = new Set<number>();
  const THUMB_RETRY_AFTER_MS = 20_000;
  let thumbObserver: IntersectionObserver | null = null;
  let unlistenThumb: (() => void) | null = null;

  function scheduleThumbDownload(gid: number) {
    pendingThumbGids.add(gid);
    if (thumbDebounceTimer) clearTimeout(thumbDebounceTimer);
    thumbDebounceTimer = setTimeout(() => {
      const gids = [...pendingThumbGids];
      pendingThumbGids.clear();
      if (gids.length === 0) return;
      for (const g of gids) requestedThumbGids.add(g);
      downloadThumbnailsForGids(gids).catch(() => {
        for (const g of gids) requestedThumbGids.delete(g);
      });
      setTimeout(() => {
        for (const gid of gids) {
          const g = galleries.find(x => x.gid === gid);
          if (!g || !g.thumb_path) requestedThumbGids.delete(gid);
        }
        // trigger re-observation by re-observing unresolved rows
        reobserveUnresolved();
      }, THUMB_RETRY_AFTER_MS);
    }, 150);
  }

  function reobserveUnresolved() {
    if (!thumbObserver) return;
    const rows = document.querySelectorAll<HTMLElement>(".gallery-row[data-gid]");
    for (const row of rows) {
      const gid = Number(row.dataset.gid);
      const g = galleries.find(x => x.gid === gid);
      if (g && !g.thumb_path && !requestedThumbGids.has(gid)) {
        thumbObserver.unobserve(row);
        thumbObserver.observe(row);
      }
    }
  }

  function setupThumbObserver() {
    if (thumbObserver) thumbObserver.disconnect();
    thumbObserver = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const gid = Number((entry.target as HTMLElement).dataset.gid);
        const g = galleries.find(x => x.gid === gid);
        if (g && !g.thumb_path && !requestedThumbGids.has(gid)) {
          scheduleThumbDownload(gid);
        }
      }
    }, { rootMargin: "200px" });
  }

  function observeRows() {
    if (!thumbObserver) return;
    const rows = document.querySelectorAll<HTMLElement>(".gallery-row[data-gid]");
    for (const row of rows) thumbObserver.observe(row);
  }

  onMount(async () => {
    setupThumbObserver();
    unlistenThumb = await onThumbnailReady((event) => {
      const idx = galleries.findIndex(g => g.gid === event.gid);
      if (idx >= 0) {
        galleries[idx] = { ...galleries[idx], thumb_path: event.path };
        galleries = [...galleries];
      }
    });
    load(null, null);
  });

  onDestroy(() => {
    thumbObserver?.disconnect();
    unlistenThumb?.();
    if (thumbDebounceTimer) clearTimeout(thumbDebounceTimer);
  });

  async function load(favcat: number | null, cursor: string | null) {
    loading = true;
    error = null;
    requestedThumbGids.clear();
    pendingThumbGids.clear();
    try {
      const result = await fetchFavorites(favcat, cursor);
      galleries = result.galleries;
      if (result.folders.length > 0) folders = result.folders;
      hasMore = result.has_more;
      nextUrl = result.next_url;
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
      // Observe after DOM updates
      setTimeout(observeRows, 0);
    }
  }

  async function loadMore() {
    if (!hasMore || loadingMore || !nextUrl) return;
    loadingMore = true;
    try {
      const result = await fetchFavorites(selectedFavcat, nextUrl);
      galleries = [...galleries, ...result.galleries];
      if (result.folders.length > 0) folders = result.folders;
      hasMore = result.has_more;
      nextUrl = result.next_url;
    } catch (err) {
      error = String(err);
    } finally {
      loadingMore = false;
      setTimeout(observeRows, 0);
    }
  }

  function selectFolder(favcat: number | null) {
    selectedFavcat = favcat;
    nextUrl = null;
    load(favcat, null);
  }

  function openDetail(gallery: Gallery) {
    $detailOpenedAsLocal = false;
    $detailGallery = gallery;
  }
</script>

<div class="favorites-page">
  <!-- Folder tabs -->
  <div class="folder-tabs">
    <button
      class="folder-tab"
      class:active={selectedFavcat === null}
      onclick={() => selectFolder(null)}
    >
      {$t("favorites_page.all")}
    </button>
    {#each folders as folder}
      <button
        class="folder-tab"
        class:active={selectedFavcat === folder.index}
        onclick={() => selectFolder(folder.index)}
        style="--fol-color: {folderColor(folder.index)}"
      >
        <span class="folder-dot" style="background: {folderColor(folder.index)}"></span>
        {folder.name}
        {#if folder.count > 0}
          <span class="folder-count">{folder.count}</span>
        {/if}
      </button>
    {/each}
  </div>

  <div class="content">
    {#if loading}
      <div class="center-msg">
        <div class="spinner"></div>
        <p>{$t("common.loading")}</p>
      </div>
    {:else if error}
      <div class="center-msg error">
        <p>{$t("common.error", { message: error })}</p>
        <button class="retry-btn" onclick={() => load(selectedFavcat, null)}>
          {$t("reader.retry")}
        </button>
      </div>
    {:else if galleries.length === 0}
      <div class="center-msg empty">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="empty-icon">
          <path d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"/>
        </svg>
        <p>{$t("favorites_page.empty")}</p>
      </div>
    {:else}
      <div class="gallery-list">
        {#each galleries as gallery (gallery.gid)}
          <button class="gallery-row" data-gid={gallery.gid} onclick={() => openDetail(gallery)}>
            <div class="thumb-wrap">
              {#if gallery.thumb_path}
                <img src={convertFileSrc(gallery.thumb_path)} alt={gallery.title} />
              {:else}
                <div class="thumb-placeholder"></div>
              {/if}
            </div>
            <div class="row-info">
              <span class="row-title">{gallery.title}</span>
              <div class="row-meta">
                <span class="cat-badge" style="background:{categoryColor(gallery.category)}">{gallery.category}</span>
                <span class="row-pages">{gallery.file_count}p</span>
                <span class="row-rating">{"★".repeat(Math.round(gallery.rating))}{gallery.rating.toFixed(1)}</span>
              </div>
            </div>
          </button>
        {/each}
      </div>

      {#if hasMore}
        <div class="load-more-wrap">
          <button class="load-more-btn" onclick={loadMore} disabled={loadingMore}>
            {loadingMore ? $t("common.loading") : $t("favorites_page.load_more")}
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .favorites-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* ── Folder tabs ───────────────────────────────── */

  .folder-tabs {
    display: flex;
    gap: 4px;
    padding: 10px 16px 6px;
    flex-shrink: 0;
    overflow-x: auto;
    border-bottom: 1px solid var(--border);
    scrollbar-width: none;
  }

  .folder-tabs::-webkit-scrollbar { display: none; }

  .folder-tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.78rem;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
    flex-shrink: 0;
  }

  .folder-tab:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .folder-tab.active {
    background: var(--accent-subtle);
    color: var(--accent);
    border-color: transparent;
    font-weight: 600;
  }

  .folder-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .folder-count {
    font-size: 0.68rem;
    background: var(--bg-elevated);
    color: var(--text-muted);
    border-radius: 10px;
    padding: 1px 6px;
  }

  /* ── Content area ──────────────────────────────── */

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 12px 16px;
  }

  .center-msg {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 12px;
    color: var(--text-muted);
    min-height: 200px;
  }

  .center-msg p { margin: 0; font-size: 0.84rem; }

  .center-msg.error p { color: var(--red); }

  .empty-icon { color: var(--text-muted); opacity: 0.4; }

  .spinner {
    width: 22px;
    height: 22px;
    border: 2.5px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .retry-btn {
    padding: 6px 16px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
  }

  .retry-btn:hover { background: var(--bg-hover); color: var(--text-primary); }

  /* ── Gallery list ──────────────────────────────── */

  .gallery-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .gallery-row {
    display: flex;
    gap: 12px;
    padding: 8px;
    border-radius: var(--radius-md);
    border: 1px solid var(--card-border);
    background: var(--card-bg);
    cursor: pointer;
    text-align: left;
    align-items: flex-start;
    transition: border-color 0.12s, background 0.12s;
  }

  .gallery-row:hover {
    border-color: var(--card-border-hover);
    background: var(--bg-hover);
  }

  .thumb-wrap {
    width: 56px;
    height: 80px;
    flex-shrink: 0;
    border-radius: 4px;
    overflow: hidden;
    background: var(--bg-tertiary);
  }

  .thumb-wrap img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .thumb-placeholder {
    width: 100%;
    height: 100%;
    background: var(--bg-elevated);
  }

  .row-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 2px;
  }

  .row-title {
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    line-height: 1.4;
  }

  .row-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .cat-badge {
    font-size: 0.68rem;
    font-weight: 600;
    color: #fff;
    border-radius: 3px;
    padding: 1px 6px;
  }

  .row-pages,
  .row-rating {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  /* ── Load more ────────────────────────────────── */

  .load-more-wrap {
    display: flex;
    justify-content: center;
    padding: 16px 0 4px;
  }

  .load-more-btn {
    padding: 7px 24px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }

  .load-more-btn:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .load-more-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
