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

import type {
  AppErrorDto,
  AppInfo,
  AuthorInput,
  BookFile,
  BookInput,
  CategoryInput,
  FileKind,
  LanguageInput,
  LibraryHealth,
  LibrarySummary,
  PublisherInput,
  Quote,
  QuoteInput,
  QuoteWithBook,
  SeriesInput,
  StorageMode,
  TagInput,
  NoteBlockInput,
  NotePageInput,
} from "@/bindings";

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
let shellProbe: Promise<boolean> | null = null;
export function isTauri(): Promise<boolean> {
  shellProbe ??= (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow();
      return true;
    } catch {
      return false;
    }
  })();
  return shellProbe;
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
 * Outcome of a *read* command — the same discipline the write path already has.
 *
 * P0-5: these helpers used to collapse a backend error into `EMPTY_SUMMARY`/`null`, so a
 * corrupt, locked or missing database rendered exactly like a brand-new library: clean
 * empty state, zero counters, no log line. A failure now reaches the caller intact and
 * the view decides how to say it. The browser preview keeps its designed fallback — no
 * shell is not an error, it is a different runtime.
 */
export type ReadResult<T> = { status: "ok"; data: T } | { status: "error"; error: AppErrorDto };

/** The browser preview: no backend exists, so the empty value really is the answer. */
function preview<T>(data: T): ReadResult<T> {
  return { status: "ok", data };
}

/** A native invoke failure (no shell behind an existing table, IPC broken) as an error. */
function invokeFailure(cause: unknown): AppErrorDto {
  return {
    code: "shell.invoke_failed",
    message: cause instanceof Error ? cause.message : String(cause),
    i18n_key: "error.shell.invoke_failed",
    details: null,
  };
}

/**
 * Fetch the library summary.
 *
 * `year` selects the window for the "finished this year" counter. Outside the shell this
 * resolves to [`EMPTY_SUMMARY`] so the dashboard renders its designed empty state; inside
 * it, a failed command arrives as `{ status: "error" }` with the backend's `AppErrorDto`.
 */
export async function getLibrarySummary(year?: number): Promise<ReadResult<LibrarySummary>> {
  const cmds = await commands();
  if (cmds === null || !(await isTauri())) return preview(EMPTY_SUMMARY);
  try {
    const res = await cmds.librarySummary(year ?? null);
    return res.status === "ok" ? res : { status: "error", error: res.error };
  } catch (e) {
    return { status: "error", error: invokeFailure(e) };
  }
}

/** Fetch installation facts for Settings → About. */
export async function getAppInfo(): Promise<ReadResult<AppInfo | null>> {
  const cmds = await commands();
  if (cmds === null || !(await isTauri())) return preview(null);
  try {
    const res = await cmds.appInfo();
    return res.status === "ok" ? res : { status: "error", error: res.error };
  } catch (e) {
    return { status: "error", error: invokeFailure(e) };
  }
}

/** Run the storage health check. */
export async function getLibraryHealth(): Promise<ReadResult<LibraryHealth | null>> {
  const cmds = await commands();
  if (cmds === null || !(await isTauri())) return preview(null);
  try {
    const res = await cmds.libraryHealth();
    return res.status === "ok" ? res : { status: "error", error: res.error };
  } catch (e) {
    return { status: "error", error: invokeFailure(e) };
  }
}

/** Read one setting; `ok(null)` means the key has never been written. */
export async function getSetting<T>(key: string): Promise<ReadResult<T | null>> {
  const cmds = await commands();
  if (cmds === null || !(await isTauri())) return preview(null);
  try {
    const res = await cmds.settingsGet(key);
    // The wire type is `Json`; the caller knows the setting's own shape, which is why this
    // module is the documented home of the cast.
    return res.status === "ok"
      ? { status: "ok", data: res.data as unknown as T | null }
      : { status: "error", error: res.error };
  } catch (e) {
    return { status: "error", error: invokeFailure(e) };
  }
}

