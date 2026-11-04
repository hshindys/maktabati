<script lang="ts">
  import { FileUp, TriangleAlert } from "@lucide/svelte";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import { locale, translatorFor, type MessageKey } from "../i18n";
  import { messageFor } from "../errors";
  import { formatNumber } from "../format";
  import { preferences } from "../settings";
  import { toast } from "../toast";
  import { commitGoodreads, parseGoodreads, type GoodreadsRow } from "../api";
  import type { AppErrorDto, GoodreadsImportFailure } from "@/bindings";

  /**
   * Goodreads import — CSV in, catalogue rows out (docs/04 §4.6).
   *
   * Both parsing and import decisions run in Rust. This view only previews parsed rows,
   * sends them to the importer, and renders its explicit per-row outcomes.
   */

  let csvText = $state("");
  let fileName = $state("");
  let rows = $state<GoodreadsRow[]>([]);
  let warnings = $state<GoodreadsRow[]>([]);
  let parsing = $state(false);
  let committing = $state(false);
  let parseError = $state<string | null>(null);
  let fileInput = $state<HTMLInputElement | null>(null);

  let done = $state(0);
  let skipped = $state(0);
  let failures = $state<GoodreadsImportFailure[]>([]);
  let commitError = $state<AppErrorDto | null>(null);

  let t = $derived(translatorFor($locale));
  let digits = $derived($preferences.digits);
  const num = (v: number) => formatNumber(v, { digits });
  /** Goodreads ships at most a few thousand rows; the preview table caps itself. */
  const PREVIEW_CAP = 25;

  async function onFile(e: Event): Promise<void> {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    fileName = file.name;
    csvText = await file.text();
    // A newly picked file replaces whatever the paste box held.
    void parse();
  }

  async function parse(): Promise<void> {
    parseError = null;
    commitError = null;
    rows = [];
    warnings = [];
    failures = [];
    done = 0;
    skipped = 0;
    if (csvText.trim().length === 0) return;
    parsing = true;
    try {
      const res = await parseGoodreads(csvText);
      if (res.status === "ok") {
        rows = res.data;
        warnings = res.data.filter((row) => row.warnings.length > 0);
      } else {
        parseError = messageFor(res.error, t);
      }
    } finally {
      parsing = false;
    }
  }

  async function commit(): Promise<void> {
    if (committing || rows.length === 0) return;
    committing = true;
    done = 0;
    skipped = 0;
    failures = [];
    commitError = null;
    try {
      const res = await commitGoodreads(rows);
      if (res.status === "ok") {
        done = res.data.imported;
        skipped = res.data.skipped;
        failures = res.data.failures;
        if (failures.length === 0) toast.success(t("toast.imported"));
      } else {
        commitError = res.error;
        toast.error(messageFor(res.error, t));
      }
    } finally {
      committing = false;
    }
  }
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-4)] px-[var(--sp-6)] py-[var(--sp-5)]">
  <header>
    <h1 class="text-[var(--fs-xl)] font-semibold text-ink-1">{t("import.title")}</h1>
    <p class="mt-1 text-[var(--fs-base)] text-ink-2">{t("import.intro")}</p>
  </header>

  <!-- The file picker and the paste box feed the same parse, so either way in works. -->
  <Card>
    <div class="flex flex-wrap items-center gap-[var(--sp-3)]">
      <input
        bind:this={fileInput}
        type="file"
        accept=".csv,text/csv"
        class="sr-only"
        onchange={(e) => void onFile(e)}
      />
      <Button variant="secondary" loading={parsing && rows.length === 0} onclick={() => void fileInput?.click()}>
        {#snippet iconStart()}
          <FileUp class="size-4 no-mirror" strokeWidth={1.75} />
        {/snippet}
        {t("import.pick")}
      </Button>
      <span class="text-[var(--fs-sm)] text-ink-3">{fileName || t("import.orPaste")}</span>
    </div>

    <textarea
      class="mt-[var(--sp-3)] h-32 w-full resize-y rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-[var(--sp-3)] font-mono text-[var(--fs-sm)] text-ink-1 outline-none focus:border-accent"
      bind:value={csvText}
      placeholder={t("import.orPaste")}
      aria-label={t("import.orPaste")}
    ></textarea>

    <div class="mt-[var(--sp-3)] flex items-center gap-[var(--sp-3)]">
      <Button variant="primary" loading={parsing} disabled={csvText.trim().length === 0} onclick={() => void parse()}>
        {parsing ? t("import.parsing") : t("import.parse")}
      </Button>
      {#if rows.length > 0 && !committing}
        <Button variant="primary" onclick={() => void commit()}>
          {t("import.commit")} — {t("import.previewCount", { n: num(rows.length) })}
        </Button>
      {:else if committing}
        <span role="status" class="text-[var(--fs-sm)] text-ink-2">{t("import.committing")}</span>
      {/if}
    </div>

    <p class="mt-[var(--sp-3)] text-[var(--fs-sm)] text-ink-3">{t("import.mappingHint")}</p>

    {#if rows.length > 0}
      <p class="mt-[var(--sp-2)] text-[var(--fs-sm)] text-ink-3">{t("import.reviewImported")}</p>
    {/if}
  </Card>

  {#if parseError !== null}
    <Card>
      <div role="alert" class="flex items-start gap-[var(--sp-3)]">
        <TriangleAlert class="mt-0.5 size-5 shrink-0 text-danger no-mirror" strokeWidth={2} />
        <div>
          <p class="text-[var(--fs-md)] font-semibold text-danger">{t("import.parseFailed")}</p>
          <p class="mt-0.5 text-[var(--fs-base)] text-ink-2" dir="auto">{parseError}</p>
        </div>
      </div>
    </Card>
  {/if}

  {#if commitError !== null}
    <Card>
      <p role="alert" class="bidi-auto text-[var(--fs-sm)] text-danger" dir="auto">
        {messageFor(commitError, t)}
      </p>
    </Card>
  {/if}

  {#if skipped > 0 || failures.length > 0}
    <Card>
      <div role={failures.length > 0 ? "alert" : "status"} class="flex flex-col gap-[var(--sp-2)]">
        {#if skipped > 0}
          <p class="text-[var(--fs-base)] text-ink-2">
            {t("import.skipped", { n: num(skipped) })}
          </p>
        {/if}
        {#if failures.length > 0}
          <div class="flex items-center gap-2 text-[var(--fs-md)] font-semibold text-danger">
            <TriangleAlert class="size-5 no-mirror" strokeWidth={2} />
            {t("import.summary", {
              done: num(done),
              failed: num(failures.length),
            })}
          </div>
          <ul class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {#each failures.slice(0, 10) as failure (failure.title + failure.error.code)}
              <li class="break-all" dir="auto">• {failure.title}: {messageFor(failure.error, t)}</li>
            {/each}
            {#if failures.length > 10}
              <li>{t("import.moreRows", { m: num(failures.length - 10) })}</li>
            {/if}
          </ul>
        {/if}
      </div>
    </Card>
  {/if}

  {#if warnings.length > 0}
    <Card>
      <p class="text-[var(--fs-sm)] text-ink-2">
        {t("import.warningsOverview", { n: num(warnings.length) })}
      </p>
      <ul class="mt-[var(--sp-2)] flex flex-col gap-1">
        {#each warnings.slice(0, 8) as row (row.goodreads_id)}
          <li class="text-[var(--fs-sm)] text-ink-3" dir="auto">
            <span class="text-ink-2">{row.title}</span>
            — {row.warnings.join(" · ")}
          </li>
        {/each}
      </ul>
    </Card>
  {/if}

  {#if rows.length > 0}
    <Card>
      <div class="flex items-baseline justify-between gap-[var(--sp-3)]">
        <h2 class="text-[var(--fs-md)] font-semibold text-ink-1">{t("import.preview")}</h2>
        <span class="tnum bidi-ltr text-[var(--fs-sm)] text-ink-2">
          {t("import.previewCount", { n: num(rows.length) })}
        </span>
      </div>

      <div class="mt-[var(--sp-3)] overflow-x-auto rounded-[var(--r-md)] border border-border-1">
        <table class="w-full border-collapse text-[var(--fs-sm)]">
          <thead>
            <tr class="bg-surface-2 text-ink-3">
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.title")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.author")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.isbn")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.status")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.rating")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.year")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.pages")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.publisher")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.binding")}</th>
              <th scope="col" class="px-[var(--sp-3)] py-[var(--sp-2)] text-start font-medium">{t("import.col.tags")}</th>
            </tr>
          </thead>
          <tbody>
            {#each rows.slice(0, PREVIEW_CAP) as row (row.goodreads_id)}
              <tr class="border-t border-border-1 hover:bg-surface-2">
                <td class="bidi-auto max-w-[14rem] truncate px-[var(--sp-3)] py-[var(--sp-2)] text-ink-1" dir="auto">{row.title}</td>
                <td class="bidi-auto max-w-[10rem] truncate px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2" dir="auto">{row.author}</td>
                <td class="bidi-ltr tnum px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2">
                  {row.isbn13 ?? row.isbn10 ?? "—"}
                </td>
                <td class="px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2">{t(`status.${row.status}` as MessageKey)}</td>
                <td class="tnum bidi-ltr px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2">
                  {row.rating === null || row.rating <= 0 ? "—" : `${num(row.rating)}/5`}
                </td>
                <td class="tnum bidi-ltr px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2">{row.publication_year ?? "—"}</td>
                <td class="tnum bidi-ltr px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2">{row.page_count ?? "—"}</td>
                <td class="bidi-auto max-w-[10rem] truncate px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2" dir="auto">{row.publisher ?? "—"}</td>
                <td class="px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2">{row.binding ?? "—"}</td>
                <td class="bidi-auto max-w-[12rem] truncate px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2" dir="auto">
                  {row.tags.length === 0 ? t("import.untagged") : row.tags.join(" · ")}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      {#if rows.length > PREVIEW_CAP}
        <p class="mt-[var(--sp-2)] text-[var(--fs-sm)] text-ink-3">
          {t("import.showing", { n: num(PREVIEW_CAP) })} {t("import.moreRows", { m: num(rows.length - PREVIEW_CAP) })}
        </p>
      {/if}
    </Card>
  {:else if parseError === null && !parsing}
    <!-- Nothing parsed yet and nothing wrong: the designed, teaching empty state. -->
    <Card>
      <EmptyState title={t("import.title")} body={t("import.intro")} compact />
    </Card>
  {/if}
</div>
