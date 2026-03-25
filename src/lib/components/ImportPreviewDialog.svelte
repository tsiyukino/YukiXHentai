<script lang="ts">
  import { t } from "$lib/i18n";
  import { confirmImportLocalFolder, type ImportPreview, type LocalGalleryMeta } from "$lib/api/library";
  import type { Gallery } from "$lib/api/galleries";

  const EH_CATEGORIES = [
    "Doujinshi", "Manga", "Artist CG", "Game CG", "Western",
    "Non-H", "Image Set", "Cosplay", "Asian Porn", "Misc",
  ];

  interface Props {
    folderPath: string;
    preview: ImportPreview;
    onConfirm: (gallery: Gallery) => void;
    onClose: () => void;
  }

  let { folderPath, preview, onConfirm, onClose }: Props = $props();

  let title = $state(preview.parsed_meta?.title ?? preview.detected_title);
  let titleJpn = $state(preview.parsed_meta?.titleJpn ?? "");
  let category = $state(preview.parsed_meta?.category ?? "Misc");
  let uploader = $state(preview.parsed_meta?.uploader ?? "");
  let description = $state(preview.parsed_meta?.description ?? "");

  let saving = $state(false);
  let saveError = $state<string | null>(null);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }

  async function handleConfirm() {
    if (!title.trim()) return;
    saving = true;
    saveError = null;
    try {
      const meta: LocalGalleryMeta = {
        gid: preview.detected_gid,
        token: preview.detected_token,
        title: title.trim(),
        titleJpn: titleJpn.trim() || undefined,
        category,
        uploader: uploader.trim() || undefined,
        description: description.trim() || undefined,
        tags: preview.parsed_meta?.tags ?? [],
        pages: preview.parsed_meta?.pages ?? [],
      };
      const gallery = await confirmImportLocalFolder(folderPath, meta);
      onConfirm(gallery as Gallery);
    } catch (err) {
      saveError = String(err);
    } finally {
      saving = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Backdrop -->
<div class="dialog-backdrop" onclick={onClose} role="presentation"></div>

<!-- Dialog -->
<div class="dialog" role="dialog" aria-modal="true" aria-labelledby="import-preview-title">
  <div class="dialog-header">
    <span class="dialog-title" id="import-preview-title">{$t("local.import_preview_title")}</span>
    <button class="close-btn" onclick={onClose} aria-label="Close">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>
  </div>

  <div class="dialog-body">
    <!-- Metadata status badge -->
    {#if preview.metadata_found}
      <div class="badge badge-success">{$t("local.metadata_found")}</div>
    {:else}
      <div class="badge badge-muted">{$t("local.no_metadata")}</div>
    {/if}

    <!-- Page count -->
    <p class="page-count-line">{$t("local.page_count", { count: preview.page_count })}</p>

    <!-- Fields -->
    <div class="field-group">
      <label class="field-label" for="imp-title">{$t("local.detected_title")}</label>
      <input id="imp-title" class="text-input" type="text" bind:value={title} placeholder={$t("local.detected_title")} />
    </div>

    <div class="field-group">
      <label class="field-label" for="imp-title-jpn">Japanese title</label>
      <input id="imp-title-jpn" class="text-input" type="text" bind:value={titleJpn} placeholder="(optional)" />
    </div>

    <div class="field-row-2">
      <div class="field-group">
        <label class="field-label" for="imp-cat">Category</label>
        <select id="imp-cat" class="select-input" bind:value={category}>
          {#each EH_CATEGORIES as cat}
            <option value={cat}>{cat}</option>
          {/each}
        </select>
      </div>
      <div class="field-group">
        <label class="field-label" for="imp-uploader">Uploader</label>
        <input id="imp-uploader" class="text-input" type="text" bind:value={uploader} placeholder="(optional)" />
      </div>
    </div>

    <div class="field-group">
      <label class="field-label" for="imp-desc">{$t("local.description")}</label>
      <textarea id="imp-desc" class="text-input textarea" bind:value={description} rows="2" placeholder="(optional)"></textarea>
    </div>

    <!-- Tags from metadata -->
    {#if preview.parsed_meta?.tags && preview.parsed_meta.tags.length > 0}
      <div class="field-group">
        <span class="field-label">{$t("local.tags_from_exh")}</span>
        <div class="tag-chips">
          {#each preview.parsed_meta.tags.slice(0, 20) as tag}
            <span class="tag-chip">{tag}</span>
          {/each}
          {#if preview.parsed_meta.tags.length > 20}
            <span class="tag-more">+{preview.parsed_meta.tags.length - 20}</span>
          {/if}
        </div>
      </div>
    {/if}

    <!-- Sample filenames -->
    {#if preview.sample_filenames.length > 0}
      <div class="field-group">
        <span class="field-label">{$t("local.sample_files")}</span>
        <div class="sample-files">
          {#each preview.sample_filenames.slice(0, 20) as fname}
            <span class="sample-file">{fname}</span>
          {/each}
        </div>
      </div>
    {/if}

    {#if saveError}
      <p class="error-msg">{saveError}</p>
    {/if}
  </div>

  <div class="dialog-actions">
    <button class="btn-secondary" onclick={onClose} disabled={saving}>
      {$t("local.import_cancel")}
    </button>
    <button class="btn-primary" onclick={handleConfirm} disabled={saving || !title.trim()}>
      {saving ? $t("common.loading") : $t("local.import_confirm")}
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
    width: 480px;
    max-width: calc(100vw - 32px);
    max-height: 90vh;
    display: flex;
    flex-direction: column;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 18px 12px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
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

  .dialog-body {
    flex: 1;
    overflow-y: auto;
    padding: 14px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 3px 8px;
    border-radius: 10px;
    font-size: 0.7rem;
    font-weight: 500;
    align-self: flex-start;
  }

  .badge-success {
    background: var(--success-bg);
    color: var(--green);
  }

  .badge-muted {
    background: var(--bg-tertiary);
    color: var(--text-muted);
  }

  .page-count-line {
    margin: 0;
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-row-2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .field-label {
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .text-input {
    padding: 6px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.82rem;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s;
    width: 100%;
    box-sizing: border-box;
  }

  .text-input:focus { border-color: var(--accent); }

  .select-input {
    padding: 6px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.82rem;
    outline: none;
    width: 100%;
  }

  .textarea {
    resize: vertical;
    min-height: 56px;
  }

  .tag-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .tag-chip {
    padding: 2px 8px;
    border-radius: 10px;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    font-size: 0.7rem;
  }

  .tag-more {
    font-size: 0.7rem;
    color: var(--text-muted);
    padding: 2px 4px;
  }

  .sample-files {
    max-height: 120px;
    overflow-y: auto;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .sample-file {
    font-size: 0.72rem;
    color: var(--text-secondary);
    font-family: 'SF Mono', 'Fira Code', monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
    padding: 12px 18px 16px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
  }

  .btn-primary,
  .btn-secondary {
    padding: 6px 14px;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    border: none;
    transition: opacity 0.15s, background 0.15s;
  }

  .btn-primary:disabled,
  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: var(--accent);
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) { background: var(--accent-hover); }

  .btn-secondary {
    background: var(--bg-secondary);
    color: var(--text-secondary);
    border: 1px solid var(--border-strong);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
</style>
