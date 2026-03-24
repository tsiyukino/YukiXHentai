<script lang="ts">
  import type { Gallery, Tag } from "$lib/api/galleries";
  import type { ReadProgress } from "$lib/api/reader";
  import { thumbSrc } from "$lib/utils/thumb";
  import { categoryColor } from "$lib/utils/category";
  import { currentPage } from "$lib/stores/navigation";
  import { searchQuery, searchResults } from "$lib/stores/search";

  let {
    gallery,
    progress,
    onOpen,
  }: {
    gallery: Gallery;
    progress?: ReadProgress | null;
    onOpen?: (gallery: Gallery) => void;
  } = $props();

  let imgSrc = $derived(thumbSrc(gallery.thumb_path, gallery.thumb_url));
  let catColor = $derived(categoryColor(gallery.category));
  let langTag = $derived(gallery.tags.find(t => t.namespace === "language")?.name ?? null);
  let tagPills = $derived(gallery.tags.filter(t => t.namespace !== "language").slice(0, 8));
  let hasMoreTags = $derived(gallery.tags.filter(t => t.namespace !== "language").length > 8);
  let imgLoaded = $state(false);
  let imgRetries = $state(0);
  let retrySuffix = $state("");
  const MAX_RETRIES = 3;

  // Append a cache-busting suffix on retry so Svelte sees a new src.
  let displaySrc = $derived(imgSrc ? imgSrc + retrySuffix : "");

  // Reset loaded state when the underlying src changes (e.g. thumb_path arrives).
  let prevImgSrc = $state(imgSrc);
  $effect(() => {
    if (imgSrc !== prevImgSrc) {
      prevImgSrc = imgSrc;
      imgLoaded = false;
      imgRetries = 0;
      retrySuffix = "";
    }
  });

  function handleClick() {
    onOpen?.(gallery);
  }

  function handleImgLoad() {
    imgLoaded = true;
  }

  function handleImgError() {
    if (imgRetries < MAX_RETRIES && imgSrc) {
      imgRetries++;
      setTimeout(() => {
        retrySuffix = (imgSrc.includes("?") ? "&" : "?") + "_r=" + imgRetries;
      }, 1000 * imgRetries);
    }
  }

  function handleTagClick(e: MouseEvent, tag: Tag) {
    e.stopPropagation();
    const query = tag.name.includes(" ")
      ? `${tag.namespace}:"${tag.name}"`
      : `${tag.namespace}:${tag.name}`;
    $searchQuery = query;
    $searchResults = [];
    $currentPage = "search";
  }
</script>

<div
  class="list-item"
  onclick={handleClick}
  role="button"
  tabindex="0"
  onkeydown={(e) => { if (e.key === 'Enter') handleClick(); }}
>
  <div class="thumb">
    <div class="thumb-skeleton" class:hidden={imgLoaded}></div>
    {#if displaySrc}
      <img src={displaySrc} alt={gallery.title} loading="lazy" class:loaded={imgLoaded} onload={handleImgLoad} onerror={handleImgError} />
    {/if}
  </div>
  <div class="middle">
    <h3 class="title" title={gallery.title}>{gallery.title}</h3>
    <div class="tag-row">
      {#each tagPills as tag}
        <button class="tag-pill" onclick={(e) => handleTagClick(e, tag)} title="{tag.namespace}:{tag.name}">{tag.namespace}:{tag.name}</button>
      {/each}
      {#if hasMoreTags}
        <span class="tag-more">...</span>
      {/if}
    </div>
  </div>
  <div class="right">
    <span class="category-pill" style="background:{catColor}">{gallery.category}</span>
    {#if langTag}
      <span class="lang-badge">{langTag.slice(0, 2).toUpperCase()}</span>
    {/if}
    <span class="pages">{gallery.file_count}p</span>
    {#if progress?.is_completed}
      <span class="done-badge">Done</span>
    {/if}
  </div>
</div>

<style>
  .list-item {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    padding: 0.65rem 1rem;
    border-radius: var(--radius-md);
    background: var(--card-bg);
    cursor: pointer;
    transition: background 0.15s, box-shadow 0.15s;
    contain: layout style paint;
    content-visibility: auto;
    contain-intrinsic-size: auto 72px;
    border: 1px solid var(--card-border);
    box-shadow: var(--shadow-sm);
  }

  .list-item:hover {
    box-shadow: var(--shadow-md);
    border-color: var(--card-border-hover);
  }

  .thumb {
    width: 52px;
    height: 70px;
    flex-shrink: 0;
    border-radius: 6px;
    overflow: hidden;
    background: var(--bg-tertiary);
    position: relative;
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    opacity: 0;
    transition: opacity 0.2s ease;
    position: absolute;
    inset: 0;
  }

  .thumb img.loaded {
    opacity: 1;
  }

  .thumb-skeleton {
    width: 100%;
    height: 100%;
    background: var(--bg-tertiary);
    animation: skeleton-pulse 1.8s ease-in-out infinite;
  }

  .thumb-skeleton.hidden {
    animation: none;
    opacity: 0;
  }

  @keyframes skeleton-pulse {
    0%, 100% { opacity: 0.3; }
    50% { opacity: 0.5; }
  }

  .middle {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .title {
    font-size: 0.85rem;
    font-weight: 600;
    margin: 0;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tag-row {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    overflow: hidden;
    max-height: 1.5rem;
  }

  .tag-pill {
    font-size: 0.65rem;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    padding: 2px 8px;
    border-radius: 6px;
    white-space: nowrap;
    border: none;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .tag-pill:hover {
    background: var(--accent-subtle);
    color: var(--accent);
  }

  .tag-more {
    font-size: 0.65rem;
    color: var(--text-muted);
  }

  .right {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.3rem;
    flex-shrink: 0;
  }

  .category-pill {
    font-size: 0.6rem;
    font-weight: 700;
    color: #fff;
    padding: 3px 8px;
    border-radius: 6px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    white-space: nowrap;
  }

  .lang-badge {
    font-size: 0.62rem;
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    padding: 2px 8px;
    border-radius: 6px;
  }

  .pages {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .done-badge {
    font-size: 0.58rem;
    font-weight: 700;
    color: #fff;
    background: var(--green);
    padding: 3px 8px;
    border-radius: 6px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
</style>
