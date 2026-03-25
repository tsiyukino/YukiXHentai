import { invoke } from "@tauri-apps/api/core";
import type { LocalReaderGallery } from "$lib/stores/localLibrary";

export interface LocalPage {
  gid: number;
  page_index: number;
  file_path: string;
  source_url?: string;
  width?: number;
  height?: number;
}

/** Build a LocalReaderGallery from a gallery title/gid and its pages array. */
export function buildLocalReaderGallery(
  gid: number,
  title: string,
  pages: LocalPage[]
): LocalReaderGallery {
  return {
    gid,
    title,
    pages: pages.map(p => ({ page_index: p.page_index, file_path: p.file_path })),
    total_pages: pages.length,
  };
}

export interface ReadCacheStats {
  used_bytes: number;
  max_bytes: number;
  file_count: number;
}

export interface GalleryMetadataPatch {
  title?: string;
  titleJpn?: string;
  category?: string;
  uploader?: string;
  description?: string;
  tagsAdd?: { namespace: string; name: string }[];
  tagsRemove?: { namespace: string; name: string }[];
}

export interface ImportPreview {
  detected_title: string;
  detected_gid?: number;
  detected_token?: string;
  metadata_found: boolean;
  page_count: number;
  sample_filenames: string[];
  parsed_meta?: LocalGalleryMeta;
}

export interface LocalGalleryMeta {
  gid?: number;
  token?: string;
  title: string;
  titleJpn?: string;
  category: string;
  uploader?: string;
  description?: string;
  tags: string[];
  pages: LocalPageMeta[];
}

export interface LocalPageMeta {
  index: number;
  filename: string;
  sourceUrl?: string;
  width?: number;
  height?: number;
}

export interface QueueEntry {
  gid: number;
  token?: string;
  title?: string;
  already_local: boolean;
}

export interface ResolvedGallery {
  gid: number;
  token?: string;
  title?: string;
  error?: string;
}

export interface SubmitEntry {
  gid: number;
  token: string;
}

export interface SubmitResult {
  queued: number;
  skipped_already_local: number;
}

export interface DownloadQueueStatus {
  queued: number;
  downloading: number;
  completed: number;
  failed: number;
  current_gid?: number;
  current_title?: string;
  current_page?: number;
  total_pages?: number;
}

export function getLocalGalleries(offset: number, limit: number) {
  return invoke<{ galleries: any[]; totalCount: number }>("get_local_galleries", { offset, limit });
}

export function getLocalGalleryPages(gid: number) {
  return invoke<LocalPage[]>("get_local_gallery_pages", { gid });
}

export function updateGalleryMetadata(gid: number, patch: GalleryMetadataPatch) {
  return invoke<void>("update_gallery_metadata", { gid, patch });
}

export function reorderLocalPages(gid: number, newOrder: number[]) {
  return invoke<void>("reorder_local_pages", { gid, newOrder });
}

export function insertLocalPages(gid: number, filePaths: string[], insertAfterIndex: number) {
  return invoke<LocalPage[]>("insert_local_pages", { gid, filePaths, insertAfterIndex });
}

export function removeLocalPage(gid: number, pageIndex: number, deleteFile: boolean) {
  return invoke<void>("remove_local_page", { gid, pageIndex, deleteFile });
}

export function setLocalGalleryCover(gid: number, filePath: string) {
  return invoke<string>("set_local_gallery_cover", { gid, filePath });
}

export function importLocalFolder(folderPath: string) {
  return invoke<ImportPreview>("import_local_folder", { folderPath });
}

export function confirmImportLocalFolder(folderPath: string, meta: LocalGalleryMeta) {
  return invoke<any>("confirm_import_local_folder", { folderPath, meta });
}

export function getReadCacheStats() {
  return invoke<ReadCacheStats>("get_read_cache_stats");
}

export function setReadCacheMaxMb(mb: number) {
  return invoke<void>("set_read_cache_max_mb", { mb });
}

export function clearReadCache() {
  return invoke<number>("clear_read_cache");
}

export function getHistoryRetentionDays() {
  return invoke<number>("get_history_retention_days");
}

export function setHistoryRetentionDays(days: number) {
  return invoke<void>("set_history_retention_days", { days });
}

export function parseDownloadQueueJson(filePath: string) {
  return invoke<QueueEntry[]>("parse_download_queue_json", { filePath });
}

export function resolveGalleryToken(gid: number) {
  return invoke<ResolvedGallery>("resolve_gallery_token", { gid });
}

export function submitDownloadQueue(entries: SubmitEntry[], downloadOriginals: boolean, subfolder?: string) {
  return invoke<SubmitResult>("submit_download_queue", { entries, downloadOriginals, subfolder });
}

export function getDownloadQueueStatus() {
  return invoke<DownloadQueueStatus>("get_download_queue_status");
}

export function pauseDownloadQueue() {
  return invoke<void>("pause_download_queue");
}

export function resumeDownloadQueue() {
  return invoke<void>("resume_download_queue");
}

export function cancelDownloadQueue(gid?: number) {
  return invoke<void>("cancel_download_queue", { gid });
}

export function deleteLocalGallery(gid: number) {
  return invoke<void>("delete_local_gallery", { gid });
}

/**
 * Sync a local gallery from its origin site.
 * Only works for galleries with origin + remote_gid set.
 * Currently a placeholder — sync logic is not yet implemented.
 */
export function syncLocalGallery(gid: number) {
  return invoke<void>("sync_local_gallery", { gid });
}

export function getLibraryDir() {
  return invoke<string>("get_library_dir");
}

export function setLibraryDir(path: string) {
  return invoke<void>("set_library_dir", { path });
}
