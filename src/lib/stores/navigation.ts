import { writable } from "svelte/store";

export type NavPage =
  | "home"
  | "search"
  | "popular"
  | "favorites"
  | "watched"
  | "history"
  | "downloads"
  | "settings";

export const currentPage = writable<NavPage>("home");
export const sidebarCollapsed = writable<boolean>(false);
