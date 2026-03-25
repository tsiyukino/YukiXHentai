<script lang="ts">
  import { t } from "$lib/i18n";
  import {
    addFavorite,
    removeFavorite,
    getFavoriteFolders,
    folderColor,
  } from "$lib/api/favorites";
  import type { FavoriteFolder } from "$lib/api/favorites";

  interface Props {
    gid: number;
    token: string;
    /** Current favorite folder index, or null if not favorited */
    currentFavcat: number | null;
    currentNote: string;
    onClose: () => void;
    onUpdated: (favcat: number | null, favnote: string) => void;
  }

  let { gid, token, currentFavcat, currentNote, onClose, onUpdated }: Props = $props();

  let folders = $state<FavoriteFolder[]>([]);
  let selectedFavcat = $state<number>(currentFavcat ?? 0);
  let favnote = $state(currentNote);
  let saving = $state(false);
  let error = $state<string | null>(null);

  // Load folder names on mount.
  $effect(() => {
    getFavoriteFolders().then((f) => {
      if (f.length > 0) {
        folders = f;
      } else {
        // Fallback: generate default names.
        folders = Array.from({ length: 10 }, (_, i) => ({
          index: i,
          name: `Favorite ${i}`,
          count: 0,
        }));
      }
    }).catch(() => {
      folders = Array.from({ length: 10 }, (_, i) => ({
        index: i,
        name: `Favorite ${i}`,
        count: 0,
      }));
    });
  });

  async function handleSave() {
    saving = true;
    error = null;
    try {
      await addFavorite(gid, token, selectedFavcat, favnote.slice(0, 200));
      onUpdated(selectedFavcat, favnote);
      onClose();
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  }

  async function handleRemove() {
    saving = true;
    error = null;
    try {
      await removeFavorite(gid, token);
      onUpdated(null, "");
      onClose();
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Backdrop -->
<div class="dialog-backdrop" onclick={onClose} role="presentation"></div>

<!-- Dialog -->
<div class="dialog" role="dialog" aria-modal="true">
  <div class="dialog-header">
    <span class="dialog-title">{$t("favorites_dialog.title")}</span>
    <button class="close-btn" onclick={onClose} aria-label="Close">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>
  </div>

  <div class="folder-grid">
    {#each folders as folder}
      <button
        class="folder-btn"
        class:selected={selectedFavcat === folder.index}
        onclick={() => selectedFavcat = folder.index}
        style="--folder-color: {folderColor(folder.index)}"
      >
        <span class="folder-dot"></span>
        <span class="folder-name">{folder.name}</span>
      </button>
    {/each}
  </div>

  <div class="note-area">
    <label class="note-label" for="favnote">{$t("favorites_dialog.note")}</label>
    <textarea
      id="favnote"
      class="note-input"
      bind:value={favnote}
      placeholder={$t("favorites_dialog.note_placeholder")}
      maxlength="200"
      rows="3"
    ></textarea>
    <span class="char-count">{favnote.length}/200</span>
  </div>

  {#if error}
    <p class="error-msg">{error}</p>
  {/if}

  <div class="dialog-actions">
    {#if currentFavcat !== null}
      <button class="btn-danger" onclick={handleRemove} disabled={saving}>
        {$t("favorites_dialog.remove")}
      </button>
    {/if}
    <button class="btn-secondary" onclick={onClose} disabled={saving}>
      {$t("common.cancel")}
    </button>
    <button class="btn-primary" onclick={handleSave} disabled={saving}>
      {saving ? $t("common.loading") : $t("favorites_dialog.save")}
    </button>
  </div>
</div>

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: var(--overlay-bg);
    z-index: 600;
  }

  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 601;
    background: var(--bg-primary);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
    width: 340px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .dialog-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .close-btn {
    width: 24px;
    height: 24px;
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

  .folder-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 6px;
  }

  .folder-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s, color 0.12s;
    text-align: left;
    overflow: hidden;
  }

  .folder-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
    border-color: var(--border-strong);
  }

  .folder-btn.selected {
    border-color: var(--folder-color);
    background: color-mix(in srgb, var(--folder-color) 12%, var(--bg-primary));
    color: var(--text-primary);
  }

  .folder-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--folder-color);
    flex-shrink: 0;
  }

  .folder-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .note-area {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .note-label {
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--text-muted);
  }

  .note-input {
    resize: vertical;
    padding: 7px 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.82rem;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s;
    min-height: 60px;
  }

  .note-input:focus {
    border-color: var(--accent);
  }

  .char-count {
    font-size: 0.7rem;
    color: var(--text-muted);
    text-align: right;
  }

  .error-msg {
    margin: 0;
    font-size: 0.78rem;
    color: var(--red);
  }

  .dialog-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: 2px;
  }

  .btn-primary,
  .btn-secondary,
  .btn-danger {
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    border: none;
    transition: opacity 0.15s, background 0.15s;
  }

  .btn-primary:disabled,
  .btn-secondary:disabled,
  .btn-danger:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: var(--accent);
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn-secondary {
    background: var(--bg-secondary);
    color: var(--text-secondary);
    border: 1px solid var(--border-strong);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .btn-danger {
    background: var(--danger-bg);
    color: var(--red);
    border: 1px solid var(--danger-border);
    margin-right: auto;
  }

  .btn-danger:hover:not(:disabled) {
    opacity: 0.85;
  }
</style>
