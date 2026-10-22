<script lang="ts">
  import { Check, FileText, TriangleAlert } from "@lucide/svelte";

  import Button from "../components/Button.svelte";
  import Combobox from "../components/Combobox.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import Stars from "../components/Stars.svelte";
  import TagInput from "../components/TagInput.svelte";

  import * as api from "../api";
  import { locale, translatorFor, type MessageKey } from "../i18n";
  import { canonicalIsbn, isValidIsbn } from "../isbn";
  import { messageFor } from "../errors";
  import { clearDraft, EMPTY_DRAFT, loadDraft, saveDraft, type BookDraft, type EntityRef, type SourceMode } from "../draft";
  import { addBookOpen, closeAddBook } from "../wizard";
  import { fileName, pickFiles } from "../pickFile";
  import { notifySaved, pushToast } from "../toast";
  import type {
    Author,
    AuthorCredit,
    Book,
    BookFormat,
    BookInput,
    Category,
    FileKind,
    Language,
    Publisher,
    ReadingStatus,
    Series,
    StorageMode,
    Tag,
  } from "@/bindings";

  /**
   * Add book — the wizard of docs/04-ux-design.md §4.5, opened as a modal from any view
   * (Dashboard CTA, Library empty state, `Ctrl+N`).
   *
   * The draft lives in `localStorage` (`draft.ts`), so a closed window costs nothing;
   * entities picked in the comboboxes are resolved to ids at save time, creating what the
   * user typed but the library does not have yet. Every failure surfaces as a message —
   * validation, duplicate, or a rejected write — and a save with possible duplicates only
   * proceeds when the user says "save anyway".
   *
   * The files step sits *before* review for one reason: a file cannot be attached until the
   * book exists, so what review shows is a promise rather than a result. `submit` attaches
   * each picked file after `book_create` succeeds, and a failed attach reports without
   * undoing the book that was just saved.
   *
   * The cover drop-zone is deliberately absent until `crates/files` can ingest one: a
   * control that cannot deliver a cover would be a lie in the UI.
   */

  let t = $derived(translatorFor($locale));

  const STEPS: MessageKey[] = [
    "wizard.steps.source",
    "wizard.steps.details",
    "wizard.steps.classify",
    "wizard.steps.files",
    "wizard.steps.review",
  ];
  const FORMATS: BookFormat[] = ["print", "ebook", "audiobook", "pdf", "other"];
  const STATUSES: ReadingStatus[] = ["to_read", "reading", "finished", "paused", "abandoned"];
  /** Step 1's two entry paths, in the order they are shown. */
  const SOURCE_MODES: SourceMode[] = ["manual", "isbn"];

  let draft = $state<BookDraft>(loadDraft());
  let step = $derived(draft.step);

  // Catalogue lists behind the comboboxes, loaded once when the wizard opens. In a
  // browser preview every call resolves to `NO_SHELL` and the lists stay empty — the
  // create rows still work as far as the draft is concerned, and the save reports why
  // nothing was written.
  let authors = $state<Author[]>([]);
  let publishers = $state<Publisher[]>([]);
  let seriesList = $state<Series[]>([]);
  let categories = $state<Category[]>([]);
  let tags = $state<Tag[]>([]);
  let languages = $state<Language[]>([]);

  let dupMatches = $state<Book[]>([]);
  let saveErrors = $state<string[]>([]);
  let saving = $state(false);

  /**
   * How picked files are stored. One control rather than a per-row switch: the choice is
   * about the *library*, not about one file, and a row that quietly disagreed with the
   * setting below it would be a bug waiting to be noticed.
   */
  let fileMode = $state<StorageMode>("copy");
  let picking = $state(false);

  $effect(() => {
    saveDraft(draft);
  });

  $effect(() => {
    if ($addBookOpen) void loadCatalogue();
  });

  async function loadCatalogue(): Promise<void> {
    const [a, p, s, c, tg, l] = await Promise.all([
      api.listAuthors(),
      api.listPublishers(),
      api.listSeries(),
      api.listCategories(),
      api.listTags(),
      api.listActiveLanguages(),
    ]);
    if (a.status === "ok") authors = a.data;
    if (p.status === "ok") publishers = p.data;
    if (s.status === "ok") seriesList = s.data;
    if (c.status === "ok") categories = c.data;
    if (tg.status === "ok") tags = tg.data;
    if (l.status === "ok") {
      languages = l.data;
      // The draft may carry a language the library no longer lists; fall back to the first.
      if (l.data.length > 0 && !l.data.some((x) => x.code === draft.languageCode)) {
        draft.languageCode = l.data[0]?.code ?? EMPTY_DRAFT.languageCode;
      }
    }
  }

  // ── Options for the pickers ──────────────────────────────────────────────────────────────

  /** A preview-safe fallback so the language select is never empty outside the shell. */
  const FALLBACK_LANGUAGES = [
    { code: "ar", ar: "العربية", en: "Arabic" },
    { code: "en", ar: "الإنجليزية", en: "English" },
  ];

  let languageOptions = $derived(
    languages.length > 0
      ? languages.map((x) => ({ code: x.code, name: $locale === "ar" ? x.name_ar : x.name_en }))
      : FALLBACK_LANGUAGES.map((x) => ({ code: x.code, name: $locale === "ar" ? x.ar : x.en })),
  );

  let authorOptions = $derived(authors.map((x): EntityRef => ({ id: x.id, name: x.name })));
  let publisherOptions = $derived(publishers.map((x): EntityRef => ({ id: x.id, name: x.name })));
  let seriesOptions = $derived(seriesList.map((x): EntityRef => ({ id: x.id, name: x.name })));

  function catName(c: Category): string {
    return $locale === "ar" ? c.name_ar : c.name_en;
  }
  let categoryOptions = $derived(categories.map((x): EntityRef => ({ id: x.id, name: catName(x) })));

  let authorRef = $derived(draft.authors[0] ?? null);
  function setAuthor(next: EntityRef | null): void {
    draft.authors = next ? [next] : [];
  }

  let categoryRef = $derived.by((): EntityRef | null => {
    const id = draft.categories[0];
    if (!id) return null;
    const found = categories.find((x) => x.id === id);
    return found ? { id: found.id, name: catName(found) } : null;
  });
  function setCategory(next: EntityRef | null): void {
    draft.categories = next ? [next.id ?? ""] : [];
  }

  // ── Inline create ────────────────────────────────────────────────────────────────────────

  /** A failed inline create must not lose the typed name: it stays in the draft as
   *  `{ id: null, name }` and the save pass retries, reporting there if it still fails. */
  async function createAuthor(name: string): Promise<EntityRef> {
    const res = await api.createAuthor({
      name,
      name_original: null,
      bio: null,
      nationality: null,
      birth_year: null,
      death_year: null,
      photo_path: null,
    });
    if (res.status === "ok") {
      authors = [...authors, res.data];
      return { id: res.data.id, name };
    }
    pushToast(`${t("wizard.field.authors")} «${name}»: ${messageFor(res.error, t)}`, {
      tone: "error",
    });
    return { id: null, name };
  }

  async function createPublisher(name: string): Promise<EntityRef> {
    const res = await api.createPublisher({
      name,
      country: null,
      city: null,
      website: null,
      notes: null,
      cover_path: null,
    });
    if (res.status === "ok") {
      publishers = [...publishers, res.data];
      return { id: res.data.id, name };
    }
    pushToast(`${t("wizard.field.publisher")} «${name}»: ${messageFor(res.error, t)}`, {
      tone: "error",
    });
    return { id: null, name };
  }

  async function createSeries(name: string): Promise<EntityRef> {
    const res = await api.createSeries({ name, description: null });
    if (res.status === "ok") {
      seriesList = [...seriesList, res.data];
      return { id: res.data.id, name };
    }
    pushToast(`${t("wizard.field.series")} «${name}»: ${messageFor(res.error, t)}`, {
      tone: "error",
    });
    return { id: null, name };
  }

  async function createCategory(name: string): Promise<EntityRef> {
    const res = await api.createCategory(
      {
        // A stable machine key: the typed name when it has Latin letters to slug,
        // otherwise a short random one — names are localized, keys never are.
        key: slugKey(name),
        name_ar: name,
        // The bilingual field starts as the typed name; the Categories screen (M7)
        // is where the other script gets edited properly.
        name_en: name,
        icon: null,
        color: null,
      },
      null,
    );
    if (res.status === "ok") {
      categories = [...categories, res.data];
      return { id: res.data.id, name };
    }
    pushToast(`${t("wizard.field.categories")} «${name}»: ${messageFor(res.error, t)}`, {
      tone: "error",
    });
    return { id: null, name };
  }

  function slugKey(name: string): string {
    const slug = name
      .toLocaleLowerCase()
      .replace(/[^\p{Letter}\p{Number}]+/gu, "-")
      .replace(/^-+|-+$/g, "");
    const latin = /^[a-z0-9-]+$/.test(slug) ? slug : "";
    return latin || `cat-${Math.random().toString(36).slice(2, 8)}`;
  }

  // ── Validation and saving ────────────────────────────────────────────────────────────────

  let isbnInvalid = $derived(draft.isbn.trim() !== "" && !isValidIsbn(draft.isbn));

  function intOrNull(raw: string): number | null {
    const s = raw.trim();
    if (!s) return null;
    const n = Number(s);
    return Number.isInteger(n) ? n : null;
  }

  /** Check what the user typed; returns messages and the step that holds the first problem. */
  function validate(): { errors: string[]; badStep: number } {
    const errors: string[] = [];
    let badStep = 0;
    if (!draft.title.trim()) {
      errors.push(`${t("wizard.field.title")}: ${t("error.validation.empty_field")}`);
      badStep = 2;
    }
    if (isbnInvalid) {
      errors.push(t("wizard.isbn.invalid"));
      if (!badStep) badStep = 1;
    }
    for (const [label, raw] of [
      [t("wizard.field.year"), draft.year],
      [t("wizard.field.pages"), draft.pages],
      [t("wizard.field.seriesOrder"), draft.seriesOrder],
    ] as const) {
      if (raw.trim() !== "" && intOrNull(raw) === null) {
        errors.push(`${label}: ${t("wizard.error.number")}`);
        if (!badStep) badStep = 2;
      }
    }
    return { errors, badStep };
  }

  /** The book as typed so far — ids as known, `authors`/`tags` left for the save pass. */
  function baseInput(): BookInput {
    const isbn = canonicalIsbn(draft.isbn);
    return {
      title: draft.title.trim(),
      subtitle: draft.subtitle.trim() || null,
      original_title: null,
      language_code: draft.languageCode || null,
      isbn10: isbn.length === 10 ? isbn : null,
      isbn13: isbn.length === 13 ? isbn : null,
      edition: draft.edition.trim() || null,
      publication_year: intOrNull(draft.year),
      page_count: intOrNull(draft.pages),
      format: (FORMATS.includes(draft.format as BookFormat) ? draft.format : null) as BookFormat | null,
      description: draft.description.trim() || null,
      series_id: draft.series?.id ?? null,
      series_order: intOrNull(draft.seriesOrder),
      room: null,
      shelf: null,
      box_name: null,
      acquired_date: null,
      acquired_from: null,
      price_minor: null,
      price_currency: null,
      condition: null,
      rating: draft.rating > 0 ? draft.rating : null,
      current_page: null,
      reading_status: (STATUSES.includes(draft.status as ReadingStatus)
        ? draft.status
        : "to_read") as ReadingStatus,
      cover_path: null,
      cover_source: null,
      source: "manual",
      external_ids: null,
      authors: [],
      publisher_id: draft.publisher?.id ?? null,
      categories: draft.categories.filter(Boolean),
      tags: [],
    };
  }

  /** Ask the backend whether this looks like a book already in the library. */
  async function checkDuplicates(): Promise<void> {
    const res = await api.findDuplicateBooks(baseInput());
    dupMatches = res.status === "ok" ? res.data : [];
  }

  async function submit(force: boolean): Promise<void> {
    if (saving) return;
    saveErrors = [];

    const { errors, badStep } = validate();
    if (errors.length > 0) {
      saveErrors = errors;
      if (badStep) draft.step = badStep;
      return;
    }

    if (!force && dupMatches.length > 0) return; // the warning panel is up — its button saves.

    saving = true;
    try {
      // Resolve what the user typed into ids, creating the missing entities. This pass
      // reports into the error panel instead of toasting — one surface for one failure.
      const errs: string[] = [];
      const credits: AuthorCredit[] = [];
      for (const a of draft.authors) {
        let id = a.id;
        if (!id) {
          const res = await api.createAuthor({
            name: a.name,
            name_original: null,
            bio: null,
            nationality: null,
            birth_year: null,
            death_year: null,
            photo_path: null,
          });
          if (res.status === "ok") {
            authors = [...authors, res.data];
            id = res.data.id;
          } else {
            errs.push(`${t("wizard.field.authors")} «${a.name}»: ${messageFor(res.error, t)}`);
          }
        }
        if (id) credits.push({ author_id: id, role: "author" });
      }

      let publisherId = draft.publisher?.id ?? null;
      if (!publisherId && draft.publisher) {
        const res = await api.createPublisher({
          name: draft.publisher.name,
          country: null,
          city: null,
          website: null,
          notes: null,
          cover_path: null,
        });
        if (res.status === "ok") {
          publishers = [...publishers, res.data];
          publisherId = res.data.id;
        } else {
          errs.push(
            `${t("wizard.field.publisher")} «${draft.publisher.name}»: ${messageFor(res.error, t)}`,
          );
        }
      }

      let seriesId = draft.series?.id ?? null;
      if (!seriesId && draft.series) {
        const res = await api.createSeries({ name: draft.series.name, description: null });
        if (res.status === "ok") {
          seriesList = [...seriesList, res.data];
          seriesId = res.data.id;
        } else {
          errs.push(
            `${t("wizard.field.series")} «${draft.series.name}»: ${messageFor(res.error, t)}`,
          );
        }
      }

      const tagIds: string[] = [];
      for (const name of draft.tags) {
        const known = tags.find((x) => x.name.toLocaleLowerCase() === name.toLocaleLowerCase());
        if (known) {
          tagIds.push(known.id);
          continue;
        }
        const res = await api.createTag({ name, color: null });
        if (res.status === "ok") {
          tags = [...tags, res.data];
          tagIds.push(res.data.id);
        } else {
          errs.push(`${t("wizard.field.tags")} «${name}»: ${messageFor(res.error, t)}`);
        }
      }

      if (errs.length > 0) {
        saveErrors = errs;
        return;
      }

      const input: BookInput = {
        ...baseInput(),
        authors: credits,
        publisher_id: publisherId,
        series_id: seriesId,
        tags: tagIds,
      };

      const res = await api.createBook(input);
      if (res.status !== "ok") {
        saveErrors = [messageFor(res.error, t)];
        return;
      }

      // The book exists now, so the picked files can be attached. A failed attach is a
      // failure *of the attachment*, not of the book: the book stays, the file is not on
      // it, and the message names which. The draft is cleared either way — the book is in
      // the library and the file can be attached again from its page.
      const bookId = res.data.id;
      const failed: string[] = [];
      for (const file of draft.files) {
        const attach = await api.attachFile(
          bookId,
          file.kind,
          file.storageMode,
          file.path,
          null,
          null,
        );
        if (attach.status !== "ok") failed.push(`${fileName(file.path)}: ${messageFor(attach.error, t)}`);
      }

      notifySaved();
      clearDraft();
      draft = { ...EMPTY_DRAFT };
      dupMatches = [];
      if (failed.length > 0) {
        saveErrors = [t("wizard.files.attachFailed"), ...failed];
      } else {
        closeAddBook();
      }
    } finally {
      saving = false;
    }
  }

  /** The label a `FileKind` renders as — the kind's own vocabulary, not the book format's. */
  const kindLabel = (kind: FileKind): string => t(`wizard.fileKind.${kind}` as MessageKey);

  async function pickFile(): Promise<void> {
    if (picking) return;
    picking = true;
    try {
      const picked = await pickFiles(t("wizard.files.pick"), {
        books: t("wizard.files.filterBooks"),
        audio: t("wizard.files.filterAudio"),
      });
      if (picked === null) return;
      // A path already on the list is the same file twice. In copy mode that would hit
      // `ux_files_sha_copy` and come back as a duplicate the user never asked for.
      const known = new Set(draft.files.map((f) => f.path));
      draft.files = [
        ...draft.files,
        ...picked.filter((f) => !known.has(f.path)).map((f) => ({ ...f, storageMode: fileMode })),
      ];
    } finally {
      picking = false;
    }
  }

  function removeFile(index: number): void {
    draft.files = draft.files.filter((_, i) => i !== index);
  }

  /** The storage select rewrites every row: one setting, one list, no silent disagreement. */
  function setFileMode(next: StorageMode): void {
    fileMode = next;
    draft.files = draft.files.map((f) => ({ ...f, storageMode: next }));
  }

  function next(): void {
    saveErrors = [];
    if (draft.step === 2) {
      // Gate the advance on the details step so a missing title never reaches review.
      const { errors } = validate();
      const stepTwo = errors.filter((e) => !e.startsWith(t("wizard.isbn.invalid")));
      if (stepTwo.length > 0) {
        saveErrors = stepTwo;
        return;
      }
    }
    draft.step = Math.min(5, draft.step + 1);
    if (draft.step === 5) void checkDuplicates();
  }

  function back(): void {
    saveErrors = [];
    draft.step = Math.max(1, draft.step - 1);
  }
