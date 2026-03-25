<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "$lib/i18n";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
  import {
    updateGalleryMetadata,
    getLocalGalleryPages,
    reorderLocalPages,
    removeLocalPage,
    insertLocalPages,
    setLocalGalleryCover,
    type LocalPage,
  } from "$lib/api/library";
  import { thumbSrc } from "$lib/utils/thumb";
  import type { Gallery, Tag } from "$lib/api/galleries";

  const EH_CATEGORIES = [
    "Doujinshi", "Manga", "Artist CG", "Game CG", "Western",
    "Non-H", "Image Set", "Cosplay", "Asian Porn", "Misc",
  ];

  const COMMON_NAMESPACES = [
    "artist", "group", "parody", "character", "language", "female", "male", "misc",
  ];

  interface Props {
    gallery: Gallery;
    onClose: () => void;
  }

  let { gallery, onClose }: Props = $props();

  // Form state
  let title = $state(gallery.title);
  let titleJpn = $state(gallery.title_jpn ?? "");
  let category = $state(gallery.category);
  let uploader = $state(gallery.uploader ?? "");
  let description = $state((gallery as any).description ?? "");
  let tags = $state<Tag[]>([...gallery.tags]);
  let coverPath = $state<string | null>(gallery.thumb_path);

  // Dirty tracking — compare against original values
  const origTitle = gallery.title;
  const origTitleJpn = gallery.title_jpn ?? "";
  const origCategory = gallery.category;
  const origUploader = gallery.uploader ?? "";
  const origDescription = (gallery as any).description ?? "";

  let dirty = $derived(
    title !== origTitle ||
    titleJpn !== origTitleJpn ||
    category !== origCategory ||
    uploader !== origUploader ||
    description !== origDescription
  );

  // New tag form
  let newTagNs = $state("");
  let newTagName = $state("");

  // Pages state
  let pages = $state<LocalPage[]>([]);
  let pagesLoading = $state(false);
  let pagesError = $state<string | null>(null);

  // Save state
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let saveSuccess = $state(false);

  // Drag state for page reorder
  let dragFromIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);

  onMount(async () => {
    await loadPages();
  });

  async function loadPages() {
    pagesLoading = true;
    pagesError = null;
    try {
      pages = await getLocalGalleryPages(gallery.gid);
    } catch (err) {
      pagesError = String(err);
    } finally {
      pagesLoading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") attemptClose();
  }

  function attemptClose() {
    if (dirty && !confirm($t("local.unsaved_changes"))) return;
    onClose();
  }

  async function handleSave() {
    saving = true;
    saveError = null;
    saveSuccess = false;
    try {
      await updateGalleryMetadata(gallery.gid, {
        title: title.trim() || undefined,
        titleJpn: titleJpn.trim() || undefined,
        category,
        uploader: uploader.trim() || undefined,
        description: description.trim() || undefined,
      });
      dirty = false;
      saveSuccess = true;
      setTimeout(() => { saveSuccess = false; }, 2500);
    } catch (err) {
      saveError = String(err);
    } finally {
      saving = false;
    }
  }

  async function handleAddTag() {
    const name = newTagName.trim();
    if (!name) return;
    const ns = newTagNs.trim() || "misc";
    const newTag: Tag = { namespace: ns, name };
    try {
      await updateGalleryMetadata(gallery.gid, { tagsAdd: [{ namespace: ns, name }] });
      tags = [...tags, newTag];
      newTagNs = "";
      newTagName = "";
    } catch (err) {
      saveError = String(err);
    }
  }

  async function handleRemoveTag(tag: Tag) {
    try {
      await updateGalleryMetadata(gallery.gid, { tagsRemove: [{ namespace: tag.namespace, name: tag.name }] });
      tags = tags.filter((t) => !(t.namespace === tag.namespace && t.name === tag.name));
    } catch (err) {
      saveError = String(err);
    }
  }

  async function handleChangeCover() {
    try {
      const filePath = await openFileDialog({
        multiple: false,
        filters: [{ name: "Image", extensions: ["jpg", "jpeg", "png", "webp"] }],
      });
      if (!filePath) return;
      const newPath = await setLocalGalleryCover(gallery.gid, filePath as string);
      coverPath = newPath;
    } catch (err) {
      saveError = String(err);
    }
  }

  async function handleRemovePage(page: LocalPage) {
    const deleteDisk = confirm($t("local.remove_page_confirm"));
    try {
      await removeLocalPage(gallery.gid, page.page_index, deleteDisk);
      pages = pages.filter((p) => p.page_index !== page.page_index);
    } catch (err) {
      saveError = String(err);
    }
  }

  async function handleInsertPages() {
    try {
      const rawPaths = await openFileDialog({
        multiple: true,
        filters: [{ name: "Image", extensions: ["jpg", "jpeg", "png", "webp"] }],
      });
      if (!rawPaths || (rawPaths as string[]).length === 0) return;
      const insertAfter = pages.length > 0 ? pages[pages.length - 1].pageIndex : -1;
      const newPages = await insertLocalPages(gallery.gid, rawPaths as string[], insertAfter);
      pages = [...pages, ...newPages];
    } catch (err) {
      saveError = String(err);
    }
  }

  // Drag reorder
  function handleDragStart(e: DragEvent, idx: number) {
    dragFromIndex = idx;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
    }
  }

  function handleDragOver(e: DragEvent, idx: number) {
    e.preventDefault();
    dragOverIndex = idx;
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "move";
    }
  }

  function handleDragLeave() {
    dragOverIndex = null;
  }

  async function handleDrop(e: DragEvent, toIndex: number) {
    e.preventDefault();
    dragOverIndex = null;
    if (dragFromIndex === null || dragFromIndex === toIndex) { dragFromIndex = null; return; }

    const newPages = [...pages];
    const [moved] = newPages.splice(dragFromIndex, 1);
    newPages.splice(toIndex, 0, moved);
    pages = newPages;
    dragFromIndex = null;

    try {
      await reorderLocalPages(gallery.gid, newPages.map((p) => p.page_index));
    } catch (err) {
      saveError = String(err);
      await loadPages(); // revert on error
    }
  }

  function handleDragEnd() {
    dragFromIndex = null;
    dragOverIndex = null;
  }

  // Group tags by namespace for display
  let tagGroups = $derived(() => {
    const groups: Record<string, Tag[]> = {};
    for (const tag of tags) {
      if (!groups[tag.namespace]) groups[tag.namespace] = [];
      groups[tag.namespace].push(tag);
    }
    return Object.entries(groups);
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="editor-overlay" role="dialog" aria-modal="true" aria-label="Edit Metadata">
  <!-- Header -->
  <div class="editor-header">
    <span class="editor-title">{$t("local.edit_metadata")}</span>
    <div class="header-actions">
      {#if saveSuccess}
        <span class="save-success">Saved</span>
      {/if}
      <button class="btn-primary" onclick={handleSave} disabled={saving}>
        {saving ? $t("common.loading") : $t("local.save")}
      </button>
      <button class="close-btn" onclick={attemptClose} aria-label="Close">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>
  </div>

  <div class="editor-body">
    {#if saveError}
      <div class="error-banner">{saveError}</div>
    {/if}

    <!-- Cover -->
    <section class="section">
      <h3 class="section-title">{$t("local.cover")}</h3>
      <div class="cover-row">
        <div class="cover-thumb">
          {#if coverPath}
            <img src={convertFileSrc(coverPath)} alt="cover" />
          {:else}
            <div class="cover-placeholder"></div>
          {/if}
        </div>
        <button class="btn-outline" onclick={handleChangeCover}>{$t("local.change_cover")}</button>
      </div>
    </section>

    <!-- Basic fields -->
    <section class="section">
      <h3 class="section-title">Basic Info</h3>
      <div class="field-group">
        <label class="field-label" for="meta-title">Title</label>
        <input id="meta-title" class="text-input" type="text" bind:value={title} />
      </div>
      <div class="field-group">
        <label class="field-label" for="meta-title-jpn">Japanese Title</label>
        <input id="meta-title-jpn" class="text-input" type="text" bind:value={titleJpn} placeholder="(optional)" />
      </div>
      <div class="field-row-2">
        <div class="field-group">
          <label class="field-label" for="meta-cat">Category</label>
          <select id="meta-cat" class="select-input" bind:value={category}>
            {#each EH_CATEGORIES as cat}
              <option value={cat}>{cat}</option>
            {/each}
          </select>
        </div>
        <div class="field-group">
          <label class="field-label" for="meta-uploader">Uploader</label>
          <input id="meta-uploader" class="text-input" type="text" bind:value={uploader} placeholder="(optional)" />
        </div>
      </div>
      <div class="field-group">
        <label class="field-label" for="meta-desc">{$t("local.description")}</label>
        <textarea id="meta-desc" class="text-input textarea" bind:value={description} rows="4" placeholder="(optional)"></textarea>
      </div>
    </section>

    <!-- Tags -->
    <section class="section">
      <h3 class="section-title">{$t("local.tags_local")}</h3>

      {#each tagGroups() as [ns, nsTag]}
        <div class="tag-group">
          <span class="tag-ns">{ns}:</span>
          <div class="tag-chips">
            {#each nsTag as tag}
              <span class="tag-chip">
                {tag.name}
                <button class="tag-remove" onclick={() => handleRemoveTag(tag)} aria-label="Remove tag">
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                    <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                  </svg>
                </button>
              </span>
            {/each}
          </div>
        </div>
      {/each}

      <!-- Add tag form -->
      <div class="add-tag-row">
        <input
          class="text-input tag-ns-input"
          type="text"
          bind:value={newTagNs}
          placeholder={$t("local.tag_namespace")}
          list="ns-suggestions"
        />
        <datalist id="ns-suggestions">
          {#each COMMON_NAMESPACES as ns}
            <option value={ns}></option>
          {/each}
        </datalist>
        <input
          class="text-input tag-val-input"
          type="text"
          bind:value={newTagName}
          placeholder={$t("local.tag_value")}
          onkeydown={(e) => { if (e.key === "Enter") handleAddTag(); }}
        />
        <button class="btn-outline" onclick={handleAddTag} disabled={!newTagName.trim()}>
          {$t("local.add_tag")}
        </button>
      </div>
    </section>

    <!-- Pages -->
    <section class="section">
      <div class="pages-header">
        <h3 class="section-title">Pages ({pages.length})</h3>
        <button class="btn-outline" onclick={handleInsertPages}>{$t("local.insert_pages")}</button>
      </div>

      {#if pagesLoading}
        <div class="pages-loading">
          <div class="spinner"></div>
        </div>
      {:else if pagesError}
        <p class="error-text">{pagesError}</p>
      {:else if pages.length > 0}
        <div class="pages-grid">
          {#each pages as page, idx (page.page_index)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="page-thumb-wrap"
              class:drag-over={dragOverIndex === idx}
              draggable="true"
              ondragstart={(e) => handleDragStart(e, idx)}
              ondragover={(e) => handleDragOver(e, idx)}
              ondragleave={handleDragLeave}
              ondrop={(e) => handleDrop(e, idx)}
              ondragend={handleDragEnd}
            >
              <div class="page-thumb">
                <img src={convertFileSrc(page.file_path)} alt={$t("local.page_index", { n: page.page_index + 1 })} loading="lazy" />
                <span class="page-num">{page.page_index + 1}</span>
                <button
                  class="page-remove-btn"
                  onclick={() => handleRemovePage(page)}
                  aria-label={$t("local.remove_page")}
                  title={$t("local.remove_page")}
                >
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                    <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                  </svg>
                </button>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <p class="empty-pages">{$t("local.empty_hint")}</p>
      {/if}
    </section>
  </div>
</div>

<style>
  .editor-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: var(--bg-primary);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* ── Header ─────────────────────────────────────── */

  .editor-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--bg-primary);
  }

  .editor-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .save-success {
    font-size: 0.74rem;
    color: var(--green);
    font-weight: 500;
  }

  .close-btn {
    width: 28px;
    height: 28px;
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

  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  /* ── Body ────────────────────────────────────────── */

  .editor-body {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    max-width: 700px;
    width: 100%;
    margin: 0 auto;
  }

  .error-banner {
    padding: 8px 12px;
    background: var(--danger-bg);
    border: 1px solid var(--danger-border);
    border-radius: var(--radius-sm);
    color: var(--red);
    font-size: 0.78rem;
  }

  /* ── Sections ───────────────────────────────────── */

  .section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .section-title {
    margin: 0;
    font-size: 0.78rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }

  /* ── Cover ──────────────────────────────────────── */

  .cover-row {
    display: flex;
    align-items: flex-end;
    gap: 12px;
  }

  .cover-thumb {
    width: 80px;
    height: 112px;
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: var(--bg-tertiary);
    flex-shrink: 0;
    border: 1px solid var(--border-strong);
  }

  .cover-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .cover-placeholder {
    width: 100%;
    height: 100%;
    background: var(--bg-elevated);
  }

  /* ── Fields ─────────────────────────────────────── */

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .field-label {
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--text-muted);
  }

  .text-input {
    padding: 6px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.82rem;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s;
    width: 100%;
    box-sizing: border-box;
  }

  .text-input:focus { border-color: var(--accent); }

  .select-input {
    padding: 6px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.82rem;
    outline: none;
    width: 100%;
  }

  .textarea {
    resize: vertical;
    min-height: 80px;
  }

  /* ── Tags ───────────────────────────────────────── */

  .tag-group {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    flex-wrap: wrap;
  }

  .tag-ns {
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--text-muted);
    min-width: 64px;
    padding-top: 4px;
    flex-shrink: 0;
  }

  .tag-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px 2px 8px;
    border-radius: 10px;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    font-size: 0.72rem;
    border: 1px solid var(--border);
  }

  .tag-remove {
    width: 14px;
    height: 14px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    transition: color 0.1s, background 0.1s;
  }

  .tag-remove:hover {
    color: var(--red);
    background: var(--danger-bg);
  }

  .add-tag-row {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }

  .tag-ns-input { width: 140px; flex-shrink: 0; }
  .tag-val-input { flex: 1; min-width: 120px; }

  /* ── Pages ──────────────────────────────────────── */

  .pages-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .pages-loading {
    display: flex;
    justify-content: center;
    padding: 16px;
  }

  .spinner {
    width: 20px;
    height: 20px;
    border: 2.5px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .pages-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
    gap: 8px;
  }

  .page-thumb-wrap {
    cursor: grab;
    border-radius: var(--radius-sm);
    transition: outline 0.1s;
  }

  .page-thumb-wrap:active { cursor: grabbing; }

  .page-thumb-wrap.drag-over {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .page-thumb {
    position: relative;
    width: 100%;
    aspect-ratio: 3/4;
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
  }

  .page-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .page-num {
    position: absolute;
    bottom: 3px;
    left: 4px;
    font-size: 0.6rem;
    font-weight: 600;
    color: #fff;
    text-shadow: 0 1px 3px rgba(0,0,0,0.8);
    pointer-events: none;
  }

  .page-remove-btn {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: none;
    background: rgba(0,0,0,0.6);
    color: #fff;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .page-thumb:hover .page-remove-btn { opacity: 1; }

  .empty-pages {
    font-size: 0.78rem;
    color: var(--text-muted);
    margin: 0;
  }

  .error-text {
    font-size: 0.78rem;
    color: var(--red);
    margin: 0;
  }

  /* ── Buttons ────────────────────────────────────── */

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

  .btn-primary:hover:not(:disabled) { background: var(--accent-hover); }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-outline {
    padding: 5px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    white-space: nowrap;
  }

  .btn-outline:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .btn-outline:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