/** Write one setting. */
export async function setSetting(key: string, value: unknown): Promise<ReadResult<null>> {
  const cmds = await commands();
  if (cmds === null || !(await isTauri())) return preview(null);
  try {
    const res = await cmds.settingsSet(key, value as never);
    return res.status === "ok" ? { status: "ok", data: null } : { status: "error", error: res.error };
  } catch (e) {
    return { status: "error", error: invokeFailure(e) };
  }
}

/** Count of books in a given status, falling back to 0. */
export function countFor(summary: LibrarySummary, status: ReadingStatus): number {
  return summary.by_status.find(([s]) => s === status)?.[1] ?? 0;
}

// ── Catalogue CRUD (M3) ──────────────────────────────────────────────────────────────────
//
// Unlike the read helpers above, these never turn a failure into a fake success: the wizard
// prints field validation and duplicate warnings *from* the error, so an outcome has to reach
// the caller intact. Outside the shell there is no backend at all, which is an outcome too —
// `NO_SHELL`, never an empty OK that would pretend a write happened.

/** Result of a command exactly as `tauri-specta` shapes it: data on success, `AppErrorDto` on failure. */
export type Outcome<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: AppErrorDto };

/** What every catalogue call resolves to in a plain browser: nothing ran. */
const NO_SHELL: Outcome<never> = {
  status: "error",
  error: {
    code: "shell.unavailable",
    message: "not running inside the Tauri shell",
    i18n_key: "error.shell.unavailable",
    details: null,
  },
};

/**
 * Invoke one bindings command.
 *
 * `f` receives the command table so each call stays checked against the generated signature;
 * a native rejection — the `invoke` API itself being absent — maps to `NO_SHELL`.
 */
async function run<T>(
  f: (cmds: typeof import("@/bindings").commands) => Promise<Outcome<T>>,
): Promise<Outcome<T>> {
  const cmds = await commands();
  if (cmds === null) return NO_SHELL;
  try {
    return await f(cmds);
  } catch {
    return NO_SHELL;
  }
}

// ── Authors ───────────────────────────────────────────────────────────────────────────────

/** Create an author from the wizard's inline form. */
export const createAuthor = (input: AuthorInput) => run((c) => c.authorCreate(input));
/** Fetch one author by id. */
export const getAuthor = (id: string) => run((c) => c.authorGet(id));
/** List every author. */
export const listAuthors = () => run((c) => c.authorList());
/** Update an author's editable fields. */
export const updateAuthor = (id: string, input: AuthorInput) =>
  run((c) => c.authorUpdate(id, input));
/** Delete an author and its book credits; the books themselves remain. */
export const deleteAuthor = (id: string) => run((c) => c.authorDelete(id));
/** Merge `sourceId` into `targetId`: the credits move, the source row goes, the books stay. */
export const mergeAuthor = (sourceId: string, targetId: string) =>
  run((c) => c.authorMerge(sourceId, targetId));
/** Substring search over the normalised name, for autocomplete (20 rows unless `limit` says). */
export const searchAuthors = (query: string, limit?: number) =>
  run((c) => c.authorSearch(query, limit ?? null));

// ── Publishers ────────────────────────────────────────────────────────────────────────────

/** Create a publisher from the wizard's inline form. */
export const createPublisher = (input: PublisherInput) => run((c) => c.publisherCreate(input));
/** Fetch one publisher by id. */
export const getPublisher = (id: string) => run((c) => c.publisherGet(id));
/** List every publisher. */
export const listPublishers = () => run((c) => c.publisherList());
/** Update a publisher's editable fields. */
export const updatePublisher = (id: string, input: PublisherInput) =>
  run((c) => c.publisherUpdate(id, input));
/** Delete a publisher; books that credited it drop the link (`ON DELETE SET NULL`). */
export const deletePublisher = (id: string) => run((c) => c.publisherDelete(id));
/** Substring search over the normalised name, for autocomplete (20 rows unless `limit` says). */
export const searchPublishers = (query: string, limit?: number) =>
  run((c) => c.publisherSearch(query, limit ?? null));

