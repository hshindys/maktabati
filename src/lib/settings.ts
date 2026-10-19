/**
 * User preferences: locale, direction, theme, accent, numeral system and calendar.
 *
 * Persisted to `localStorage` for now so the UI is fully functional before the backend settings
 * command lands; the surface is deliberately a plain `subscribe`/`update` pair so swapping in
 * `settings_set` from `bindings.ts` is a change inside this module only.
 */

import { writable, type Readable } from "svelte/store";
import type { CalendarStyle, DigitStyle } from "./format";

// Re-exported so views can name the settings' value types without importing `format` directly.
export type { CalendarStyle, DigitStyle } from "./format";

/** UI locale. Arabic is the default — this is an Arabic-first app. */
export type Locale = "ar" | "en";

/** Visual theme. `system` follows the OS and resolves to light or dark. */
export type ThemeChoice = "light" | "dark" | "sepia" | "system";

/** Accent presets — docs/04-ux-design.md §5.1. */
export type Accent = "green" | "indigo" | "plum" | "terracotta" | "ocean" | "gold";

export const ACCENTS: Accent[] = ["green", "indigo", "plum", "terracotta", "ocean", "gold"];
export const LOCALES: Locale[] = ["ar", "en"];
export const THEME_CHOICES: ThemeChoice[] = ["light", "dark", "sepia", "system"];

/** Everything the user can change from Settings. */
export interface Preferences {
  locale: Locale;
  theme: ThemeChoice;
  accent: Accent;
  digits: DigitStyle;
  calendar: CalendarStyle;
  sidebarCollapsed: boolean;
}

export const DEFAULT_PREFERENCES: Preferences = {
  locale: "ar",
  theme: "light",
  accent: "green",
  digits: "arabic-indic",
  calendar: "both",
  sidebarCollapsed: false,
};

const STORAGE_KEY = "maktabati.preferences.v1";

/** `rtl` for Arabic, `ltr` for everything else. */
export function directionFor(locale: Locale): "rtl" | "ltr" {
  return locale === "ar" ? "rtl" : "ltr";
}

function isLocale(v: unknown): v is Locale {
  return v === "ar" || v === "en";
}
function isTheme(v: unknown): v is ThemeChoice {
  return v === "light" || v === "dark" || v === "sepia" || v === "system";
}
function isAccent(v: unknown): v is Accent {
  return typeof v === "string" && (ACCENTS as string[]).includes(v);
}
function isDigits(v: unknown): v is DigitStyle {
  return v === "western" || v === "arabic-indic";
}
function isCalendar(v: unknown): v is CalendarStyle {
  return v === "gregorian" || v === "hijri" || v === "both";
}

/**
 * Merge persisted values over the defaults, **discarding anything unrecognised**.
 *
 * A corrupt or hand-edited entry must degrade to a known-good preference rather than put the
 * app into a state with no theme or an unknown direction.
 */
export function reconcile(
  raw: unknown,
  base: Preferences = DEFAULT_PREFERENCES,
): Preferences {
  const obj = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  return {
    locale: isLocale(obj.locale) ? obj.locale : base.locale,
    theme: isTheme(obj.theme) ? obj.theme : base.theme,
    accent: isAccent(obj.accent) ? obj.accent : base.accent,
    digits: isDigits(obj.digits) ? obj.digits : base.digits,
    calendar: isCalendar(obj.calendar) ? obj.calendar : base.calendar,
    sidebarCollapsed:
      typeof obj.sidebarCollapsed === "boolean" ? obj.sidebarCollapsed : base.sidebarCollapsed,
  };
}

function load(): Preferences {
  if (typeof localStorage === "undefined") return DEFAULT_PREFERENCES;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return DEFAULT_PREFERENCES;
    return reconcile(JSON.parse(raw));
  } catch {
    // Corrupt JSON, private mode, or a storage quota error — none of which should stop the
    // app from opening.
    return DEFAULT_PREFERENCES;
  }
}

function persist(prefs: Preferences): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // Persistence is a nicety; losing it costs a reset, not data.
  }
}

const state = writable<Preferences>(load());

/** Subscribe to the whole preference object. */
export const preferences: Readable<Preferences> = { subscribe: state.subscribe };

/** Apply a partial update. */
export function setPreferences(patch: Partial<Preferences>): void {
  state.update((current) => {
    const next = { ...current, ...patch };
    persist(next);
    applyToDocument(next);
    return next;
  });
}

/** Reset to factory defaults (used by onboarding's "start over"). */
export function resetPreferences(): void {
  state.update(() => {
    persist(DEFAULT_PREFERENCES);
    applyToDocument(DEFAULT_PREFERENCES);
    return DEFAULT_PREFERENCES;
  });
}

/**
 * Resolve `system` against the OS preference.
 *
 * Kept separate from the stored value so the Settings screen can show "System" as the choice
 * while the document still gets a concrete `light` or `dark`.
 */
export function resolveTheme(choice: ThemeChoice): "light" | "dark" | "sepia" {
  if (choice !== "system") return choice;
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/**
 * Mirror the preferences onto `<html>`.
 *
 * Everything in `app.css` keys off `data-theme` / `data-accent` / `lang` / `dir`, so this one
 * call is what makes a theme or language switch instant with no reload.
 */
export function applyToDocument(prefs: Preferences): void {
  if (typeof document === "undefined") return;
  const html = document.documentElement;
  html.dataset.theme = resolveTheme(prefs.theme);
  html.dataset.accent = prefs.accent;
  html.lang = prefs.locale;
  html.dir = directionFor(prefs.locale);
}

// Keep `system` in step with the OS while the app is open.
if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  media.addEventListener("change", () => {
    state.update((current) => {
      if (current.theme === "system") applyToDocument(current);
      return current;
    });
  });
}

// Apply once at import time so the shell never paints with the wrong theme.
applyToDocument(load());
