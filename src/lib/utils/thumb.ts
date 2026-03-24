import { convertFileSrc } from "@tauri-apps/api/core";

/**
 * Convert a local thumbnail path to a URL the webview can display.
 * Returns empty string if no local path — remote URLs don't work in the
 * webview (no cookies). The component should show a skeleton placeholder
 * until the local path is available.
 */
export function thumbSrc(thumbPath: string | null, _thumbUrl: string): string {
  if (thumbPath) {
    return convertFileSrc(thumbPath);
  }
  return "";
}
