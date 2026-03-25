<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { Gallery, Tag } from "$lib/api/galleries";
  import { categoryColor } from "$lib/utils/category";
  import { localDetailGallery } from "$lib/stores/localLibrary";

  let { gallery }: { gallery: Gallery } = $props();

  let catColor = $derived(categoryColor(gallery.category));
  let langTag = $derived(gallery.tags.find(t => t.namespace === "language")?.name ?? null);
  let tagPills = $derived(gallery.tags.filter(t => t.namespace !== "language").slice(0, 5));
  let hasMoreTags = $derived(gallery.tags.filter(t => t.namespace !== "language").length > 5);

  let imgSrc = $derived(gallery.thumb_path ? convertFileSrc(gallery.thumb_path) : "");
  let imgLoaded = $state(false);

  $effect(() => {
    // Reset load state when thumb changes.
    void gallery.thumb_path;
    imgLoaded = false;
  });

  function handleClick() {
    $localDetailGallery = gallery;
  }
</script>

<div
  class="gallery-card"
  onclick={handleClick}
  role="button"
  tabindex="0"
  onkeydown={(e) => { if (e.key === "Enter") handleClick(); }}
>
  <div class="thumb-container">
    <div class="thumb-skeleton" class:hidden={imgLoaded}></div>
    {#if imgSrc}
      <img
        src={imgSrc}
        alt={gallery.title}
        loading="lazy"
        class:loaded={imgLoaded}
        onload={() => imgLoaded = true}
        onerror={() => imgLoaded = false}
      />
    {/if}
    <div class="thumb-overlay">
      <span class="category-pill" style="--cat-color:{catColor}">{gallery.category}</span>
    </div>
  </div>
  <div class="info">
    <h3 class="title" title={gallery.title}>{gallery.title}</h3>
    <div class="meta-row">
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
          <span class="tag-pill" title="{tag.namespace}:{tag.name}">{tag.name}</span>
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
    justify-content: flex-end;
    gap: 0.35rem;
    min-width: 0;
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
  }

  .pages {
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
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
  }

  .tag-more {
    font-size: 0.75rem;
    color: var(--text-muted);
    padding: 2px 4px;
    font-weight: 500;
  }
</style>
