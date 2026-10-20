<script lang="ts">
  import { ArrowLeftRight, Barcode, BookOpen, Flame, Library, Target, TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { countFor, getLibrarySummary, listBooks, EMPTY_SUMMARY, type ReadingStatus } from "../api";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import CoverPlaceholder from "../components/CoverPlaceholder.svelte";
  import KpiCard from "../components/KpiCard.svelte";
  import Mark from "../components/Mark.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { formatDate, formatNumber, gregorianYear, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { go } from "../router";
  import { openAddBook } from "../wizard";
  import type { AppErrorDto, Book, LibrarySummary } from "@/bindings";

  /**
   * Dashboard — "understand my library" (docs/04-ux-design.md §4.1).
   *
   * The first screen after onboarding and the one that answers "where was I?" in under a
   * second, so it leads with counts rather than charts and never blocks on a spinner. A
   * *failed* summary is not an empty library (P0-5): it gets its own state with the real
   * error and a retry, because a corrupt database wearing a clean empty state is the worst
   * possible first impression.
   */

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  let year = $derived(gregorianYear(Date.now()));

  let summary = $state<LibrarySummary>(EMPTY_SUMMARY);
  /** The six most recently added books — the shelf the "recently added" strip shows. */
  let recent = $state<Book[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);

  async function load(): Promise<void> {
    loading = true;
    const [res, booksRes] = await Promise.all([getLibrarySummary(year), listBooks()]);
    if (res.status === "ok") {
      summary = res.data;
      loadError = null;
    } else {
      summary = EMPTY_SUMMARY;
      loadError = res.error;
    }
    // The shelf is decoration on top of a screen that already loaded: a failure here must
    // not replace the whole dashboard with an error card, so it just leaves the strip empty.
    recent =
      booksRes.status === "ok"
        ? [...booksRes.data].sort((a, b) => b.created_at - a.created_at).slice(0, 6)
        : [];
    loading = false;
  }

  onMount(() => void load());

  let today = $derived(
    formatDate(Date.now(), $locale, $preferences.calendar, digits),
  );

  let reading = $derived(countFor(summary, "reading" as ReadingStatus));
  let finished = $derived(countFor(summary, "finished" as ReadingStatus));
  let toRead = $derived(countFor(summary, "to_read" as ReadingStatus));

  let isEmpty = $derived(!loading && summary.books === 0);

  const num = (v: number) => formatNumber(v, { digits });
</script>

<div class="mx-auto flex max-w-6xl flex-col gap-[var(--sp-6)] px-[var(--sp-6)] py-[var(--sp-6)]">
  <header class="flex flex-wrap items-end justify-between gap-[var(--sp-4)]">
    <div>
      <h1 class="text-[var(--fs-2xl)] font-semibold">
        {t("dashboard.greeting")}
      </h1>
      <p class="bidi-auto mt-1 text-[var(--fs-base)] text-ink-2">{today}</p>
    </div>

    {#if !isEmpty}
      <Button variant="primary" onclick={() => go("/library")}>
        {t("library.title")}
      </Button>
    {/if}
  </header>

  {#if loading}
    <!-- Skeletons, not spinners (docs/04-ux-design.md §5.5). -->
    <div class="grid grid-cols-2 gap-[var(--sp-4)] lg:grid-cols-4">
      {#each Array(4) as _, i (i)}
        <div class="h-[7.5rem] animate-pulse rounded-[var(--r-lg)] border border-border-1 bg-surface-2"></div>
      {/each}
    </div>
  {:else if loadError}
    <!-- A backend failure is not an empty library (P0-5): name it, show the real
         message, offer the retry. -->
    <Card>
      <div role="alert" class="flex flex-col items-start gap-[var(--sp-3)]">
        <div class="flex items-center gap-2 text-[var(--fs-md)] font-semibold text-danger">
          <TriangleAlert class="size-5 no-mirror" strokeWidth={2} />
          {t("error.title")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">
          {messageFor(loadError, t)}
        </p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else if isEmpty}
    <!-- A teaching empty state (P1-10): the first screen should show how the product works,
         not sit as a small centred card. One primary action, two ways to get books in. -->
    <div
      class="flex flex-col items-center gap-[var(--sp-6)] rounded-[var(--r-xl)] border border-border-1 bg-surface-1 px-[var(--sp-6)] py-[var(--sp-8)] text-center shadow-[var(--sh-1)]"
    >
      <div class="flex flex-col items-center gap-[var(--sp-2)]">
        <Mark class="size-16" />
        <h2 class="mt-[var(--sp-2)] text-[var(--fs-xl)] font-semibold">{t("dashboard.empty.title")}</h2>
        <p class="max-w-md text-[var(--fs-base)] text-ink-2">{t("dashboard.empty.body")}</p>
      </div>

      <Button variant="primary" size="lg" onclick={() => openAddBook()}>
        {t("dashboard.empty.action")}
      </Button>

      <div class="grid w-full max-w-2xl gap-[var(--sp-4)] sm:grid-cols-2">
        <button
          type="button"
          onclick={() => go("/transfer")}
          class="flex items-start gap-[var(--sp-3)] rounded-[var(--r-lg)] border border-border-1 bg-surface-2 p-[var(--sp-4)] text-start transition-colors hover:border-border-2 hover:bg-surface-3"
        >
          <ArrowLeftRight class="mt-0.5 size-5 shrink-0 text-accent no-mirror" strokeWidth={1.75} />
          <span>
            <span class="block text-[var(--fs-base)] font-medium text-ink-1">
              {t("dashboard.empty.importTitle")}
            </span>
            <span class="mt-0.5 block text-[var(--fs-sm)] text-ink-3">
              {t("dashboard.empty.importBody")}
            </span>
          </span>
        </button>

        <button
          type="button"
          onclick={() => openAddBook()}
          class="flex items-start gap-[var(--sp-3)] rounded-[var(--r-lg)] border border-border-1 bg-surface-2 p-[var(--sp-4)] text-start transition-colors hover:border-border-2 hover:bg-surface-3"
        >
          <Barcode class="mt-0.5 size-5 shrink-0 text-accent no-mirror" strokeWidth={1.75} />
          <span>
            <span class="block text-[var(--fs-base)] font-medium text-ink-1">
              {t("dashboard.empty.scanTitle")}
            </span>
            <span class="mt-0.5 block text-[var(--fs-sm)] text-ink-3">
              {t("dashboard.empty.scanBody")}
            </span>
          </span>
        </button>
      </div>
    </div>
  {:else}
    <!-- KPI row -->
    <div class="grid grid-cols-2 gap-[var(--sp-4)] lg:grid-cols-4">
      <KpiCard
        label={t("dashboard.kpi.total")}
        value={num(summary.books)}
        caption="{t("dashboard.kpi.books")} · {num(summary.authors)} {t("nav.authors")}"
      >
        {#snippet icon()}
          <Library class="size-4.5" strokeWidth={1.75} />
        {/snippet}
      </KpiCard>

      <KpiCard
        label={t("dashboard.kpi.reading")}
        value={num(reading)}
        tone="accent"
        caption="{num(toRead)} {t("status.to_read")}"
      >
        {#snippet icon()}
          <BookOpen class="size-4.5" strokeWidth={1.75} />
        {/snippet}
      </KpiCard>

      <KpiCard
        label={t("dashboard.kpi.pagesThisYear")}
        value={num(summary.pages_total)}
        caption="{num(summary.categories)} {t("nav.categories")}"
      >
        {#snippet icon()}
          <Target class="size-4.5" strokeWidth={1.75} />
        {/snippet}
      </KpiCard>

      <KpiCard
        label={t("dashboard.kpi.goal")}
        value={num(summary.finished_in_year)}
        tone={summary.finished_in_year > 0 ? "success" : "default"}
        caption="{num(finished)} {t("status.finished")} · {num(summary.year)}"
      >
        {#snippet icon()}
          <Flame class="size-4.5" strokeWidth={1.75} />
        {/snippet}
        {#snippet footer()}
          <ProgressBar value={0} label={t("dashboard.kpi.goal")} size="sm" showValue />
        {/snippet}
      </KpiCard>
    </div>

    <!-- Secondary stats -->
    <div class="grid gap-[var(--sp-4)] lg:grid-cols-2">
      <Card>
        <h2 class="mb-[var(--sp-4)] text-[var(--fs-md)] font-semibold">
          {t("dashboard.inYourLibrary")}
        </h2>
        <dl class="grid grid-cols-2 gap-[var(--sp-4)]">
          <div>
            <dt class="text-[var(--fs-sm)] text-ink-3">{t("nav.quotes")}</dt>
            <!-- The cell keeps the page direction so the number aligns under its label; the
                 digits are isolated inside it (docs/04 §7.5) rather than flipping the whole
                 cell to LTR, which slid every value to the opposite edge. -->
            <dd class="tnum text-[var(--fs-xl)] font-semibold"><span class="bidi-ltr">{num(summary.quotes)}</span></dd>
          </div>
          <div>
            <dt class="text-[var(--fs-sm)] text-ink-3">{t("nav.notes")}</dt>
            <dd class="tnum text-[var(--fs-xl)] font-semibold"><span class="bidi-ltr">{num(summary.note_pages)}</span></dd>
          </div>
          <div>
            <dt class="text-[var(--fs-sm)] text-ink-3">{t("nav.publishers")}</dt>
            <dd class="tnum text-[var(--fs-xl)] font-semibold"><span class="bidi-ltr">{num(summary.publishers)}</span></dd>
          </div>
          <div>
            <dt class="text-[var(--fs-sm)] text-ink-3">{t("nav.shelves")}</dt>
            <dd class="tnum text-[var(--fs-xl)] font-semibold"><span class="bidi-ltr">{num(summary.shelves)}</span></dd>
          </div>
        </dl>
      </Card>

      <Card>
        <h2 class="mb-[var(--sp-4)] text-[var(--fs-md)] font-semibold">{t("library.title")}</h2>
        {#if summary.by_language.length === 0}
          <p class="text-[var(--fs-base)] text-ink-3">{t("common.none")}</p>
        {:else}
          <ul class="flex flex-col gap-[var(--sp-3)]">
            {#each summary.by_language as [code, count] (code)}
              <li class="flex items-center justify-between gap-[var(--sp-3)]">
                <span class="bidi-ltr rounded-[var(--r-sm)] bg-surface-2 px-2 py-0.5 font-mono text-[var(--fs-sm)] uppercase text-ink-2">
                  {code}
                </span>
                <span class="tnum bidi-ltr text-[var(--fs-base)] text-ink-1">{num(count)}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </Card>
    </div>

    <!-- Recently added — real books and their generated covers (the same title treatment a
         coverless book keeps in the library browser). Each links to its book page. -->
    {#if recent.length > 0}
      <section>
        <h2 class="mb-[var(--sp-3)] text-[var(--fs-md)] font-semibold">
          {t("dashboard.recentlyAdded")}
        </h2>
        <div class="flex gap-[var(--sp-4)] overflow-x-auto pb-[var(--sp-2)]">
          {#each recent as book (book.id)}
            <a href="#/book/{book.id}" class="shrink-0" title={book.title}>
              <CoverPlaceholder title={book.title} language={book.language_code} height={180} />
            </a>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>
