<script lang="ts">
  import { t } from "$lib/i18n";

  let {
    titleKey,
    messageKey,
    icon = "home",
  }: {
    titleKey: string;
    messageKey: string;
    icon?: string;
  } = $props();
</script>

<div class="placeholder-page">
  <div class="placeholder-content">
    <div class="placeholder-icon">
      {#if icon === "search"}
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      {:else if icon === "popular"}
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2L15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2z"/></svg>
      {:else if icon === "favorites"}
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20.84 4.61a5.5 5.5 0 00-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 00-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 000-7.78z"/></svg>
      {:else if icon === "watched"}
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
      {:else if icon === "downloads"}
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
      {:else}
        <svg width="48" height="48" viewBox="0 0 48 48" fill="none"><rect x="6" y="10" width="36" height="28" rx="4" stroke="currentColor" stroke-width="2"/><path d="M6 20h36M18 10v28" stroke="currentColor" stroke-width="2" opacity="0.3"/></svg>
      {/if}
    </div>
    <h2>{$t(titleKey)}</h2>
    <p>{$t(messageKey)}</p>
  </div>
</div>

<style>
  .placeholder-page {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .placeholder-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    color: var(--text-muted);
    text-align: center;
    padding: 2rem;
  }

  .placeholder-icon {
    opacity: 0.15;
    margin-bottom: 0.75rem;
  }

  h2 {
    margin: 0;
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  p {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-muted);
  }
</style>
