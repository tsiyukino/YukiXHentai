<script lang="ts">
  import { t } from "$lib/i18n";
  import { invoke } from "@tauri-apps/api/core";
  import {
    parseDownloadQueueJson,
    resolveGalleryToken,
    submitDownloadQueue,
    type QueueEntry,
    type ResolvedGallery,
    type SubmitEntry,
  } from "$lib/api/library";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  type EntryStatus = "pending" | "resolving" | "ready" | "already_local" | "failed";

  interface UIEntry {
    gid: number;
    token: string | null;
    title: string | null;
    alreadyLocal: boolean;
    status: EntryStatus;
    error?: string;
  }

  let activeTab = $state<"manual" | "json">("manual");
  let manualText = $state("");
  let entries = $state<UIEntry[]>([]);
  let resolving = $state(false);
  let submitting = $state(false);
  let submitMessage = $state<string | null>(null);
  let submitError = $state<string | null>(null);

  // Options
  let downloadOriginals = $state(false);
  let subfolder = $state("");

  // Parse manual textarea
  function parseManualInput(): UIEntry[] {
    const lines = manualText.split("\n").map((l) => l.trim()).filter(Boolean);
    const result: UIEntry[] = [];
    for (const line of lines) {
      // Full URL: /g/(\d+)/([a-f0-9]+)/
      const urlMatch = line.match(/\/g\/(\d+)\/([a-f0-9]+)\//);
      if (urlMatch) {
        result.push({ gid: Number(urlMatch[1]), token: urlMatch[2], title: null, alreadyLocal: false, status: "ready" });
        continue;
      }
      // gid:token
      const colonMatch = line.match(/^(\d+):([a-f0-9]+)$/);
      if (colonMatch) {
        result.push({ gid: Number(colonMatch[1]), token: colonMatch[2], title: null, alreadyLocal: false, status: "ready" });
        continue;
      }
      // integer only
      const intMatch = line.match(/^(\d+)$/);
      if (intMatch) {
        result.push({ gid: Number(intMatch[1]), token: null, title: null, alreadyLocal: false, status: "pending" });
        continue;
      }
    }
    return result;
  }

  function handleParseManual() {
    entries = parseManualInput();
    submitMessage = null;
    submitError = null;
  }

  async function handlePickJson() {
    try {
      const filePath = await invoke<string | null>("pick_file_dialog", {
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!filePath) return;
      const parsed = await parseDownloadQueueJson(filePath);
      entries = parsed.map((e): UIEntry => ({
        gid: e.gid,
        token: e.token ?? null,
        title: e.title ?? null,
        alreadyLocal: e.alreadyLocal,
        status: e.alreadyLocal ? "already_local" : (e.token ? "ready" : "pending"),
      }));
      submitMessage = null;
      submitError = null;
    } catch (err) {
      submitError = String(err);
    }
  }

  async function handleResolveAll() {
    resolving = true;
    const pending = entries.filter((e) => e.status === "pending" || (!e.token && !e.alreadyLocal));
    for (const entry of pending) {
      const idx = entries.findIndex((e) => e.gid === entry.gid);
      if (idx < 0) continue;
      entries[idx] = { ...entries[idx], status: "resolving" };
      entries = [...entries];
      try {
        const resolved: ResolvedGallery = await resolveGalleryToken(entry.gid);
        if (resolved.error) {
          entries[idx] = { ...entries[idx], status: "failed", error: resolved.error };
        } else {
          entries[idx] = {
            ...entries[idx],
            token: resolved.token ?? null,
            title: resolved.title ?? null,
            status: resolved.token ? "ready" : "failed",
            error: resolved.token ? undefined : "Token not found",
          };
        }
      } catch (err) {
        entries[idx] = { ...entries[idx], status: "failed", error: String(err) };
      }
      entries = [...entries];
    }
    resolving = false;
  }

  async function handleQueueAll() {
    const ready = entries.filter((e) => e.status === "ready" && e.token);
    if (ready.length === 0) return;
    submitting = true;
    submitError = null;
    submitMessage = null;
    try {
      const toSubmit: SubmitEntry[] = ready.map((e) => ({ gid: e.gid, token: e.token! }));
      const result = await submitDownloadQueue(toSubmit, downloadOriginals, subfolder.trim() || undefined);
      submitMessage = $t("queue.queued_count", { count: result.queued });
    } catch (err) {
      submitError = String(err);
    } finally {
      submitting = false;
    }
  }

  function handleClear() {
    entries = [];
    manualText = "";
    submitMessage = null;
    submitError = null;
  }

  function handleClose() {
    const readyCount = entries.filter((e) => e.status === "ready").length;
    if (readyCount > 0 && !submitMessage) {
      if (!confirm($t("queue.close_confirm"))) return;
    }
    onClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") handleClose();
  }

  let summary = $derived(() => {
    const ready = entries.filter((e) => e.status === "ready").length;
    const alreadyLocal = entries.filter((e) => e.status === "already_local").length;
    const failed = entries.filter((e) => e.status === "failed").length;
    const res = entries.filter((e) => e.status === "resolving").length;
    return { ready, alreadyLocal, failed, resolving: res };
  });

  function statusLabel(status: EntryStatus): string {
    switch (status) {
      case "pending": return $t("queue.status_pending");
      case "resolving": return $t("queue.status_resolving");
      case "ready": return $t("queue.status_ready");
      case "already_local": return $t("queue.status_already_local");
      case "failed": return $t("queue.status_failed");
    }
  }

  function statusClass(status: EntryStatus): string {
    switch (status) {
      case "pending": return "badge-muted";
      case "resolving": return "badge-info";
      case "ready": return "badge-success";
      case "already_local": return "badge-alt";
      case "failed": return "badge-danger";
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="queue-overlay" role="dialog" aria-modal="true">
  <!-- Header -->
  <div class="overlay-header">
    <span class="overlay-title">{$t("queue.title")}</span>
    <button class="close-btn" onclick={handleClose} aria-label="Close">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>
  </div>

  <div class="overlay-body">
    <!-- Tabs -->
    <div class="tab-row">
      <button class="tab-btn" class:active={activeTab === "manual"} onclick={() => { activeTab = "manual"; }}>
        {$t("queue.manual_tab")}
      </button>
      <button class="tab-btn" class:active={activeTab === "json"} onclick={() => { activeTab = "json"; }}>
        {$t("queue.json_tab")}
      </button>
    </div>

    <!-- Input area -->
    {#if activeTab === "manual"}
      <div class="input-area">
        <textarea
          class="manual-textarea"
          bind:value={manualText}
          placeholder={$t("queue.textarea_placeholder")}
          rows="8"
        ></textarea>
        <button class="btn-primary" onclick={handleParseManual} disabled={!manualText.trim()}>
          Parse
        </button>
      </div>
    {:else}
      <div class="json-area">
        <button class="btn-outline" onclick={handlePickJson}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
          {$t("queue.pick_json")}
        </button>
      </div>
    {/if}

    <!-- Options -->
    <div class="options-row">
      <label class="toggle-label">
        <input type="checkbox" bind:checked={downloadOriginals} />
        <span>{$t("queue.download_originals")}</span>
      </label>
      <input
        class="text-input"
        type="text"
        bind:value={subfolder}
        placeholder={$t("queue.subfolder")}
      />
    </div>

    <!-- Entries list -->
    {#if entries.length > 0}
      <div class="entries-list">
        {#each entries as entry (entry.gid)}
          <div class="entry-row">
            <div class="entry-info">
              <span class="entry-gid">{entry.gid}</span>
              {#if entry.title}
                <span class="entry-title">{entry.title}</span>
              {/if}
              {#if entry.error}
                <span class="entry-error">{entry.error}</span>
              {/if}
            </div>
            <span class="status-badge {statusClass(entry.status)}">{statusLabel(entry.status)}</span>
          </div>
        {/each}
      </div>

      <!-- Summary bar -->
      <div class="summary-bar">
        {$t("queue.summary", {
          ready: summary().ready,
          alreadyLocal: summary().alreadyLocal,
          failed: summary().failed,
          resolving: summary().resolving,
        })}
      </div>
    {/if}

    {#if submitMessage}
      <p class="success-msg">{submitMessage}</p>
    {/if}
    {#if submitError}
      <p class="error-msg">{submitError}</p>
    {/if}
  </div>

  <!-- Footer actions -->
  <div class="overlay-footer">
    <button
      class="btn-outline"
      onclick={handleResolveAll}
      disabled={resolving || entries.filter((e) => e.status === "pending").length === 0}
    >
      {$t("queue.resolve_all")}
    </button>
    <button
      class="btn-primary"
      onclick={handleQueueAll}
      disabled={submitting || entries.filter((e) => e.status === "ready").length === 0}
    >
      {submitting ? $t("common.loading") : $t("queue.queue_all")}
    </button>
    <button class="btn-outline" onclick={handleClear} disabled={entries.length === 0}>
      {$t("queue.clear")}
    </button>
    <button class="btn-secondary" onclick={handleClose}>
      {$t("queue.close")}
    </button>
  </div>
</div>

<style>
  .queue-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: var(--bg-primary);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* ── Header ─────────────────────────────────────── */

  .overlay-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .overlay-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .close-btn {
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: color 0.15s, background 0.15s;
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  /* ── Body ────────────────────────────────────────── */

  .overlay-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 700px;
    width: 100%;
    margin: 0 auto;
  }

  /* ── Tabs ────────────────────────────────────────── */

  .tab-row {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--border);
  }

  .tab-btn {
    padding: 7px 14px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color 0.12s, border-color 0.12s;
  }

  .tab-btn:hover {
    color: var(--text-primary);
  }

  .tab-btn.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }

  /* ── Input areas ─────────────────────────────────── */

  .input-area {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .manual-textarea {
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.78rem;
    font-family: 'SF Mono', 'Fira Code', monospace;
    resize: vertical;
    outline: none;
    transition: border-color 0.15s;
    width: 100%;
    box-sizing: border-box;
  }

  .manual-textarea:focus { border-color: var(--accent); }

  .json-area {
    padding: 12px 0;
  }

  /* ── Options ─────────────────────────────────────── */

  .options-row {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.78rem;
    color: var(--text-secondary);
    cursor: pointer;
    white-space: nowrap;
  }

  .text-input {
    padding: 5px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.78rem;
    outline: none;
    transition: border-color 0.15s;
    flex: 1;
    min-width: 160px;
  }

  .text-input:focus { border-color: var(--accent); }

  /* ── Entries list ───────────────────────────────── */

  .entries-list {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    max-height: 360px;
    overflow-y: auto;
  }

  .entry-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 12px;
    border-bottom: 1px solid var(--border);
    gap: 10px;
  }

  .entry-row:last-child { border-bottom: none; }

  .entry-info {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }

  .entry-gid {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
  }

  .entry-title {
    font-size: 0.72rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-error {
    font-size: 0.7rem;
    color: var(--red);
  }

  /* ── Status badges ──────────────────────────────── */

  .status-badge {
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 0.68rem;
    font-weight: 500;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .badge-muted {
    background: var(--bg-tertiary);
    color: var(--text-muted);
  }

  .badge-info {
    background: var(--accent-subtle);
    color: var(--accent);
  }

  .badge-success {
    background: var(--success-bg);
    color: var(--green);
  }

  .badge-alt {
    background: var(--bg-elevated);
    color: var(--text-secondary);
  }

  .badge-danger {
    background: var(--danger-bg);
    color: var(--red);
  }

  /* ── Summary ─────────────────────────────────────── */

  .summary-bar {
    font-size: 0.74rem;
    color: var(--text-muted);
    padding: 4px 0;
  }

  .success-msg {
    margin: 0;
    font-size: 0.78rem;
    color: var(--green);
  }

  .error-msg {
    margin: 0;
    font-size: 0.78rem;
    color: var(--red);
  }

  /* ── Footer ─────────────────────────────────────── */

  .overlay-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  /* ── Buttons ────────────────────────────────────── */

  .btn-primary {
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
    white-space: nowrap;
  }

  .btn-primary:hover:not(:disabled) { background: var(--accent-hover); }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn-outline {
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .btn-outline:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .btn-outline:disabled { opacity: 0.4; cursor: not-allowed; }

  .btn-secondary {
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    white-space: nowrap;
    margin-left: auto;
  }

  .btn-secondary:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
</style>
