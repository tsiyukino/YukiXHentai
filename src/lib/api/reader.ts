import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ── Types ─────────────────────────────────────────────────────────────────

export interface GalleryPageEntry {
  page_index: number;
  page_url: string;
  image_path: string | null;
  thumb_url: string | null;
  imgkey: string | null;
}

export interface ImageDownloadProgressEvent {
  gid: number;
  page_index: number;
  status: "queued" | "downloading" | "done" | "error" | "rate_limited";
  path: string | null;
  error: string | null;
}

export interface GalleryPages {
  gid: number;
  token: string;
  title: string;
  pages: GalleryPageEntry[];
  total_pages: number;
  showkey: string | null;
}

export interface ReadProgress {
  gid: number;
  last_page_read: number;
  total_pages: number;
  last_read_at: number;
  is_completed: boolean;
}

export interface ReadingSession {
  id: number;
  gid: number;
  opened_at: number;
  closed_at: number | null;
  pages_read: number;
}

// ── Gallery pages ─────────────────────────────────────────────────────────

export async function getGalleryPages(gid: number, token: string, forceRefresh?: boolean): Promise<GalleryPages> {
  return invoke("get_gallery_pages", { gid, token, forceRefresh });
}

// ── Gallery pages batch ───────────────────────────────────────────────────

export interface GalleryPagesBatchResult {
  gid: number;
  pages: GalleryPageEntry[];
  showkey: string | null;
  total_pages: number;
  has_next_page: boolean;
  detail_page: number;
}

/**
 * Fetch a single detail page (p=N) from ExHentai for a gallery.
 * Returns only the entries on that page, plus total_pages and has_next_page.
 * The frontend calls this on-demand as the user scrolls.
 *
 * pagesPerBatch: how many thumbnails ExHentai shows per detail page (20 or 40).
 * Pass the count from p=0's result.pages.length for p=1+. Omit for p=0.
 */
export async function getGalleryPagesBatch(
  gid: number,
  token: string,
  detailPage: number,
  pagesPerBatch?: number
): Promise<GalleryPagesBatchResult> {
  return invoke("get_gallery_pages_batch", { gid, token, detailPage, pagesPerBatch });
}

// ── Detail panel API ─────────────────────────────────────────────────────

/**
 * Fetch extended metadata via the ExHentai JSON API (fast, ~200ms).
 * Updates DB and returns the enriched Gallery object.
 */
export async function fetchGalleryMetadata(gid: number, token: string): Promise<import("$lib/api/galleries").Gallery> {
  return invoke("fetch_gallery_metadata", { gid, token });
}

/**
 * Download a page thumbnail through Rust (with cookies), returns local path.
 * Cached in page-thumbs/{gid}/{page}.jpg.
 */
export async function getPageThumbnail(
  gid: number,
  pageIndex: number,
  thumbUrl: string
): Promise<string> {
  return invoke("get_page_thumbnail", { gid, pageIndex, thumbUrl });
}

/**
 * Set the active gallery for page thumbnail downloads.
 * Pass null to cancel all pending page thumbnail downloads.
 */
export async function setActiveDetailGallery(gid: number | null): Promise<void> {
  return invoke("set_active_detail_gallery", { gid });
}

export async function getGalleryImage(
  gid: number,
  pageIndex: number,
  pageUrl: string,
  imgkey?: string | null,
  showkey?: string | null
): Promise<string> {
  return invoke("get_gallery_image", {
    gid,
    pageIndex,
    pageUrl,
    imgkey: imgkey ?? null,
    showkey: showkey ?? null,
  });
}

// ── Read progress ─────────────────────────────────────────────────────────

export async function updateReadProgress(progress: ReadProgress): Promise<void> {
  return invoke("update_read_progress", { progress });
}

export async function getReadProgress(gid: number): Promise<ReadProgress | null> {
  return invoke("get_read_progress", { gid });
}

export async function getReadProgressBatch(gids: number[]): Promise<ReadProgress[]> {
  return invoke("get_read_progress_batch", { gids });
}

// ── Reading sessions ──────────────────────────────────────────────────────

export async function startReadingSession(gid: number, openedAt: number): Promise<number> {
  return invoke("start_reading_session", { gid, openedAt });
}

export async function endReadingSession(
  sessionId: number,
  closedAt: number,
  pagesRead: number
): Promise<void> {
  return invoke("end_reading_session", { sessionId, closedAt, pagesRead });
}

export async function getReadingHistory(
  limit: number,
  offset: number
): Promise<ReadingSession[]> {
  return invoke("get_reading_history", { limit, offset });
}

// ── UI config ────────────────────────────────────────────────────────

export async function getDetailPreviewSize(): Promise<number> {
  return invoke("get_detail_preview_size");
}

export async function setDetailPreviewSize(size: number): Promise<void> {
  return invoke("set_detail_preview_size", { size });
}

export async function getTheme(): Promise<string> {
  return invoke("get_theme");
}

export async function setTheme(theme: string): Promise<void> {
  return invoke("set_theme", { theme });
}

// ── Cache management ─────────────────────────────────────────────────────

export async function getCacheDir(): Promise<string> {
  return invoke("get_cache_dir");
}

export async function setCacheDir(path: string): Promise<void> {
  return invoke("set_cache_dir", { path });
}

export async function clearImageCache(): Promise<number> {
  return invoke("clear_image_cache");
}

// ── Download cancellation ─────────────────────────────────────────────────

/**
 * Cancel all in-progress and queued image downloads for a gallery.
 * Pass null/undefined to cancel all galleries.
 */
export async function cancelImageDownloads(gid?: number | null): Promise<void> {
  return invoke("cancel_image_downloads", { gid: gid ?? null });
}

/**
 * Register a new download session for a gallery.
 * Cancels any previous session for the same gallery.
 */
export async function registerDownloadSession(gid: number): Promise<void> {
  return invoke("register_download_session", { gid });
}

// ── Download progress events ─────────────────────────────────────────────

export async function onImageDownloadProgress(
  callback: (event: ImageDownloadProgressEvent) => void
): Promise<UnlistenFn> {
  return listen<ImageDownloadProgressEvent>("image-download-progress", (e) =>
    callback(e.payload)
  );
}

// ── Gallery pages batch events ───────────────────────────────────────────

export interface GalleryPagesBatchEvent {
  gid: number;
  pages: GalleryPageEntry[];
  showkey: string | null;
  total_pages: number;
}

export async function onGalleryPagesBatch(
  callback: (event: GalleryPagesBatchEvent) => void
): Promise<UnlistenFn> {
  return listen<GalleryPagesBatchEvent>("gallery-pages-batch", (e) =>
    callback(e.payload)
  );
}
