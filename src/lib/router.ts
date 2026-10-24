/**
 * Hash-based router.
 *
 * Hash routing is deliberate: `file://` and the Tauri custom protocol do not serve unknown
 * paths, so HTML5 history routing would 404 on a hard reload. The hash also gives browser-style
 * back/forward (Alt+←/→) for free, which is a design requirement.
 */

import { derived, writable, type Readable } from "svelte/store";

/** A parsed location. */
export interface RouteMatch {
  /** The registered pattern, e.g. `/book/:id`. */
  pattern: string;
  /** The concrete path, e.g. `/book/0193…`. */
  path: string;
  /** Captured segments from `:param` placeholders. */
  params: Record<string, string>;
}

/**
 * Every route the shell knows about.
 *
 * Order matters only for readability — patterns are matched most-specific-first by the number
 * of static segments, so `/book/:id` never shadows `/library`.
 */
export const ROUTES = [
  "/",
  "/library",
  "/reading",
  "/notes",
  "/quotes",
  "/authors",
  "/publishers",
  "/series",
  "/taxonomy",
  "/lending",
  "/goals",
  "/transfer",
  "/settings",
  "/settings/backup",
  "/settings/diagnostics",
  "/dev/gallery",
  "/book/:id",
  "/notes/:pageId",
] as const;

export type RoutePattern = (typeof ROUTES)[number];

/** Split a path into segments, ignoring empty ones. */
function segments(path: string): string[] {
  return path.split("/").filter((s) => s.length > 0);
}

/**
 * Match `path` against `pattern`.
 *
 * Only a single level of `:param` capture is supported — that is all the app needs, and a
 * glob/optional-segment grammar would be code with no user.
 */
export function match(pattern: string, path: string): Record<string, string> | null {
  const p = segments(pattern);
  const u = segments(path);
  if (p.length !== u.length) return null;
  const params: Record<string, string> = {};
  for (let i = 0; i < p.length; i += 1) {
    const pat = p[i];
    const val = u[i];
    if (pat.startsWith(":")) {
      if (val.length === 0) return null;
      params[pat.slice(1)] = decodeURIComponent(val);
    } else if (pat !== val) {
      return null;
    }
  }
  return params;
}

/** Score a pattern so the most specific one wins when several match. */
function specificity(pattern: string): number {
  return segments(pattern).reduce(
    (score, seg) => score + (seg.startsWith(":") ? 1 : 10),
    0,
  );
}

/** Resolve a path against the route table. Unmatched paths fall back to `/`. */
export function resolve(path: string): RouteMatch {
  const clean = path.startsWith("/") ? path : `/${path}`;
  const candidates = [...ROUTES]
    .sort((a, b) => specificity(b) - specificity(a))
    .filter((pattern) => match(pattern, clean) !== null);
  const pattern = candidates[0] ?? "/";
  return {
    pattern,
    path: clean,
    params: match(pattern, clean) ?? {},
  };
}

/** Read the current hash, always as an absolute path. */
export function currentPath(): string {
  if (typeof window === "undefined") return "/";
  const raw = window.location.hash.replace(/^#/, "");
  return raw.length > 0 ? raw : "/";
}

const pathStore = writable<string>(currentPath());

/** The live route, re-derived whenever the hash changes. */
export const route: Readable<RouteMatch> = derived(pathStore, resolve);

/** Navigate without adding a history entry. */
export function replace(path: string): void {
  window.location.replace(`#${path}`);
}

/** Navigate, adding a history entry so Back works. */
export function go(path: string): void {
  if (currentPath() === path) return;
  window.location.hash = path;
}

/** `true` when `path` is the active route (used for `aria-current`). */
export function isActive(pattern: string, current: string): boolean {
  if (pattern === "/") return current === "/";
  return current === pattern || current.startsWith(`${pattern}/`);
}

/**
 * The rail href a concrete path should light up.
 *
 * A detail page keeps its owning section lit instead of clearing the whole rail: opening a
 * book must not make المكتبة go dark, because a rail with nothing selected reads as "lost".
 * `/book/:id` is the one alias that has no `/book` route of its own; every other two-segment
 * path (`/settings/backup`) owns its first segment already.
 */
export function activeHref(current: string): string {
  if (current === "/") return "/";
  if (current.startsWith("/book/")) return "/library";
  const [head] = current.split("/").filter(Boolean);
  const parent = head === undefined ? "/" : `/${head}`;
  // `ROUTES` is a tuple, so its `includes` only accepts a route literal; widening to
  // `string` is the point of the check.
  return (ROUTES as readonly string[]).includes(parent) ? parent : "/";
}

// Keep the store in step with the browser's own back/forward buttons.
if (typeof window !== "undefined") {
  window.addEventListener("hashchange", () => {
    pathStore.set(currentPath());
  });
  // Normalise a cold load with no hash so `route` is never an empty match.
  if (window.location.hash.length === 0) {
    window.location.replace("#/");
  }
}
