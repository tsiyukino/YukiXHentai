<script lang="ts">
  import { t } from "$lib/i18n";
  import { currentPage, sidebarCollapsed } from "$lib/stores/navigation";
  import type { NavPage } from "$lib/stores/navigation";
  import { theme } from "$lib/stores/ui";
  import type { Theme } from "$lib/stores/ui";
  import { setTheme } from "$lib/api/reader";
  import { searchQuery, searchResults } from "$lib/stores/search";

  const navGroups: { label: string; items: { page: NavPage; icon: string; key: string }[] }[] = [
    {
      label: "NAVIGATION",
      items: [
        { page: "home", icon: "home", key: "nav.home" },
        { page: "search", icon: "search", key: "nav.search" },
        { page: "popular", icon: "popular", key: "nav.popular" },
        { page: "favorites", icon: "favorites", key: "nav.favorites" },
        { page: "watched", icon: "watched", key: "nav.watched" },
      ],
    },
    {
      label: "LIBRARY",
      items: [
        { page: "history", icon: "history", key: "nav.history" },
        { page: "downloads", icon: "downloads", key: "nav.downloads" },
      ],
    },
  ];

  let sidebarSearchQuery = $state("");

  function navigate(page: NavPage) {
    $currentPage = page;
  }

  function toggleCollapse() {
    $sidebarCollapsed = !$sidebarCollapsed;
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && sidebarSearchQuery.trim()) {
      // Set the search store query so SearchPage picks it up
      $searchQuery = sidebarSearchQuery.trim();
      $searchResults = []; // Clear old results so SearchPage triggers a fresh search
      $currentPage = "search";
      sidebarSearchQuery = "";
    }
  }

  function toggleTheme() {
    const newTheme: Theme = $theme === "light" ? "dark" : "light";
    $theme = newTheme;
    setTheme(newTheme).catch(() => {});
  }
</script>

