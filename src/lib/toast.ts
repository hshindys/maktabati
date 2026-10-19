/**
 * Toasts — transient confirmations and recoverable errors.
 *
 * Placement is `bottom-inline-start` (not `bottom-left`) so an Arabic session puts them in the
 * bottom-right and the OS clock/edge gestures are never covered. Auto-dismiss is 4 s per
 * `docs/04-ux-design.md` §5.5, and an error toast stays until dismissed because it usually
 * carries an action the user needs to read.
 */

import { writable } from "svelte/store";

export type ToastTone = "success" | "error" | "warning" | "info";

export interface Toast {
  id: string;
  tone: ToastTone;
  message: string;
  /** Optional inline action, e.g. "Undo". */
  actionLabel?: string;
  /** Milliseconds before dismissal; `null` means "until dismissed". */
  durationMs: number | null;
}

const DEFAULT_DURATION_MS = 4000;
/** Never let one storm of failures flood the screen. */
const MAX_VISIBLE = 4;

let counter = 0;

function nextId(): string {
  counter += 1;
  return `toast-${counter}`;
}

export const toasts = writable<Toast[]>([]);

export interface ToastOptions {
  tone?: ToastTone;
  durationMs?: number | null;
  actionLabel?: string;
}

/** Show a toast. Returns its id so the caller can dismiss it early. */
export function pushToast(message: string, options: ToastOptions = {}): string {
  const { tone = "info", actionLabel, durationMs } = options;
  const id = nextId();
  const toast: Toast = {
    id,
    tone,
    message,
    ...(actionLabel === undefined ? {} : { actionLabel }),
    // An error carries information the user must act on, so it never auto-dismisses.
    durationMs: durationMs === undefined ? (tone === "error" ? null : DEFAULT_DURATION_MS) : durationMs,
  };

  toasts.update((list) => [...list, toast].slice(-MAX_VISIBLE));

  if (toast.durationMs !== null) {
    window.setTimeout(() => dismissToast(id), toast.durationMs);
  }
  return id;
}

/** Convenience wrappers, one per tone. */
export const toast = {
  success: (message: string, o: Omit<ToastOptions, "tone"> = {}) =>
    pushToast(message, { ...o, tone: "success" }),
  error: (message: string, o: Omit<ToastOptions, "tone"> = {}) =>
    pushToast(message, { ...o, tone: "error" }),
  warning: (message: string, o: Omit<ToastOptions, "tone"> = {}) =>
    pushToast(message, { ...o, tone: "warning" }),
  info: (message: string, o: Omit<ToastOptions, "tone"> = {}) =>
    pushToast(message, { ...o, tone: "info" }),
};

/** Remove one toast. */
export function dismissToast(id: string): void {
  toasts.update((list) => list.filter((t) => t.id !== id));
}

/** Remove every toast (used when the locale changes and messages are re-rendered). */
export function clearToasts(): void {
  toasts.set([]);
}

/** Show a `success` toast — the common case, kept short at call sites. */
export function notifySaved(): string {
  return pushToast("saved", { tone: "success" });
}
