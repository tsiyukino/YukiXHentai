import { writable } from "svelte/store";

export const isLoggedIn = writable<boolean>(false);
export const authLoading = writable<boolean>(false);
export const authMessage = writable<string>("");
