/**
 * The typed bridge to the Rust backend.
 *
 * Commands are defined once in `src-tauri/src/commands` and exported to
 * `src/bindings.ts` by `tauri-specta` (ADR-0002). This module is the only place the frontend
 * imports them, so:
 *
 *  - a command rename surfaces here and nowhere else;
 *  - running `npm run dev` in a plain browser degrades to a documented empty state instead of
 *    throwing — which is what makes the component gallery usable during design review.
 */

import type { LibrarySummary, AppInfo, LibraryHealth } from "@/bindings";

/** Reading status, mirroring `domain::enums::ReadingStatus`. */
export type ReadingStatus = "to_read" | "reading" | "finished" | "paused" | "abandoned";

/** The empty summary — every counter zero, every status present. */
export const EMPTY_SUMMARY: LibrarySummary = {
  books: 0,
  by_status: [],
  by_language: [],
  authors: 0,
  publishers: 0,
  categories: 0,
  tags: 0,
  quotes: 0,
  note_pages: 0,
  files: 0,
  shelves: 0,
  pages_total: 0,
  finished_in_year: 0,
  year: new Date().getUTCFullYear(),
  schema_version: 0,
};

/** `true` when the page is running inside the Tauri shell rather than a browser. */
export async function isTauri(): Promise<boolean> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    getCurrentWindow();
    return true;
  } catch {
    return false;
  }
}

/** Load the command table, or `null` when there is none (browser preview). */
async function commands(): Promise<typeof import("@/bindings").commands | null> {
  try {
    return (await import("@/bindings")).commands;
  } catch {
    return null;
  }
}

/**
 * Fetch the library summary.
 *
 * `year` selects the window for the "finished this year" counter. Outside Tauri this resolves
 * to [`EMPTY_SUMMARY`] so the dashboard renders its designed empty state rather than a spinner
 * that never resolves.
 */
export async function getLibrarySummary(year?: number): Promise<LibrarySummary> {
  const cmds = await commands();
  if (cmds === null) return EMPTY_SUMMARY;
  try {
    // `typedError` resolves with a discriminated union instead of rejecting, so a failed
    // command arrives as `{ status: "error" }` and must be narrowed here — the `catch` below
    // only covers native failures such as the invoke not existing outside Tauri.
    const res = await cmds.librarySummary(year ?? null);
    return res.status === "ok" ? res.data : EMPTY_SUMMARY;
  } catch {
    return EMPTY_SUMMARY;
  }
}

/** Fetch installation facts for Settings → About. */
export async function getAppInfo(): Promise<AppInfo | null> {
  const cmds = await commands();
  if (cmds === null) return null;
  try {
    const res = await cmds.appInfo();
    return res.status === "ok" ? res.data : null;
  } catch {
    return null;
  }
}

/** Run the storage health check. */
export async function getLibraryHealth(): Promise<LibraryHealth | null> {
  const cmds = await commands();
  if (cmds === null) return null;
  try {
    const res = await cmds.libraryHealth();
    return res.status === "ok" ? res.data : null;
  } catch {
    return null;
  }
}

/** Read one setting. */
export async function getSetting<T>(key: string): Promise<T | null> {
  const cmds = await commands();
  if (cmds === null) return null;
  try {
    const res = await cmds.settingsGet(key);
    // The wire type is `Json`; the caller knows the setting's own shape, which is why this
    // module is the documented home of the cast.
    return res.status === "ok" ? (res.data as unknown as T | null) : null;
  } catch {
    return null;
  }
}

/** Write one setting. */
export async function setSetting(key: string, value: unknown): Promise<boolean> {
  const cmds = await commands();
  if (cmds === null) return false;
  try {
    const res = await cmds.settingsSet(key, value as never);
    return res.status === "ok";
  } catch {
    return false;
  }
}

/** Count of books in a given status, falling back to 0. */
export function countFor(summary: LibrarySummary, status: ReadingStatus): number {
  return summary.by_status.find(([s]) => s === status)?.[1] ?? 0;
}
