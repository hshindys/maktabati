<script lang="ts">
  import { TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { listBooks, setBookPage } from "../api";
  import Button from "../components/Button.svelte";
  import BookCover from "../components/BookCover.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { go } from "../router";
  import { toast } from "../toast";
  import type { AppErrorDto, Book } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  let books = $state<Book[]>([]);
  let drafts = $state<Record<string, string>>({});
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let saving = $state<string | null>(null);

  const num = (value: number) => formatNumber(value, { digits });

  let readingBooks = $derived(
    books
      .filter((book) => book.reading_status === "reading")
      .sort((a, b) => b.updated_at - a.updated_at),
  );

  async function load(): Promise<void> {
    loading = true;
    const result = await listBooks();
    if (result.status === "ok") {
      books = result.data;
      drafts = Object.fromEntries(
        result.data
          .filter((book) => book.reading_status === "reading")
          .map((book) => [book.id, book.current_page === null ? "" : String(book.current_page)]),
      );
      loadError = null;
    } else if (result.error.code !== "shell.unavailable") {
      loadError = result.error;
    } else {
      books = [];
      drafts = {};
      loadError = null;
    }
    loading = false;
  }

  async function savePage(book: Book): Promise<void> {
    if (saving !== null) return;
    const raw = drafts[book.id]?.trim() ?? "";
    const parsed = raw.length === 0 ? null : Number(raw);
    if (parsed !== null && (!Number.isInteger(parsed) || parsed < 0)) {
      toast.error(t("book.page.invalid"));
      return;
    }

    saving = book.id;
    try {
      const result = await setBookPage(book.id, parsed);
      if (result.status === "ok") {
        books = books.map((item) => (item.id === book.id ? result.data : item));
        toast.success(t("toast.pageUpdated"));
      } else {
        toast.error(messageFor(result.error, t));
      }
    } finally {
      saving = null;
    }
  }

  onMount(() => void load());
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-5)] px-[var(--sp-6)] py-[var(--sp-5)]">
  <header class="flex flex-wrap items-end justify-between gap-[var(--sp-3)]">
    <div>
      <h1 class="text-[var(--fs-2xl)] font-semibold">{t("reading.title")}</h1>
      <p class="mt-1 text-[var(--fs-base)] text-ink-2">
        {t("reading.count", { n: num(readingBooks.length) })}
      </p>
    </div>
  </header>

  {#if loading}
    <div class="h-28 animate-pulse rounded-[var(--r-lg)] border border-border-1 bg-surface-2"></div>
  {:else if loadError}
    <Card>
      <div role="alert" class="flex flex-col items-start gap-[var(--sp-3)]">
        <div class="flex items-center gap-2 text-[var(--fs-md)] font-semibold text-danger">
          <TriangleAlert class="size-5 no-mirror" strokeWidth={2} />
          {t("error.title")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else if readingBooks.length === 0}
    <Card>
      <EmptyState title={t("reading.empty.title")} body={t("reading.empty.body")} />
      <div class="mt-[var(--sp-4)] flex justify-center">
        <Button variant="primary" onclick={() => go("/library")}>{t("library.title")}</Button>
      </div>
    </Card>
  {:else}
    <div class="grid gap-[var(--sp-4)]">
      {#each readingBooks as book (book.id)}
        <Card>
          <div class="flex flex-col gap-[var(--sp-4)] sm:flex-row">
            <button
              type="button"
              class="flex min-w-0 items-center gap-[var(--sp-4)] text-start"
              onclick={() => go(`/book/${book.id}`)}
            >
              <BookCover
                title={book.title}
                language={book.language_code}
                coverPath={book.cover_path}
                height={96}
                class="shrink-0"
              />
              <span class="min-w-0">
                <span class="block truncate text-[var(--fs-lg)] font-semibold text-ink-1">{book.title}</span>
                <span class="mt-1 block text-[var(--fs-sm)] text-ink-3">{t("reading.openBook")}</span>
              </span>
            </button>

            <div class="min-w-0 flex-1">
              <ProgressBar value={book.progress_pct} label={t("book.progress")} showValue />
              <form
                class="mt-[var(--sp-3)] flex flex-wrap items-end gap-[var(--sp-3)]"
                onsubmit={(event) => {
                  event.preventDefault();
                  void savePage(book);
                }}
              >
                <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
                  {t("book.currentPage")}
                  <input
                    type="text"
                    inputmode="numeric"
                    bind:value={drafts[book.id]}
                    dir="ltr"
                    class="tnum h-9 w-24 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
                  />
                </label>
                {#if book.page_count !== null}
                  <span class="pb-2 text-[var(--fs-sm)] text-ink-3">
                    {t("book.pageOf", { total: num(book.page_count) })}
                  </span>
                {/if}
                <Button
                  type="button"
                  variant="primary"
                  size="sm"
                  loading={saving === book.id}
                  onclick={() => void savePage(book)}
                >
                  {t("common.save")}
                </Button>
              </form>
            </div>
          </div>
        </Card>
      {/each}
    </div>
  {/if}
</div>
