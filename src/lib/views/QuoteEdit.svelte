<script lang="ts">
  import Button from "../components/Button.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { bookHits, createQuote, searchLibrary, updateQuote } from "../api";
  import type { AppErrorDto, Quote, QuoteColor, QuoteInput, SearchResult } from "@/bindings";

  /**
   * Quote form — add or edit one passage (الاقتباس).
   *
   * Create happens on a book (a quote must name the book it was lifted from): from a book
   * page the book is given, from the global quotes screen the form searches the indexed
   * library and asks for one — never a dropdown over the whole catalogue. Edit happens
   * wherever the quote is rendered. The form submits a complete `QuoteInput`:
   * `quote_update` replaces the row scalars, so omitted fields would blank them.
   *
   * Every failure lands in the panel rather than a toast — one surface for one problem, like
   * the edit-wizard the modal mirrors.
   */

  interface Props {
    open: boolean;
    /** The book a *new* quote goes on; unused when editing an existing one. */
    bookId?: string | null;
    /** The quote to edit, or `null` when creating. */
    quote?: Quote | null;
    onclose: () => void;
    /** Called after a successful save so the caller can reload its data. */
    onsaved: () => void;
  }

  let {
    open,
    bookId = null,
    quote = null,
    onclose,
    onsaved,
  }: Props = $props();

  let t = $derived(translatorFor($locale));

  /** The reader highlight palette — the six values the backend validates (domain::QuoteColor). */
  const COLORS: QuoteColor[] = ["yellow", "green", "blue", "pink", "red", "purple"];

  let text = $state("");
  let note = $state("");
  let page = $state("");
  let chapter = $state("");
  let color = $state<QuoteColor>("yellow");

  let saving = $state(false);
  let error = $state<string | null>(null);

  // ── Book picker (only when creating without a given book) ───────────────────────

  /**
   * Creating without a book in hand — the global quotes screen — asks for one; a book page
   * passes its book and editing already has one, so neither shows a picker.
   */
  let pickMode = $derived(open && quote === null && (bookId ?? "").length === 0);

  let pickedBook = $state<SearchResult | null>(null);
  let bookQuery = $state("");
  let bookResults = $state<SearchResult[]>([]);
  let bookPending = $state(false);
  /** A real failure, surfaced so an empty list is never mistaken for "no such book". */
  let bookSearchError = $state<AppErrorDto | null>(null);
  /** Keyboard cursor into `bookResults`; driven by ↑/↓ and by hovering a row. */
  let bookIndex = $state(0);

  const BOOK_LIMIT = 8;

  // Seed the fields whenever the dialog opens — creating or reopening lands the same place.
  $effect(() => {
    if (!open) return;
    text = quote?.text ?? "";
    note = quote?.note ?? "";
    page = quote?.page === null || quote?.page === undefined ? "" : String(quote?.page);
    chapter = quote?.chapter ?? "";
    color = quote?.color ?? "yellow";
    error = null;
    pickedBook = null;
    bookQuery = "";
    bookResults = [];
    bookIndex = 0;
    bookSearchError = null;
  });

  /** Debounced search over the FTS index — the webview never loads the whole library. */
  let bookRequestId = 0;
  $effect(() => {
    if (!pickMode) return;
    const currentQuery = bookQuery;
    const currentId = ++bookRequestId;
    bookPending = true;
    bookSearchError = null;
    const timer = setTimeout(() => {
      void searchLibrary(currentQuery, BOOK_LIMIT).then((res) => {
        if (currentId !== bookRequestId) return;
        if (res.status === "ok") {
          // Only books may own a quote — `search` mixes kinds on purpose.
          bookResults = bookHits(res.data);
        } else {
          bookResults = [];
          bookSearchError = res.error.code === "shell.unavailable" ? null : res.error;
        }
        bookPending = false;
      });
    }, 150);
    return () => {
      clearTimeout(timer);
      bookRequestId += 1;
    };
  });

  function chooseBook(result: SearchResult): void {
    pickedBook = result;
    bookQuery = "";
    bookResults = [];
    bookIndex = 0;
    error = null;
  }

  function clearBook(): void {
    pickedBook = null;
    bookIndex = 0;
  }

  function onBookKeydown(e: KeyboardEvent): void {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (bookResults.length === 0) return;
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      bookIndex = (bookIndex + step + bookResults.length) % bookResults.length;
    } else if (e.key === "Enter") {
      // Enter picks the highlighted book; it must never submit the form mid-search.
      e.preventDefault();
      const hit = bookResults[bookIndex];
      if (hit) chooseBook(hit);
    }
  }

  /** The page field accepts one to five digits; the backend clamps the rest. */
  function parsePage(): number | null {
    const trimmed = page.trim();
    if (trimmed.length === 0) return null;
    const value = Number.parseInt(trimmed, 10);
    if (!Number.isFinite(value) || value < 0) return null;
    return value;
  }

  async function save(): Promise<void> {
    if (saving) return;
    if (text.trim().length === 0) {
      error = t("quote.error.emptyText");
      return;
    }
    const targetBook = quote?.book_id ?? bookId ?? pickedBook?.id ?? null;
    if (targetBook === null || targetBook.length === 0) {
      // Unreachable through the UI: both create paths name a book (given, or picked above).
      // Says what is wrong instead of a silent no-op if a future caller forgets.
      error = t("quote.error.noBook");
      return;
    }
    saving = true;
    error = null;
    const input: QuoteInput = {
      text: text.trim(),
      note: note.trim().length === 0 ? null : note.trim(),
      page: parsePage(),
      chapter: chapter.trim().length === 0 ? null : chapter.trim(),
      color,
    };
    const res =
      quote === null
        ? await createQuote(targetBook, input)
        : await updateQuote(quote.id, input);
    saving = false;
    if (res.status === "ok") {
      onsaved();
      onclose();
      return;
    }
    error = messageFor(res.error, t);
  }
