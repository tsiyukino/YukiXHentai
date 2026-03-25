<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { t } from "$lib/i18n";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { localDetailGallery } from "$lib/stores/localLibrary";
  import { viewMode, libraryRefreshTick } from "$lib/stores/ui";

  import { categoryColor } from "$lib/utils/category";
  import { thumbSrc } from "$lib/utils/thumb";
  import LocalGalleryCard from "./LocalGalleryCard.svelte";
  import GalleryListItem from "./GalleryListItem.svelte";
  import ImportPreviewDialog from "./ImportPreviewDialog.svelte";
  import QueueDownloadPage from "./QueueDownloadPage.svelte";
  import {
    getLocalGalleries,
    getDownloadQueueStatus,
    pauseDownloadQueue,
    resumeDownloadQueue,
    cancelDownloadQueue,
    importLocalFolder,
    type DownloadQueueStatus,
    type ImportPreview,
  } from "$lib/api/library";
  import type { Gallery } from "$lib/api/galleries";

  const LIMIT = 50;

  let galleries = $state<Gallery[]>([]);
  let totalCount = $state(0);
  let loading = $state(false);
  let loadingMore = $state(false);
  let error = $state<string | null>(null);
  let filterText = $state("");
  let offset = $state(0);
  let hasMore = $state(false);

  // Download status banner
  let downloadStatus = $state<DownloadQueueStatus | null>(null);
  let statusPollTimer: ReturnType<typeof setInterval> | null = null;
  let unlistenProgress: (() => void) | null = null;
  let bannerPaused = $state(false);

  // Three-dot menu
  let menuOpen = $state(false);
  let menuRef = $state<HTMLElement | null>(null);

  // Folder path input dialog (shown before import)
  let showFolderInput = $state(false);
  let folderInputValue = $state("");
  let folderInputError = $state("");

  // Import preview dialog
  let importPreview = $state<ImportPreview | null>(null);
  let importFolderPath = $state("");
  let importingFolder = $state(false);

  // Queue download overlay
  let showQueueDownload = $state(false);

  // Filtered galleries (client-side)
  let filtered = $derived(() => {
    const q = filterText.trim().toLowerCase();
    if (!q) return galleries;
    return galleries.filter((g) => {
      if (g.title.toLowerCase().includes(q)) return true;
      if (g.category.toLowerCase().includes(q)) return true;
      if (g.uploader && g.uploader.toLowerCase().includes(q)) return true;
      if (g.tags.some((t) => t.name.toLowerCase().includes(q) || t.namespace.toLowerCase().includes(q))) return true;
      return false;
    });
  });

  onMount(async () => {
    await loadInitial();
    await pollStatus();
    statusPollTimer = setInterval(pollStatus, 3000);
    try {
      unlistenProgress = await listen("local-download-progress", async (event: any) => {
        await pollStatus();
        // Reload gallery list when a download finishes successfully.
        if (event.payload?.status === "done") {
          await loadInitial();
        }
      });
    } catch {}
  });

  onDestroy(() => {
    if (statusPollTimer) clearInterval(statusPollTimer);
    unlistenProgress?.();
  });

  // Reload when another component (e.g. GalleryDetail after delete) signals a refresh.
  $effect(() => {
    const tick = $libraryRefreshTick;
    if (tick > 0) {
      loadInitial();
    }
  });

  async function loadInitial() {
    loading = true;
    error = null;
    offset = 0;
    try {
      const res = await getLocalGalleries(0, LIMIT);
      galleries = res.galleries as Gallery[];
      totalCount = res.totalCount;
      hasMore = galleries.length < totalCount;
      offset = galleries.length;
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    if (loadingMore || !hasMore) return;
    loadingMore = true;
    try {
      const res = await getLocalGalleries(offset, LIMIT);
      const newGalleries = res.galleries as Gallery[];
      galleries = [...galleries, ...newGalleries];
      totalCount = res.totalCount;
      offset += newGalleries.length;
      hasMore = galleries.length < totalCount;
    } catch (err) {
      error = String(err);
    } finally {
      loadingMore = false;
    }
  }

  async function pollStatus() {
    try {
      const status = await getDownloadQueueStatus();
      downloadStatus = status;
      bannerPaused = status.downloading === 0 && status.queued > 0;
    } catch {
      downloadStatus = null;
    }
  }

  function openDetail(gallery: Gallery) {
    $localDetailGallery = gallery;
  }

  function handleScroll(e: Event) {
    const el = e.target as HTMLElement;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 300) {
      loadMore();
    }
  }

  function toggleMenu() {
    menuOpen = !menuOpen;
  }

  function closeMenu() {
    menuOpen = false;
  }

  function handleMenuKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") closeMenu();
  }

  function handleImportFolder() {
    closeMenu();
    folderInputValue = "";
    folderInputError = "";
    showFolderInput = true;
  }

  async function confirmFolderInput() {
    const path = folderInputValue.trim();
    if (!path) { folderInputError = "Please enter a folder path."; return; }
    showFolderInput = false;
    importingFolder = true;
    folderInputError = "";
    try {
      const preview = await importLocalFolder(path);
      importFolderPath = path;
      importPreview = preview;
    } catch (err) {
      error = String(err);
    } finally {
      importingFolder = false;
    }
  }

  async function handleImportJson() {
    closeMenu();
    showQueueDownload = true;
  }

  function handleQueueDownload() {
    closeMenu();
    showQueueDownload = true;
  }

  function handleImportConfirmed(gallery: Gallery) {
    importPreview = null;
    importFolderPath = "";
    galleries = [gallery, ...galleries];
    totalCount += 1;
  }

  function handleImportClose() {
    importPreview = null;
    importFolderPath = "";
  }

  async function handleBannerPause() {
    try {
      await pauseDownloadQueue();
      await pollStatus();
    } catch {}
  }

  async function handleBannerResume() {
    try {
      await resumeDownloadQueue();
      await pollStatus();
    } catch {}
  }

  async function handleBannerCancel() {
    if (!confirm($t("download_banner.cancel_confirm"))) return;
    try {
      await cancelDownloadQueue();
      await pollStatus();
    } catch {}
  }

  let isBannerActive = $derived(
    downloadStatus !== null &&
    (downloadStatus.downloading > 0 || downloadStatus.queued > 0)
  );
