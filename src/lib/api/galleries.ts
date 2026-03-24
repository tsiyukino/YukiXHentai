import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Tag {
  namespace: string;
  name: string;
}

export interface Gallery {
  gid: number;
  token: string;
  title: string;
  title_jpn: string | null;
  category: string;
  thumb_url: string;
  thumb_path: string | null;
  uploader: string | null;
  posted: number;
  rating: number;
  file_count: number;
  file_size: number | null;
  tags: Tag[];
}

export interface SyncResult {
  galleries_synced: number;
  has_next_page: boolean;
  message: string;
}

export interface SyncProgress {
  current_page: number;
  total_pages: number;
  thumbs_downloaded: number;
  thumbs_total: number;
  galleries_synced: number;
  message: string;
  done: boolean;
}

export interface GalleryPage {
  galleries: Gallery[];
  total_count: number;
}

export async function syncGalleryPage(page?: number): Promise<SyncResult> {
  return invoke("sync_gallery_page", { page: page ?? null });
}

export async function syncGalleries(depth?: number): Promise<SyncResult> {
  return invoke("sync_galleries", { depth: depth ?? null });
}

export async function onSyncProgress(
  callback: (progress: SyncProgress) => void
): Promise<UnlistenFn> {
  return listen<SyncProgress>("sync-progress", (event) => {
    callback(event.payload);
  });
}

export async function getGalleries(offset: number, limit: number): Promise<GalleryPage> {
  return invoke("get_galleries", { offset, limit });
}

export async function getGalleriesByGids(gids: number[]): Promise<Gallery[]> {
  return invoke("get_galleries_by_gids", { gids });
}

// ── Search & Filter types ────────────────────────────────────────────────

export interface TagFilter {
  namespace: string;
  name: string;
}

export interface FilterParams {
  query?: string | null;
  tags_include: TagFilter[];
  tags_exclude: TagFilter[];
  categories: string[];
  rating_min?: number | null;
  rating_max?: number | null;
  pages_min?: number | null;
  pages_max?: number | null;
  date_min?: number | null;
  date_max?: number | null;
  language?: string | null;
  uploader?: string | null;
}

export interface SortField {
  field: string;
  direction: "Asc" | "Desc";
}

export interface SortParams {
  fields: SortField[];
}

export function emptyFilter(): FilterParams {
  return {
    query: null,
    tags_include: [],
    tags_exclude: [],
    categories: [],
    rating_min: null,
    rating_max: null,
    pages_min: null,
    pages_max: null,
    date_min: null,
    date_max: null,
    language: null,
    uploader: null,
  };
}

export function defaultSort(): SortParams {
  return { fields: [{ field: "posted", direction: "Desc" }] };
}

// ── Preset types & API ───────────────────────────────────────────────────

export interface FilterPreset {
  id: number;
  name: string;
  filter: FilterParams;
  sort: SortParams;
}

export async function savePreset(
  name: string,
  filter: FilterParams,
  sort: SortParams
): Promise<FilterPreset> {
  return invoke("save_preset", { name, filter, sort });
}

export async function getPresets(): Promise<FilterPreset[]> {
  return invoke("get_presets");
}

export async function deletePreset(id: number): Promise<void> {
  return invoke("delete_preset", { id });
}

// ── Single-page sync (infinite scroll) ───────────────────────────────────

export interface SyncPageResult {
  galleries: Gallery[];
  has_more: boolean;
}

export interface ThumbnailReadyEvent {
  gid: number;
  path: string;
}

export async function syncNextPage(): Promise<SyncPageResult> {
  return invoke("sync_next_page");
}

export async function resetSyncCursor(): Promise<void> {
  return invoke("reset_sync_cursor");
}

export async function downloadThumbnailsForGids(gids: number[]): Promise<number> {
  return invoke("download_thumbnails_for_gids", { gids });
}

export async function onThumbnailReady(
  callback: (event: ThumbnailReadyEvent) => void
): Promise<UnlistenFn> {
  return listen<ThumbnailReadyEvent>("thumbnail-ready", (e) => callback(e.payload));
}

export async function searchGalleries(
  filter: FilterParams,
  sort: SortParams,
  offset: number,
  limit: number
): Promise<GalleryPage> {
  return invoke("search_galleries", { filter, sort, offset, limit });
}

// ── Metadata enrichment ─────────────────────────────────────────────────

export interface GalleryEnrichedEvent {
  gallery: Gallery;
}

export async function startEnrichment(): Promise<number> {
  return invoke("start_enrichment");
}

export async function onGalleryEnriched(
  callback: (event: GalleryEnrichedEvent) => void
): Promise<UnlistenFn> {
  return listen<GalleryEnrichedEvent>("gallery-enriched", (e) => callback(e.payload));
}

// ── ExHentai server-side search ──────────────────────────────────────────

export interface AdvancedSearchOptions {
  search_name?: boolean;
  search_tags?: boolean;
  search_description?: boolean;
  show_expunged?: boolean;
  search_torrent_filenames?: boolean;
  only_with_torrents?: boolean;
  search_low_power_tags?: boolean;
  search_downvoted_tags?: boolean;
  /** Minimum star rating (2–5). Undefined = disabled. */
  minimum_rating?: number | null;
  /** Minimum page count. Undefined/null = no minimum. */
  min_pages?: number | null;
  /** Maximum page count. Undefined/null = no maximum. */
  max_pages?: number | null;
}

export interface TagSuggestion {
  namespace: string;
  name: string;
}

export interface ExhSearchResult {
  galleries: Gallery[];
  has_more: boolean;
  /** Full URL for the next page (from #unext href). Pass back for cursor-based pagination. */
  next_url: string | null;
}

export async function searchExhentai(
  query: string,
  nextUrl?: string | null,
  categoryMask?: number,
  advancedOptions?: AdvancedSearchOptions
): Promise<ExhSearchResult> {
  return invoke("search_exhentai", {
    query,
    nextUrl: nextUrl ?? null,
    categoryMask: categoryMask ?? null,
    advancedOptions: advancedOptions ?? null,
  });
}

// ── Search history ───────────────────────────────────────────────────────

export interface SearchHistoryEntry {
  id: number;
  query: string;
  searched_at: number;
}

export async function getSearchHistory(limit?: number): Promise<SearchHistoryEntry[]> {
  return invoke("get_search_history", { limit: limit ?? null });
}

export async function clearSearchHistory(): Promise<void> {
  return invoke("clear_search_history");
}

export async function searchTagsAutocomplete(query: string): Promise<TagSuggestion[]> {
  return invoke("search_tags_autocomplete", { query });
}