</script>

<Modal open={$addBookOpen} onclose={() => closeAddBook()} labelledBy="add-book-title">
  <header class="flex items-center justify-between px-6 pt-5 pb-3">
    <h2 id="add-book-title" class="text-[var(--fs-md)] font-semibold">{t("wizard.title")}</h2>
    <span class="text-[var(--fs-sm)] text-ink-3" aria-live="polite">
      {t("wizard.stepOf", { n: step, total: 5 })}
    </span>
  </header>

  <!-- Step rail: step 1 leads at the start (right in RTL); done steps keep their number
       as a check, so progress reads at a glance. -->
  <div class="flex items-center gap-2 border-b border-border-1 px-6 pb-3">
    {#each STEPS as key, i (key)}
      {@const n = i + 1}
      {@const state = n === step ? "now" : n < step ? "done" : "next"}
      <span
        class="inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[var(--fs-sm)] {state ===
        'now'
          ? 'bg-accent/10 font-medium text-accent'
          : state === 'done'
            ? 'text-ink-2'
            : 'text-ink-3'}"
      >
        <span
          class="inline-flex size-5 items-center justify-center rounded-full text-[var(--fs-xs)] {state ===
          'now'
            ? 'bg-accent text-accent-fg'
            : state === 'done'
              ? 'bg-accent/15 text-accent'
              : 'bg-surface-3 text-ink-3'}"
        >
          {#if state === "done"}
            <Check class="size-3 no-mirror" strokeWidth={3} />
          {:else}
            {n}
          {/if}
        </span>
        {t(key)}
      </span>
    {/each}
  </div>

  <div class="flex min-h-0 flex-1 flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5">
    {#if saveErrors.length > 0}
      <div
        role="alert"
        class="rounded-[var(--r-md)] border border-danger/40 bg-danger/5 p-[var(--sp-4)]"
      >
        <div class="flex items-center gap-2 text-[var(--fs-sm)] font-medium text-danger">
          <TriangleAlert class="size-4 no-mirror" strokeWidth={2} />
          {t("wizard.validation")}
        </div>
        <ul class="mt-1 list-inside list-disc text-[var(--fs-sm)] text-danger">
          {#each saveErrors as error (error)}
            <li dir="auto">{error}</li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if step === 1}
      <!-- The cover drop-zone (docs/04 §4.5) is not here on purpose: it lands with the
           cover backend (`crates/files` ingest + the cover commands), because a control
           that cannot store a cover would be a lie in the UI. -->

      <!-- Two entry paths, stated up front. `manual` is the default because it is the common
           case: most of a personal library has no ISBN on the spine, and a wizard that opens
           on a number field reads as a wall even when the number is optional. -->
      <div class="flex flex-col gap-[var(--sp-2)]">
        <span class="text-[var(--fs-sm)] font-medium text-ink-2">
          {t("wizard.source.label")}
        </span>
        <div
          class="flex rounded-[var(--r-md)] border border-border-1 bg-surface-1 p-0.5"
          role="radiogroup"
          aria-label={t("wizard.source.label")}
        >
          {#each SOURCE_MODES as mode (mode)}
            <button
              type="button"
              role="radio"
              aria-checked={draft.sourceMode === mode}
              onclick={() => (draft.sourceMode = mode)}
              class="flex-1 rounded-[var(--r-sm)] px-[var(--sp-3)] py-1.5 text-[var(--fs-sm)] transition-colors {draft.sourceMode ===
              mode
                ? 'bg-accent text-accent-fg'
                : 'text-ink-2 hover:bg-surface-2'}"
            >
              {mode === "isbn" ? t("wizard.source.isbn") : t("wizard.source.manual")}
            </button>
          {/each}
        </div>
        <span class="text-[var(--fs-xs)] text-ink-3">
          {draft.sourceMode === "isbn"
            ? t("wizard.source.isbnHint")
            : t("wizard.source.manualHint")}
        </span>
      </div>

      {#if draft.sourceMode === "isbn"}
        <Field
          label={t("wizard.isbn")}
          for="wiz-isbn"
          hint={t("wizard.isbn.hint")}
          error={isbnInvalid ? t("wizard.isbn.invalid") : null}
        >
          <input
            id="wiz-isbn"
            bind:value={draft.isbn}
            placeholder="978-0-306-40615-7"
            class="bidi-ltr h-9 w-full rounded-[var(--r-md)] border bg-surface-1 px-[var(--sp-3)] font-mono text-[var(--fs-base)] outline-none focus:border-accent {isbnInvalid
              ? 'border-danger/60'
              : 'border-border-2'}"
          />
        </Field>
      {/if}

    {:else if step === 2}
      <Field label={t("wizard.field.title")} required for="wiz-title">
        <input
          id="wiz-title"
          bind:value={draft.title}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
        />
      </Field>

      <Field label={t("wizard.field.subtitle")} for="wiz-subtitle" hint={t("wizard.optional")}>
        <input
          id="wiz-subtitle"
          bind:value={draft.subtitle}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
        />
      </Field>

      <div class="grid gap-[var(--sp-4)] sm:grid-cols-2">
        <Combobox
          label={t("wizard.field.authors")}
          value={authorRef}
          onchange={setAuthor}
          options={authorOptions}
          placeholder={t("common.search")}
          createLabel={t("wizard.add", { name: t("wizard.field.authors") })}
          oncreate={createAuthor}
          removeLabel={t("common.delete")}
        />
        <Combobox
          label={t("wizard.field.publisher")}
          value={draft.publisher}
          onchange={(next) => (draft.publisher = next)}
          options={publisherOptions}
          placeholder={t("common.search")}
          createLabel={t("wizard.add", { name: t("wizard.field.publisher") })}
          oncreate={createPublisher}
          removeLabel={t("common.delete")}
          hint={t("wizard.optional")}
        />
      </div>

      <div class="grid gap-[var(--sp-4)] sm:grid-cols-[1fr_150px]">
        <Combobox
          label={t("wizard.field.series")}
          value={draft.series}
          onchange={(next) => (draft.series = next)}
          options={seriesOptions}
          placeholder={t("common.search")}
          createLabel={t("wizard.add", { name: t("wizard.field.series") })}
          oncreate={createSeries}
          removeLabel={t("common.delete")}
          hint={t("wizard.optional")}
        />
        <Field label={t("wizard.field.seriesOrder")} for="wiz-order" hint={t("wizard.optional")}>
          <input
            id="wiz-order"
            inputmode="numeric"
            bind:value={draft.seriesOrder}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          />
        </Field>
      </div>

      <div class="grid gap-[var(--sp-4)] sm:grid-cols-3">
        <Field label={t("wizard.field.language")} for="wiz-lang">
          <select
            id="wiz-lang"
            bind:value={draft.languageCode}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          >
            {#each languageOptions as lang (lang.code)}
              <option value={lang.code}>{lang.name}</option>
            {/each}
          </select>
        </Field>
        <Field label={t("wizard.field.pages")} for="wiz-pages" hint={t("wizard.optional")}>
          <input
            id="wiz-pages"
            inputmode="numeric"
            bind:value={draft.pages}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          />
        </Field>
        <Field label={t("wizard.field.year")} for="wiz-year" hint={t("wizard.optional")}>
          <input
            id="wiz-year"
            inputmode="numeric"
            bind:value={draft.year}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          />
        </Field>
      </div>

      <div class="grid gap-[var(--sp-4)] sm:grid-cols-2">
        <Field label={t("wizard.field.edition")} for="wiz-edition" hint={t("wizard.optional")}>
          <input
            id="wiz-edition"
            bind:value={draft.edition}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          />
        </Field>
        <Field label={t("wizard.field.format")} for="wiz-format" hint={t("wizard.optional")}>
          <select
            id="wiz-format"
            bind:value={draft.format}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          >
            <option value="">{t("common.none")}</option>
            {#each FORMATS as format (format)}
              <option value={format}>{t(`wizard.format.${format}` as MessageKey)}</option>
            {/each}
          </select>
        </Field>
      </div>

    {:else if step === 3}
      <Combobox
        label={t("wizard.field.categories")}
        value={categoryRef}
        onchange={setCategory}
        options={categoryOptions}
        placeholder={t("common.search")}
        createLabel={t("wizard.add", { name: t("wizard.field.categories") })}
        oncreate={createCategory}
        removeLabel={t("common.delete")}
      />

      <TagInput
        label={t("wizard.field.tags")}
        value={draft.tags}
        onchange={(next) => (draft.tags = next)}
        placeholder={t("wizard.tags.placeholder")}
        hint={t("wizard.tags.hint")}
        removeLabel={t("common.delete")}
      />

      <div class="grid gap-[var(--sp-4)] sm:grid-cols-2">
        <Field label={t("wizard.field.status")} for="wiz-status">
          <select
            id="wiz-status"
            bind:value={draft.status}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          >
            {#each STATUSES as status (status)}
              <option value={status}>{t(`status.${status}` as MessageKey)}</option>
            {/each}
          </select>
        </Field>
        <div class="flex flex-col gap-1">
          <span class="text-[var(--fs-sm)] text-ink-2">{t("wizard.field.rating")}</span>
          <div class="flex h-9 items-center">
            <Stars
              value={draft.rating}
              label={t("wizard.field.rating")}
              onchange={(next) => (draft.rating = next)}
            />
          </div>
        </div>
      </div>

      <Field label={t("wizard.field.description")} for="wiz-desc" hint={t("wizard.optional")}>
        <textarea
          id="wiz-desc"
          rows="4"
          bind:value={draft.description}
          class="w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] py-2 text-[var(--fs-base)] outline-none focus:border-accent"
        ></textarea>
      </Field>

    {:else if step === 4}
      <!-- Files: what will be attached, and how. The attach itself happens in `submit`,
           after the book exists — this step only records the choice. -->
      <div class="flex flex-col gap-[var(--sp-4)]">
        <div>
          <span class="text-[var(--fs-sm)] text-ink-2">{t("wizard.files.title")}</span>
          <p class="mt-1 text-[var(--fs-sm)] text-ink-3">{t("wizard.files.body")}</p>
        </div>

        {#if draft.files.length === 0}
          <p
            class="rounded-[var(--r-md)] border border-border-1 bg-surface-2 px-[var(--sp-4)] py-[var(--sp-3)] text-[var(--fs-sm)] text-ink-3"
          >
            {t("wizard.files.none")}
          </p>
        {:else}
          <ul class="flex flex-col gap-2">
            {#each draft.files as file, i (file.path)}
              <li
                class="flex flex-wrap items-center gap-2 rounded-[var(--r-md)] border border-border-1 bg-surface-1 px-[var(--sp-3)] py-2"
              >
                <FileText class="size-4 shrink-0 text-ink-3 no-mirror" strokeWidth={1.75} />
                <span class="min-w-0 flex-1 truncate text-[var(--fs-base)] text-ink-1" dir="auto">
                  {fileName(file.path)}
                </span>
                <span
                  class="rounded-[var(--r-full)] border border-border-2 bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2"
                >
                  {kindLabel(file.kind)}
                </span>
                <button
                  type="button"
                  class="text-[var(--fs-sm)] text-ink-3 transition-colors hover:text-danger"
                  onclick={() => removeFile(i)}
                >
                  {t("common.delete")}
                </button>
              </li>
            {/each}
          </ul>
        {/if}

        <div class="flex flex-wrap items-center gap-2.5">
          <Button variant="secondary" loading={picking} onclick={() => void pickFile()}>
            {#snippet iconStart()}
              <FileText class="size-4 no-mirror" strokeWidth={1.75} />
            {/snippet}
            {t("wizard.files.pick")}
          </Button>
          {#if draft.files.length > 0}
            <span class="text-[var(--fs-sm)] text-ink-3">
              {t("wizard.files.willAttach", { n: draft.files.length })}
            </span>
          {/if}
        </div>

        <Field
          label={t("wizard.files.storageMode")}
          for="wiz-file-mode"
          hint={t("wizard.files.mode.hint")}
        >
          <select
            id="wiz-file-mode"
            value={fileMode}
            onchange={(e) => setFileMode(e.currentTarget.value as StorageMode)}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          >
            <option value="copy">{t("wizard.files.mode.copy")}</option>
            <option value="reference">{t("wizard.files.mode.reference")}</option>
          </select>
        </Field>
      </div>

    {:else if step === 5}
      <!-- Review: terms at the start, values at the end — the meta table of the book page. -->
      <div class="flex flex-col gap-[var(--sp-4)] sm:flex-row">
        <dl class="min-w-0 flex-1 text-[var(--fs-base)]">
          {#snippet row(label: string, value: string)}
            <div class="flex items-baseline justify-between gap-4 py-1">
              <dt class="shrink-0 text-[var(--fs-sm)] text-ink-3">{label}</dt>
              <dd class="min-w-0 truncate text-ink-1" dir="auto">{value}</dd>
            </div>
          {/snippet}

          {@render row(t("wizard.field.title"), draft.title)}
          {#if draft.subtitle.trim()}
            {@render row(t("wizard.field.subtitle"), draft.subtitle)}
          {/if}
          {#if authorRef}
            {@render row(t("wizard.field.authors"), authorRef.name)}
          {/if}
          {#if draft.publisher}
            {@render row(t("wizard.field.publisher"), draft.publisher.name)}
          {/if}
          {#if draft.series}
            {@render row(
              t("wizard.field.series"),
              draft.seriesOrder.trim()
                ? `${draft.series.name} · ${draft.seriesOrder}`
                : draft.series.name,
            )}
          {/if}
          {#if draft.isbn.trim()}
            {@render row(t("wizard.isbn"), canonicalIsbn(draft.isbn))}
          {/if}
          {#if draft.year.trim() || draft.pages.trim()}
            {@render row(
              `${t("wizard.field.year")} · ${t("wizard.field.pages")}`,
              [draft.year, draft.pages].filter(Boolean).join(" · "),
            )}
          {/if}
          {#if categoryRef}
            {@render row(t("wizard.field.categories"), categoryRef.name)}
          {/if}
          {#if draft.tags.length > 0}
            {@render row(t("wizard.field.tags"), draft.tags.join(" · "))}
          {/if}
          {#if draft.edition.trim()}
            {@render row(t("wizard.field.edition"), draft.edition)}
          {/if}
          {#if draft.format}
            {@render row(
              t("wizard.field.format"),
              t(`wizard.format.${draft.format as BookFormat}` as MessageKey),
            )}
          {/if}
          {@render row(t("wizard.field.status"), t(`status.${draft.status}` as MessageKey))}
          {#if draft.files.length > 0}
            {@render row(
              t("wizard.steps.files"),
              draft.files.map((f) => fileName(f.path)).join(" · "),
            )}
          {/if}
          {#if draft.description.trim()}
            {@render row(t("wizard.field.description"), draft.description)}
          {/if}
        </dl>

        <div class="flex items-start justify-center sm:w-32">
          <Stars value={draft.rating} readonly label={t("wizard.field.rating")} size="lg" />
        </div>
      </div>

      {#if dupMatches.length > 0}
        <div
          role="alert"
          class="rounded-[var(--r-md)] border border-warning/40 bg-warning-soft p-[var(--sp-4)]"
        >
          <div class="flex items-center gap-2 text-[var(--fs-sm)] font-medium text-warning">
            <TriangleAlert class="size-4 no-mirror" strokeWidth={2} />
            {t("wizard.review.duplicate")}
          </div>
          <p class="mt-1 text-[var(--fs-sm)] text-ink-2">{t("wizard.review.duplicateBody")}</p>
          <ul class="mt-2 flex flex-col gap-1">
            {#each dupMatches as book (book.id)}
              <li class="text-[var(--fs-sm)] text-ink-1" dir="auto">
                {book.title}{#if book.publication_year}&nbsp;·&nbsp;{book.publication_year}{/if}
              </li>
            {/each}
          </ul>
          <div class="mt-3">
            <Button variant="primary" size="sm" onclick={() => void submit(true)}>
              {t("wizard.review.override")}
            </Button>
          </div>
        </div>
      {/if}
    {/if}
  </div>

  <!-- DOM order is the logical one — cancel, back, next — which RTL mirrors into the
       designed footer: primary action at the end (left), إلغاء at the start (right). -->
  <footer class="flex items-center justify-between border-t border-border-1 px-6 py-4">
    <Button variant="ghost" onclick={() => closeAddBook()} disabled={saving}>
      {t("common.cancel")}
    </Button>
    <div class="flex items-center gap-2.5">
      {#if step > 1}
        <Button onclick={back} disabled={saving}>{t("common.back")}</Button>
      {/if}
      {#if step < 5}
        <Button variant="primary" onclick={next} disabled={saving}>{t("common.next")}</Button>
      {:else}
        <Button variant="primary" loading={saving} onclick={() => void submit(false)}>
          {#snippet iconStart()}
            <Check class="size-4 no-mirror" strokeWidth={2.5} />
          {/snippet}
          {t("common.save")}
        </Button>
      {/if}
    </div>
  </footer>
</Modal>
