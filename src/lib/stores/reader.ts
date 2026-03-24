import { writable } from "svelte/store";
import type { GalleryPages } from "$lib/api/reader";
import type { Gallery } from "$lib/api/galleries";

/** Currently open gallery in the reader (null = reader closed). */
export const readerGallery = writable<GalleryPages | null>(null);

/** Current 0-based page index in the reader. */
export const readerPage = writable<number>(0);

/** Reader mode: "page" or "scroll". */
export const readerMode = writable<"page" | "scroll">("page");

/** Reading session ID (from start_reading_session). */
export const readerSessionId = writable<number | null>(null);

/** The Gallery (detail store type) that was open when the reader was launched.
 *  Used to navigate back to the detail page when closing the reader. */
export const readerSourceGallery = writable<Gallery | null>(null);