// ── Series ────────────────────────────────────────────────────────────────────────────────

/** Create a series from the wizard's inline form. */
export const createSeries = (input: SeriesInput) => run((c) => c.seriesCreate(input));
/** Fetch one series by id. */
export const getSeries = (id: string) => run((c) => c.seriesGet(id));
/** List every series. */
export const listSeries = () => run((c) => c.seriesList());
/** Update a series' editable fields. */
export const updateSeries = (id: string, input: SeriesInput) =>
  run((c) => c.seriesUpdate(id, input));
/** Delete a series; books in it drop the link (`ON DELETE SET NULL`). */
export const deleteSeries = (id: string) => run((c) => c.seriesDelete(id));
/** Substring search over the normalised name, for autocomplete (20 rows unless `limit` says). */
export const searchSeries = (query: string, limit?: number) =>
  run((c) => c.seriesSearch(query, limit ?? null));

// ── Tags ──────────────────────────────────────────────────────────────────────────────────

/** Create a tag from the wizard's inline form. */
export const createTag = (input: TagInput) => run((c) => c.tagCreate(input));
/** Fetch one tag by id. */
export const getTag = (id: string) => run((c) => c.tagGet(id));
/** List every tag. */
export const listTags = () => run((c) => c.tagList());
/** Update a tag's editable fields. */
export const updateTag = (id: string, input: TagInput) => run((c) => c.tagUpdate(id, input));
/** Delete a tag; its book-tag links go with it. */
export const deleteTag = (id: string) => run((c) => c.tagDelete(id));
/** Substring search over the normalised name, for autocomplete (20 rows unless `limit` says). */
export const searchTags = (query: string, limit?: number) =>
  run((c) => c.tagSearch(query, limit ?? null));

// ── Languages ─────────────────────────────────────────────────────────────────────────────

/** Create a language; its `code` is the key and never changes. */
export const createLanguage = (input: LanguageInput) => run((c) => c.languageCreate(input));
/** Fetch one language by its code. */
export const getLanguage = (code: string) => run((c) => c.languageGet(code));
/** List every language, active or not. */
export const listLanguages = () => run((c) => c.languageList());
/** List only the active languages — the wizard's language picker. */
export const listActiveLanguages = () => run((c) => c.languageListActive());
/** Update a language's editable fields (never its `code`). */
export const updateLanguage = (code: string, input: LanguageInput) =>
  run((c) => c.languageUpdate(code, input));
/** Delete a language; fails while any book still uses it. */
export const deleteLanguage = (code: string) => run((c) => c.languageDelete(code));

// ── Categories (tree) ─────────────────────────────────────────────────────────────────────

/** Create a category under `parentId`, or at the root when it is omitted. */
export const createCategory = (input: CategoryInput, parentId?: string | null) =>
  run((c) => c.categoryCreate(input, parentId ?? null));
/** Fetch one category by id. */
export const getCategory = (id: string) => run((c) => c.categoryGet(id));
/** List the whole tree in path order. */
export const listCategories = () => run((c) => c.categoryList());
/** List a category's direct children (omitted parent = the roots). */
export const listCategoryChildren = (parentId?: string | null) =>
  run((c) => c.categoryChildren(parentId ?? null));
/** Update a category's name and colour; key, parent and position never change. */
export const updateCategory = (id: string, input: CategoryInput) =>
  run((c) => c.categoryUpdate(id, input));
/** Delete a category and its whole subtree, plus their book links. */
export const deleteCategory = (id: string) => run((c) => c.categoryDelete(id));

// ── Books ─────────────────────────────────────────────────────────────────────────────────

