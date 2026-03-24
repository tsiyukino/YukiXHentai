import { writable } from "svelte/store";
import type { Gallery, FilterParams, SortParams, SyncProgress, TagFilter } from "$lib/api/galleries";
import { emptyFilter, defaultSort } from "$lib/api/galleries";

export const galleries = writable<Gallery[]>([]);
export const totalCount = writable<number>(0);
export const syncing = writable<boolean>(false);
export const syncMessage = writable<string>("");
export const syncProgress = writable<SyncProgress | null>(null);
export const browsePage = writable<number>(0);

// Search state.
export const activeFilter = writable<FilterParams>(emptyFilter());
export const activeSort = writable<SortParams>(defaultSort());
export const searchActive = writable<boolean>(false);

// Quick visibility filter (home shelf). Applied client-side on top of infinite scroll.
export const quickFilter = writable<string>("");

// Home filter panel state. Pure view-layer hide filter; does not query DB or IPC.
// Persists within the session (survives nav switches).
export interface HomeFilterState {
  tagsInclude: TagFilter[];
  tagsExclude: TagFilter[];
  categories: string[];    // empty = show all
  ratingMin: number | null;
  pagesMin: number | null;
  pagesMax: number | null;
  language: string;        // empty = any
  uploader: string;        // empty = any
}

export function emptyHomeFilter(): HomeFilterState {
  return {
    tagsInclude: [],
    tagsExclude: [],
    categories: [],
    ratingMin: null,
    pagesMin: null,
    pagesMax: null,
    language: "",
    uploader: "",
  };
}

export function isHomeFilterActive(f: HomeFilterState): boolean {
  return (
    f.tagsInclude.length > 0 ||
    f.tagsExclude.length > 0 ||
    f.categories.length > 0 ||
    f.ratingMin !== null ||
    f.pagesMin !== null ||
    f.pagesMax !== null ||
    f.language.trim() !== "" ||
    f.uploader.trim() !== ""
  );
}

export const homeFilter = writable<HomeFilterState>(emptyHomeFilter());

// ── Sort stores ──────────────────────────────────────────────────────────────

export type SortField = "rating" | "posted" | "pages" | "title";
export type SortScopeMode = "count" | "days";

export interface SortScope {
  mode: SortScopeMode;
  count: number;   // used when mode === "count"
  days: number;    // used when mode === "days"
  field: SortField;
  dir: "asc" | "desc";
}

export interface SortFetchProgress {
  fetching: boolean;
  loaded: number;
  target: number;
  estimatedSeconds: number;
  cancelled: boolean;
}

export function emptySortScope(): SortScope {
  return { mode: "count", count: 0, days: 30, field: "posted", dir: "desc" };
}

export function emptySortFetchProgress(): SortFetchProgress {
  return { fetching: false, loaded: 0, target: 0, estimatedSeconds: 0, cancelled: false };
}

// sortScope: what to sort (set by FilterPanel on "Sort" click; count default = current galleries.length)
export const sortScope = writable<SortScope>(emptySortScope());
// sortFetchProgress: progress of the pre-sort fetch phase
export const sortFetchProgress = writable<SortFetchProgress>(emptySortFetchProgress());
// sortActive: true = show sorted view (finite list), false = normal infinite scroll
export const sortActive = writable<boolean>(false);
// sortedGalleries: the final sorted+filtered snapshot, rendered when sortActive is true
export const sortedGalleries = writable<Gallery[]>([]);
