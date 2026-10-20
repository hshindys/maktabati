/**
 * The one way an IPC error becomes text on screen.
 *
 * Every failure crossing IPC is an `AppErrorDto` with an `i18n_key` (ADR-0010); the
 * catalogue lookup only succeeds when the key is really in the shipped catalogue, and
 * falls back to the raw message otherwise — a backend-only code must degrade to
 * something readable, never to `undefined`.
 */

import { ar, type MessageKey } from "./i18n";
import type { AppErrorDto } from "@/bindings";

/** Localised text for an error, given the active translator. */
export function messageFor(error: AppErrorDto, t: (key: MessageKey) => string): string {
  const key = error.i18n_key;
  return key !== null && key in ar ? t(key as MessageKey) : error.message;
}
