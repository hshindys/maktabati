<script lang="ts">
  import { Check, Trash2, TriangleAlert } from "@lucide/svelte";

  import Button from "../components/Button.svelte";
  import Combobox from "../components/Combobox.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import Stars from "../components/Stars.svelte";
  import TagInput from "../components/TagInput.svelte";

  import * as api from "../api";
  import { locale, translatorFor, type MessageKey } from "../i18n";
  import { messageFor } from "../errors";
  import type { EntityRef } from "../draft";
  import { pushToast } from "../toast";
  import type {
    Author,
    AuthorCredit,
    AuthorRole,
    BookDetail,
    BookFormat,
    BookInput,
    Category,
    Language,
    Publisher,
    ReadingStatus,
    Series,
    Tag,
  } from "@/bindings";

  /**
   * Edit book — the per-book edit modal, opened from the book page header.
   *
   * `book_update` replaces the scalar fields *and* the credits/categories/tags in one
   * transaction, so the form always submits a complete `BookInput`: it starts from the
   * stored row (acquisition, location, cover, ISBN, source stay untouched) and edits only
   * what the screen exposes. Typed-but-unknown entities go through the same resolve pass
   * as the add wizard (create, then use the id), and every failure lands in the panel
   * instead of a toast — one surface for one problem.
   */

  interface Props {
    open: boolean;
    detail: BookDetail;
    authors: Author[];
    publishers: Publisher[];
    seriesList: Series[];
    categories: Category[];
    tags: Tag[];
    onclose: () => void;
    /** Called after a successful save so the caller can reload its data. */
    onsaved: () => void;
  }

  let {
    open,
    detail,
    authors,
    publishers,
    seriesList,
    categories,
    tags,
    onclose,
    onsaved,
  }: Props = $props();

  let t = $derived(translatorFor($locale));

  const FORMATS: BookFormat[] = ["print", "ebook", "audiobook", "pdf", "other"];
  const STATUSES: ReadingStatus[] = ["to_read", "reading", "finished", "paused", "abandoned"];
  const ROLES: AuthorRole[] = ["author", "translator", "editor", "illustrator", "foreword", "other"];

  /** One editable credit line — `id: null` means "typed but not created yet". */
  interface CreditRow {
    id: string | null;
    name: string;
    role: AuthorRole;
  }

  let title = $state("");
  let subtitle = $state("");
  let year = $state("");
  let pages = $state("");
  let edition = $state("");
  let seriesOrder = $state("");
  let description = $state("");
  let status = $state<ReadingStatus>("to_read");
  let format = $state<BookFormat | "">("");
  let languageCode = $state("ar");
  let rating = $state(0);

  let credits = $state<CreditRow[]>([]);
  let publisher = $state<EntityRef | null>(null);
  let series = $state<EntityRef | null>(null);
  let categoryRef = $state<EntityRef | null>(null);
  let tagNames = $state<string[]>([]);

  let languages = $state<Language[]>([]);
  let saveErrors = $state<string[]>([]);
  let saving = $state(false);

  // Seed the form whenever the modal opens (finish the edit, reopen the form).
  $effect(() => {
    if (!open) return;
    const book = detail.book;
    title = book.title;
    subtitle = book.subtitle ?? "";
    year = book.publication_year === null ? "" : String(book.publication_year);
    pages = book.page_count === null ? "" : String(book.page_count);
    edition = book.edition ?? "";
    seriesOrder = book.series_order === null ? "" : String(book.series_order);
    description = book.description ?? "";
    status = book.reading_status;
    format = book.format;
    languageCode = book.language_code;
    rating = book.rating ?? 0;
    credits = detail.authors.map((c: AuthorCredit) => ({ id: c.author_id, name: "", role: c.role }));
    publisher = book.publisher_id ? { id: book.publisher_id, name: publisherName(book.publisher_id) ?? "" } : null;
    series = book.series_id ? { id: book.series_id, name: seriesName(book.series_id) ?? "" } : null;
    categoryRef = detail.categories[0] ? { id: detail.categories[0], name: categoryName(detail.categories[0]) } : null;
    tagNames = detail.tags.map((tid) => tagName(tid));
    saveErrors = [];
  });

  // The other-language lists this form needs beyond what the book page already loaded.
  $effect(() => {
    if (!open || languages.length > 0) return;
    void api.listActiveLanguages().then((res) => {
      if (res.status === "ok") languages = res.data;
    });
  });

  // ── Name resolution for the rows seeded from ids ────────────────────────────────────────

  let authorById = $derived(new Map(authors.map((a) => [a.id, a])));
  const publisherById = $derived(new Map(publishers.map((p) => [p.id, p])));
  const seriesById = $derived(new Map(seriesList.map((s) => [s.id, s])));
  const categoryById = $derived(new Map(categories.map((c) => [c.id, c])));
  const tagById = $derived(new Map(tags.map((tg) => [tg.id, tg])));

  function publisherName(pid: string): string | null {
    return publisherById.get(pid)?.name ?? pid.slice(0, 8);
  }
  function seriesName(sid: string): string | null {
    return seriesById.get(sid)?.name ?? sid.slice(0, 8);
  }
  function categoryName(cid: string): string {
    const c = categoryById.get(cid);
    if (c === undefined) return cid.slice(0, 8);
    return $locale === "ar" ? c.name_ar : c.name_en;
  }
  function tagName(tid: string): string {
    return tagById.get(tid)?.name ?? tid.slice(0, 8);
  }

  /** A credit whose author row is gone still shows its id fragment instead of vanishing. */
  function seedCredit(credit: { id: string | null; name: string; role: AuthorRole }): EntityRef | null {
    if (credit.id === null) return credit.name ? { id: null, name: credit.name } : null;
    const known = authorById.get(credit.id);
    return { id: credit.id, name: known?.name ?? credit.id.slice(0, 8) };
  }

  let languageOptions = $derived(
    languages.map((x) => ({ code: x.code, name: $locale === "ar" ? x.name_ar : x.name_en })),
  );
  let authorOptions = $derived(authors.map((x): EntityRef => ({ id: x.id, name: x.name })));
  let publisherOptions = $derived(publishers.map((x): EntityRef => ({ id: x.id, name: x.name })));
  let seriesOptions = $derived(seriesList.map((x): EntityRef => ({ id: x.id, name: x.name })));
  let categoryOptions = $derived(categories.map((x): EntityRef => ({ id: x.id, name: categoryName(x.id) })));

  function setCreditAuthor(index: number, next: EntityRef | null): void {
    const row = credits[index];
    if (row === undefined) return;
    if (next === null) {
      credits = credits.filter((_, i) => i !== index);
    } else {
      credits = credits.map((c, i) => (i === index ? { ...c, id: next.id, name: next.name } : c));
    }
  }

  function addCredit(next: EntityRef | null): void {
    if (next === null || next.id === null) return; // picks only; creation goes through oncreate
    if (credits.some((c) => c.id === next.id)) return; // one credit per author
    credits = [...credits, { id: next.id, name: next.name, role: "author" as AuthorRole }];
  }

  function removeCredit(index: number): void {
    credits = credits.filter((_, i) => i !== index);
  }

  function setCreditRole(index: number, next: string): void {
    credits = credits.map((c, i) => (i === index ? { ...c, role: next as AuthorRole } : c));
  }

  // ── Inline create (same discipline as the wizard) ───────────────────────────────────────

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
    if (res.status === "ok") return { id: res.data.id, name };
    saveErrors = [...saveErrors, `${t("wizard.field.authors")} «${name}»: ${messageFor(res.error, t)}`];
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
    if (res.status === "ok") return { id: res.data.id, name };
    saveErrors = [...saveErrors, `${t("wizard.field.publisher")} «${name}»: ${messageFor(res.error, t)}`];
    return { id: null, name };
  }

  async function createSeries(name: string): Promise<EntityRef> {
    const res = await api.createSeries({ name, description: null });
    if (res.status === "ok") return { id: res.data.id, name };
    saveErrors = [...saveErrors, `${t("wizard.field.series")} «${name}»: ${messageFor(res.error, t)}`];
    return { id: null, name };
  }

  // ── Validation and save ─────────────────────────────────────────────────────────────────

  function intOrNull(raw: string): number | null {
    const s = raw.trim();
    if (!s) return null;
    const n = Number(s);
    return Number.isInteger(n) ? n : null;
  }

  function floatOrNull(raw: string): number | null {
    const s = raw.trim().replace(",", ".");
    if (!s) return null;
    const n = Number(s);
    return Number.isFinite(n) ? n : null;
  }

  function validate(): string[] {
    const errors: string[] = [];
    if (title.trim() === "") errors.push(`${t("wizard.field.title")}: ${t("error.validation.empty_field")}`);
    for (const [label, raw, parse] of [
      [t("wizard.field.year"), year, intOrNull],
      [t("wizard.field.pages"), pages, intOrNull],
      [t("wizard.field.seriesOrder"), seriesOrder, floatOrNull],
    ] as const) {
      if (raw.trim() !== "" && parse(raw) === null) errors.push(`${label}: ${t("wizard.error.number")}`);
    }
    return errors;
  }

  async function submit(): Promise<void> {
    if (saving) return;
    saveErrors = [];
    const errors = validate();
    if (errors.length > 0) {
      saveErrors = errors;
      return;
    }

    saving = true;
    try {
      const errs: string[] = [];
      const outCredits: AuthorCredit[] = [];
      for (const row of credits) {
        let id = row.id;
        if (!id && row.name.trim() !== "") {
          const created = await createAuthor(row.name.trim());
          id = created.id;
        }
        if (id) outCredits.push({ author_id: id, role: row.role });
      }

      let publisherId = publisher?.id ?? null;
      if (!publisherId && publisher && publisher.name.trim() !== "") {
        const created = await createPublisher(publisher.name.trim());
        publisherId = created.id;
      }

      let seriesId = series?.id ?? null;
      if (!seriesId && series && series.name.trim() !== "") {
        const created = await createSeries(series.name.trim());
        seriesId = created.id;
      }

      const tagIds: string[] = [];
      for (const name of tagNames) {
        const known = tags.find((x) => x.name.toLocaleLowerCase() === name.toLocaleLowerCase());
        if (known) {
          tagIds.push(known.id);
          continue;
        }
        const res = await api.createTag({ name, color: null });
        if (res.status === "ok") tagIds.push(res.data.id);
        else errs.push(`${t("wizard.field.tags")} «${name}»: ${messageFor(res.error, t)}`);
      }

      if (errs.length > 0) {
        saveErrors = errs;
        return;
      }

      // Everything the form does not expose goes back exactly as the row stores it.
      const book = detail.book;
      const input: BookInput = {
        title: title.trim(),
        subtitle: subtitle.trim() || null,
        original_title: book.original_title,
        language_code: languageCode || null,
        isbn10: book.isbn10,
        isbn13: book.isbn13,
        edition: edition.trim() || null,
        publication_year: intOrNull(year),
        page_count: intOrNull(pages),
        format: (FORMATS.includes(format as BookFormat) ? format : null) as BookFormat | null,
        description: description.trim() || null,
        series_id: seriesId,
        series_order: floatOrNull(seriesOrder),
        room: book.room,
        shelf: book.shelf,
        box_name: book.box_name,
        acquired_date: book.acquired_date,
        acquired_from: book.acquired_from,
        price_minor: book.price_minor,
        price_currency: book.price_currency,
        condition: book.condition,
        rating: rating > 0 ? rating : null,
        current_page: book.current_page,
        reading_status: status,
        cover_path: book.cover_path,
        cover_source: book.cover_source,
        source: book.source,
        external_ids: book.external_ids,
        authors: outCredits,
        publisher_id: publisherId,
        categories: categoryRef?.id ? [categoryRef.id] : [],
        tags: tagIds,
      };

      const res = await api.updateBook(book.id, input);
      if (res.status !== "ok") {
        saveErrors = [messageFor(res.error, t)];
        return;
      }
      pushToast(t("toast.saved"), { tone: "success" });
      onsaved();
      onclose();
    } finally {
      saving = false;
    }
  }
