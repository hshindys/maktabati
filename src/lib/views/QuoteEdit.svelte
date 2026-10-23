<script lang="ts">
  import Button from "../components/Button.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { createQuote, updateQuote } from "../api";
  import type { Quote, QuoteColor, QuoteInput } from "@/bindings";

  /**
   * Quote form — add or edit one passage (الاقتباس).
   *
   * Create happens on a book (a quote must name the book it was lifted from), edit happens
   * wherever the quote is rendered. The form submits a complete `QuoteInput`: `quote_update`
   * replaces the row scalars, so omitted fields would blank them.
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

  // Seed the fields whenever the dialog opens — creating or reopening lands the same place.
  $effect(() => {
    if (!open) return;
    text = quote?.text ?? "";
    note = quote?.note ?? "";
    page = quote?.page === null || quote?.page === undefined ? "" : String(quote?.page);
    chapter = quote?.chapter ?? "";
    color = quote?.color ?? "yellow";
    error = null;
  });

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
    const targetBook = quote?.book_id ?? bookId;
    if (targetBook === null || targetBook.length === 0) {
      // Unreachable through the UI: both create paths name a book. Says what is wrong
      // instead of a silent no-op if a future caller forgets.
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
