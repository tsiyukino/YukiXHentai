<script lang="ts">
  import { t } from "$lib/i18n";
  import { currentPage } from "$lib/stores/navigation";
  import { theme } from "$lib/stores/ui";
  import type { Theme } from "$lib/stores/ui";
  import { setTheme } from "$lib/api/reader";

  function goSettings() {
    $currentPage = "settings";
  }

  function toggleTheme() {
    const newTheme: Theme = $theme === "light" ? "dark" : "light";
    $theme = newTheme;
    setTheme(newTheme).catch(() => {});
  }
</script>

<header class="mobile-top-bar">
  <span class="app-name">{$t("app.name")}</span>
  <div class="actions">
    <button class="action-btn" onclick={toggleTheme} aria-label={$theme === "light" ? $t("settings.dark") : $t("settings.light")}>
      {#if $theme === "light"}
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg>
      {:else}
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>
      {/if}
    </button>
    <button
      class="action-btn"
      class:active={$currentPage === "settings"}
      onclick={goSettings}
      aria-label={$t("nav.settings")}
    >
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/></svg>
    </button>
  </div>
</header>

<style>
  .mobile-top-bar {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    padding: 0 16px;
    padding-top: env(safe-area-inset-top, 0px);
    height: calc(52px + env(safe-area-inset-top, 0px));
    background: var(--bg-primary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    z-index: 100;
  }

  .app-name {
    font-size: 1rem;
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.01em;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .action-btn {
    width: 44px;
    height: 44px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: background 0.15s, color 0.15s;
    -webkit-tap-highlight-color: transparent;
  }

  .action-btn:active {
    background: var(--bg-hover);
  }

  .action-btn.active {
    color: var(--accent);
  }
</style>
