<script lang="ts">
  import { Grid3x3, List, Rows3, TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { listBooks } from "../api";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import BookCover from "../components/BookCover.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Stars from "../components/Stars.svelte";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { formatNumber } from "../format";
  import { preferences } from "../settings";
  import { openAddBook } from "../wizard";
  import type { AppErrorDto, Book } from "@/bindings";

  /**
   * Library browser — the screen the app is judged on (docs/04-ux-design.md §4.2).
   *
   * It now lists the real catalogue: `book_list` returns every scalar row in one call, which
   * is what keeps a 50k-book library instant. Author credits are deliberately *not* shown
   * here — they live only on the per-book query (`book_detail`), and joining them per row
   * would be one query per book. The name arrives with search's projection in M4; until then
   * every row carries the title, year, status and rating the row itself owns.
   *
   * Outside the shell there is no backend, which is not an error but a different runtime:
   * the designed empty state renders instead of a failure (see `load`).
   */

  type ViewMode = "grid" | "list" | "table";
  const VIEWS: Array<{ mode: ViewMode; icon: typeof Grid3x3; key: "library.view.grid" | "library.view.list" | "library.view.table" }> = [
    { mode: "grid", icon: Grid3x3, key: "library.view.grid" },
    { mode: "list", icon: List, key: "library.view.list" },
    { mode: "table", icon: Rows3, key: "library.view.table" },
  ];

  const SORTS = [
    "library.sort.recent",
    "library.sort.title",
    "library.sort.author",
    "library.sort.year",
    "library.sort.rating",
    "library.sort.progress",
  ] as const;
  type SortKey = (typeof SORTS)[number];

  let t = $derived(translatorFor($locale));
  let view = $state<ViewMode>("grid");
  let sort = $state<SortKey>("library.sort.recent");

  let books = $state<Book[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);

  async function load(): Promise<void> {
    loading = true;
    const res = await listBooks();
    if (res.status === "ok") {
      books = res.data;
      loadError = null;
    } else if (res.error.code === "shell.unavailable") {
      // A browser preview has no library and no backend — that is a runtime, not a fault,
      // so it gets the designed empty state rather than an error card.
      books = [];
      loadError = null;
    } else {
      books = [];
      loadError = res.error;
    }
    loading = false;
  }

  onMount(() => void load());

  /** `null` sorts last in both directions, so "no year" never beats a real one. */
  function cmpNullable(a: number | null, b: number | null): number {
    if (a === null && b === null) return 0;
    if (a === null) return 1;
    if (b === null) return -1;
    return a - b;
  }

  let sorted = $derived.by(() => {
    const rows = [...books];
    const byTitle = (a: Book, b: Book) =>
      a.title.localeCompare(b.title, $locale === "ar" ? "ar" : "en");
    switch (sort) {
      case "library.sort.title":
        return rows.sort(byTitle);
      // `Book` carries no author field (see the note above), so this sort orders by the title
      // the author's work is filed under until M4's projection ships the credits.
      case "library.sort.author":
        return rows.sort(byTitle);
      case "library.sort.year":
        return rows.sort((a, b) => cmpNullable(b.publication_year, a.publication_year));
      case "library.sort.rating":
        return rows.sort((a, b) => cmpNullable(b.rating, a.rating));
      case "library.sort.progress":
        return rows.sort((a, b) => b.progress_pct - a.progress_pct);
      default:
        return rows.sort((a, b) => b.created_at - a.created_at);
    }
  });

  let count = $derived(formatNumber(books.length, { digits: $preferences.digits }));
  const num = (v: number) => formatNumber(v, { digits: $preferences.digits });
  const yearOf = (b: Book) => (b.publication_year === null ? "" : num(b.publication_year));
  /** The row's one-line status: where it sits in the reading cycle, plus the year. */
  const metaOf = (b: Book) => {
    const year = yearOf(b);
    const status = t(`status.${b.reading_status}`);
    return year.length > 0 ? `${status} · ${year}` : status;
  };
</script>

<div class="flex min-h-full flex-col">
  <div class="flex flex-wrap items-center gap-[var(--sp-3)] border-b border-border-1 bg-surface-1 px-[var(--sp-5)] py-[var(--sp-3)]">
    <h1 class="text-[var(--fs-lg)] font-semibold">{t("library.title")}</h1>
    <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
      {count} {t("library.count")}
    </span>

    <div class="ms-auto flex items-center gap-[var(--sp-3)]">
      <!-- Sort -->
      <label class="flex items-center gap-[var(--sp-2)]">
        <span class="text-[var(--fs-sm)] text-ink-3">{t("library.sort")}</span>
        <select
          bind:value={sort}
          class="h-8 rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-2)] text-[var(--fs-sm)] text-ink-1"
        >
          {#each SORTS as key (key)}
            <option value={key}>{t(key)}</option>
          {/each}
        </select>
      </label>

      <!-- View mode -->
      <div class="flex rounded-[var(--r-md)] border border-border-1 bg-surface-1 p-0.5" role="radiogroup" aria-label={t("library.view.grid")}>
        {#each VIEWS as item (item.mode)}
          {@const Icon = item.icon}
          <button
            type="button"
            role="radio"
            aria-checked={view === item.mode}
            aria-label={t(item.key)}
            title={t(item.key)}
            onclick={() => (view = item.mode)}
            class="grid size-7 place-items-center rounded-[var(--r-sm)] transition-colors {view === item.mode
              ? 'bg-accent text-accent-fg'
              : 'text-ink-2 hover:bg-surface-2'}"
          >
            <Icon class="size-4 no-mirror" strokeWidth={1.75} />
          </button>
        {/each}
      </div>

      <Button variant="primary" size="sm" onclick={openAddBook}>{t("common.add")}</Button>
    </div>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto px-[var(--sp-5)] py-[var(--sp-5)]">
    {#if loading}
      <!-- Skeletons, not spinners (docs/04-ux-design.md §5.5). -->
      <div class="grid grid-cols-2 gap-[var(--sp-4)] sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6">
        {#each Array(12) as _, i (i)}
          <div class="h-[17rem] animate-pulse rounded-[var(--r-lg)] border border-border-1 bg-surface-2"></div>
        {/each}
      </div>
    {:else if loadError}
      <!-- A backend failure is not an empty library (P0-5): name it, show the real message,
           offer the retry. -->
      <Card>
        <div role="alert" class="flex flex-col items-start gap-[var(--sp-3)]">
          <div class="flex items-center gap-2 text-[var(--fs-md)] font-semibold text-danger">
            <TriangleAlert class="size-5 no-mirror" strokeWidth={2} />
            {t("library.loadFailed")}
          </div>
          <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
          <Button onclick={() => void load()}>{t("common.retry")}</Button>
        </div>
      </Card>
    {:else if books.length === 0}
      <!-- A genuinely empty library teaches the two ways in, rather than saying "no results". -->
      <Card>
        <EmptyState
          title={t("library.noBooks.title")}
          body={t("library.noBooks.body")}
          actionLabel={t("dashboard.empty.action")}
          onaction={openAddBook}
        />
      </Card>
    {:else if view === "grid"}
      <ul class="grid grid-cols-2 gap-[var(--sp-5)] sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6">
        {#each sorted as book (book.id)}
          <li>
            <a
              href="#/book/{book.id}"
              class="group flex flex-col gap-[var(--sp-2)] rounded-[var(--r-lg)] p-[var(--sp-2)] transition-colors hover:bg-surface-2"
            >
              <span class="flex justify-center overflow-hidden rounded-[var(--r-md)] transition-transform duration-[var(--dur-base)] group-hover:-translate-y-0.5">
                <BookCover title={book.title} language={book.language_code} coverPath={book.cover_path} height={180} />
              </span>
              <span class="bidi-auto line-clamp-2 text-[var(--fs-base)] font-medium text-ink-1">
                {book.title}
              </span>
              <span class="bidi-auto text-[var(--fs-sm)] text-ink-3">{metaOf(book)}</span>
              {#if book.rating !== null}
                <Stars value={book.rating} readonly size="sm" label={t("book.rating.label")} />
              {/if}
            </a>
          </li>
        {/each}
      </ul>
    {:else if view === "list"}
      <ul class="flex flex-col gap-[var(--sp-2)]">
        {#each sorted as book (book.id)}
          <li>
            <a
              href="#/book/{book.id}"
              class="flex items-center gap-[var(--sp-4)] rounded-[var(--r-lg)] border border-border-1 bg-surface-1 p-[var(--sp-3)] transition-colors hover:border-border-2 hover:bg-surface-2"
            >
              <BookCover title={book.title} language={book.language_code} coverPath={book.cover_path} height={56} />
              <span class="flex min-w-0 flex-1 flex-col">
                <span class="bidi-auto truncate text-[var(--fs-base)] font-medium text-ink-1">
                  {book.title}
                </span>
                <span class="bidi-auto truncate text-[var(--fs-sm)] text-ink-3">{metaOf(book)}</span>
              </span>
              {#if book.rating !== null}
                <Stars value={book.rating} readonly size="sm" label={t("book.rating.label")} />
              {/if}
            </a>
          </li>
        {/each}
      </ul>
    {:else}
      <!-- Table: the dense view for cross-column comparison. Titles still link to the book. -->
      <div class="overflow-x-auto rounded-[var(--r-lg)] border border-border-1 bg-surface-1">
        <table class="w-full border-collapse text-[var(--fs-base)]">
          <thead>
            <tr class="border-b border-border-1 bg-surface-2 text-start text-[var(--fs-sm)] text-ink-3">
              <th scope="col" class="px-[var(--sp-4)] py-[var(--sp-3)] text-start font-medium">{t("wizard.field.title")}</th>
              <th scope="col" class="px-[var(--sp-4)] py-[var(--sp-3)] text-start font-medium">{t("library.sort.year")}</th>
              <th scope="col" class="px-[var(--sp-4)] py-[var(--sp-3)] text-start font-medium">{t("wizard.field.pages")}</th>
              <th scope="col" class="px-[var(--sp-4)] py-[var(--sp-3)] text-start font-medium">{t("wizard.field.status")}</th>
              <th scope="col" class="px-[var(--sp-4)] py-[var(--sp-3)] text-start font-medium">{t("book.rating.label")}</th>
            </tr>
          </thead>
          <tbody>
            {#each sorted as book (book.id)}
              <tr class="border-b border-border-1 last:border-0 hover:bg-surface-2">
                <td class="px-[var(--sp-4)] py-[var(--sp-3)]">
                  <a href="#/book/{book.id}" class="bidi-auto text-ink-1 hover:text-accent hover:underline">
                    {book.title}
                  </a>
                </td>
                <td class="tnum bidi-ltr px-[var(--sp-4)] py-[var(--sp-3)] text-ink-2">{yearOf(book) || "—"}</td>
                <td class="tnum bidi-ltr px-[var(--sp-4)] py-[var(--sp-3)] text-ink-2">
                  {book.page_count === null ? "—" : num(book.page_count)}
                </td>
                <td class="px-[var(--sp-4)] py-[var(--sp-3)] text-ink-2">{t(`status.${book.reading_status}`)}</td>
                <td class="px-[var(--sp-4)] py-[var(--sp-3)]">
                  {#if book.rating !== null}
                    <Stars value={book.rating} readonly size="sm" label={t("book.rating.label")} />
                  {:else}
                    <span class="text-ink-3">—</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>
