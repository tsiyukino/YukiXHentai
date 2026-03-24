<script lang="ts">
  import type { TagFilter } from "$lib/api/galleries";
  import { homeFilter, emptyHomeFilter, galleries, sortScope, sortActive, emptySortScope } from "$lib/stores/galleries";
  import type { SortField, SortScopeMode } from "$lib/stores/galleries";
  import TagInputAutocomplete from "./TagInputAutocomplete.svelte";

  let { onClose, onSort }: { onClose: () => void; onSort?: () => void } = $props();

  // Mirror store into local state for the form inputs.
  let tagsInclude = $state<TagFilter[]>($homeFilter.tagsInclude);
  let tagsExclude = $state<TagFilter[]>($homeFilter.tagsExclude);
  let categories  = $state<string[]>($homeFilter.categories);
  let ratingMin   = $state($homeFilter.ratingMin !== null ? String($homeFilter.ratingMin) : "");
  let pagesMin    = $state($homeFilter.pagesMin  !== null ? String($homeFilter.pagesMin)  : "");
  let pagesMax    = $state($homeFilter.pagesMax  !== null ? String($homeFilter.pagesMax)  : "");
  let language    = $state($homeFilter.language);
  let uploader    = $state($homeFilter.uploader);

  // Sort state (local form copy)
  let sortScopeMode  = $state<SortScopeMode>($sortScope.mode);
  let sortCount      = $state<string>(
    $sortScope.count > 0 ? String($sortScope.count) : "100"
  );
  let sortDays       = $state<string>(String($sortScope.days));
  let sortField      = $state<SortField>($sortScope.field);
  let sortDir        = $state<"asc" | "desc">($sortScope.dir);
  let customCount    = $state(false);

  // When mode switches, clear the other
  function setSortMode(mode: SortScopeMode) {
    sortScopeMode = mode;
  }

  function setPresetCount(n: number) {
    sortCount = String(n);
    customCount = false;
    sortScopeMode = "count";
  }

  function handleCountInput(val: string) {
    sortCount = val;
    customCount = true;
    sortScopeMode = "count";
  }

  const COUNT_PRESETS = [100, 250, 500, 1000];

  const ALL_CATEGORIES = [
    "Doujinshi", "Manga", "Artist CG", "Game CG", "Western",
    "Non-H", "Image Set", "Cosplay", "Asian Porn", "Misc",
  ];

  function toggleCategory(cat: string) {
    categories = categories.includes(cat)
      ? categories.filter(c => c !== cat)
      : [...categories, cat];
  }

  function commit() {
    $homeFilter = {
      tagsInclude,
      tagsExclude,
      categories,
      ratingMin:  ratingMin.trim()  ? parseFloat(ratingMin)  : null,
      pagesMin:   pagesMin.trim()   ? parseInt(pagesMin)    : null,
      pagesMax:   pagesMax.trim()   ? parseInt(pagesMax)    : null,
      language:   language.trim(),
      uploader:   uploader.trim(),
    };
    onClose();
  }

  function handleClear() {
    tagsInclude = [];
    tagsExclude = [];
    categories  = [];
    ratingMin   = "";
    pagesMin    = "";
    pagesMax    = "";
    language    = "";
    uploader    = "";
    $homeFilter = emptyHomeFilter();
    onClose();
  }

  function handleSort() {
    // Commit the filter state first
    $homeFilter = {
      tagsInclude,
      tagsExclude,
      categories,
      ratingMin:  ratingMin.trim()  ? parseFloat(ratingMin)  : null,
      pagesMin:   pagesMin.trim()   ? parseInt(pagesMin)    : null,
      pagesMax:   pagesMax.trim()   ? parseInt(pagesMax)    : null,
      language:   language.trim(),
      uploader:   uploader.trim(),
    };

    const count = parseInt(sortCount) || $galleries.length || 100;
    const days = parseInt(sortDays) || 30;

    $sortScope = {
      mode: sortScopeMode,
      count,
      days,
      field: sortField,
      dir: sortDir,
    };

    onSort?.();
    onClose();
  }

  function clearSort() {
    $sortActive = false;
    sortScopeMode = "count";
    sortCount = String($galleries.length || 100);
    sortDays = "30";
    sortField = "posted";
    sortDir = "desc";
    $sortScope = emptySortScope();
  }
