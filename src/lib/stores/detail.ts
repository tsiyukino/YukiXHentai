import { writable } from "svelte/store";
import type { Gallery } from "$lib/api/galleries";
import type { GalleryPageEntry } from "$lib/api/reader";

/** Gallery shown in the detail slide-in panel (null = closed). */
export const detailGallery = writable<Gallery | null>(null);

/** Shared page thumbnail paths for the currently open detail gallery.
 *  Values are raw local filesystem paths (not convertFileSrc'd).
 *  Reset when the detail gallery changes. */
export const detailPageThumbs = writable<{ gid: number; paths: Record<number, string> } | null>(null);

/** Shared batch-loading state for the currently open gallery.
 *  Lets the reader continue batch fetching from where the detail page left off,
 *  and write newly-fetched entries back so the detail page sees them on return.
 *  `fetchedDetailPages` is a Set (mutated in-place) — readers must NOT replace it.
 *  `pageEntries` is mutated in-place; both components read/write individual keys.
 *  `totalPageCount` is set once from the first batch HTML parse and never recalculated. */
export const detailBatchState = writable<{
  gid: number;
  token: string;
  showkey: string | null;
  pagesPerBatch: number;
  /** Total pages from the HTML parse (authoritative, never recalculated). */
  totalPageCount: number;
  /** Which detail pages (p=0,1,2…) have been fully fetched. Mutated in-place. */
  fetchedDetailPages: Set<number>;
  /** Sparse map of page_index → entry for all fetched pages. Mutated in-place. */
  pageEntries: Record<number, GalleryPageEntry>;
} | null>(null);