</script>

<Modal {open} onclose={() => (saving ? undefined : onclose())} labelledBy="quote-form-title">
  <h2 id="quote-form-title" class="border-b border-border-1 px-6 py-4 text-[var(--fs-lg)] font-semibold text-ink-1">
    {quote === null ? t("quote.form.add") : t("quote.form.edit")}
  </h2>

  <form class="flex flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5" onsubmit={(e) => { e.preventDefault(); void save(); }}>
    {#if pickMode}
      <!-- The book a quote belongs to, chosen through the index — see `pickMode`. -->
      <Field
        label={t("quote.form.book")}
        for={pickedBook === null ? "quote-book-search" : undefined}
      >
        {#if pickedBook !== null}
          <span class="flex items-center gap-[var(--sp-2)] rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] py-2">
            <span class="bidi-auto min-w-0 flex-1 truncate text-[var(--fs-base)] text-ink-1" dir="auto">
              {pickedBook.title}
              {#if pickedBook.detail !== null}
                <span class="block truncate text-[var(--fs-sm)] text-ink-3">{pickedBook.detail}</span>
              {/if}
            </span>
            <button
              type="button"
              class="shrink-0 rounded-[var(--r-md)] text-[var(--fs-sm)] text-accent transition-colors hover:underline"
              onclick={clearBook}
            >
              {t("quote.form.bookChange")}
            </button>
          </span>
        {:else}
          <span class="flex flex-col gap-[var(--sp-2)]">
            <input
              id="quote-book-search"
              type="text"
              bind:value={bookQuery}
              autocomplete="off"
              role="combobox"
              aria-expanded={bookResults.length > 0}
              aria-controls="quote-book-results"
              aria-autocomplete="list"
              aria-activedescendant={bookResults.length > 0 ? `quote-book-option-${bookIndex}` : undefined}
              placeholder={t("quote.form.bookSearch")}
              onkeydown={onBookKeydown}
              class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
            />
            {#if bookSearchError !== null}
              <p role="alert" class="bidi-auto text-[var(--fs-sm)] text-danger" dir="auto">{bookSearchError.message}</p>
            {:else if bookPending}
              <p role="status" class="text-[var(--fs-sm)] text-ink-3">{t("palette.searching")}</p>
            {:else if bookResults.length === 0}
              <p class="text-[var(--fs-sm)] text-ink-3">{t("palette.noResults")}</p>
            {:else}
              <ul id="quote-book-results" role="listbox" aria-label={t("quote.form.book")} class="flex max-h-48 flex-col gap-0.5 overflow-y-auto">
                {#each bookResults as result, i (`${result.kind}-${result.id}`)}
                  <li>
                    <button
                      type="button"
                      id="quote-book-option-{i}"
                      role="option"
                      aria-selected={i === bookIndex}
                      class="flex w-full items-center gap-[var(--sp-3)] rounded-[var(--r-md)] px-[var(--sp-3)] py-[var(--sp-2)] text-start transition-colors {i === bookIndex
                        ? 'bg-surface-2 text-ink-1'
                        : 'text-ink-2 hover:bg-surface-2'}"
                      onpointerenter={() => (bookIndex = i)}
                      onclick={() => chooseBook(result)}
                    >
                      <span class="bidi-auto min-w-0 flex-1 truncate text-[var(--fs-base)]" dir="auto">
                        {result.title}
                        {#if result.detail !== null}
                          <span class="block truncate text-[var(--fs-sm)] text-ink-3">{result.detail}</span>
                        {/if}
                      </span>
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </span>
        {/if}
      </Field>
    {/if}

    <Field label={t("quote.text")} for="quote-text" hint={t("quote.text.hint")}>
      <textarea
        id="quote-text"
        bind:value={text}
        rows={4}
        maxlength={8000}
        required
        class="w-full resize-y rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] py-[var(--sp-2)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      ></textarea>
    </Field>

    <div class="grid grid-cols-1 gap-[var(--sp-4)] sm:grid-cols-3">
      <Field label={t("quote.page")} for="quote-page">
        <input
          id="quote-page"
          type="text"
          inputmode="numeric"
          bind:value={page}
          class="tnum bidi-ltr h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        />
      </Field>
      <Field label={t("quote.chapter")} for="quote-chapter">
        <input
          id="quote-chapter"
          type="text"
          bind:value={chapter}
          class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        />
      </Field>
      <Field label={t("quote.color")} for="quote-color">
        <select
          id="quote-color"
          bind:value={color}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        >
          {#each COLORS as c (c)}
            <option value={c}>{t(`quote.color.${c}`)}</option>
          {/each}
        </select>
      </Field>
    </div>

    <Field label={t("quote.note")} for="quote-note" hint={t("quote.note.hint")}>
      <textarea
        id="quote-note"
        bind:value={note}
        rows={2}
        maxlength={2000}
        class="w-full resize-y rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] py-[var(--sp-2)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      ></textarea>
    </Field>

    {#if error !== null}
      <p role="alert" class="text-[var(--fs-base)] text-danger" dir="auto">{error}</p>
    {/if}

    <footer class="flex items-center justify-end gap-[var(--sp-3)] border-t border-border-1 pt-4">
      <Button variant="ghost" onclick={onclose} disabled={saving}>{t("common.cancel")}</Button>
      <Button type="submit" variant="primary" loading={saving}>
        {quote === null ? t("common.save") : t("common.save")}
      </Button>
    </footer>
  </form>
</Modal>
