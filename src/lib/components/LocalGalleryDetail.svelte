<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { t } from "$lib/i18n";
  import { categoryColor } from "$lib/utils/category";
  import { localDetailGallery, localReaderGallery, localReaderPage, localReaderMode, localReaderSourceGallery } from "$lib/stores/localLibrary";
  import { libraryRefreshTick } from "$lib/stores/ui";
  import { getLocalGalleryPages, deleteLocalGallery, buildLocalReaderGallery } from "$lib/api/library";
  import { getLocalReadProgress, startLocalReadingSession } from "$lib/api/reader";
  import LocalMetadataEditor from "./LocalMetadataEditor.svelte";
  import type { LocalPage } from "$lib/api/library";

  let gallery = $derived($localDetailGallery);

  let pages = $state<LocalPage[]>([]);
  let loadingPages = $state(false);
  let currentGid = $state<number | null>(null);

  let showMetadataEditor = $state(false);
  let deleting = $state(false);
  let deleteMessage = $state("");

  let imgLoaded = $state(false);
  let imgSrc = $derived(
    gallery?.thumb_path ? convertFileSrc(gallery.thumb_path) : ""
  );
  let catColor = $derived(gallery ? categoryColor(gallery.category) : "#9e9e9e");
  let langTag = $derived(gallery?.tags.find(t => t.namespace === "language")?.name ?? null);
  let date = $derived(
    gallery && gallery.posted > 0
      ? new Date(gallery.posted * 1000).toLocaleDateString()
      : ""
  );

  // Load pages whenever the gallery changes.
  $effect(() => {
    const g = $localDetailGallery;
    imgLoaded = false;
    pages = [];
    deleteMessage = "";
    showMetadataEditor = false;
    deleting = false;

    if (g) {
      currentGid = g.gid;
      loadPages(g.gid);
    } else {
      currentGid = null;
    }
  });

  async function loadPages(gid: number) {
    loadingPages = true;
    try {
      const result = await getLocalGalleryPages(gid);
      if (currentGid === gid) {
        pages = result;
      }
    } catch (err) {
      console.error("Failed to load local pages:", err);
    } finally {
      if (currentGid === gid) loadingPages = false;
    }
  }

  function openReader(startPage: number) {
    $localReaderPage = startPage;
    $localReaderMode = "page";
    $localReaderSourceGallery = gallery!;
    $localReaderGallery = buildLocalReaderGallery(gallery!.gid, gallery!.title, pages);
    $localDetailGallery = null;
  }

  async function handleRead(startPage = 0) {
    if (!gallery || pages.length === 0) return;
    try {
      const progress = await getLocalReadProgress(gallery.gid);
      if (progress && !progress.is_completed) startPage = progress.last_page_read;
    } catch { /* ignore */ }
    await startLocalReadingSession(gallery.gid, Math.floor(Date.now() / 1000)).catch(() => {});
    openReader(startPage);
  }

  async function handleOpenPage(pageIdx: number) {
    if (!gallery || pages.length === 0) return;
    await startLocalReadingSession(gallery.gid, Math.floor(Date.now() / 1000)).catch(() => {});
    openReader(pageIdx);
  }

  async function handleDelete() {
    if (!gallery) return;
    const confirmed = confirm($t("detail.delete_local_confirm"));
    if (!confirmed) return;
    deleting = true;
    try {
      await deleteLocalGallery(gallery.gid);
      $libraryRefreshTick += 1;
      $localDetailGallery = null;
    } catch (err) {
      deleteMessage = String(err);
      deleting = false;
    }
  }

  function handleClose() {
    $localDetailGallery = null;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && gallery && !showMetadataEditor) {
      handleClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if gallery}
  <div class="detail-overlay" onclick={handleClose} role="presentation"></div>
  <div class="detail-panel">
    <div class="detail-header">
      <button class="back-btn" onclick={handleClose}>
        <svg width="18" height="18" viewBox="0 0 16 16" fill="none">
          <path d="M10 4L6 8l4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        {$t("common.back")}
      </button>
    </div>

    <div class="detail-body">
      <!-- Cover + title -->
      <div class="top-section">
        <div class="preview">
          <div class="preview-skeleton" class:hidden={imgLoaded}></div>
          {#if imgSrc}
            <img src={imgSrc} alt={gallery.title} class:loaded={imgLoaded} onload={() => imgLoaded = true} />
          {:else if pages.length > 0}
            <img src={convertFileSrc(pages[0].filePath)} alt={gallery.title} class:loaded={imgLoaded} onload={() => imgLoaded = true} />
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
          {/if}
        </div>
      </div>

      <!-- Metadata -->
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
        {#if date}
          <div class="meta-item">
            <span class="meta-label">{$t("detail.uploaded")}</span>
            <span class="meta-value">{date}</span>
          </div>
        {/if}
      </div>

      <!-- Actions -->
      <div class="actions">
        <button class="action-btn primary" onclick={() => handleRead()}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h6a4 4 0 014 4v14a3 3 0 00-3-3H2z"/><path d="M22 3h-6a4 4 0 00-4 4v14a3 3 0 013-3h7z"/></svg>
          {$t("detail.read")}
        </button>
        <button class="action-btn" onclick={() => showMetadataEditor = true}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
          {$t("local.edit_metadata")}
        </button>
        <button class="action-btn danger" onclick={handleDelete} disabled={deleting}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 011-1h4a1 1 0 011 1v2"/></svg>
          {deleting ? $t("common.loading") : $t("detail.delete_local")}
        </button>
      </div>
      {#if deleteMessage}
        <p class="error-msg">{deleteMessage}</p>
      {/if}

      <!-- Tags -->
      {#if gallery.tags.length > 0}
        <div class="tags-section">
          <h3>{$t("detail.tags")}</h3>
          <div class="tag-groups">
            {#each (() => {
              const groups = new Map<string, string[]>();
              for (const tag of gallery.tags) {
                const arr = groups.get(tag.namespace) || [];
                arr.push(tag.name);
                groups.set(tag.namespace, arr);
              }
              return Array.from(groups.entries()).map(([ns, names]) => ({ namespace: ns, names }));
            })() as group}
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

      <!-- Page previews -->
      {#if pages.length > 0}
        <div class="pages-section">
          <h3>{$t("detail.page_count")} ({pages.length})</h3>
          <div class="pages-grid">
            {#each pages as page (page.page_index)}
              <button class="page-thumb-wrapper" onclick={() => handleOpenPage(page.page_index)}>
                <div class="page-thumb">
                  <img src={convertFileSrc(page.file_path)} alt="Page {page.page_index + 1}" loading="lazy" />
                </div>
                <span class="page-num-label">{page.page_index + 1}</span>
              </button>
            {/each}
          </div>
        </div>
      {:else if loadingPages}
        <div class="pages-section">
          <h3>{$t("detail.page_count")}</h3>
          <div class="pages-grid">
            {#each { length: gallery.file_count || 8 } as _, i}
              <div class="page-thumb-skeleton-item">
                <span class="page-thumb-skeleton-num">{i + 1}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if showMetadataEditor && gallery}
  <LocalMetadataEditor
    gallery={gallery}
    onClose={() => { showMetadataEditor = false; }}
  />
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
  }

  @keyframes slideIn {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }

  .detail-header {
    display: flex;
    align-items: center;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--bg-primary);
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
  }

  .title-jpn {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .uploader {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .meta-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1.5rem;
  }

  .meta-item {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .meta-label {
    font-size: 0.7rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .meta-value {
    font-size: 0.82rem;
    color: var(--text-primary);
    font-weight: 500;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.45rem 0.85rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-tertiary);
    color: var(--text-primary);
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .action-btn:hover {
    background: var(--bg-elevated);
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .action-btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .action-btn.primary:hover {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }

  .action-btn.danger {
    border-color: var(--danger-border);
    background: var(--danger-bg);
    color: var(--red);
  }

  .action-btn.danger:hover {
    opacity: 0.85;
  }

  .error-msg {
    margin: 0;
    font-size: 0.8rem;
    color: var(--red);
  }

  .tags-section h3,
  .pages-section h3 {
    margin: 0 0 0.6rem;
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .tag-groups {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .tag-group {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.3rem;
  }

  .tag-namespace {
    font-size: 0.72rem;
    color: var(--text-muted);
    min-width: 70px;
    flex-shrink: 0;
  }

  .tag-names {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .tag-chip {
    font-size: 0.72rem;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    padding: 2px 7px;
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .pages-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 0.5rem;
  }

  .page-thumb-wrapper {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    border-radius: var(--radius-sm);
    transition: opacity 0.15s;
  }

  .page-thumb-wrapper:hover {
    opacity: 0.8;
  }

  .page-thumb {
    width: 100%;
    aspect-ratio: 2 / 3;
    overflow: hidden;
    border-radius: var(--radius-sm);
    background: var(--bg-tertiary);
  }

  .page-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .page-num-label {
    font-size: 0.7rem;
    color: var(--text-muted);
  }

  .page-thumb-skeleton-item {
    aspect-ratio: 2 / 3;
    background: var(--bg-tertiary);
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    animation: skeleton-pulse 1.8s ease-in-out infinite;
  }

  .page-thumb-skeleton-num {
    font-size: 0.75rem;
    color: var(--text-muted);
  }
</style>
