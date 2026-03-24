import { get } from "svelte/store";
import { getPageThumbnail } from "$lib/api/reader";
import { detailPageThumbs } from "$lib/stores/detail";

// ── Page thumbnail download service ──────────────────────────────────────────
//
// Shared singleton that manages concurrent page-thumbnail downloads for both
// GalleryDetail and GalleryReader. Both components call enqueue(); results land
// in the detailPageThumbs store and are delivered via the onReady callback.
//
// Concurrency: up to MAX_CONCURRENT downloads in flight simultaneously.
// Callers cancel by calling reset() on gallery change or component destroy.

const MAX_CONCURRENT = 6;

let activeGid: number | null = null;
let queue: { gid: number; pageIdx: number; thumbUrl: string }[] = [];
let downloadingSet = new Set<number>(); // pageIdx values currently in-flight
let onReadyCallback: ((pageIdx: number, rawPath: string) => void) | null = null;

/** Register the callback invoked when a thumbnail finishes downloading.
 *  Only one subscriber at a time (last write wins). Call with null on destroy. */
export function setThumbReadyCallback(
  cb: ((pageIdx: number, rawPath: string) => void) | null
): void {
  onReadyCallback = cb;
}

/** Enqueue a page thumbnail for download.
 *  Skips silently if already in-flight or already cached in detailPageThumbs. */
export function enqueuePageThumb(gid: number, pageIdx: number, thumbUrl: string): void {
  // Already cached in shared store — nothing to do.
  const cur = get(detailPageThumbs);
  if (cur && cur.gid === gid && pageIdx in cur.paths) return;

  // Already queued or downloading.
  if (downloadingSet.has(pageIdx)) return;
  if (queue.some((q) => q.gid === gid && q.pageIdx === pageIdx)) return;

  queue.push({ gid, pageIdx, thumbUrl });
  _pump();
}

/** Drop all queued (not yet started) downloads and reset state.
 *  In-flight downloads will resolve but their results will be ignored (gid mismatch). */
export function resetPageThumbs(gid: number): void {
  activeGid = gid;
  queue = [];
  downloadingSet = new Set();
}

function _pump(): void {
  while (downloadingSet.size < MAX_CONCURRENT && queue.length > 0) {
    const item = queue.shift()!;

    // Skip if gallery has changed since this was enqueued.
    if (item.gid !== activeGid) continue;

    // Skip if now cached (may have arrived via the other component).
    const cur = get(detailPageThumbs);
    if (cur && cur.gid === item.gid && item.pageIdx in cur.paths) continue;

    downloadingSet.add(item.pageIdx);
    _download(item.gid, item.pageIdx, item.thumbUrl).finally(() => {
      downloadingSet.delete(item.pageIdx);
      _pump();
    });
  }
}

async function _download(gid: number, pageIdx: number, thumbUrl: string): Promise<void> {
  try {
    const rawPath = await getPageThumbnail(gid, pageIdx, thumbUrl);

    // Discard if gallery changed while we were waiting.
    if (gid !== activeGid) return;

    // Write to shared store (in-place to avoid spurious reactive triggers).
    const cur = get(detailPageThumbs);
    if (cur && cur.gid === gid) {
      cur.paths[pageIdx] = rawPath;
      detailPageThumbs.set(cur);
    }

    // Notify current subscriber.
    onReadyCallback?.(pageIdx, rawPath);
  } catch {
    // Cancelled or network error — leave skeleton in place.
  }
}
