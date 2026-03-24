import { writable, derived, get } from "svelte/store";
import en from "./en.json";
import zh from "./zh.json";
import ja from "./ja.json";

export type Locale = "en" | "zh" | "ja";

const locales: Record<Locale, Record<string, unknown>> = { en, zh, ja };

export const locale = writable<Locale>("en");

/**
 * Resolve a dotted key path from a nested object.
 * e.g. resolve(obj, "nav.home") -> obj.nav.home
 */
function resolve(obj: Record<string, unknown>, path: string): string {
  let current: unknown = obj;
  for (const key of path.split(".")) {
    if (current == null || typeof current !== "object") return path;
    current = (current as Record<string, unknown>)[key];
  }
  return typeof current === "string" ? current : path;
}

/**
 * Translate a key, with optional interpolation.
 * Usage: $t("gallery.count", { count: 42 })
 */
export const t = derived(locale, ($locale) => {
  const messages = locales[$locale] ?? locales.en;
  return (key: string, params?: Record<string, string | number>): string => {
    let result = resolve(messages as Record<string, unknown>, key);
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        result = result.replaceAll(`{${k}}`, String(v));
      }
    }
    return result;
  };
});

/** Available locale options for the language selector. */
export const localeOptions: { value: Locale; label: string }[] = [
  { value: "en", label: "English" },
  { value: "zh", label: "中文" },
  { value: "ja", label: "日本語" },
];
