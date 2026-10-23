/**
 * Goodreads → Maktabati field mapping, shared by the import screen.
 *
 * The commit loop in `views/Import.svelte` orchestrates the calls; the pure decisions
 * live here so they can be tested against the same fixtures the Rust parser's tests use
 * (`crates/metadata/src/goodreads.rs`).
 */

import { canonicalIsbn } from "./isbn";
import type { BookFormat, ReadingStatus } from "@/bindings";

/**
 * Goodreads' exclusive-shelf vocabulary → Maktabati reading status.
 * `read` collapses into `finished` because a completed read is the same record either
 * way; `to_read` and `unread` both mean "not started" to the tracker.
 */
export const STATUS_MAP: Record<string, ReadingStatus> = {
  read: "finished",
  reading: "reading",
  to_read: "to_read",
  unread: "to_read",
};

/** The status a book keeps when the shelf could not be named. */
export const DEFAULT_STATUS: ReadingStatus = "to_read";

/**
 * Goodreads `Binding` → book format — the mirror of
 * `metadata::goodreads::binding_to_format`. Several paper spellings are all "print"
 * to Maktabati (it keeps only the top-level format), and an empty column means no hint.
 */
export function bindingToFormat(binding: string | null): BookFormat | null {
  if (binding === null) return null;
  let value = binding.trim().toLowerCase();
  if (value.length === 0) return null;
  // "e-book" contains a hyphen that would break a naive `E-BOOK` upper-case compare.
  value = value.replace(/[-_]+/g, "");
  if (/audio|cd|cassette/.test(value)) return "audiobook";
  if (value.includes("pdf")) return "pdf";
  if (/kindle|nook|ebook|digital/.test(value)) return "ebook";
  return "print";
}

/**
 * Goodreads' 0..5 whole-star rating doubles into Maktabati's half-star units (0..10).
 * `0` and `null` are Goodreads' "unrated", not a zero opinion, so both stay `null`.
 */
export function ratingToHalfStars(stars: number | null): number | null {
  if (stars === null || stars <= 0) return null;
  return Math.min(10, Math.round(stars * 2));
}

/** Normalized ISBN, ready for the store-side comparison (Goodreads loves `="…"` cells). */
export function cleanIsbn(raw: string | null): string | null {
  if (raw === null) return null;
  // An ISBN is digits plus at most a final X, so everything else — spreadsheet
  // guard quotes, separators — disappears before canonicalising.
  const stripped = raw.replace(/[^\dXx]/g, "");
  const canonical = canonicalIsbn(stripped).toUpperCase();
  return canonical.length > 0 ? canonical : null;
}