</script>

<svelte:window onclick={(e) => {
  if (menuOpen && menuRef && !menuRef.contains(e.target as Node)) {
    closeMenu();
  }
}} onkeydown={handleMenuKeydown} />

<div class="local-page">
  <!-- Header -->
  <div class="header">
    <div class="header-left">
      <h1 class="page-title">{$t("local.title")}</h1>
      <span class="count-badge">
        {#if filterText && filtered().length !== galleries.length}
          {filtered().length} / {$t("local.gallery_count", { count: totalCount })}
        {:else}
          {$t("local.gallery_count", { count: totalCount })}
        {/if}
      </span>
    </div>
    <div class="header-right">
      <input
        class="filter-input"
        type="text"
        placeholder={$t("local.filter_placeholder")}
        bind:value={filterText}
      />
      <div class="menu-wrap" bind:this={menuRef}>
        <button class="icon-btn" onclick={toggleMenu} aria-label="Menu" disabled={importingFolder}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <circle cx="12" cy="5" r="1.5"/><circle cx="12" cy="12" r="1.5"/><circle cx="12" cy="19" r="1.5"/>
          </svg>
        </button>
        {#if menuOpen}
          <div class="dropdown-menu" role="menu">
            <button class="dropdown-item" onclick={handleImportFolder} role="menuitem">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/></svg>
              {$t("local.import_folder")}
            </button>
            <button class="dropdown-item" onclick={handleImportJson} role="menuitem">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
              {$t("local.import_json")}
            </button>
            <div class="dropdown-divider"></div>
            <button class="dropdown-item" onclick={handleQueueDownload} role="menuitem">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
              {$t("local.queue_download")}
            </button>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- Download banner -->
  {#if isBannerActive && downloadStatus}
    <div class="download-banner">
      <div class="banner-info">
        <div class="banner-spinner"></div>
        <div class="banner-text">
          <span class="banner-title">
            {#if downloadStatus.current_title}
              {$t("download_banner.downloading", { title: downloadStatus.current_title })}
            {:else}
              {$t("download_banner.downloading", { title: "..." })}
            {/if}
          </span>
          {#if downloadStatus.current_page !== undefined && downloadStatus.total_pages}
            <span class="banner-progress">
              {$t("download_banner.page_progress", { current: downloadStatus.current_page, total: downloadStatus.total_pages })}
            </span>
          {/if}
        </div>
      </div>
      <div class="banner-actions">
        {#if bannerPaused}
          <button class="banner-btn" onclick={handleBannerResume}>{$t("download_banner.resume")}</button>
        {:else}
          <button class="banner-btn" onclick={handleBannerPause}>{$t("download_banner.pause")}</button>
        {/if}
        <button class="banner-btn danger" onclick={handleBannerCancel}>{$t("download_banner.cancel")}</button>
      </div>
    </div>
  {/if}

  <!-- Content -->
  <div class="content" onscroll={handleScroll}>
    {#if loading}
      <div class="center-msg">
        <div class="spinner"></div>
        <p>{$t("common.loading")}</p>
      </div>
    {:else if error}
      <div class="center-msg">
        <p class="error-text">{$t("common.error", { message: error })}</p>
        <button class="retry-btn" onclick={loadInitial}>{$t("reader.retry")}</button>
      </div>
    {:else if filtered().length === 0}
      <div class="center-msg empty">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" class="empty-icon">
          <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/>
        </svg>
        <p class="empty-title">
          {#if filterText}
            {$t("gallery.no_match")}
          {:else}
            {$t("local.empty_title")}
          {/if}
        </p>
        {#if !filterText}
          <p class="empty-hint">{$t("local.empty_hint")}</p>
        {/if}
      </div>
    {:else if $viewMode === "cards"}
      <div class="gallery-grid">
        {#each filtered() as gallery (gallery.gid)}
          <LocalGalleryCard {gallery} />
        {/each}
      </div>
    {:else}
      <div class="gallery-list">
        {#each filtered() as gallery (gallery.gid)}
          <GalleryListItem {gallery} onOpen={openDetail} />
        {/each}
      </div>
    {/if}

    {#if hasMore && !loading && !filterText}
      <div class="load-more-wrap">
        <button class="load-more-btn" onclick={loadMore} disabled={loadingMore}>
          {loadingMore ? $t("common.loading") : $t("favorites_page.load_more")}
        </button>
      </div>
    {/if}
  </div>
</div>

<!-- Folder path input dialog -->
{#if showFolderInput}
  <div class="dialog-backdrop" onclick={() => { showFolderInput = false; }} role="presentation"></div>
  <div class="dialog" role="dialog" aria-modal="true">
    <div class="dialog-header">
      <span class="dialog-title">{$t("local.import_folder")}</span>
      <button class="close-btn" onclick={() => { showFolderInput = false; }}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>
    <label class="dialog-label">{$t("local.folder_path_label")}</label>
    <input
      class="dialog-input"
      type="text"
      bind:value={folderInputValue}
      placeholder={$t("local.folder_path_placeholder")}
      onkeydown={(e) => { if (e.key === "Enter") confirmFolderInput(); if (e.key === "Escape") showFolderInput = false; }}
      autofocus
    />
    {#if folderInputError}
      <p class="dialog-error">{folderInputError}</p>
    {/if}
    <div class="dialog-actions">
      <button class="btn-secondary" onclick={() => { showFolderInput = false; }}>{$t("common.cancel")}</button>
      <button class="btn-primary" onclick={confirmFolderInput}>{$t("common.confirm")}</button>
    </div>
  </div>
{/if}

<!-- Import preview dialog -->
{#if importPreview}
  <ImportPreviewDialog
    folderPath={importFolderPath}
    preview={importPreview}
    onConfirm={handleImportConfirmed}
    onClose={handleImportClose}
  />
{/if}

<!-- Queue download overlay -->
{#if showQueueDownload}
  <QueueDownloadPage onClose={() => { showQueueDownload = false; loadInitial(); }} />
{/if}

<style>
  .local-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* ── Header ─────────────────────────────────────── */

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 16px 8px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
    background: var(--bg-primary);
  }

  .header-left {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }

  .page-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
    white-space: nowrap;
  }

  .count-badge {
    font-size: 0.72rem;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .filter-input {
    padding: 5px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.78rem;
    outline: none;
    width: 220px;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .filter-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-subtle);
  }

  .icon-btn {
    width: 30px;
    height: 30px;
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.12s, color 0.12s;
    flex-shrink: 0;
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ── Dropdown menu ───────────────────────────────── */

  .menu-wrap {
    position: relative;
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 200;
    background: var(--bg-primary);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
    min-width: 180px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .dropdown-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.78rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    transition: background 0.1s, color 0.1s;
  }

  .dropdown-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .dropdown-divider {
    height: 1px;
    background: var(--border);
    margin: 3px 0;
  }

  /* ── Download banner ────────────────────────────── */

  .download-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 16px;
    background: var(--accent-subtle);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .banner-info {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .banner-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .banner-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .banner-title {
    font-size: 0.78rem;
    font-weight: 500;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .banner-progress {
    font-size: 0.7rem;
    color: var(--text-muted);
  }

  .banner-actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }

  .banner-btn {
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-primary);
    color: var(--text-secondary);
    font-size: 0.74rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .banner-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .banner-btn.danger {
    color: var(--red);
    border-color: var(--danger-border);
    background: var(--danger-bg);
  }

  .banner-btn.danger:hover {
    opacity: 0.85;
  }

  /* ── Content ─────────────────────────────────────── */

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
    min-height: 200px;
    gap: 10px;
    color: var(--text-muted);
  }

  .center-msg p { margin: 0; font-size: 0.84rem; }
  .error-text { color: var(--red); }

  .empty-icon { opacity: 0.3; }

  .empty-title {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .empty-hint {
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  .spinner {
    width: 22px;
    height: 22px;
    border: 2.5px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  .retry-btn {
    padding: 5px 14px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
  }

  .retry-btn:hover { background: var(--bg-hover); color: var(--text-primary); }

  /* ── Gallery grid / list ─────────────────────────── */

  .gallery-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 12px;
  }

  .gallery-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  /* ── Load more ───────────────────────────────────── */

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

  /* ── Folder input dialog (FavoriteDialog pattern) ── */

  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: var(--overlay-bg);
    z-index: 600;
  }

  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 601;
    background: var(--bg-primary);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
    width: 420px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .dialog-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .close-btn {
    width: 24px;
    height: 24px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: color 0.15s, background 0.15s;
  }

  .close-btn:hover { color: var(--text-primary); background: var(--bg-hover); }

  .dialog-label {
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--text-muted);
  }

  .dialog-input {
    padding: 7px 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.82rem;
    outline: none;
    width: 100%;
    box-sizing: border-box;
    transition: border-color 0.15s;
  }

  .dialog-input:focus { border-color: var(--accent); }

  .dialog-error {
    margin: 0;
    font-size: 0.75rem;
    color: var(--red);
  }

  .dialog-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .btn-primary {
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
  }

  .btn-primary:hover { background: var(--accent-hover); }

  .btn-secondary {
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .btn-secondary:hover { background: var(--bg-hover); color: var(--text-primary); }
</style>