</script>

<div class="filter-panel">
  <!-- Tags -->
  <div class="field">
    <label>Tags</label>
    <TagInputAutocomplete
      bind:includeTags={tagsInclude}
      bind:excludeTags={tagsExclude}
    />
  </div>

  <!-- Categories -->
  <div class="field">
    <label>Categories</label>
    <div class="category-grid">
      {#each ALL_CATEGORIES as cat}
        <label class="cat-check">
          <input
            type="checkbox"
            checked={categories.includes(cat)}
            onchange={() => toggleCategory(cat)}
          />
          <span>{cat}</span>
        </label>
      {/each}
    </div>
  </div>

  <!-- Rating -->
  <div class="field">
    <label>Min rating</label>
    <input type="number" placeholder="e.g. 4" min="0" max="5" step="0.5" bind:value={ratingMin} />
  </div>

  <!-- Pages -->
  <div class="field">
    <label>Page count</label>
    <div class="range-row">
      <input type="number" placeholder="Min" min="0" bind:value={pagesMin} />
      <span class="range-sep">–</span>
      <input type="number" placeholder="Max" min="0" bind:value={pagesMax} />
    </div>
  </div>

  <!-- Language -->
  <div class="field">
    <label>Language</label>
    <input type="text" placeholder="e.g. english" bind:value={language} />
  </div>

  <!-- Uploader -->
  <div class="field">
    <label>Uploader</label>
    <input type="text" placeholder="uploader name" bind:value={uploader} />
  </div>

  <!-- Sort divider -->
  <div class="section-divider">
    <span>Sorted By</span>
  </div>

  <!-- Sort field + direction -->
  <div class="field">
    <label>Field</label>
    <div class="sort-field-row">
      <select bind:value={sortField} class="sort-select">
        <option value="posted">Date posted</option>
        <option value="rating">Rating</option>
        <option value="pages">Page count</option>
        <option value="title">Title</option>
      </select>
      <button
        class="dir-btn"
        class:active={sortDir === "desc"}
        onclick={() => sortDir = sortDir === "desc" ? "asc" : "desc"}
        title={sortDir === "desc" ? "Descending" : "Ascending"}
      >
        {#if sortDir === "desc"}
          <svg width="13" height="13" viewBox="0 0 16 16" fill="none">
            <path d="M8 3v10M4 9l4 4 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          Desc
        {:else}
          <svg width="13" height="13" viewBox="0 0 16 16" fill="none">
            <path d="M8 13V3M4 7l4-4 4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          Asc
        {/if}
      </button>
    </div>
  </div>

  <!-- Sort scope -->
  <div class="field">
    <label>Scope</label>
    <div class="scope-tabs">
      <button
        class="scope-tab"
        class:active={sortScopeMode === "count"}
        onclick={() => setSortMode("count")}
      >Gallery count</button>
      <button
        class="scope-tab"
        class:active={sortScopeMode === "days"}
        onclick={() => setSortMode("days")}
      >Date range</button>
    </div>

    {#if sortScopeMode === "count"}
      <div class="scope-body">
        <div class="preset-row">
          {#each COUNT_PRESETS as p}
            <button
              class="preset-btn"
              class:active={!customCount && sortCount === String(p)}
              onclick={() => setPresetCount(p)}
            >{p}</button>
          {/each}
        </div>
        <div class="custom-count-row">
          <span class="scope-label">Sort across</span>
          <input
            type="number"
            class="scope-num-input"
            min="1"
            placeholder={String($galleries.length || 100)}
            value={sortCount}
            oninput={(e) => handleCountInput(e.currentTarget.value)}
            onfocus={() => customCount = true}
          />
          <span class="scope-label">galleries</span>
        </div>
        <p class="scope-hint">
          {#if $galleries.length > 0 && (parseInt(sortCount) || 0) <= $galleries.length}
            Instant — already loaded
          {:else if $galleries.length > 0}
            {@const needed = (parseInt(sortCount) || 0) - $galleries.length}
            {@const pages = Math.ceil(needed / 25)}
            Needs ~{pages} more page{pages === 1 ? "" : "s"} (~{pages * 2}s)
          {/if}
        </p>
      </div>
    {:else}
      <div class="scope-body">
        <div class="custom-count-row">
          <span class="scope-label">Last</span>
          <input
            type="number"
            class="scope-num-input"
            min="1"
            bind:value={sortDays}
          />
          <span class="scope-label">days</span>
        </div>
        <p class="scope-hint">Fetches until oldest gallery exceeds date cutoff</p>
      </div>
    {/if}
  </div>

  <!-- Actions -->
  <div class="actions">
    <button class="btn-sort" onclick={handleSort}>Sort</button>
    <button class="btn-primary" onclick={commit}>Apply</button>
    <button class="btn-ghost" onclick={handleClear}>Clear</button>
  </div>
</div>

<style>
  .filter-panel {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1.25rem;
    font-size: 0.8rem;
    flex: 1;
    overflow-y: auto;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .field > label {
    font-weight: 600;
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .field input[type="text"],
  .field input[type="number"] {
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    background: var(--bg-secondary);
    color: var(--text-primary);
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .field input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  .range-row {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .range-row input[type="number"] {
    flex: 1;
    min-width: 0;
  }

  .range-sep {
    color: var(--text-muted);
    font-size: 0.75rem;
  }

  .category-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.15rem 0.5rem;
  }

  .cat-check {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.75rem;
    cursor: pointer;
    color: var(--text-secondary);
    padding: 0.15rem 0;
  }

  .cat-check input {
    margin: 0;
    accent-color: var(--accent);
  }

  /* ── Sort section ────────────────────────────────────────────── */

  .section-divider {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0.25rem 0 -0.25rem;
  }

  .section-divider::before,
  .section-divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  .section-divider span {
    font-size: 0.68rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .sort-field-row {
    display: flex;
    gap: 0.4rem;
  }

  .sort-select {
    flex: 1;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.8rem;
    outline: none;
    cursor: pointer;
    transition: border-color 0.15s;
  }

  .sort-select:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  .dir-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.45rem 0.65rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.1s, color 0.1s;
  }

  .dir-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .dir-btn.active {
    background: var(--accent-subtle);
    color: var(--accent);
    border-color: var(--accent);
  }

  .scope-tabs {
    display: flex;
    gap: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .scope-tab {
    flex: 1;
    padding: 0.4rem 0;
    border: none;
    background: var(--bg-secondary);
    color: var(--text-muted);
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .scope-tab:not(:last-child) {
    border-right: 1px solid var(--border-strong);
  }

  .scope-tab.active {
    background: var(--accent);
    color: #fff;
  }

  .scope-body {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin-top: 0.3rem;
  }

  .preset-row {
    display: flex;
    gap: 0.3rem;
    flex-wrap: wrap;
  }

  .preset-btn {
    padding: 0.3rem 0.7rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.73rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }

  .preset-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .preset-btn.active {
    background: var(--accent-subtle);
    color: var(--accent);
    border-color: var(--accent);
  }

  .custom-count-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .scope-label {
    font-size: 0.75rem;
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .scope-num-input {
    width: 70px;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.78rem;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
    text-align: center;
  }

  .scope-num-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  .scope-hint {
    margin: 0;
    font-size: 0.7rem;
    color: var(--text-muted);
    font-style: italic;
  }

  /* ── Actions ─────────────────────────────────────────────────── */

  .actions {
    display: flex;
    gap: 0.4rem;
    padding-top: 0.5rem;
    border-top: 1px solid var(--border);
  }

  .btn-sort {
    flex: 1;
    padding: 0.45rem 0;
    border-radius: var(--radius-sm);
    border: 1px solid var(--accent);
    background: transparent;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .btn-sort:hover {
    background: var(--accent-subtle);
  }

  .btn-primary {
    flex: 1;
    padding: 0.45rem 0;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }

  .btn-primary:hover {
    background: var(--accent-hover);
  }

  .btn-ghost {
    padding: 0.45rem 0.85rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .btn-ghost:hover {
    background: var(--bg-hover);
  }
</style>
