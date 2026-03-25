<script lang="ts">
  import type { Gallery, Tag } from "$lib/api/galleries";
  import type { ReadProgress } from "$lib/api/reader";
  import { t } from "$lib/i18n";
  import { thumbSrc } from "$lib/utils/thumb";
  import { categoryColor } from "$lib/utils/category";
  import { currentPage } from "$lib/stores/navigation";
  import { searchResults, searchIncludeTags } from "$lib/stores/search";

  let {
    gallery,
    progress,
    onOpen,
    overrideImgSrc,
    noTagNav = false,
  }: {
    gallery: Gallery;
    progress?: ReadProgress | null;
    onOpen?: (gallery: Gallery) => void;
    /** Override computed thumbnail src (e.g. for local galleries using convertFileSrc). */
    overrideImgSrc?: string;
    /** Disable tag-click navigation (e.g. local library context). */
    noTagNav?: boolean;
  } = $props();

  function handleTagClick(e: MouseEvent, tag: Tag) {
    if (noTagNav) return;
    e.stopPropagation();
    if (!$searchIncludeTags.some(t => t.namespace === tag.namespace && t.name === tag.name)) {
      $searchIncludeTags = [...$searchIncludeTags, { namespace: tag.namespace, name: tag.name }];
    }
    $searchResults = [];
    $currentPage = "search";
  }

  let imgSrc = $derived(overrideImgSrc ?? thumbSrc(gallery.thumb_path, gallery.thumb_url));
  let catColor = $derived(categoryColor(gallery.category));
  let stars = $derived("★".repeat(Math.round(gallery.rating)));
  let langTag = $derived(gallery.tags.find(t => t.namespace === "language")?.name ?? null);

  let tagPills = $derived(
    gallery.tags
      .filter(t => t.namespace !== "language")
      .slice(0, 5)
  );
  let hasMoreTags = $derived(gallery.tags.filter(t => t.namespace !== "language").length > 5);
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
      // Use reactive state for retry — don't touch the DOM directly.
      setTimeout(() => {
        retrySuffix = (imgSrc.includes("?") ? "&" : "?") + "_r=" + imgRetries;
      }, 1000 * imgRetries);
    }
  }
</script>

<div
  class="gallery-card"
  onclick={handleClick}
  role="button"
  tabindex="0"
  onkeydown={(e) => { if (e.key === 'Enter') handleClick(); }}
>
  <div class="thumb-container">
    <div class="thumb-skeleton" class:hidden={imgLoaded}></div>
    {#if displaySrc}
      <img src={displaySrc} alt={gallery.title} loading="lazy" class:loaded={imgLoaded} onload={handleImgLoad} onerror={handleImgError} />
    {/if}
    <div class="thumb-overlay">
      <span class="category-pill" style="--cat-color:{catColor}">{gallery.category}</span>
      {#if progress?.is_completed}
        <span class="completed-badge">{$t("gallery.done")}</span>
      {/if}
    </div>
  </div>
  <div class="info">
    <h3 class="title" title={gallery.title}>{gallery.title}</h3>
    <div class="meta-row">
      <span class="rating" title="{gallery.rating.toFixed(1)}">{stars}</span>
      <div class="meta-right">
        {#if langTag}
          <span class="lang-badge">{langTag.slice(0, 2).toUpperCase()}</span>
        {/if}
        <span class="pages">{gallery.file_count}p</span>
      </div>
    </div>
    {#if tagPills.length > 0}
      <div class="tag-row">
        {#each tagPills as tag}
          <button class="tag-pill" onclick={(e) => handleTagClick(e, tag)} title="{tag.namespace}:{tag.name}">{tag.name}</button>
        {/each}
        {#if hasMoreTags}
          <span class="tag-more">+{gallery.tags.filter(t => t.namespace !== "language").length - 5}</span>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .gallery-card {
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: var(--card-bg);
    cursor: pointer;
    display: flex;
    flex-direction: column;
    contain: layout style paint;
    content-visibility: auto;
    contain-intrinsic-size: auto 300px;
    border: 1px solid var(--card-border);
    box-shadow: var(--shadow-sm);
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }

  .gallery-card:hover {
    border-color: var(--card-border-hover);
    box-shadow: var(--shadow-md);
  }

  .thumb-container {
    position: relative;
    width: 100%;
    aspect-ratio: 3 / 4;
    overflow: hidden;
    background: var(--bg-tertiary);
  }

  .thumb-container img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    opacity: 0;
    transition: opacity 0.2s ease;
    position: absolute;
    inset: 0;
  }

  .thumb-container img.loaded {
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

  .thumb-overlay {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    padding: 6px 8px;
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    pointer-events: none;
  }

  .category-pill {
    font-size: 0.6rem;
    font-weight: 700;
    color: #fff;
    padding: 3px 8px;
    border-radius: 6px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    background: var(--cat-color);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 50%;
  }

  .completed-badge {
    font-size: 0.58rem;
    font-weight: 700;
    color: #fff;
    background: var(--green);
    padding: 3px 8px;
    border-radius: 6px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .info {
    padding: 0.75rem 0.85rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    flex: none;
    height: 8.5rem;
    overflow: hidden;
  }

  .title {
    font-size: 0.875rem;
    font-weight: 700;
    margin: 0;
    line-height: 1.4;
    color: var(--text-primary);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .meta-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.35rem;
    min-width: 0;
  }

  .rating {
    color: var(--yellow);
    font-size: 0.75rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    letter-spacing: 0.5px;
  }

  .meta-right {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    min-width: 0;
    flex-shrink: 0;
  }

  .lang-badge {
    font-size: 0.68rem;
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    padding: 2px 8px;
    border-radius: 6px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pages {
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .tag-row {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    overflow: hidden;
    max-height: 2.5rem;
    margin-top: auto;
  }

  .tag-pill {
    font-size: 0.72rem;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    padding: 2px 8px;
    border-radius: 6px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
    border: none;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .tag-pill:hover {
    background: var(--accent-subtle);
    color: var(--accent);
  }

  .tag-more {
    font-size: 0.75rem;
    color: var(--text-muted);
    padding: 2px 4px;
    font-weight: 500;
  }
</style>
