/**
 * The add-book wizard's place in the shell.
 *
 * The wizard is a modal, not a route (docs/04-ux-design.md §4.5), so it cannot live in the
 * router table. A one-line store keeps that decision honest: any view (the dashboard's empty
 * state, the library's toolbar, a future book page) opens the same single modal, and the
 * `Ctrl+N` binding at the shell level goes through the same door.
 */

import { writable } from "svelte/store";

/** Whether the add-book modal is open. */
export const addBookOpen = writable(false);

/** Open the wizard (button, empty-state call to action, `Ctrl+N`). */
export function openAddBook(): void {
  addBookOpen.set(true);
}

/** Close it; the draft is kept, so closing is never a destructive act. */
export function closeAddBook(): void {
  addBookOpen.set(false);
}
