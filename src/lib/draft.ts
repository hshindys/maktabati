/**
 * The add-book wizard's draft — what the user typed, kept across reopenings.
 *
 * Persisted to `localStorage` under its own key, same pattern and same rationale as
 * `settings.ts`: an accidental close must never cost a half-filled form. The draft is
 * cleared on a successful save and deliberately *kept* on cancel, because a cancel usually
 * means "not now", not "never". Entity picks are stored as `{ id, name }` so the wizard can
 * restore what was chosen even before the ids exist: `id: null` means "this name was typed
 * but not created yet" and is resolved against the backend at save time.
 */

const STORAGE_KEY = "maktabati.draft.book.v1";

/** A catalogue entry picked in a combobox — or typed but not yet created. */
export interface EntityRef {
  /** Existing entity id, or `null` when the name is new. */
  id: string | null;
  name: string;
}

/** Everything the wizard collects, as raw strings — parsing happens once, at save. */
export interface BookDraft {
  step: number;
  isbn: string;
  title: string;
  subtitle: string;
  authors: EntityRef[];
  publisher: EntityRef | null;
  series: EntityRef | null;
  seriesOrder: string;
  year: string;
  pages: string;
  languageCode: string;
  format: string;
  edition: string;
  categories: string[];
  tags: string[];
  status: string;
  rating: number;
  description: string;
}

export const EMPTY_DRAFT: BookDraft = {
  step: 1,
  isbn: "",
  title: "",
  subtitle: "",
  authors: [],
  publisher: null,
  series: null,
  seriesOrder: "",
  year: "",
  pages: "",
  languageCode: "ar",
  format: "",
  edition: "",
  categories: [],
  tags: [],
  status: "to_read",
  rating: 0,
  description: "",
};

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}

function strList(v: unknown): string[] {
  return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
}

function ref(v: unknown): EntityRef | null {
  if (typeof v !== "object" || v === null) return null;
  const o = v as Record<string, unknown>;
  const name = str(o.name).trim();
  if (!name) return null;
  return { id: typeof o.id === "string" && o.id ? o.id : null, name };
}

function refList(v: unknown): EntityRef[] {
  return Array.isArray(v) ? v.map(ref).filter((r): r is EntityRef => r !== null) : [];
}

function int(v: unknown, min: number, max: number): number {
  return typeof v === "number" && Number.isFinite(v) && v >= min && v <= max ? Math.round(v) : 0;
}

/**
 * Merge a persisted value over the empty draft, **discarding anything unrecognised**.
 *
 * A hand-edited or stale entry must degrade to a clean form rather than put the wizard
 * into a state with an unknown step or an out-of-range rating.
 */
export function reconcile(raw: unknown): BookDraft {
  const obj = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  const step = typeof obj.step === "number" && obj.step >= 1 && obj.step <= 4 ? obj.step : 1;
  return {
    step,
    isbn: str(obj.isbn),
    title: str(obj.title),
    subtitle: str(obj.subtitle),
    authors: refList(obj.authors),
    publisher: ref(obj.publisher),
    series: ref(obj.series),
    seriesOrder: str(obj.seriesOrder),
    year: str(obj.year),
    pages: str(obj.pages),
    languageCode: str(obj.languageCode) || EMPTY_DRAFT.languageCode,
    format: str(obj.format),
    edition: str(obj.edition),
    categories: strList(obj.categories),
    tags: strList(obj.tags),
    status: str(obj.status) || EMPTY_DRAFT.status,
    rating: int(obj.rating, 0, 10),
    description: str(obj.description),
  };
}

/** Read the persisted draft; anything unreadable degrades to a fresh form. */
export function loadDraft(): BookDraft {
  if (typeof localStorage === "undefined") return EMPTY_DRAFT;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return EMPTY_DRAFT;
    return reconcile(JSON.parse(raw));
  } catch {
    // Corrupt JSON, private mode, or a quota error — none of which may block the wizard.
    return EMPTY_DRAFT;
  }
}

/** Persist the current draft; persistence is a nicety, losing it costs a retype. */
export function saveDraft(draft: BookDraft): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(draft));
  } catch {
    // See above.
  }
}

/** Drop the draft — called after a successful save. */
export function clearDraft(): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    // See above.
  }
}
