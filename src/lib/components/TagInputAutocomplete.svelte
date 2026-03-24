<script lang="ts">
  import { searchTagsAutocomplete } from "$lib/api/galleries";
  import type { TagSuggestion } from "$lib/api/galleries";

  let {
    includeTags = $bindable<TagSuggestion[]>([]),
    excludeTags = $bindable<TagSuggestion[]>([]),
    onFocus,
  }: {
    includeTags: TagSuggestion[];
    excludeTags: TagSuggestion[];
    /** Called when the input is focused (e.g. to close sibling dropdowns). */
    onFocus?: () => void;
  } = $props();

  let tagInput = $state("");
  let tagSuggestions = $state<TagSuggestion[]>([]);
  let showTagDropdown = $state(false);
  let autocompleteTimer: ReturnType<typeof setTimeout> | null = null;

  function handleInput(value: string) {
    tagInput = value;
    if (autocompleteTimer) clearTimeout(autocompleteTimer);
    if (!value.trim()) { tagSuggestions = []; showTagDropdown = false; return; }
    autocompleteTimer = setTimeout(async () => {
      try {
        tagSuggestions = await searchTagsAutocomplete(value.trim());
        showTagDropdown = tagSuggestions.length > 0;
      } catch { tagSuggestions = []; showTagDropdown = false; }
    }, 200);
  }

  function handleFocus() {
    onFocus?.();
  }

  function handleBlur() {
    setTimeout(() => { showTagDropdown = false; }, 200);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") { showTagDropdown = false; tagInput = ""; }
  }

  function addInclude(tag: TagSuggestion) {
    if (!includeTags.some(t => t.namespace === tag.namespace && t.name === tag.name)) {
      includeTags = [...includeTags, tag];
    }
    tagInput = "";
    tagSuggestions = [];
    showTagDropdown = false;
  }

  function addExclude(tag: TagSuggestion) {
    if (!excludeTags.some(t => t.namespace === tag.namespace && t.name === tag.name)) {
      excludeTags = [...excludeTags, tag];
    }
    tagInput = "";
    tagSuggestions = [];
    showTagDropdown = false;
  }

  function removeInclude(tag: TagSuggestion) {
    includeTags = includeTags.filter(t => !(t.namespace === tag.namespace && t.name === tag.name));
  }

  function removeExclude(tag: TagSuggestion) {
    excludeTags = excludeTags.filter(t => !(t.namespace === tag.namespace && t.name === tag.name));
  }

  function toggleMode(tag: TagSuggestion, mode: "include" | "exclude") {
    if (mode === "include") { removeInclude(tag); addExclude(tag); }
    else { removeExclude(tag); addInclude(tag); }
  }

  export function clear() {
    tagInput = "";
    tagSuggestions = [];
    showTagDropdown = false;
    includeTags = [];
    excludeTags = [];
  }
</script>

<!-- Tag input with autocomplete -->
<div class="tag-field-wrap">
  <input
    class="tag-input"
    type="text"
    placeholder="Add tag filter…"
    value={tagInput}
    oninput={(e) => handleInput(e.currentTarget.value)}
    onkeydown={handleKeydown}
    onfocus={handleFocus}
    onblur={handleBlur}
  />
  {#if tagInput}
    <button class="input-clear" onclick={() => { tagInput = ""; tagSuggestions = []; showTagDropdown = false; }}>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
        <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
    </button>
  {/if}

  {#if showTagDropdown}
    <div class="tag-dropdown">
      {#each tagSuggestions as suggestion}
        <div class="tag-suggestion-row">
          <button class="tag-suggestion-name" onmousedown={() => addInclude(suggestion)}>
            <span class="tag-ns">{suggestion.namespace}</span>:{suggestion.name}
          </button>
          <button class="tag-suggestion-exclude" onmousedown={() => addExclude(suggestion)} title="Exclude">−</button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Tag chips -->
{#if includeTags.length > 0 || excludeTags.length > 0}
  <div class="tag-chips">
    {#each includeTags as tag}
      <span class="tag-chip include">
        <button class="tag-chip-mode" onclick={() => toggleMode(tag, "include")} title="Toggle to exclude">+</button>
        <span class="tag-chip-label"><span class="tag-ns">{tag.namespace}</span>:{tag.name}</span>
        <button class="tag-chip-remove" onclick={() => removeInclude(tag)}>×</button>
      </span>
    {/each}
    {#each excludeTags as tag}
      <span class="tag-chip exclude">
        <button class="tag-chip-mode" onclick={() => toggleMode(tag, "exclude")} title="Toggle to include">−</button>
        <span class="tag-chip-label"><span class="tag-ns">{tag.namespace}</span>:{tag.name}</span>
        <button class="tag-chip-remove" onclick={() => removeExclude(tag)}>×</button>
      </span>
    {/each}
  </div>
{/if}

<style>
  .tag-field-wrap {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
  }

  .tag-input {
    width: 100%;
    padding: 0.6rem 2.2rem 0.6rem 0.75rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.88rem;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .tag-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  .tag-input::placeholder {
    color: var(--text-muted);
  }

  .input-clear {
    position: absolute;
    right: 0.5rem;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.2rem;
    display: flex;
    align-items: center;
    border-radius: 2px;
    transition: color 0.15s;
  }

  .input-clear:hover {
    color: var(--text-primary);
  }

  .tag-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin-top: 3px;
    background: var(--bg-primary);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    z-index: 50;
    overflow: hidden;
  }

  .tag-suggestion-row {
    display: flex;
    align-items: stretch;
  }

  .tag-suggestion-name {
    flex: 1;
    padding: 0.4rem 0.75rem;
    background: none;
    border: none;
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s, color 0.1s;
  }

  .tag-suggestion-name:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .tag-suggestion-exclude {
    padding: 0 0.6rem;
    background: none;
    border: none;
    border-left: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 1rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .tag-suggestion-exclude:hover {
    background: rgba(var(--red-rgb, 200, 50, 50), 0.1);
    color: var(--red);
  }

  .tag-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 0.4rem;
  }

  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    border-radius: 20px;
    border: 1px solid;
    font-size: 0.72rem;
    font-weight: 500;
    padding: 2px 6px 2px 3px;
  }

  .tag-chip.include {
    background: rgba(34, 197, 94, 0.12);
    border-color: rgba(34, 197, 94, 0.4);
    color: rgb(22, 163, 74);
  }

  .tag-chip.exclude {
    background: rgba(239, 68, 68, 0.12);
    border-color: rgba(239, 68, 68, 0.4);
    color: rgb(220, 38, 38);
  }

  .tag-chip-mode {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.82rem;
    font-weight: 700;
    color: inherit;
    padding: 0 2px;
    line-height: 1;
    opacity: 0.8;
    transition: opacity 0.1s;
  }

  .tag-chip-mode:hover { opacity: 1; }

  .tag-chip-label {
    user-select: none;
  }

  .tag-chip-remove {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.85rem;
    color: inherit;
    padding: 0 1px;
    line-height: 1;
    opacity: 0.6;
    transition: opacity 0.1s;
  }

  .tag-chip-remove:hover { opacity: 1; }

  .tag-ns {
    opacity: 0.65;
  }
</style>
