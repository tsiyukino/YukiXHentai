<script lang="ts">
  import { t } from "$lib/i18n";
  import { currentPage } from "$lib/stores/navigation";
  import type { NavPage } from "$lib/stores/navigation";
  import { sidebarDrawerOpen } from "$lib/stores/ui";

  const tabs: { page: NavPage; icon: string; key: string }[] = [
    { page: "home",      icon: "home",      key: "nav.home" },
    { page: "search",    icon: "search",    key: "nav.search" },
    { page: "favorites", icon: "favorites", key: "nav.favorites" },
    { page: "history",   icon: "history",   key: "nav.history" },
    { page: "downloads", icon: "downloads", key: "nav.downloads" },
  ];

  function navigate(page: NavPage) {
    $currentPage = page;
  }

  function openDrawer() {
    $sidebarDrawerOpen = true;
  }
</script>

<nav class="bottom-tab-bar" role="navigation" aria-label="Main navigation">
  <!-- Hamburger to open full sidebar drawer -->
  <button class="tab-btn menu-btn" onclick={openDrawer} aria-label="Open menu">
    <span class="tab-icon">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="3" y1="6" x2="21" y2="6"/>
        <line x1="3" y1="12" x2="21" y2="12"/>
        <line x1="3" y1="18" x2="21" y2="18"/>
      </svg>
    </span>
  </button>

  {#each tabs as tab}
    <button
      class="tab-btn"
      class:active={$currentPage === tab.page}
      onclick={() => navigate(tab.page)}
      aria-label={$t(tab.key)}
      aria-current={$currentPage === tab.page ? "page" : undefined}
    >
      <span class="tab-icon">
        {#if tab.icon === "home"}
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>
        {:else if tab.icon === "search"}
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
        {:else if tab.icon === "favorites"}
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"/></svg>
        {:else if tab.icon === "history"}
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
        {:else if tab.icon === "downloads"}
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
        {/if}
      </span>
    </button>
  {/each}
</nav>

<style>
  .bottom-tab-bar {
    display: flex;
    align-items: stretch;
    background: var(--bg-primary);
    border-top: 1px solid var(--border-strong);
    flex-shrink: 0;
    height: 56px;
    z-index: 200;
  }

  .tab-btn {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    /* Minimum touch target */
    min-height: 44px;
    padding: 0;
    transition: color 0.15s;
    -webkit-tap-highlight-color: transparent;
  }

  .tab-btn:active {
    background: var(--bg-hover);
  }

  .tab-btn.active {
    color: var(--accent);
  }

  .tab-icon {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .menu-btn {
    color: var(--text-secondary);
    max-width: 48px;
  }
</style>
