import { invoke } from "@tauri-apps/api/core";
import type { Gallery } from "./galleries";

// ── Types ──────────────────────────────────────────────────────────────────

export interface FavoriteFolder {
  index: number;
  name: string;
  count: number;
}

export interface FavoriteStatus {
  gid: number;
  /** null = not favorited; 0–9 = folder index */
  favcat: number | null;
  favnote: string;
}

export interface FavoritesResult {
  galleries: Gallery[];
  folders: FavoriteFolder[];
  has_more: boolean;
  next_url: string | null;
}

// ── Folder colors (index → CSS color) ────────────────────────────────────

export const FOLDER_COLORS: Record<number, string> = {
  0: "#9e9e9e",
  1: "#fc4e4e",
  2: "#fcb417",
  3: "#dde500",
  4: "#17b91b",
  5: "#36b940",
  6: "#68c9de",
  7: "#5050d7",
  8: "#9755f5",
  9: "#fe93ff",
};

export function folderColor(index: number): string {
  return FOLDER_COLORS[index] ?? "#9e9e9e";
}

// ── API functions ──────────────────────────────────────────────────────────

export async function getFavoriteStatus(gid: number): Promise<FavoriteStatus> {
  return invoke("get_favorite_status", { gid });
}

export async function addFavorite(
  gid: number,
  token: string,
  favcat: number,
  favnote: string
): Promise<void> {
  return invoke("add_favorite", { gid, token, favcat, favnote });
}

export async function removeFavorite(gid: number, token: string): Promise<void> {
  return invoke("remove_favorite", { gid, token });
}

export async function fetchFavorites(
  favcat?: number | null,
  nextUrl?: string | null
): Promise<FavoritesResult> {
  return invoke("fetch_favorites", {
    favcat: favcat ?? null,
    nextUrl: nextUrl ?? null,
  });
}

export async function getFavoriteFolders(): Promise<FavoriteFolder[]> {
  return invoke("get_favorite_folders");
}