/** Create a book with its author credits, categories and tags. */
export const createBook = (input: BookInput) => run((c) => c.bookCreate(input));
/** Fetch one book's own row. */
export const getBook = (id: string) => run((c) => c.bookGet(id));
/** Fetch a book with the relations the per-book page renders, in one round trip. */
export const getBookDetail = (id: string) => run((c) => c.bookDetail(id));
/** List every book (filtering beyond this arrives with search, M4). */
export const listBooks = () => run((c) => c.bookList());
/** Update a book and its relations from an edited form. */
export const updateBook = (id: string, input: BookInput) =>
  run((c) => c.bookUpdate(id, input));
/** Delete a book with its cycles, links, notes, files and search row. */
export const deleteBook = (id: string) => run((c) => c.bookDelete(id));
/** Books whose canonical ISBN matches (either ISBN form is accepted). */
export const findBooksByIsbn = (isbn: string) => run((c) => c.bookFindByIsbn(isbn));
/**
 * Books that look like the one being saved — the save-time duplicate warning, which never
 * blocks the save. Pass `excludeId` while editing so the book does not match itself.
 */
export const findDuplicateBooks = (input: BookInput, excludeId?: string | null) =>
  run((c) => c.bookFindDuplicates(input, excludeId ?? null));

// ── Files (attachments) ──────────────────────────────────────────────────────────────────

/**
 * Attach a file to a book.
 *
 * `storageMode` is the whole design decision the wizard makes: `"copy"` lays the bytes down in
 * `<data_dir>/files/` so the library owns them (and refuses a second copy of identical bytes),
 * `"reference"` remembers the user's own path and copies nothing — so deleting it deletes nothing
 * either. `kind` only drives the picker's filter and the row's label.
 */
export const attachFile = (
  bookId: string,
  kind: FileKind,
  storageMode: StorageMode,
  sourcePath: string,
  mime?: string | null,
  pageCount?: number | null,
) =>
  run((c) =>
    c.fileAttach(bookId, kind, storageMode, sourcePath, mime ?? null, pageCount ?? null),
  );

/** Every attachment on a book, oldest first. */
export const listFiles = (bookId: string) => run((c) => c.fileList(bookId));

/** Remove an attachment. A copied blob survives while any other row shares its hash. */
export const deleteFile = (id: string) => run((c) => c.fileDelete(id));

/** Open an attachment with the system's default application. */
export const openFile = (id: string) => run((c) => c.fileOpen(id));

/** Reveal an attachment in the file manager. */
export const revealFile = (id: string) => run((c) => c.fileReveal(id));

/** Shape of one attachment row, for pages that render them. */
export type { BookFile, FileKind, StorageMode };

// ── Quotes (الاقتباسات) ──────────────────────────────────────────────────────────────────

/** Create a quote on a book. */
export const createQuote = (bookId: string, input: QuoteInput) =>
  run((c) => c.quoteCreate(bookId, input));
/** List a book's quotes, newest first. */
export const listBookQuotes = (bookId: string) => run((c) => c.quoteList(bookId));
/** List every quote in the library, with owning book title + first author. */
export const listAllQuotes = () => run((c) => c.quoteListAll());
/** Edit a quote (full input replace). */
export const updateQuote = (id: string, input: QuoteInput) =>
  run((c) => c.quoteUpdate(id, input));
/** Delete a quote. */
export const deleteQuote = (id: string) => run((c) => c.quoteDelete(id));

/** Shape of one quote row. */
export type { Quote, QuoteWithBook };
/** Highlight palette (reader palette, shared with quotes). */
export type QuoteColor = import("@/bindings").QuoteColor;

// ── Notes (الملاحظات) ─────────────────────────────────────────────────────────────────────

/** Create a note page (the `bookId`/`parentId` anchors bind at creation, never after). */
export const createNotePage = (input: NotePageInput) => run((c) => c.notePageCreate(input));
/** Fetch one note page by id. */
export const getNotePage = (id: string) => run((c) => c.notePageGet(id));
/** List note pages, pinned first then newest edit; a `bookId` narrows to one book. */
export const listNotePages = (bookId?: string | null) => run((c) => c.notePageList(bookId ?? null));
/** Edit a page's title, icon and pin; anchors and position are fixed. */
export const updateNotePage = (id: string, input: NotePageInput) =>
  run((c) => c.notePageUpdate(id, input));
