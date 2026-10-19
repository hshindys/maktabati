<script lang="ts">
  import { BookOpen, Flame, Library, Target } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { countFor, getLibrarySummary, type ReadingStatus } from "../api";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import CoverPlaceholder from "../components/CoverPlaceholder.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import KpiCard from "../components/KpiCard.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import { locale, translatorFor } from "../i18n";
  import { formatDate, formatNumber, gregorianYear, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { go } from "../router";
  import type { LibrarySummary } from "@/bindings";
  import { EMPTY_SUMMARY } from "../api";

  /**
   * Dashboard — "understand my reading" (docs/04-ux-design.md §4.1).
   *
   * The first screen after onboarding and the one that answers "where was I?" in under a
   * second, so it leads with counts rather than charts and never blocks on a spinner: the
   * summary request resolves or the designed empty state takes its place.
   */

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  let year = $derived(gregorianYear(Date.now()));

  let summary = $state<LibrarySummary>(EMPTY_SUMMARY);
  let loading = $state(true);

  onMount(async () => {
    summary = await getLibrarySummary(year);
    loading = false;
  });

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
  {:else if isEmpty}
    <Card>
      <EmptyState
        title={t("dashboard.empty.title")}
        body={t("dashboard.empty.body")}
        actionLabel={t("dashboard.empty.action")}
        onaction={() => go("/library")}
      />
    </Card>
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
          {t("status.to_read")}
        </h2>
        <dl class="grid grid-cols-2 gap-[var(--sp-4)]">
          <div>
            <dt class="text-[var(--fs-sm)] text-ink-3">{t("nav.quotes")}</dt>
            <dd class="tnum bidi-ltr text-[var(--fs-xl)] font-semibold">{num(summary.quotes)}</dd>
          </div>
          <div>
            <dt class="text-[var(--fs-sm)] text-ink-3">{t("nav.notes")}</dt>
            <dd class="tnum bidi-ltr text-[var(--fs-xl)] font-semibold">{num(summary.note_pages)}</dd>
          </div>
          <div>
            <dt class="text-[var(--fs-sm)] text-ink-3">{t("nav.publishers")}</dt>
            <dd class="tnum bidi-ltr text-[var(--fs-xl)] font-semibold">{num(summary.publishers)}</dd>
          </div>
          <div>
            <dt class="text-[var(--fs-sm)] text-ink-3">{t("nav.shelves")}</dt>
            <dd class="tnum bidi-ltr text-[var(--fs-xl)] font-semibold">{num(summary.shelves)}</dd>
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

    <!-- Recently added — real covers once the book CRUD lands; the generated placeholder shows
         the Arabic title treatment that every coverless book will use. -->
    <section>
      <h2 class="mb-[var(--sp-3)] text-[var(--fs-md)] font-semibold">
        {t("dashboard.recentlyAdded")}
      </h2>
      <div class="flex gap-[var(--sp-4)] overflow-x-auto pb-[var(--sp-2)]">
        {#each summary.by_status.slice(0, 6) as [status] (status)}
          <div class="shrink-0">
            <CoverPlaceholder title={t(`status.${status}`)} height={180} />
          </div>
        {/each}
      </div>
    </section>
  {/if}
</div>
