<script lang="ts">
  import { Search } from "@lucide/svelte";

  import type { AppErrorDto, SearchResult } from "@/bindings";

  import { searchLibrary } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { go } from "../router";
  import Modal from "./Modal.svelte";

  /**
   * SearchPalette — the global search behind the title-bar field and `Ctrl/Cmd+K`.
   *
   * Search is debounced and runs in Rust against the local FTS5 index; the webview never loads
   * the whole library just to filter it.
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

  /** Set when the shell has no backend (browser preview) — a designed state, not a fault. */
  let preview = $state(false);
  /** A real failure, surfaced verbatim so it is never mistaken for an empty library. */
  let searchError = $state<AppErrorDto | null>(null);

  let query = $state("");
  let results = $state<SearchResult[]>([]);
  let pending = $state(false);
  /** Keyboard cursor into `results`; driven by ↑/↓ and by hovering a row. */
  let index = $state(0);

  const LIMIT = 8;

  $effect(() => {
    if (!open) return;
    // Each opening starts from a clean query and cursor.
    query = "";
    index = 0;
  });

  let requestId = 0;
  $effect(() => {
    if (!open) return;
    const currentQuery = query;
    const currentId = ++requestId;
    pending = true;
    preview = false;
    searchError = null;
    const timer = setTimeout(() => {
      void searchLibrary(currentQuery, LIMIT).then((res) => {
        if (currentId !== requestId) return;
        if (res.status === "ok") {
          results = res.data;
        } else if (res.error.code === "shell.unavailable") {
          preview = true;
          results = [];
        } else {
          searchError = res.error;
          results = [];
        }
        pending = false;
      });
    }, 150);
    return () => {
      clearTimeout(timer);
      requestId += 1;
    };
  });

  function choose(result: SearchResult): void {
    if (result.kind === "book") go(`/book/${result.id}`);
    else if (result.kind === "note") go(`/notes/${result.id}`);
    else if (result.kind === "quote" && result.book_id) go(`/book/${result.book_id}`);
    else if (result.kind === "author") go("/authors");
    onclose();
  }

  function kindLabel(kind: SearchResult["kind"]): string {
    if (kind === "book") return t("palette.kind.book");
    if (kind === "note") return t("palette.kind.note");
    if (kind === "quote") return t("palette.kind.quote");
    return t("palette.kind.author");
  }

  function statusLabel(status: string): string {
    if (status === "to_read") return t("status.to_read");
    if (status === "reading") return t("status.reading");
    if (status === "finished") return t("status.finished");
    if (status === "paused") return t("status.paused");
    if (status === "abandoned") return t("status.abandoned");
    return status;
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

  {#if searchError}
    <p class="px-[var(--sp-4)] py-[var(--sp-5)] text-[var(--fs-sm)] text-danger" role="alert">
      {searchError.message}
    </p>
  {:else if preview}
    <p class="px-[var(--sp-4)] py-[var(--sp-5)] text-[var(--fs-sm)] text-ink-3">
      {t("palette.unavailable")}
    </p>
  {:else if pending}
    <p class="px-[var(--sp-4)] py-[var(--sp-5)] text-[var(--fs-sm)] text-ink-3" role="status">
      {t("palette.searching")}
    </p>
  {:else if results.length === 0}
    <p class="px-[var(--sp-4)] py-[var(--sp-5)] text-[var(--fs-sm)] text-ink-3">
      {t("palette.noResults")}
    </p>
  {:else}
    <div id="palette-results" role="listbox" aria-label={t("shell.search")} class="max-h-[50vh] overflow-y-auto p-[var(--sp-2)]">
      {#each results as result, i (`${result.kind}-${result.id}`)}
        <button
          type="button"
          id="palette-option-{i}"
          role="option"
          aria-selected={i === index}
          class="flex w-full items-center gap-[var(--sp-3)] rounded-[var(--r-md)] px-[var(--sp-3)] py-[var(--sp-2)] text-start transition-colors {i === index
            ? 'bg-surface-2 text-ink-1'
            : 'text-ink-2 hover:bg-surface-2'}"
          onpointerenter={() => (index = i)}
          onclick={() => choose(result)}
        >
          <!-- `min-w-0` lets the title shrink so the status never gets pushed out of the row. -->
          <span class="bidi-auto min-w-0 flex-1 truncate text-[var(--fs-base)]">
            {result.title}
            {#if result.detail}
              <span class="block truncate text-[var(--fs-sm)] text-ink-3">{result.detail}</span>
            {/if}
          </span>
          <span class="shrink-0 text-[var(--fs-sm)] text-ink-3">
            {#if result.status}
              {statusLabel(result.status)}
            {:else}
              {kindLabel(result.kind)}
            {/if}
          </span>
        </button>
      {/each}
    </div>
  {/if}
</Modal>