/** Delete a page; nested pages and its blocks cascade with it. */
export const deleteNotePage = (id: string) => run((c) => c.notePageDelete(id));

/** List a page's blocks in content order. */
export const listNoteBlocks = (pageId: string) => run((c) => c.noteBlockList(pageId));
/** Insert a block after `afterBlockId` (`null` = first). */
export const insertNoteBlock = (pageId: string, afterBlockId: string | null, input: NoteBlockInput) =>
  run((c) => c.noteBlockInsert(pageId, afterBlockId, input));
/** Edit a block's kind or payload. */
export const updateNoteBlock = (blockId: string, input: NoteBlockInput) =>
  run((c) => c.noteBlockUpdate(blockId, input));
/** Move a block to sit after `afterBlockId` (`null` = first). */
export const moveNoteBlock = (blockId: string, afterBlockId: string | null) =>
  run((c) => c.noteBlockMove(blockId, afterBlockId));
/** Delete a block. */
export const deleteNoteBlock = (blockId: string) => run((c) => c.noteBlockDelete(blockId));

/** Shapes of the notes surface. */
export type { NotePage, NoteBlock, NoteBlockType } from "@/bindings";

// ── Storage: backup ──────────────────────────────────────────────────────────────────────

/** Write a verified SQLite copy into `backups/manual-<stamp>.db`; returns the absolute path. */
export const backupLibrary = () => run((c) => c.backupLibrary());

// ── Enrichment (Open Library suggestions) ────────────────────────────────────────────────

/**
 * Suggest metadata for an ISBN. **No writes** — the caller answers the
 * i18n-prompted import/merge/discard question before anything is applied.
 * `ok(null)` = Open Library has no such record (an honest miss, not an error).
 */
export const enrichByIsbn = (isbn: string) => run((c) => c.metadataEnrich(isbn));

/** Same, but by title (+ optional author) through Open Library's search index. */
export const enrichByTitleAuthor = (title: string, author: string) =>
  run((c) => c.metadataEnrichSearch(title, author));

// ── Hermes (loopback HTTP admin API) ─────────────────────────────────────────────────────

/** Start the loopback server (127.0.0.1:17321); errors surface `hermes.bind`. */
export const hermesStart = () => run((c) => c.hermesStart());
/** Stop the server; idempotent. */
export const hermesStop = () => run((c) => c.hermesStop());
/** Report the server state for the Settings card. */
export const hermesStatus = () => run((c) => c.hermesStatus());

/** Shape of a Hermes status answer. */
export type HermesStatus = import("@/bindings").HermesStatus;

// ── Book covers ──────────────────────────────────────────────────────────────────────────

/** Copy a picked image into the library and set it as the book's cover. */
export const setCover = (bookId: string, sourcePath: string) =>
  run((c) => c.coverSet(bookId, sourcePath));

/** Clear the cover and delete its copied bytes. */
export const removeCover = (bookId: string) => run((c) => c.coverRemove(bookId));

/** Absolute path of the cover (for `convertFileSrc`); `ok(null)` = no cover on disk. */
export const coverUrl = (bookId: string) => run((c) => c.coverUrl(bookId));

// ── Quick reading-status change ──────────────────────────────────────────────────────────

/** Change only the reading status — the "mark as read" tap on the book page. */
export const setBookStatus = (bookId: string, status: ReadingStatus) =>
  run((c) => c.bookStatusSet(bookId, status));

// ── Goodreads import ─────────────────────────────────────────────────────────────────────

/** Parse a Goodreads "Export Library" CSV into rows, entirely offline. */
export const parseGoodreads = (text: string) => run((c) => c.goodreadsParse(text));

/** Shape of one parsed Goodreads row. */
export type GoodreadsRow = import("@/bindings").GoodreadsRow;
