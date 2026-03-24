import { writable } from "svelte/store";
import type { Gallery, AdvancedSearchOptions, TagSuggestion } from "$lib/api/galleries";

// ── Search sort types ─────────────────────────────────────────────────────────

export type SearchSortField = "rating" | "posted" | "pages" | "title";

export interface SearchSortScope {
  count: number;
  field: SearchSortField;
  dir: "asc" | "desc";
}

export interface SearchSortFetchProgress {
  fetching: boolean;
  loaded: number;
  target: number;
  estimatedSeconds: number;
}

export function emptySearchSortScope(): SearchSortScope {
  return { count: 100, field: "posted", dir: "desc" };
}

export function emptySearchSortFetchProgress(): SearchSortFetchProgress {
  return { fetching: false, loaded: 0, target: 0, estimatedSeconds: 0 };
}

/** Galleries returned from ExHentai server-side search. */
export const searchResults = writable<Gallery[]>([]);

/** Current search free text (NOT the combined f_search string). Never contains tag syntax. */
export const searchQuery = writable<string>("");

/** Next page URL from #unext href (cursor-based pagination). Null when no more pages. */
export const searchNextUrl = writable<string | null>(null);

/** Whether more results are available on the server. */
export const searchHasMore = writable<boolean>(false);

/** Whether a search request is in-flight. */
export const searchLoading = writable<boolean>(false);

/** Category exclusion bitmask. 0 = show all. */
export const searchCategoryMask = writable<number>(0);

/** Advanced search options. */
export const searchAdvancedOptions = writable<AdvancedSearchOptions>({
  search_name: true,
  search_tags: true,
  search_description: false,
  show_expunged: false,
  search_torrent_filenames: false,
  only_with_torrents: false,
  search_low_power_tags: false,
  search_downvoted_tags: false,
  minimum_rating: null,
  min_pages: null,
  max_pages: null,
});

/** Tags to include in search (appended as namespace:"name" to f_search). */
export const searchIncludeTags = writable<TagSuggestion[]>([]);

/** Tags to exclude from search (appended as -namespace:"name" to f_search). */
export const searchExcludeTags = writable<TagSuggestion[]>([]);

/** Quick hide filter (client-side, same as homepage). */
export const searchHideFilter = writable<string>("");

// ── Sort stores ───────────────────────────────────────────────────────────────

/** Scope for the search sort: count, field, direction. */
export const searchSortScope = writable<SearchSortScope>(emptySearchSortScope());

/** Progress of the pre-sort fetch phase. */
export const searchSortFetchProgress = writable<SearchSortFetchProgress>(emptySearchSortFetchProgress());

/** True while the sorted view is active (infinite scroll paused). */
export const searchSortActive = writable<boolean>(false);

/** Final sorted+filtered snapshot rendered when searchSortActive is true. */
export const searchSortedGalleries = writable<Gallery[]>([]);