<nav class="sidebar" class:collapsed={$sidebarCollapsed}>
  <!-- Collapsed icon strip -->
  {#if $sidebarCollapsed}
    <div class="collapsed-strip">
      <button class="collapse-toggle" onclick={toggleCollapse} title="Expand sidebar">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/></svg>
      </button>

      {#each navGroups as group}
        {#each group.items as item}
          <button
            class="icon-btn"
            class:active={$currentPage === item.page}
            onclick={() => navigate(item.page)}
            title={$t(item.key)}
          >
            <span class="nav-icon">
              {#if item.icon === "home"}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>
              {:else if item.icon === "search"}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
              {:else if item.icon === "popular"}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2L15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2z"/></svg>
              {:else if item.icon === "favorites"}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"/></svg>
              {:else if item.icon === "watched"}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
              {:else if item.icon === "history"}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
              {:else if item.icon === "downloads"}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
              {/if}
            </span>
          </button>
        {/each}
      {/each}

      <!-- Bottom icons -->
      <div class="collapsed-bottom">
        <button
          class="icon-btn"
          onclick={toggleTheme}
          title={$theme === "light" ? $t("settings.dark") : $t("settings.light")}
        >
          {#if $theme === "light"}
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg>
          {:else}
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>
          {/if}
        </button>
        <button
          class="icon-btn"
          class:active={$currentPage === "settings"}
          onclick={() => navigate("settings")}
          title={$t("nav.settings")}
        >
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
        </button>
      </div>
    </div>
  {:else}
    <!-- Expanded sidebar -->
    <div class="expanded-content">
      <div class="sidebar-top">
        <div class="brand-row">
          <span class="brand-name">{$t("app.name")}</span>
          <button class="collapse-toggle" onclick={toggleCollapse} title="Collapse sidebar">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/></svg>
          </button>
        </div>

        <!-- Search bar -->
        <div class="search-bar">
          <svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
          <input
            type="text"
            placeholder="Search"
            bind:value={sidebarSearchQuery}
            onkeydown={handleSearchKeydown}
          />
          <span class="search-shortcut">/</span>
        </div>
      </div>

      <div class="nav-sections">
        {#each navGroups as group}
          <div class="nav-group">
            <span class="group-label">{group.label}</span>
            {#each group.items as item}
              <button
                class="nav-item"
                class:active={$currentPage === item.page}
                onclick={() => navigate(item.page)}
              >
                <span class="nav-icon">
                  {#if item.icon === "home"}
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>
                  {:else if item.icon === "search"}
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                  {:else if item.icon === "popular"}
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2L15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2z"/></svg>
                  {:else if item.icon === "favorites"}
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"/></svg>
                  {:else if item.icon === "watched"}
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
                  {:else if item.icon === "history"}
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                  {:else if item.icon === "downloads"}
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                  {/if}
                </span>
                <span class="nav-label">{$t(item.key)}</span>
              </button>
            {/each}
          </div>
        {/each}
      </div>

      <!-- Bottom section: theme toggle + settings -->
      <div class="sidebar-bottom">
        <button class="nav-item" onclick={toggleTheme}>
          <span class="nav-icon">
            {#if $theme === "light"}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg>
            {:else}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>
            {/if}
          </span>
          <span class="nav-label">{$theme === "light" ? $t("settings.dark") : $t("settings.light")}</span>
        </button>
        <button
          class="nav-item"
          class:active={$currentPage === "settings"}
          onclick={() => navigate("settings")}
        >
          <span class="nav-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
          </span>
          <span class="nav-label">{$t("nav.settings")}</span>
        </button>
      </div>
    </div>
  {/if}
</nav>

<style>
  .sidebar {
    width: 220px;
    min-width: 220px;
    height: 100%;
    background: var(--bg-primary);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    transition: width 0.2s ease, min-width 0.2s ease;
    flex-shrink: 0;
    overflow: hidden;
  }

  .sidebar.collapsed {
    width: 56px;
    min-width: 56px;
  }

  /* ── Collapsed strip ────────────────────────────────── */

  .collapsed-strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 12px 0;
    gap: 4px;
    height: 100%;
  }

  .collapse-toggle {
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, color 0.15s;
    margin-bottom: 8px;
    flex-shrink: 0;
  }

  .collapse-toggle:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .icon-btn {
    width: 40px;
    height: 40px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, color 0.15s;
  }

  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .icon-btn.active {
    background: var(--accent-subtle);
    color: var(--accent);
  }

  .collapsed-bottom {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding-bottom: 4px;
  }

  /* ── Expanded sidebar ──────────────────────────────── */

  .expanded-content {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .sidebar-top {
    padding: 16px 16px 8px;
    flex-shrink: 0;
  }

  .brand-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }

  .brand-name {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.01em;
  }

  .search-bar {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    color: var(--text-muted);
    pointer-events: none;
  }

  .search-bar input {
    width: 100%;
    padding: 8px 36px 8px 34px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.82rem;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .search-bar input::placeholder {
    color: var(--text-muted);
  }

  .search-bar input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  .search-shortcut {
    position: absolute;
    right: 10px;
    font-size: 0.68rem;
    font-weight: 600;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    padding: 1px 6px;
    line-height: 1.4;
    pointer-events: none;
  }

  /* ── Nav sections ──────────────────────────────────── */

  .nav-sections {
    flex: 1;
    overflow-y: auto;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .nav-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .group-label {
    font-size: 0.65rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    padding: 0 8px;
    margin-bottom: 6px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.84rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    width: 100%;
  }

  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-item.active {
    background: var(--accent-subtle);
    color: var(--accent);
    font-weight: 600;
  }

  .nav-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 20px;
    height: 20px;
  }

  .nav-label {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ── Bottom section ────────────────────────────────── */

  .sidebar-bottom {
    padding: 8px 12px 16px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
  }
</style>
