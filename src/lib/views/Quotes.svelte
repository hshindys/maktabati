<script lang="ts">
  import { Pencil, Quote as QuoteIcon, Trash2, TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import { deleteQuote, listAllQuotes } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { formatDate, formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { toast } from "../toast";
  import QuoteEdit from "./QuoteEdit.svelte";
  import type { AppErrorDto, Quote, QuoteWithBook } from "@/bindings";
  import type { QuoteColor } from "@/bindings";

  /**
   * Quotes screen — every passage lifted from every book, newest first.
   *
   * `quote_list_all` does the join server-side (title + first author), so the screen never
   * queries per row. A quote must name the book it came from — that is the id the backend
   * stores — so creation happens on a book page, and this screen is deliberately read-only
   * + edit/delete: an "add" button here would have to invent a book.
   */

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  const num = (v: number) => formatNumber(v, { digits });
  const dateOf = (ms: number) => formatDate(ms, $locale, $preferences.calendar, digits);

  let quotes = $state<QuoteWithBook[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let working = $state(false);

  async function load(): Promise<void> {
    loading = true;
    const res = await listAllQuotes();
    if (res.status === "ok") {
      quotes = res.data;
      loadError = null;
    } else if (res.error.code === "shell.unavailable") {
      // Browser preview: no backend, so the designed empty state is the honest one.
      quotes = [];
      loadError = null;
    } else {
      quotes = [];
      loadError = res.error;
    }
    loading = false;
  }

  onMount(() => void load());

  // ── Edit + delete ─────────────────────────────────────────────────────────

  let editing = $state<Quote | null>(null);
  let editorOpen = $state(false);

  function openEditor(row: Quote): void {
    editing = row;
    editorOpen = true;
  }

  async function removeQuote(row: QuoteWithBook): Promise<void> {
    if (working) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("quotes.deleteConfirm"), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deleteQuote(row.quote.id);
      toast[res.status === "ok" ? "success" : "error"](
        res.status === "ok" ? t("toast.quoteDeleted") : messageFor(res.error, t),
      );
      await load();
    } finally {
      working = false;
    }
  }

  /** Highlight dots — same six classes as the book page tab (see app.css `@theme`). */
  const COLOR_CLASS: Record<QuoteColor, string> = {
    yellow: "bg-quote-yellow",
    green: "bg-quote-green",
    blue: "bg-quote-blue",
    pink: "bg-quote-pink",
    red: "bg-quote-red",
    purple: "bg-quote-purple",
  };
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-4)] px-[var(--sp-6)] py-[var(--sp-5)]">
  <header class="flex items-baseline gap-[var(--sp-3)]">
    <h1 class="text-[var(--fs-xl)] font-semibold text-ink-1">{t("quotes.title")}</h1>
    {#if quotes.length > 0}
      <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
        {num(quotes.length)} {t("quotes.count")}
      </span>
    {/if}
  </header>

  {#if loading}
    <div class="flex flex-col gap-[var(--sp-3)]">
      {#each [0, 1, 2] as i (i)}
        <div class="h-24 animate-pulse rounded-[var(--r-lg)] border border-border-1 bg-surface-2"></div>
      {/each}
    </div>
  {:else if loadError}
    <Card>
      <div role="alert" class="flex flex-col items-start gap-[var(--sp-3)]">
        <div class="flex items-center gap-2 text-[var(--fs-md)] font-semibold text-danger">
          <TriangleAlert class="size-5 no-mirror" strokeWidth={2} />
          {t("quotes.loadFailed")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else if quotes.length === 0}
    <!-- Teaching empty state: the only place a quote is born is a book page, so point there. -->
    <Card>
      <EmptyState title={t("quotes.empty.title")} body={t("quotes.empty.body")} />
    </Card>
  {:else}
    <ul class="flex flex-col gap-[var(--sp-3)]">
      {#each quotes as row (row.quote.id)}
        <li>
          <Card>
            <span class="flex items-start gap-[var(--sp-3)]">
              <span class="mt-1.5 size-3 shrink-0 rounded-full {COLOR_CLASS[row.quote.color]}" aria-hidden="true"></span>
              <span class="flex min-w-0 flex-1 flex-col gap-[var(--sp-2)]">
                <blockquote class="bidi-auto text-[var(--fs-base)] leading-relaxed text-ink-1" dir="auto">
                  {row.quote.text}
                </blockquote>
                {#if row.quote.note !== null}
                  <p class="bidi-auto text-[var(--fs-sm)] text-ink-2" dir="auto">{row.quote.note}</p>
                {/if}
                <span class="flex flex-wrap items-center gap-[var(--sp-2)] text-[var(--fs-sm)] text-ink-3">
                  {#if row.quote.page !== null}
                    <span class="bidi-ltr tnum">{num(row.quote.page)}</span>
                  {/if}
                  {#if row.quote.chapter !== null}
                    <span class="bidi-auto" dir="auto">{row.quote.chapter}</span>
                  {/if}
                  {#if row.quote.page !== null || row.quote.chapter !== null}
                    <span aria-hidden="true">·</span>
                  {/if}
                  <!-- The owning book is the one fact only this screen can show — it is a link. -->
                  <QuoteIcon class="size-3.5 no-mirror" strokeWidth={1.75} />
                  <a
                    href="#/book/{row.quote.book_id}"
                    class="bidi-auto text-ink-2 transition-colors hover:text-accent hover:underline"
                    dir="auto"
                  >
                    {row.book_title}{row.book_author === null ? "" : ` — ${row.book_author}`}
                  </a>
                  <span aria-hidden="true">·</span>
                  <span>{dateOf(row.quote.created_at)}</span>

                  <span class="ms-auto flex items-center gap-1">
                    <button
                      type="button"
                      aria-label={t("common.edit")}
                      title={t("common.edit")}
                      onclick={() => openEditor(row.quote)}
                      class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:text-ink-1"
                    >
                      <Pencil class="size-3.5 no-mirror" strokeWidth={1.75} />
                    </button>
                    <button
                      type="button"
                      aria-label={t("common.delete")}
                      title={t("common.delete")}
                      disabled={working}
                      onclick={() => void removeQuote(row)}
                      class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:border-danger/50 hover:text-danger disabled:opacity-50"
                    >
                      <Trash2 class="size-3.5 no-mirror" strokeWidth={1.75} />
                    </button>
                  </span>
                </span>
              </span>
            </span>
          </Card>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<QuoteEdit
  open={editorOpen}
  bookId={null}
  quote={editing}
  onclose={() => (editorOpen = false)}
  onsaved={() => {
    toast.success(t("toast.quoteSaved"));
    void load();
  }}
/>
