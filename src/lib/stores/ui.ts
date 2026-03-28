import { readable, writable, derived } from "svelte/store";

export type ViewMode = "cards" | "list";

/** Device class based on viewport width. Updated on window resize.
 *  Can be manually overridden (e.g. Settings layout preview).
 *  Call `.set(value)` to pin to a specific class; call `.auto()` to revert to auto-detection. */
export type DeviceClass = "phone" | "tablet" | "desktop";

function createDeviceClass() {
  function classify(w: number): DeviceClass {
    if (w < 600) return "phone";
    if (w < 1024) return "tablet";
    return "desktop";
  }

  const initial: DeviceClass =
    typeof window !== "undefined" ? classify(window.innerWidth) : "desktop";

  const { subscribe, set } = writable<DeviceClass>(initial);

  let pinned: DeviceClass | null = null;

  if (typeof window !== "undefined") {
    const handler = () => {
      if (pinned === null) set(classify(window.innerWidth));
    };
    window.addEventListener("resize", handler);
  }

  return {
    subscribe,
    /** Pin to a specific device class (disables auto-detection). */
    set(value: DeviceClass) {
      pinned = value;
      set(value);
    },
    /** Revert to auto-detection based on current window width. */
    auto() {
      pinned = null;
      if (typeof window !== "undefined") set(classify(window.innerWidth));
    },
    /** Whether a manual override is currently active. */
    get isPinned() { return pinned !== null; },
  };
}

export const deviceClass = createDeviceClass();

/** True when the mobile sidebar drawer is open (phone only). */
export const sidebarDrawerOpen = writable<boolean>(false);

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