</script>

<Modal {open} onclose={() => onclose()} labelledBy="book-edit-title">
  <header class="flex items-center justify-between px-6 pt-5 pb-3">
    <h2 id="book-edit-title" class="text-[var(--fs-md)] font-semibold">{t("book.edit")}</h2>
  </header>

  <div class="flex min-h-0 flex-1 flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5">
    {#if saveErrors.length > 0}
      <div role="alert" class="rounded-[var(--r-md)] border border-danger/40 bg-danger/5 p-[var(--sp-4)]">
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

    <Field label={t("wizard.field.title")} required for="edit-title">
      <input
        id="edit-title"
        bind:value={title}
        class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
      />
    </Field>

    <Field label={t("wizard.field.subtitle")} for="edit-subtitle" hint={t("wizard.optional")}>
      <input
        id="edit-subtitle"
        bind:value={subtitle}
        class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
      />
    </Field>

    <!-- Credits: one row per author line, exactly as book_update will store them. -->
    <div class="flex flex-col gap-2">
      <span class="text-[var(--fs-sm)] text-ink-2">{t("wizard.field.authors")}</span>
      {#each credits as row, i (`${row.id ?? row.name}-${i}`)}
        {@const ref = seedCredit(row)}
        <div class="flex items-end gap-2">
          <div class="min-w-0 flex-1">
            <Combobox
              label={t("wizard.field.authors")}
              removeLabel={t("common.delete")}
              value={ref}
              options={authorOptions}
              onchange={(next) => setCreditAuthor(i, next)}
              placeholder={t("common.search")}
              createLabel={t("wizard.add", { name: t("wizard.field.authors") })}
              oncreate={createAuthor}
            />
          </div>
          <div class="w-40">
            <Field label={t("book.edit.role")} for={`edit-role-${i}`} hint={null}>
              <select
                id={`edit-role-${i}`}
                value={row.role}
                onchange={(e) => setCreditRole(i, e.currentTarget.value)}
                class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-2)] text-[var(--fs-sm)] outline-none focus:border-accent"
              >
                {#each ROLES as role (role)}
                  <option value={role}>{t(`role.${role}` as MessageKey)}</option>
                {/each}
              </select>
            </Field>
          </div>
          <button
            type="button"
            aria-label={t("common.delete")}
            title={t("common.delete")}
            class="mb-1 inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:border-danger/50 hover:text-danger"
            onclick={() => removeCredit(i)}
          >
            <Trash2 class="size-4 no-mirror" strokeWidth={1.75} />
          </button>
        </div>
      {/each}

      <div class="w-72">
        <Combobox
          label={t("book.edit.addCredit")}
          removeLabel={t("common.delete")}
          value={null}
          options={authorOptions}
          onchange={(next) => addCredit(next)}
          placeholder={t("common.search")}
        />
      </div>
    </div>

    <div class="grid gap-[var(--sp-4)] sm:grid-cols-2">
      <Combobox
        label={t("wizard.field.publisher")}
        value={publisher}
        onchange={(next) => (publisher = next)}
        options={publisherOptions}
        placeholder={t("common.search")}
        createLabel={t("wizard.add", { name: t("wizard.field.publisher") })}
        oncreate={createPublisher}
        removeLabel={t("common.delete")}
        hint={t("wizard.optional")}
      />
      <div class="grid grid-cols-[1fr_120px] gap-2">
        <Combobox
          label={t("wizard.field.series")}
          value={series}
          onchange={(next) => (series = next)}
          options={seriesOptions}
          placeholder={t("common.search")}
          createLabel={t("wizard.add", { name: t("wizard.field.series") })}
          oncreate={createSeries}
          removeLabel={t("common.delete")}
          hint={t("wizard.optional")}
        />
        <Field label={t("wizard.field.seriesOrder")} for="edit-series-order" hint={null}>
          <input
            id="edit-series-order"
            bind:value={seriesOrder}
            class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
          />
        </Field>
      </div>
    </div>

    <div class="grid gap-[var(--sp-4)] sm:grid-cols-3">
      <Field label={t("wizard.field.language")} for="edit-lang">
        <select
          id="edit-lang"
          bind:value={languageCode}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
        >
          {#if languageOptions.length === 0}
            <option value={languageCode}>{languageCode}</option>
          {/if}
          {#each languageOptions as lang (lang.code)}
            <option value={lang.code}>{lang.name}</option>
          {/each}
        </select>
      </Field>
      <Field label={t("wizard.field.pages")} for="edit-pages" hint={t("wizard.optional")}>
        <input
          id="edit-pages"
          inputmode="numeric"
          bind:value={pages}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
        />
      </Field>
      <Field label={t("wizard.field.year")} for="edit-year" hint={t("wizard.optional")}>
        <input
          id="edit-year"
          inputmode="numeric"
          bind:value={year}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
        />
      </Field>
    </div>

    <div class="grid gap-[var(--sp-4)] sm:grid-cols-2">
      <Field label={t("wizard.field.edition")} for="edit-edition" hint={t("wizard.optional")}>
        <input
          id="edit-edition"
          bind:value={edition}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
        />
      </Field>
      <Field label={t("wizard.field.format")} for="edit-format" hint={t("wizard.optional")}>
        <select
          id="edit-format"
          bind:value={format}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
        >
          <option value="">{t("common.none")}</option>
          {#each FORMATS as f (f)}
            <option value={f}>{t(`wizard.format.${f}` as MessageKey)}</option>
          {/each}
        </select>
      </Field>
    </div>

    <div class="grid gap-[var(--sp-4)] sm:grid-cols-2">
      <Field label={t("wizard.field.status")} for="edit-status">
        <select
          id="edit-status"
          bind:value={status}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
        >
          {#each STATUSES as s (s)}
            <option value={s}>{t(`status.${s}` as MessageKey)}</option>
          {/each}
        </select>
      </Field>
      <div class="flex flex-col gap-1">
        <span class="text-[var(--fs-sm)] text-ink-2">{t("wizard.field.rating")}</span>
        <div class="flex h-9 items-center">
          <Stars value={rating} label={t("wizard.field.rating")} onchange={(next) => (rating = next)} />
        </div>
      </div>
    </div>

    <Combobox
      label={t("wizard.field.categories")}
      value={categoryRef}
      onchange={(next) => (categoryRef = next)}
      options={categoryOptions}
      placeholder={t("common.search")}
      removeLabel={t("common.delete")}
      hint={t("wizard.optional")}
    />

    <TagInput
      label={t("wizard.field.tags")}
      value={tagNames}
      onchange={(next) => (tagNames = next)}
      placeholder={t("wizard.tags.placeholder")}
      hint={t("wizard.tags.hint")}
      removeLabel={t("common.delete")}
    />

    <Field label={t("wizard.field.description")} for="edit-desc" hint={t("wizard.optional")}>
      <textarea
        id="edit-desc"
        rows="4"
        bind:value={description}
        class="w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] py-2 text-[var(--fs-base)] outline-none focus:border-accent"
      ></textarea>
    </Field>
  </div>

  <footer class="flex items-center justify-between border-t border-border-1 px-6 py-4">    <Button variant="ghost" onclick={() => onclose()} disabled={saving}>{t("common.cancel")}</Button>
    <Button variant="primary" loading={saving} onclick={() => void submit()}>
      {#snippet iconStart()}
        <Check class="size-4 no-mirror" strokeWidth={2.5} />
      {/snippet}
      {t("common.save")}
    </Button>
  </footer>
</Modal>
