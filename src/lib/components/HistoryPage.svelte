<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "$lib/i18n";
  import { getReadingHistory } from "$lib/api/reader";
  import type { ReadingSession } from "$lib/api/reader";

  let sessions = $state<ReadingSession[]>([]);
  let loading = $state(true);
  let hasMore = $state(true);

  const PAGE_SIZE = 30;

  onMount(async () => {
    await loadHistory();
  });

  async function loadHistory() {
    loading = true;
    try {
      const result = await getReadingHistory(PAGE_SIZE, 0);
      sessions = result;
      hasMore = result.length >= PAGE_SIZE;
    } catch (err) {
      console.error("Failed to load history:", err);
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    if (!hasMore || loading) return;
    loading = true;
    try {
      const result = await getReadingHistory(PAGE_SIZE, sessions.length);
      sessions = [...sessions, ...result];
      hasMore = result.length >= PAGE_SIZE;
    } catch (err) {
      console.error("Failed to load more history:", err);
    } finally {
      loading = false;
    }
  }

  function formatDate(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleString();
  }

  function formatDuration(session: ReadingSession): string {
    if (!session.closed_at) return "—";
    const secs = session.closed_at - session.opened_at;
    const mins = Math.floor(secs / 60);
    if (mins < 1) return "<1m";
    if (mins < 60) return `${mins}m`;
    return `${Math.floor(mins / 60)}h ${mins % 60}m`;
  }
</script>

<div class="history-page">
  <div class="history-header">
    <h2>{$t("history_page.title")}</h2>
  </div>

  {#if sessions.length === 0 && !loading}
    <div class="empty">
      <div class="empty-icon">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
      </div>
      <p>{$t("history_page.empty")}</p>
    </div>
  {:else}
    <div class="history-list">
      {#each sessions as session}
        <div class="history-item">
          <div class="item-left">
            <span class="gid">GID: {session.gid}</span>
            <span class="date">{formatDate(session.opened_at)}</span>
          </div>
          <div class="item-right">
            <span class="pages-read">{$t("history_page.pages_read", { count: session.pages_read })}</span>
            <span class="duration">{formatDuration(session)}</span>
          </div>
        </div>
      {/each}

      {#if hasMore}
        <button class="load-more" onclick={loadMore} disabled={loading}>
          {loading ? $t("common.loading") : "Load more"}
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .history-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .history-header {
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--border);
    background: var(--bg-primary);
    flex-shrink: 0;
  }

  .history-header h2 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 700;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 0.75rem;
    color: var(--text-muted);
  }

  .empty-icon {
    opacity: 0.2;
  }

  .empty p {
    margin: 0;
    font-size: 0.85rem;
  }

  .history-list {
    flex: 1;
    overflow-y: auto;
    padding: 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .history-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-radius: var(--radius-md);
    background: var(--bg-primary);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-sm);
    transition: box-shadow 0.15s;
  }

  .history-item:hover {
    box-shadow: var(--shadow-md);
  }

  .item-left {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .gid {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .date {
    font-size: 0.68rem;
    color: var(--text-muted);
  }

  .item-right {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.15rem;
  }

  .pages-read {
    font-size: 0.72rem;
    color: var(--text-secondary);
  }

  .duration {
    font-size: 0.65rem;
    color: var(--text-muted);
  }

  .load-more {
    align-self: center;
    padding: 0.5rem 1.25rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-primary);
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    margin-top: 0.75rem;
    transition: all 0.15s;
  }

  .load-more:hover:not(:disabled) {
    background: var(--bg-hover);
    box-shadow: var(--shadow-sm);
  }

  .load-more:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
