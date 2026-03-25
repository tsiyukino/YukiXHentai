import { writable } from "svelte/store";
import type { Gallery } from "$lib/api/galleries";

/** Gallery currently open in the local detail panel (null = closed). */
export const localDetailGallery = writable<Gallery | null>(null);

/** The detail gallery that was open when the reader was launched (used to restore detail on reader close). */
export const localReaderSourceGallery = writable<Gallery | null>(null);

/** Gallery currently open in the local reader (null = reader closed). */
export const localReaderGallery = writable<LocalReaderGallery | null>(null);

/** Current page index in the local reader. */
export const localReaderPage = writable<number>(0);

/** Mode for the local reader. */
export const localReaderMode = writable<"page" | "scroll">("page");

export interface LocalReaderGallery {
  gid: number;
  title: string;
  pages: LocalReaderPage[];
  total_pages: number;
}

export interface LocalReaderPage {
  page_index: number;
  file_path: string;
}
