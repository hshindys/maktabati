<script lang="ts">
  import { FileUp, TriangleAlert } from "@lucide/svelte";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { formatNumber } from "../format";
  import { preferences } from "../settings";
  import { toast } from "../toast";
  import {
    type GoodreadsRow,
    createAuthor,
    createBook,
    createPublisher,
    createTag,
    findBooksByIsbn,
    parseGoodreads,
    searchAuthors,
    searchPublishers,
    searchTags,
  } from "../api";
  import * as goodreads from "../goodreads";
  import type { AuthorInput, BookInput, PublisherInput, TagInput } from "@/bindings";

  /**
   * Goodreads import — CSV in, catalogue rows out (docs/04 §4.6).
   *
   * Parsing is one pure backend call (`goodreads_parse`); the commit loop is where the
   * mapping decisions live:
   *
   * - author / publisher / tags are **find-or-create**, so a re-import never duplicates
   *   them (lookups are cached per run — one search per new name);
   * - Goodreads' 0..5 rating doubles into Maktabati's half-star units (0..10); `0` is
   *   Goodreads' "unrated", not a zero-star opinion, so it stores `null`;
   * - `read`→finished, `reading`→reading, `to_read`/`unread`→to_read;
   * - the `Binding` column maps onto a book format (`binding_to_format` semantics);
   * - **series, description and cover image are null** — not columns in the export, so
   *   nothing is guessed on their behalf;
   * - the review text is visible in the preview but dropped: the reviews record
   *   (`reviews` tables) is a later milestone and staging it on books would be a lie;
   * - rows whose ISBN already sits in the library are skipped, counted separately from
   *   failures — an import you re-run must not invent second copies.
   */

  type CommitFailure = { title: string; message: string };

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
  let failures = $state<CommitFailure[]>([]);

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
    rows = [];
    warnings = [];
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

  // The mapping decisions come from `lib/goodreads.ts` — shared, tested, and the
  // documented mirror of `metadata::goodreads::binding_to_format`.
  const {
    STATUS_MAP,
    DEFAULT_STATUS,
    bindingToFormat,
    ratingToHalfStars,
    cleanIsbn,
  } = goodreads;

  // ── Find-or-create ────────────────────────────────────────────────────────
  //
  // Lookups are cached for the run: a Goodreads library repeats authors and
  // publishers heavily, and one search per distinct name beats one per row.

  function createCached(
    search: (name: string) => Promise<{ status: string; data?: unknown }>,
    list: (data: unknown) => Array<{ id: string; name: string }>,
    create: (name: string) => Promise<{ status: string; data?: unknown }>,
  ): (name: string) => Promise<string | null> {
    const cache = new Map<string, string | null>();
    const pending = new Map<string, Promise<string | null>>();
    return (name: string) => {
      const cached = cache.get(name);
      if (cached !== undefined) return Promise.resolve(cached);
      let running = pending.get(name);
      if (running === undefined) {
        running = (async () => {
          try {
            const found = await search(name);
            if (found.status === "ok") {
              const match = list(found.data).find((x) => x.name === name);
              if (match) {
                cache.set(name, match.id);
                return match.id;
              }
            }
            const created = await create(name);
            if (created.status === "ok" && created.data !== undefined) {
              const id = list([created.data])[0]?.id;
              if (id) {
                cache.set(name, id);
                return id;
              }
            }
          } catch {
            // Network/backend failure: the row imports without this relation, the
            // next run can fill it in.
          }
          cache.set(name, null);
          return null;
        })();
        pending.set(name, running);
      }
      return running;
    };
  }

  type Named = { id: string; name: string };
  const asList = (data: unknown): Named[] => (Array.isArray(data) ? (data as Named[]) : []);

  const authorNamed = (name: string): AuthorInput => ({
    name,
    name_original: null,
    bio: null,
    nationality: null,
    birth_year: null,
    death_year: null,
    photo_path: null,
  });
  const publisherNamed = (name: string): PublisherInput => ({
    name,
    country: null,
    city: null,
    website: null,
    notes: null,
    cover_path: null,
  });
  const tagNamed = (name: string): TagInput => ({ name, color: null });

  const authorIdFor = createCached(
    (n) => searchAuthors(n, 20),
    asList,
    (n) => createAuthor(authorNamed(n)),
  );
  const publisherIdFor = createCached(
    (n) => searchPublishers(n, 20),
    asList,
    (n) => createPublisher(publisherNamed(n)),
  );

  let tagIds = new Map<string, string>();

  async function tagIdsFor(names: string[]): Promise<string[]> {
    const ids: string[] = [];
    for (const name of names) {
      const known = tagIds.get(name);
      if (known !== undefined) {
        ids.push(known);
        continue;
      }
      let id: string | null = null;
      try {
        const found = await searchTags(name, 20);
        if (found.status === "ok") {
          const match = asList(found.data).find((x) => x.name === name);
          if (match) id = match.id;
        }
        if (id === null) {
          const created = await createTag(tagNamed(name));
          if (created.status === "ok") id = asList([created.data])[0]?.id ?? null;
        }
      } catch {
        // Skip an unreachable tag; the book still keeps its shelves in the title of nothing.
      }
      if (id !== null) {
        tagIds.set(name, id);
        ids.push(id);
      }
    }
    return ids;
  }

  async function commit(): Promise<void> {
    if (committing || rows.length === 0) return;
    committing = true;
    done = 0;
    skipped = 0;
    failures = [];

    for (const row of rows) {
      // A blank title still imports under its Goodreads id, so the failure list can name it.
      const title = row.title || `#${row.goodreads_id}`;
      try {
        // An ISBN already in the library means the same copy — skip, don't duplicate.
        const known13 = cleanIsbn(row.isbn13);
        const known10 = cleanIsbn(row.isbn10);
        if (known13 !== null || known10 !== null) {
          const matches = await findBooksByIsbn(known13 ?? known10 ?? "");
          if (matches.status === "ok" && matches.data.length > 0) {
            skipped += 1;
            continue;
          }
        }

        const authorId = row.author.length > 0 ? await authorIdFor(row.author) : null;
        const publisherId = row.publisher !== null ? await publisherIdFor(row.publisher) : null;
        const tags = await tagIdsFor(row.tags);

        const input: BookInput = {
          title: row.title,
          subtitle: null,
          original_title: null,
          language_code: null, // the backend defaults to `ar`
          isbn10: known10,
          isbn13: known13,
          edition: null,
          publication_year: row.publication_year,
          page_count: row.page_count,
          format: bindingToFormat(row.binding),
          description: null,
          series_id: null,
          series_order: null,
          room: null,
          shelf: null,
          box_name: null,
          acquired_date: null,
          acquired_from: null,
          price_minor: null,
          price_currency: null,
          condition: null,
          cover_path: null,
          cover_source: null,
          source: "goodreads",
          external_ids: null,
          authors: authorId === null ? [] : [{ author_id: authorId, role: "author" }],
          publisher_id: publisherId,
          categories: [],
          tags,
          reading_status: STATUS_MAP[row.status] ?? DEFAULT_STATUS,
          rating: ratingToHalfStars(row.rating),
          current_page: null,
        };
        const res = await createBook(input);
        if (res.status === "ok") {
          done += 1;
        } else {
          failures = [...failures, { title, message: messageFor(res.error, t) }];
        }
      } catch (cause) {
        failures = [
          ...failures,
          { title, message: cause instanceof Error ? cause.message : String(cause) },
        ];
      }
    }

    committing = false;
    if (failures.length === 0) {
      toast.success(t("toast.imported"));
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
        <span class="tnum text-[var(--fs-sm)] text-ink-2">
          {t("import.progress", { done: num(done), total: num(rows.length) })}
        </span>
      {/if}
    </div>

    <p class="mt-[var(--sp-3)] text-[var(--fs-sm)] text-ink-3">{t("import.mappingHint")}</p>

    {#if rows.length > 0}
      <p class="mt-[var(--sp-2)] text-[var(--fs-sm)] text-ink-3">{t("import.reviewDropped")}</p>
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
            {#each failures.slice(0, 10) as failure (failure.title + failure.message)}
              <li class="break-all" dir="auto">• {failure.title}: {failure.message}</li>
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
                  {cleanIsbn(row.isbn13) ?? cleanIsbn(row.isbn10) ?? "—"}
                </td>
                <td class="px-[var(--sp-3)] py-[var(--sp-2)] text-ink-2">{t(`status.${STATUS_MAP[row.status] ?? DEFAULT_STATUS}`)}</td>
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
