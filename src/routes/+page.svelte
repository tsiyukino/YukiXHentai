<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "$lib/i18n";
  import { getAuthStatus } from "$lib/api/auth";
  import { getTheme } from "$lib/api/reader";
  import { isLoggedIn } from "$lib/stores/auth";
  import { currentPage } from "$lib/stores/navigation";
  import { readerGallery } from "$lib/stores/reader";
  import { theme, detailExpanded } from "$lib/stores/ui";
  import type { Theme } from "$lib/stores/ui";
  import { detailGallery } from "$lib/stores/detail";
  import LoginForm from "$lib/components/LoginForm.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import WindowControls from "$lib/components/WindowControls.svelte";
  import GalleryGrid from "$lib/components/GalleryGrid.svelte";
  import GalleryReader from "$lib/components/GalleryReader.svelte";
  import GalleryDetail from "$lib/components/GalleryDetail.svelte";
  import SettingsPage from "$lib/components/SettingsPage.svelte";
  import HistoryPage from "$lib/components/HistoryPage.svelte";
  import PlaceholderPage from "$lib/components/PlaceholderPage.svelte";
  import SearchPage from "$lib/components/SearchPage.svelte";

  let ready = $state(false);

  onMount(async () => {
    try {
      const savedTheme = await getTheme();
      if (savedTheme === "dark" || savedTheme === "light") {
        $theme = savedTheme as Theme;
      }
    } catch {}
    document.documentElement.setAttribute("data-theme", $theme);
    try {
      $isLoggedIn = await getAuthStatus();
    } catch {
      $isLoggedIn = false;
    }
    ready = true;
  });

  // Keep data-theme attribute in sync with store
  $effect(() => {
    document.documentElement.setAttribute("data-theme", $theme);
  });
</script>

<div class="app">
  <div class="titlebar">
    <WindowControls />
  </div>

  {#if !ready}
    <div class="loading-screen">
      <div class="spinner"></div>
      <p>{$t("common.loading")}</p>
    </div>
  {:else if !$isLoggedIn}
    <LoginForm />
  {:else}
    <div class="app-body">
      <Sidebar />
      <main>
        <!-- Page content: hidden (display:none) when detail is in full-page mode -->
        <div class="page-content" class:hidden={$detailExpanded && !!$detailGallery}>
          {#if $currentPage === "home"}
            <GalleryGrid />
          {:else if $currentPage === "search"}
            <SearchPage />
          {:else if $currentPage === "popular"}
            <PlaceholderPage titleKey="popular_page.title" messageKey="popular_page.coming_soon" icon="popular" />
          {:else if $currentPage === "favorites"}
            <PlaceholderPage titleKey="favorites_page.title" messageKey="favorites_page.empty" icon="favorites" />
          {:else if $currentPage === "watched"}
            <PlaceholderPage titleKey="watched_page.title" messageKey="watched_page.coming_soon" icon="watched" />
          {:else if $currentPage === "history"}
            <HistoryPage />
          {:else if $currentPage === "downloads"}
            <PlaceholderPage titleKey="downloads_page.title" messageKey="downloads_page.empty" icon="downloads" />
          {:else if $currentPage === "settings"}
            <SettingsPage />
          {/if}
        </div>
        <!-- Detail panel: always rendered once inside main; fullPage switches between fixed-overlay and inline modes -->
        <GalleryDetail fullPage={$detailExpanded && !!$detailGallery} />
      </main>
    </div>
  {/if}
</div>

<!-- Reader overlay (rendered on top when active) -->
<GalleryReader />

<style>
  :root,
  :root[data-theme="light"] {
    /* ── Light design palette (reference-matched) ────────── */
    --bg-primary: #ffffff;
    --bg-secondary: #f8f8fa;
    --bg-tertiary: #f0f0f4;
    --bg-elevated: #eeeef2;
    --bg-hover: #f0f0f4;
    --text-primary: #1a1a2e;
    --text-secondary: #6b7280;
    --text-muted: #9ca3af;
    --accent: #7c3aed;
    --accent-hover: #6d28d9;
    --accent-subtle: rgba(124, 58, 237, 0.08);
    --border: #f0f0f4;
    --border-strong: #e5e7eb;
    --green: #22c55e;
    --red: #ef4444;
    --yellow: #f59e0b;
    --card-bg: #ffffff;
    --card-border: #f0f0f4;
    --card-border-hover: #d1d5db;
    --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.04);
    --shadow-md: 0 2px 8px rgba(0, 0, 0, 0.06);
    --radius-sm: 8px;
    --radius-md: 10px;
    --radius-lg: 12px;
    --scrollbar-thumb: #d1d5db;
    --scrollbar-thumb-hover: #9ca3af;
    --overlay-bg: rgba(0, 0, 0, 0.2);
    --danger-border: #fecaca;
    --danger-bg: #fef2f2;
    --success-bg: #f0fdf4;
  }

  :root[data-theme="dark"] {
    /* ── Dark design palette ──────────────────────────────── */
    --bg-primary: #111111;
    --bg-secondary: #0f0f0f;
    --bg-tertiary: #1a1a1a;
    --bg-elevated: #222222;
    --bg-hover: #252525;
    --text-primary: #e8e8e8;
    --text-secondary: #999999;
    --text-muted: #666666;
    --accent: #8b5cf6;
    --accent-hover: #7c3aed;
    --accent-subtle: rgba(139, 92, 246, 0.12);
    --border: #1e1e1e;
    --border-strong: #2a2a2a;
    --green: #22c55e;
    --red: #ef4444;
    --yellow: #f59e0b;
    --card-bg: #1a1a1a;
    --card-border: #1e1e1e;
    --card-border-hover: #333333;
    --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.2);
    --shadow-md: 0 2px 8px rgba(0, 0, 0, 0.3);
    --scrollbar-thumb: #333333;
    --scrollbar-thumb-hover: #444444;
    --overlay-bg: rgba(0, 0, 0, 0.6);
    --danger-border: rgba(239, 68, 68, 0.3);
    --danger-bg: rgba(239, 68, 68, 0.1);
    --success-bg: rgba(34, 197, 94, 0.1);
  }

  :global(body) {
    margin: 0;
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    font-size: 14px;
    line-height: 1.5;
    color: var(--text-primary);
    background-color: var(--bg-secondary);
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  :global(*) {
    box-sizing: border-box;
  }

  :global(::-webkit-scrollbar) {
    width: 6px;
  }

  :global(::-webkit-scrollbar-track) {
    background: transparent;
  }

  :global(::-webkit-scrollbar-thumb) {
    background: var(--scrollbar-thumb);
    border-radius: 3px;
  }

  :global(::-webkit-scrollbar-thumb:hover) {
    background: var(--scrollbar-thumb-hover);
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    min-width: 640px;
    min-height: 400px;
    overflow: hidden;
  }

  .titlebar {
    height: 32px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    background: var(--bg-primary);
    border-bottom: 1px solid var(--border);
    -webkit-app-region: drag;
    z-index: 100;
  }

  .app-body {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  main {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-width: 0;
    background: var(--bg-secondary);
  }

  .page-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .page-content.hidden {
    display: none;
  }

  .loading-screen {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 1.5rem;
    color: var(--text-muted);
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2.5px solid var(--border-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .loading-screen p {
    margin: 0;
    font-size: 0.82rem;
  }
</style>
