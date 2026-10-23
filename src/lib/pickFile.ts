/**
 * Picking files to attach to a book — M8, backed by `crates/files` + `repo::file`.
 *
 * Both the add-book wizard and the book page ask the same three questions about a path the
 * user chose: which `FileKind` it is, what to call it, and whether the library should copy
 * it or point at it. They live here rather than in either view because the answers are
 * facts about the file, not about the screen that asked.
 *
 * Outside a Tauri shell `open()` rejects — there is no dialog in a browser preview — so
 * `pickFiles` returns `null` and the caller leaves the list untouched. That mirrors how
 * `api.ts` reports `NO_SHELL`: a preview shows the empty state, never a fake row.
 */

import { open } from "@tauri-apps/plugin-dialog";

import type { FileKind } from "@/bindings";

/** A file the user chose, reduced to what `file_attach` needs. */
export interface PickedFile {
  path: string;
  kind: FileKind;
}

/** Book formats the library knows how to read, as extensions without the dot. */
const BOOK_EXTENSIONS = ["epub", "pdf", "mobi", "azw3", "azw", "txt", "djvu"];
/** Audio has its own `FileKind`; a separate filter keeps the picker honest about it. */
const AUDIO_EXTENSIONS = ["mp3", "m4a", "m4b", "aac", "ogg", "oga", "opus", "flac", "wav", "wma"];

/**
 * The extension decides the kind. An extension the library does not know becomes `other`
 * rather than being guessed at: a wrong kind is a wrong row in `book_files`, while `other`
 * is a row the user can see and correct.
 */
const KIND_BY_EXTENSION: Record<string, FileKind> = {
  epub: "epub",
  pdf: "pdf",
  mobi: "mobi",
  azw3: "azw3",
  azw: "azw3",
  txt: "txt",
  mp3: "audio",
  m4a: "audio",
  m4b: "audio",
  aac: "audio",
  ogg: "audio",
  oga: "audio",
  opus: "audio",
  flac: "audio",
  wav: "audio",
  wma: "audio",
};

/** The path with its directory part gone — what a list row should show. */
export function fileName(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return (cut >= 0 ? path.slice(cut + 1) : path).trim() || path;
}

/** Lower-cased extension without the dot, or `""` when the name has none. */
export function extensionOf(path: string): string {
  const name = fileName(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function kindFromPath(path: string): FileKind {
  return KIND_BY_EXTENSION[extensionOf(path)] ?? "other";
}

/**
 * Ask for one or more files.
 *
 * Returns `null` when the dialog was cancelled *or* there is no shell behind it — the two
 * are indistinguishable from here and both mean "nothing to attach", so the caller does not
 * invent an error for a user who simply closed a window.
 */
export async function pickFiles(
  title: string,
  filterLabels: { books: string; audio: string },
): Promise<PickedFile[] | null> {
  let picked: string | string[] | null;
  try {
    picked = await open({
      title,
      multiple: true,
      filters: [
        { name: filterLabels.books, extensions: BOOK_EXTENSIONS },
        { name: filterLabels.audio, extensions: AUDIO_EXTENSIONS },
      ],
    });
  } catch {
    return null;
  }
  if (picked === null) return null;
  const paths = Array.isArray(picked) ? picked : [picked];
  return paths.filter((p) => p.trim().length > 0).map((p) => ({ path: p, kind: kindFromPath(p) }));
}

/**
 * Ask for a single image, as cover picking does.
 *
 * `cover_set` validates the extension again in the repo — the filter pre-sorts what the
 * dialog shows, the repo still guards what it accepts. `null` again means "cancelled or
 * no shell": a user who dismissed the dialog sees nothing change, an error for closing
 * a window would be worse than silence.
 */
export async function pickImage(title: string, imageLabel: string): Promise<string | null> {
  try {
    const picked = await open({
      title,
      multiple: false,
      directory: false,
      filters: [{ name: imageLabel, extensions: ["jpg", "jpeg", "png", "webp"] }],
    });
    return typeof picked === "string" && picked.trim().length > 0 ? picked : null;
  } catch {
    return null;
  }
}
