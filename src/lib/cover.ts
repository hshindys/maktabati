/**
 * Resolving a book's cover image into something the webview can display — M9, backed by
 * `cover_set`/`cover_remove` and the asset protocol.
 *
 * Covers are stored as relative paths (`covers/<book_id>.<ext>`) under the data dir, the
 * same convention attachments in copy mode use. The webview cannot open a bare filesystem
 * path; it goes through Tauri's asset protocol, which `tauri.conf.json` scopes to
 * `$APPDATA/covers/*` and the CSP already allows as `asset:`. `convertFileSrc` builds that
 * URL, but only once we know the data dir — `app_info` tells us, one call per session.
 *
 * Outside a Tauri shell `getAppInfo` reports "no shell", so the cache stays `null` and every
 * caller falls back to the generated placeholder — a preview shows the honest miss, never a
 * broken image.
 */

import { convertFileSrc } from "@tauri-apps/api/core";

import { getAppInfo } from "./api";

let dataDir: Promise<string | null> | null = null;

/** The data directory, asked once and remembered; `null` when there is no shell. */
function getDataDir(): Promise<string | null> {
  dataDir ??= getAppInfo().then((res) => (res.status === "ok" ? res.data?.data_dir ?? null : null));
  return dataDir;
}

/**
 * An `src` attribute for the cover at `coverPath` (a row-relative `covers/…` path),
 * or `null` when it cannot be addressed in this session — the caller renders the
 * placeholder instead.
 */
export async function coverSrc(coverPath: string): Promise<string | null> {
  const dir = await getDataDir();
  if (dir === null) return null;
  // Forward slashes are valid element paths on Windows too; the asset protocol normalises.
  return convertFileSrc(`${dir}/${coverPath}`);
}
