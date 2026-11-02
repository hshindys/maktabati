<script lang="ts">
  import { Search } from "@lucide/svelte";

  import type { AppErrorDto, Book } from "@/bindings";

  import { listBooks } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { go } from "../router";
  import Modal from "./Modal.svelte";

  /**
   * SearchPalette — the global search behind the title-bar field and `Ctrl/Cmd+K`.
   *
   * Books only, filtered on the client over the list the shell already loads for the library
   * grid, because `bookList` is the whole search surface that exists today (server-side,
   * Arabic-normalised search is `crates/search`, still a placeholder). The list is fetched
   * once per session and reused across openings — the palette must not re-query per keystroke.
   *
   * Modal owns Escape, the backdrop and focus restoration, so this component is only the
   * input, the result rows and the keyboard cursor.
   */

  interface Props {
    open: boolean;
    onclose: () => void;
  }

  let { open, onclose }: Props = $props();

  let t = $derived(translatorFor($locale));

  /** `null` until the first load finishes; `[]` means "loaded, nothing to show". */
  let books = $state<Book[] | null>(null);
  /** Set when the shell has no backend (browser preview) — a designed state, not a fault. */
  let preview = $state(false);
  /** A real failure, surfaced verbatim so it is never mistaken for an empty library. */
  let loadError = $state<AppErrorDto | null>(null);

  let query = $state("");
  /** Keyboard cursor into `results`; driven by ↑/↓ and by hovering a row. */
  let index = $state(0);

  const LIMIT = 8;

  $effect(() => {
    if (!open) return;
    // Each opening starts from a clean query and cursor; the books themselves are kept.
    query = "";
    index = 0;
    if (books === null && loadError === null) void load();
  });

  async function load(): Promise<void> {
    const res = await listBooks();
    if (res.status === "ok") {
      books = res.data;
    } else if (res.error.code === "shell.unavailable") {
      preview = true;
      books = [];
    } else {
      loadError = res.error;
    }
  }

  let results = $derived.by(() => {
    const all = books ?? [];
    const q = query.trim().toLowerCase();
    if (q.length === 0) return all.slice(0, LIMIT);
    return all.filter((b) => b.title.toLowerCase().includes(q)).slice(0, LIMIT);
  });

  function choose(book: Book): void {
    go(`/book/${book.id}`);
    onclose();
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (results.length === 0) return;
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      index = (index + step + results.length) % results.length;
    } else if (e.key === "Enter") {
      const hit = results[index];
      // Enter on an empty result set must not navigate anywhere — there is no `hit` to open.
      if (hit) {
        e.preventDefault();
        choose(hit);
      }
    }
  }
</script>

<Modal {open} onclose={onclose} labelledBy="palette-title">
  <span id="palette-title" class="sr-only">{t("shell.search")}</span>

  <div class="flex items-center gap-[var(--sp-3)] border-b border-border-1 px-[var(--sp-4)] py-[var(--sp-3)]">
    <Search class="size-4 shrink-0 text-ink-3 no-mirror" strokeWidth={1.75} />
    <input
      type="text"
      class="h-7 w-full min-w-0 bg-transparent text-[var(--fs-base)] text-ink-1 outline-none placeholder:text-ink-3"
      placeholder={t("shell.search")}
      aria-label={t("shell.search")}
      role="combobox"
      aria-expanded={results.length > 0}
      aria-controls="palette-results"
      aria-autocomplete="list"
      aria-activedescendant={results.length > 0 ? `palette-option-${index}` : undefined}
      bind:value={query}
      oninput={() => (index = 0)}
      onkeydown={onKeydown}
    />
  </div>

  {#if loadError}
    <p class="px-[var(--sp-4)] py-[var(--sp-5)] text-[var(--fs-sm)] text-danger" role="alert">
      {loadError.message}
    </p>
  {:else if preview}
    <p class="px-[var(--sp-4)] py-[var(--sp-5)] text-[var(--fs-sm)] text-ink-3">
      {t("palette.unavailable")}
    </p>
  {:else if results.length === 0}
    <p class="px-[var(--sp-4)] py-[var(--sp-5)] text-[var(--fs-sm)] text-ink-3">
      {t("palette.noResults")}
    </p>
  {:else}
    <div id="palette-results" role="listbox" aria-label={t("shell.search")} class="max-h-[50vh] overflow-y-auto p-[var(--sp-2)]">
      {#each results as book, i (book.id)}
        <button
          type="button"
          id="palette-option-{i}"
          role="option"
          aria-selected={i === index}
          class="flex w-full items-center gap-[var(--sp-3)] rounded-[var(--r-md)] px-[var(--sp-3)] py-[var(--sp-2)] text-start transition-colors {i === index
            ? 'bg-surface-2 text-ink-1'
            : 'text-ink-2 hover:bg-surface-2'}"
          onpointerenter={() => (index = i)}
          onclick={() => choose(book)}
        >
          <!-- `min-w-0` lets the title shrink so the status never gets pushed out of the row. -->
          <span class="bidi-auto min-w-0 flex-1 truncate text-[var(--fs-base)]">{book.title}</span>
          <span class="shrink-0 text-[var(--fs-sm)] text-ink-3">{t(`status.${book.reading_status}`)}</span>
        </button>
      {/each}
    </div>
  {/if}
</Modal>
