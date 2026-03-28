import { readable, writable } from "svelte/store";

export type ViewMode = "cards" | "list";

export const viewMode = writable<ViewMode>("cards");
export const cardSize = writable<number>(165);

/** Detail panel expanded state (false = side panel, true = full content area). */
export const detailExpanded = writable<boolean>(false);

/** Detail panel preview thumbnail size in px (80–200). Default loaded from config. */
export const detailPreviewSize = writable<number>(120);

/** Color theme: "light" or "dark". Default: "light". Persisted via IPC. */
export type Theme = "light" | "dark";
export const theme = writable<Theme>("light");

/** Incremented when the local library needs a full refresh (after delete or download complete). */
export const libraryRefreshTick = writable<number>(0);

/**
 * True when running on iOS (WKWebView). Used to hide desktop-only features
 * (file pickers, custom library/cache paths) that are unsupported on iOS.
 * Detection via WKWebView's native User-Agent which always contains "iPhone" or "iPad".
 */
export const isIos = readable<boolean>(
  typeof navigator !== "undefined" &&
    /iPhone|iPad|iPod/.test(navigator.userAgent)
);
