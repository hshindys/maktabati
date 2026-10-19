/**
 * In-house i18n runtime (ADR-0009).
 *
 * Deliberately ~80 lines with no dependency: the catalogues are statically imported, so a
 * missing key is a TypeScript error instead of a runtime `undefined`, and there is no fetch,
 * no bundler plugin and no async flash of untranslated content.
 *
 * Arabic is the default locale and every string is written Arabic-first; English is the
 * translation, not the other way round.
 */

import { derived, type Readable } from "svelte/store";

import { ar, type MessageKey } from "./ar";
import { en } from "./en";
import { directionFor, preferences, type Locale } from "../settings";

export type { MessageKey };
export { ar, en };

const CATALOGS: Record<Locale, Record<MessageKey, string>> = { ar, en };

/** The active locale. */
export const locale: Readable<Locale> = derived(preferences, ($p) => $p.locale);

/** The document direction for the active locale. */
export const direction: Readable<"rtl" | "ltr"> = derived(locale, directionFor);

/**
 * Look up `key` in the active catalogue.
 *
 * `{placeholder}` segments are replaced from `params`. An unknown placeholder is left as-is so
 * a typo is visible in the UI rather than silently rendering as an empty gap.
 */
export function translate(
  catalog: Record<MessageKey, string>,
  key: MessageKey,
  params?: Record<string, string | number>,
): string {
  let text: string = catalog[key];
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.split(`{${name}}`).join(String(value));
    }
  }
  return text;
}

/**
 * A translator bound to one catalogue — passed into components so a helper function deep in a
 * tree can localize without every ancestor threading `locale` through its props.
 */
export type Translator = (key: MessageKey, params?: Record<string, string | number>) => string;

/** Build a translator for a specific locale (useful in tests). */
export function translatorFor(current: Locale): Translator {
  const catalog = CATALOGS[current];
  return (key, params) => translate(catalog, key, params);
}

/** Build a translator from an arbitrary catalog, for testing unknown-key behaviour. */
export function translatorFrom(catalog: Record<MessageKey, string>): Translator {
  return (key, params) => translate(catalog, key, params);
}

/**
 * The catalogue for a locale. Exported for the test that asserts the two files have identical
 * key sets — `en` is typed as `Record<MessageKey, string>` so *missing* keys are already a
 * compile error; this guards the other direction, an English key with no Arabic original.
 */
export function catalogFor(current: Locale): Record<MessageKey, string> {
  return CATALOGS[current];
}
