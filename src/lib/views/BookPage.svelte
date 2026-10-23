<script lang="ts">
  import {
    ArrowLeft,
    ArrowRight,
    ExternalLink,
    FileText,
    FolderOpen,
    Paperclip,
    Pencil,
    Quote as QuoteIcon,
    StickyNote,
    Trash2,
    TriangleAlert,
  } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";

  import {
    attachFile,
    deleteBook,
    deleteFile,
    deleteQuote,
    getBookDetail,
    listAuthors,
    listBookQuotes,
    listCategories,
    listFiles,
    listPublishers,
    listSeries,
    listTags,
    openFile,
    revealFile,
  } from "../api";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import CoverPlaceholder from "../components/CoverPlaceholder.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Field from "../components/Field.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import Stars from "../components/Stars.svelte";
  import { direction, locale, translatorFor, type MessageKey } from "../i18n";
  import { fileName, pickFiles } from "../pickFile";
  import { messageFor } from "../errors";
  import { toast } from "../toast";
  import { formatDate, formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { go, route } from "../router";
  import BookEdit from "./BookEdit.svelte";
  import QuoteEdit from "./QuoteEdit.svelte";
  import type {
    AppErrorDto,
    Author,
    AuthorCredit,
    BookDetail,
    BookFile,
    BookFormat,
    BookSource,
    Category,
    Condition,
    FileKind,
    Publisher,
    Quote,
    ReadingStatus,
    Series,
    StorageMode,
    Tag,
  } from "@/bindings";

  /**
   * Book page — one book, seven tabs (docs/04-ux-design.md §4.3).
   *
   * This is the screen a library without it cannot honestly call a library: every row in the
   * browser used to be a dead end (P1-6). `book_detail` already returns the book and its
   * author credits in one call, so the page asks once and resolves names from the catalogue
   * lists rather than issuing a query per relation — the difference between one round trip
   * and five on a 50k-book machine.
   *
   * Two tabs behave differently on purpose: quotes have a real chain now (repo → commands →
   * this tab), loaded when their tab opens like the files tab below. Notes still have no
   * backend and render their designed empty state with a "later milestone" note rather than a
   * fake list, because a placeholder that pretends to hold data is worse than one that says
   * it is empty.
   *
   * The files tab is the exception: `book_files` and `repo::file` exist, so it lists real
   * attachments, opens them with the system handler, and asks before removing one. It loads
   * when the tab opens rather than with the page — a book page is opened far more often than
   * its attachments are looked at.
   */

  type TabKey = "overview" | "details" | "reading" | "organize" | "notes" | "quotes" | "files";

  const TABS: Array<{ key: TabKey; icon: typeof StickyNote; labelKey: MessageKey }> = [
    { key: "overview", icon: StickyNote, labelKey: "book.tab.overview" },
    { key: "details", icon: FileText, labelKey: "book.tab.details" },
    { key: "reading", icon: StickyNote, labelKey: "book.tab.reading" },
    { key: "organize", icon: StickyNote, labelKey: "book.tab.organize" },
    { key: "notes", icon: StickyNote, labelKey: "book.tab.notes" },
    { key: "quotes", icon: QuoteIcon, labelKey: "book.tab.quotes" },
    { key: "files", icon: FileText, labelKey: "book.tab.files" },
  ];

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  let id = $derived($route.params.id ?? "");

  let detail = $state<BookDetail | null>(null);
  let loadError = $state<AppErrorDto | null>(null);
  let missing = $state(false);
  let loading = $state(true);
  let tab = $state<TabKey>("overview");

  // The catalogue lists, loaded with the book so names resolve in the same paint. In a
  // browser preview these all fail with `shell.unavailable` and the maps stay empty.
  let authors = $state<Author[]>([]);
  let categories = $state<Category[]>([]);
  let tags = $state<Tag[]>([]);
  let publishers = $state<Publisher[]>([]);
  let seriesList = $state<Series[]>([]);

  async function load(): Promise<void> {
    if (id.length === 0) {
      loading = false;
      missing = true;
      return;
    }
    loading = true;
    missing = false;
    loadError = null;
    detail = null;

    const [d, a, c, tg, p, s] = await Promise.all([
      getBookDetail(id),
      listAuthors(),
      listCategories(),
      listTags(),
      listPublishers(),
      listSeries(),
    ]);

    if (a.status === "ok") authors = a.data;
    if (c.status === "ok") categories = c.data;
    if (tg.status === "ok") tags = tg.data;
    if (p.status === "ok") publishers = p.data;
    if (s.status === "ok") seriesList = s.data;

    if (d.status === "ok") {
      detail = d.data;
    } else if (d.error.code === "storage.not_found") {
      missing = true;
    } else {
      loadError = d.error;
    }
    loading = false;
  }

  onMount(() => void load());

  // ── Edit and delete the book itself ────────────────────────────────────────────────────
  //
  // `book_update`/`book_delete` exist and are tested; the page is the place the user looks
  // for them. Delete walks the same native-confirm path an attachment delete does — the
  // whole record (quotes, notes, files) goes with the book and only a confirm stands guard.

  let editOpen = $state(false);
  let bookDeleting = false;

  async function removeBook(): Promise<void> {
    if (bookDeleting || detail === null) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("book.deleteConfirm"), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    bookDeleting = true;
    try {
      const res = await deleteBook(id);
      if (res.status === "ok") {
        toast.success(t("toast.bookDeleted"));
        go("/library");
      } else {
        toast.error(messageFor(res.error, t));
      }
    } finally {
      bookDeleting = false;
    }
  }

  // ── Attachments ────────────────────────────────────────────────────────────────────────
  //
  // Loaded on demand when the files tab opens, and re-read by both mutating paths. A failed
  // read leaves the list alone and says why in a toast; a half-updated list is worse than an
  // empty one, because it looks like the truth.

  let files = $state<BookFile[]>([]);
  let filesLoading = $state(false);
  let filesFailed = $state(false);
  let fileMode = $state<StorageMode>("copy");
  let picking = $state(false);
  let deleting = $state(false);

  async function reloadFiles(): Promise<void> {
    const res = await listFiles(id);
    if (res.status === "ok") {
      files = res.data;
      filesFailed = false;
      return;
    }
    filesFailed = true;
    toast.error(messageFor(res.error, t));
  }

  $effect(() => {
    // `tab` and `id` are the only inputs, so switching tabs and changing book both re-run
    // the query; the cleanup cancels the response from the tab the user just left.
    if (tab !== "files" || id.length === 0) return;
    let cancelled = false;
    filesLoading = true;
    listFiles(id).then((res) => {
      if (cancelled) return;
      filesLoading = false;
      files = res.status === "ok" ? res.data : [];
      filesFailed = res.status !== "ok";
    });
    return () => {
      cancelled = true;
    };
  });

  /** The label a `FileKind` renders as — the kind's own vocabulary, not the book format's. */
  const kindLabel = (kind: FileKind): string => t(`wizard.fileKind.${kind}` as MessageKey);

  async function attachPicked(): Promise<void> {
    if (picking || id.length === 0) return;
    picking = true;
    try {
      const picked = await pickFiles(t("book.files.add"), {
        books: t("wizard.files.filterBooks"),
        audio: t("wizard.files.filterAudio"),
      });
      if (picked === null) return;
      // `copy` mode stores bytes, not the source path, so the only thing to compare is the
      // name. A second copy of the same file would come back as a duplicate the user never
      // asked for.
      const known = new Set(files.map((f) => f.original_name.toLocaleLowerCase()));
      const fresh = picked.filter((f) => !known.has(fileName(f.path).toLocaleLowerCase()));
      if (fresh.length === 0) return;

      let attached = 0;
      const failed: string[] = [];
      for (const file of fresh) {
        const res = await attachFile(id, file.kind, fileMode, file.path, null, null);
        if (res.status === "ok") attached += 1;
        else failed.push(`${fileName(file.path)}: ${messageFor(res.error, t)}`);
      }
      if (attached > 0) toast.success(t("toast.attached"));
      if (failed.length > 0) toast.error(failed.join(" · "));
      await reloadFiles();
    } finally {
      picking = false;
    }
  }

  async function removeFile(row: BookFile): Promise<void> {
    if (deleting) return;
    // A native question, not an undo affordance: removing an attachment can discard the only
    // copy of some bytes, and the OS dialog is the one place that is taken seriously.
    let confirmed = false;
    try {
      confirmed = await ask(t("book.files.deleteConfirm"), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;

    deleting = true;
    try {
      const res = await deleteFile(row.id);
      toast[res.status === "ok" ? "success" : "error"](
        res.status === "ok" ? t("toast.deleted") : messageFor(res.error, t),
      );
      await reloadFiles();
    } finally {
      deleting = false;
    }
  }

  /** Open and reveal go through the commands, which resolve the stored path from the row. */
  async function openAttachment(row: BookFile): Promise<void> {
    const res = await openFile(row.id);
    if (res.status !== "ok") toast.error(messageFor(res.error, t));
  }

  async function revealAttachment(row: BookFile): Promise<void> {
    const res = await revealFile(row.id);
    if (res.status !== "ok") toast.error(messageFor(res.error, t));
  }

  // ── Quotes ─────────────────────────────────────────────────────────────────────────────
  //
  // Same shape as the attachments block: loaded when the tab opens, re-read by both
  // mutating paths, and a native confirm before the destructive one. `quote_create` needs a
  // book id, which this page already owns — that is why a quote is added here and not from
  // the browser.

  let quotes = $state<Quote[]>([]);
  let quotesEditing = $state<Quote | null>(null);
  let quoteEditorOpen = $state(false);
  let quoteWorking = $state(false);

  async function reloadQuotes(): Promise<void> {
    const res = await listBookQuotes(id);
    if (res.status === "ok") quotes = res.data;
    else toast.error(messageFor(res.error, t));
  }

  $effect(() => {
    // Same discipline as the files tab above: run when the tab opens, cancel stale answers.
    if (tab !== "quotes" || id.length === 0) return;
    let cancelled = false;
    listBookQuotes(id).then((res) => {
      if (cancelled) return;
      if (res.status === "ok") quotes = res.data;
      else toast.error(messageFor(res.error, t));
    });
    return () => {
      cancelled = true;
    };
  });

  function openQuoteEditor(row: Quote | null): void {
    quotesEditing = row;
    quoteEditorOpen = true;
  }

  async function removeQuote(row: Quote): Promise<void> {
    if (quoteWorking) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("book.quotes.deleteConfirm"), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    quoteWorking = true;
    try {
      const res = await deleteQuote(row.id);
      toast[res.status === "ok" ? "success" : "error"](
        res.status === "ok" ? t("toast.quoteDeleted") : messageFor(res.error, t),
      );
      await reloadQuotes();
    } finally {
      quoteWorking = false;
    }
  }

  /** The colour dot's Tailwind classes — one per domain value, named in `@theme`. */
  const COLOR_CLASS: Record<Quote["color"], string> = {
    yellow: "bg-quote-yellow",
    green: "bg-quote-green",
    blue: "bg-quote-blue",
    pink: "bg-quote-pink",
    red: "bg-quote-red",
    purple: "bg-quote-purple",
  };

  // ── Name resolution ────────────────────────────────────────────────────────────────────
  //
  // Ids arrive from `book_detail`; names come from the lists. An id with no row behind it
  // (a deleted author whose credit somehow survived) shows the id rather than vanishing,
  // so a broken relation is visible instead of silent.

  let authorById = $derived(new Map(authors.map((a) => [a.id, a])));
  let publisherById = $derived(new Map(publishers.map((p) => [p.id, p])));
  let seriesById = $derived(new Map(seriesList.map((s) => [s.id, s])));
  let categoryById = $derived(new Map(categories.map((c) => [c.id, c])));
  let tagById = $derived(new Map(tags.map((tg) => [tg.id, tg])));

  const authorName = (credit: AuthorCredit) => authorById.get(credit.author_id)?.name ?? credit.author_id.slice(0, 8);
  const publisherName = (pid: string | null) => (pid === null ? null : (publisherById.get(pid)?.name ?? pid.slice(0, 8)));
  const seriesName = (sid: string | null) => (sid === null ? null : (seriesById.get(sid)?.name ?? sid.slice(0, 8)));
  /** Categories carry both names; the active locale picks one. */
  const categoryName = (c: Category) => ($locale === "ar" ? c.name_ar : c.name_en);
  const tagName = (tid: string) => tagById.get(tid)?.name ?? tid.slice(0, 8);

  /** `role` only renders when it is not the plain author credit — that is the default. */
  const roleSuffix = (credit: AuthorCredit) => (credit.role === "author" ? "" : ` — ${t(`role.${credit.role}`)}`);

  // ── Formatters ────────────────────────────────────────────────────────────────────────

  const num = (v: number) => formatNumber(v, { digits });
  const dateOf = (ms: number) => formatDate(ms, $locale, $preferences.calendar, digits);
  const formatLabel = (f: BookFormat) => t(`wizard.format.${f}`);
  const statusLabel = (s: ReadingStatus) => t(`status.${s}`);
  const conditionLabel = (c: Condition) => t(`condition.${c}`);
  const sourceLabel = (s: BookSource) => t(`source.${s}`);
  /** Price is stored in minor units; only render it when both halves are present. */
  function priceOf(minor: number | null, currency: string | null): string | null {
    if (minor === null || currency === null) return null;
    return `${num(minor / 100)} ${currency}`;
  }

  // ── Tab keyboard model ────────────────────────────────────────────────────────────────
  //
  // Arrow keys walk the tablist in *visual* order, so RTL flips the mapping: the first tab
  // sits on the right, and ← moves toward the end of the list.

  let tablist = $state<HTMLDivElement | null>(null);

  function onTablistKeydown(e: KeyboardEvent): void {
    const order = TABS.map((x) => x.key);
    const current = order.indexOf(tab);
    const rtl = $direction === "rtl";
    let next: number;
    switch (e.key) {
      case "ArrowRight":
        next = rtl ? current - 1 : current + 1;
        break;
      case "ArrowLeft":
        next = rtl ? current + 1 : current - 1;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = order.length - 1;
        break;
      default:
        return;
    }
    e.preventDefault();
    const clamped = Math.min(order.length - 1, Math.max(0, next));
    const key = order[clamped] ?? "overview";
    tab = key;
    tablist?.querySelector<HTMLButtonElement>(`[data-tab="${key}"]`)?.focus();
  }

  /** The back arrow points the way the language reads. */
  let BackIcon = $derived($direction === "rtl" ? ArrowRight : ArrowLeft);
</script>

{#snippet fact(label: string, value: string, ltr = false)}
  <!-- The cell inherits the page direction, so label and value align on the same edge.
       `dir="auto"` is wrong here: Arabic-Indic digits are bidi class *AN*, not a strong
       direction, so a year resolved to LTR and slid to the opposite edge from its label.
       Genuinely LTR content (ISBNs, codes) is isolated per docs/04 §7.5 instead. -->
  <div class="flex flex-col gap-0.5">
    <dt class="text-[var(--fs-sm)] text-ink-3">{label}</dt>
    <dd class="bidi-auto text-[var(--fs-base)] text-ink-1">
      {#if ltr}<span class="bidi-ltr tnum">{value}</span>{:else}{value}{/if}
    </dd>
  </div>
{/snippet}

{#snippet chip(label: string)}
  <span class="bidi-auto rounded-[var(--r-full)] border border-border-2 bg-surface-2 px-2.5 py-0.5 text-[var(--fs-sm)] text-ink-2">
    {label}
  </span>
{/snippet}

{#snippet sectionTitle(text: string)}
  <h2 class="mb-[var(--sp-3)] text-[var(--fs-md)] font-semibold text-ink-1">{text}</h2>
{/snippet}

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-5)] px-[var(--sp-6)] py-[var(--sp-5)]">
  <!-- Breadcrumb back to the browser. A real link, so Back in the rail and Back in the
       keyboard both work without a handler. -->
  <a
    href="#/library"
    class="inline-flex w-fit items-center gap-[var(--sp-2)] rounded-[var(--r-md)] text-[var(--fs-sm)] text-ink-2 transition-colors hover:text-ink-1"
  >
    <BackIcon class="size-4" strokeWidth={1.75} />
    {t("book.section")}
  </a>

  {#if loading}
    <div class="flex gap-[var(--sp-5)]">
      <div class="h-[19rem] w-[12.7rem] shrink-0 animate-pulse rounded-[var(--r-lg)] border border-border-1 bg-surface-2"></div>
      <div class="flex flex-1 flex-col gap-[var(--sp-3)]">
        <div class="h-8 w-2/3 animate-pulse rounded-[var(--r-md)] bg-surface-2"></div>
        <div class="h-4 w-1/3 animate-pulse rounded-[var(--r-sm)] bg-surface-2"></div>
        <div class="mt-[var(--sp-3)] h-24 w-full animate-pulse rounded-[var(--r-lg)] bg-surface-2"></div>
      </div>
    </div>
  {:else if missing}
    <EmptyState
      title={t("book.notFound.title")}
      body={t("book.notFound.body")}
      actionLabel={t("book.section")}
      onaction={() => (window.location.hash = "/library")}
    />
  {:else if loadError}
    <Card>
      <div role="alert" class="flex flex-col items-start gap-[var(--sp-3)]">
        <div class="flex items-center gap-2 text-[var(--fs-md)] font-semibold text-danger">
          <TriangleAlert class="size-5 no-mirror" strokeWidth={2} />
          {t("book.loadFailed")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else if detail === null}
    <!-- No book and no error: only reachable in a browser preview, where every command
         resolves to "no shell". Say so instead of inventing a book. -->
    <EmptyState title={t("book.preview.title")} body={t("book.preview.body")} />
  {:else}
    {@const book = detail.book}
    <!-- Header: cover, title block, status and rating. -->
    <header class="flex flex-wrap items-start gap-[var(--sp-5)]">
      <CoverPlaceholder title={book.title} language={book.language_code} height={280} />

      <div class="flex min-w-[16rem] flex-1 flex-col gap-[var(--sp-3)]">
        <div class="flex items-start justify-between gap-[var(--sp-3)]">
          <div class="min-w-0">
            <h1 class="bidi-auto text-[var(--fs-2xl)] font-semibold leading-tight" dir="auto">
              {book.title}
            </h1>
            {#if book.subtitle !== null}
              <p class="bidi-auto mt-1 text-[var(--fs-lg)] font-light text-ink-2" dir="auto">{book.subtitle}</p>
            {/if}
            {#if book.original_title !== null}
              <p class="bidi-auto mt-1 text-[var(--fs-base)] text-ink-3" dir="auto">
                {t("book.originalTitle")}: {book.original_title}
              </p>
            {/if}
          </div>

          <!-- Edit and delete live in the header: the two actions a book page is looked
               for, next to the row that names the book. -->
          <div class="flex shrink-0 items-center gap-1.5">
            <button
              type="button"
              aria-label={t("book.edit")}
              title={t("book.edit")}
              onclick={() => (editOpen = true)}
              class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-2 text-ink-2 transition-colors hover:border-border-2 hover:text-ink-1"
            >
              <Pencil class="size-4 no-mirror" strokeWidth={1.75} />
            </button>
            <button
              type="button"
              aria-label={t("book.delete")}
              title={t("book.delete")}
              onclick={() => void removeBook()}
              class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-2 text-ink-2 transition-colors hover:border-danger/50 hover:text-danger"
            >
              <Trash2 class="size-4 no-mirror" strokeWidth={1.75} />
            </button>
          </div>
        </div>

        {#if detail.authors.length > 0}
          <p class="bidi-auto text-[var(--fs-base)] text-ink-2" dir="auto">
            {#each detail.authors as credit, i (credit.author_id + credit.role)}
              {#if i > 0}<span class="text-ink-3">، </span>{/if}
              <span class="font-medium text-ink-1">{authorName(credit)}</span><span class="text-ink-3">{roleSuffix(credit)}</span>
            {/each}
          </p>
        {/if}

        <div class="flex flex-wrap items-center gap-[var(--sp-3)]">
          <span
            class="rounded-[var(--r-full)] px-2.5 py-0.5 text-[var(--fs-sm)] font-medium {book.reading_status === 'finished'
              ? 'bg-success-soft text-success'
              : book.reading_status === 'reading'
                ? 'bg-accent-soft text-accent-soft-ink'
                : 'bg-surface-2 text-ink-2'}"
          >
            {statusLabel(book.reading_status)}
          </span>

          {#if book.rating !== null}
            <Stars value={book.rating} readonly size="md" label={t("book.rating.label")} />
          {/if}

          {#if book.progress_pct > 0}
            <span class="tnum bidi-ltr text-[var(--fs-sm)] text-ink-3">{num(book.progress_pct)}%</span>
          {/if}
        </div>

        <p class="bidi-auto text-[var(--fs-sm)] text-ink-3">
          {#if book.publication_year !== null}<span class="bidi-ltr">{num(book.publication_year)}</span> · {/if}
          {#if book.page_count !== null}<span class="bidi-ltr">{num(book.page_count)}</span> {t("book.pages")} · {/if}
          {formatLabel(book.format)}
          {#if publisherName(book.publisher_id) !== null} · {publisherName(book.publisher_id)}{/if}
        </p>

        <p class="bidi-auto text-[var(--fs-xs)] text-ink-3">
          {t("book.added")} {dateOf(book.created_at)}
          {#if book.updated_at !== book.created_at} · {t("book.updated")} {dateOf(book.updated_at)}{/if}
        </p>
      </div>
    </header>

    <!-- Tablist. Roving tabindex: one stop for the whole strip, arrows to walk it. The keys
         live on the buttons rather than the container because that is where focus is. -->
    <div
      class="flex gap-[var(--sp-1)] overflow-x-auto border-b border-border-1"
      role="tablist"
      aria-label={t("book.record")}
      bind:this={tablist}
    >
      {#each TABS as item (item.key)}
        <button
          type="button"
          role="tab"
          data-tab={item.key}
          id="tab-{item.key}"
          aria-selected={tab === item.key}
          aria-controls="panel-{item.key}"
          tabindex={tab === item.key ? 0 : -1}
          onclick={() => (tab = item.key)}
          onkeydown={onTablistKeydown}
          class="shrink-0 border-b-2 px-[var(--sp-4)] py-[var(--sp-3)] text-[var(--fs-base)] transition-colors {tab === item.key
            ? 'border-accent font-medium text-accent'
            : 'border-transparent text-ink-2 hover:text-ink-1'}"
        >
          {t(item.labelKey)}
        </button>
      {/each}
    </div>

    <div role="tabpanel" id="panel-{tab}" aria-labelledby="tab-{tab}" tabindex="-1" class="flex flex-col gap-[var(--sp-4)]">
      {#if tab === "overview"}
        <Card>
          {@render sectionTitle(t("book.description"))}
          {#if book.description !== null}
            <p class="bidi-auto whitespace-pre-wrap text-[var(--fs-base)] leading-relaxed text-ink-2" dir="auto">
              {book.description}
            </p>
          {:else}
            <p class="text-[var(--fs-base)] text-ink-3">{t("book.noDescription")}</p>
          {/if}
        </Card>

        <Card>
          {@render sectionTitle(t("book.progress"))}
          {#if book.progress_pct > 0 || book.current_page !== null}
            <ProgressBar value={book.progress_pct} label={t("book.progress")} showValue />
            {#if book.current_page !== null && book.page_count !== null}
              <p class="mt-[var(--sp-2)] bidi-auto text-[var(--fs-sm)] text-ink-3">
                {t("book.currentPage")}
                <span class="bidi-ltr text-ink-1">{num(book.current_page)}</span>
                {t("book.pageOf", { total: num(book.page_count) })}
              </p>
            {/if}
          {:else}
            <p class="text-[var(--fs-base)] text-ink-3">{t("book.progress.none")}</p>
          {/if}
        </Card>

        <Card>
          {@render sectionTitle(t("book.record"))}
          <dl class="grid grid-cols-2 gap-[var(--sp-4)] sm:grid-cols-3">
            {@render fact(t("book.year"), book.publication_year === null ? "—" : num(book.publication_year), true)}
            {@render fact(t("book.pages"), book.page_count === null ? "—" : num(book.page_count), true)}
            {@render fact(t("book.format"), formatLabel(book.format))}
            {@render fact(t("book.language"), book.language_code, true)}
            {@render fact(t("book.publisher"), publisherName(book.publisher_id) ?? "—")}
            {@render fact(t("book.series"), seriesName(book.series_id) ?? "—")}
          </dl>
        </Card>
      {:else if tab === "details"}
        <Card>
          {@render sectionTitle(t("book.tab.details"))}
          <dl class="grid grid-cols-2 gap-[var(--sp-4)] sm:grid-cols-3">
            {@render fact(t("book.isbn10"), book.isbn10 ?? "—", true)}
            {@render fact(t("book.isbn13"), book.isbn13 ?? "—", true)}
            {@render fact(t("book.edition"), book.edition ?? "—")}
            {@render fact(t("book.seriesOrder"), book.series_order === null ? "—" : num(book.series_order), true)}
            {@render fact(t("book.source"), sourceLabel(book.source))}
            {@render fact(t("book.language"), book.language_code, true)}
          </dl>
        </Card>

        <Card>
          {@render sectionTitle(t("book.acquire"))}
          <dl class="grid grid-cols-2 gap-[var(--sp-4)] sm:grid-cols-3">
            {@render fact(t("book.acquire.date"), book.acquired_date ?? "—")}
            {@render fact(t("book.acquire.from"), book.acquired_from ?? "—")}
            {@render fact(t("book.acquire.price"), priceOf(book.price_minor, book.price_currency) ?? "—", true)}
            {@render fact(t("book.acquire.condition"), book.condition === null ? "—" : conditionLabel(book.condition))}
            {@render fact(t("book.location.room"), book.room ?? "—")}
            {@render fact(t("book.location.shelf"), book.shelf ?? "—")}
            {@render fact(t("book.location.box"), book.box_name ?? "—")}
          </dl>
        </Card>
      {:else if tab === "reading"}
        <Card>
          {@render sectionTitle(t("book.tab.reading"))}
          <dl class="grid grid-cols-2 gap-[var(--sp-4)] sm:grid-cols-3">
            {@render fact(t("wizard.field.status"), statusLabel(book.reading_status))}
            {@render fact(t("book.currentPage"), book.current_page === null ? "—" : num(book.current_page))}
            {@render fact(t("book.rating"), book.rating === null ? "—" : num(book.rating / 2))}
          </dl>
          <div class="mt-[var(--sp-4)]">
            <ProgressBar value={book.progress_pct} label={t("book.progress")} showValue />
          </div>
        </Card>
      {:else if tab === "organize"}
        <Card>
          {@render sectionTitle(t("book.organize.categories"))}
          {#if detail.categories.length === 0}
            <p class="text-[var(--fs-base)] text-ink-3">{t("book.organize.noCategories")}</p>
          {:else}
            <ul class="flex flex-wrap gap-[var(--sp-2)]">
              {#each detail.categories as cid (cid)}
                {@const cat = categoryById.get(cid)}
                <li>
                  {@render chip(cat === undefined ? cid.slice(0, 8) : categoryName(cat))}
                </li>
              {/each}
            </ul>
          {/if}
        </Card>

        <Card>
          {@render sectionTitle(t("book.organize.tags"))}
          {#if detail.tags.length === 0}
            <p class="text-[var(--fs-base)] text-ink-3">{t("book.organize.noTags")}</p>
          {:else}
            <ul class="flex flex-wrap gap-[var(--sp-2)]">
              {#each detail.tags as tid (tid)}
                <li>{@render chip(tagName(tid))}</li>
              {/each}
            </ul>
          {/if}
        </Card>
      {:else if tab === "notes"}
        <Card>
          <EmptyState
            title={t("book.notes.empty")}
            body="{t("book.notes.body")} {t("book.soon")}"
            compact
          />
        </Card>
      {:else if tab === "quotes"}
        <Card>
          <div class="flex flex-wrap items-center justify-between gap-[var(--sp-3)]">
            {@render sectionTitle(t("book.tab.quotes"))}
            <Button variant="secondary" onclick={() => openQuoteEditor(null)}>
              {#snippet iconStart()}
                <QuoteIcon class="size-4 no-mirror" strokeWidth={1.75} />
              {/snippet}
              {t("book.quotes.add")}
            </Button>
          </div>

          {#if quotes.length === 0}
            <div class="mt-[var(--sp-3)]">
              <EmptyState title={t("book.quotes.empty")} body={t("book.quotes.body")} compact />
            </div>
          {:else}
            <ul class="flex flex-col gap-[var(--sp-3)]">
              {#each quotes as row (row.id)}
                <li class="rounded-[var(--r-md)] border border-border-1 bg-surface-1 p-[var(--sp-3)]">
                  <span class="flex items-start gap-[var(--sp-2)]">
                    <span class="mt-1.5 size-2.5 shrink-0 rounded-full {COLOR_CLASS[row.color]}" aria-hidden="true"></span>
                    <blockquote class="bidi-auto min-w-0 flex-1 text-[var(--fs-base)] leading-relaxed text-ink-1" dir="auto">
                      {row.text}
                    </blockquote>
                  </span>
                  <span class="mt-[var(--sp-2)] flex flex-wrap items-center gap-[var(--sp-2)] text-[var(--fs-sm)] text-ink-3">
                    {#if row.page !== null}
                      <span class="bidi-ltr tnum">{num(row.page)}</span>
                    {/if}
                    {#if row.chapter !== null}
                      <span class="bidi-auto" dir="auto">{row.chapter}</span>
                    {/if}
                    {#if row.page !== null || row.chapter !== null}
                      <span aria-hidden="true">·</span>
                    {/if}
                    <span>{dateOf(row.created_at)}</span>
                    <span class="ms-auto flex items-center gap-1">
                      <button
                        type="button"
                        aria-label={t("common.edit")}
                        title={t("common.edit")}
                        onclick={() => openQuoteEditor(row)}
                        class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:text-ink-1"
                      >
                        <Pencil class="size-3.5 no-mirror" strokeWidth={1.75} />
                      </button>
                      <button
                        type="button"
                        aria-label={t("common.delete")}
                        title={t("common.delete")}
                        disabled={quoteWorking}
                        onclick={() => void removeQuote(row)}
                        class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:border-danger/50 hover:text-danger disabled:opacity-50"
                      >
                        <Trash2 class="size-3.5 no-mirror" strokeWidth={1.75} />
                      </button>
                    </span>
                  </span>
                  {#if row.note !== null}
                    <p class="bidi-auto mt-[var(--sp-2)] border-t border-border-1 pt-[var(--sp-2)] text-[var(--fs-sm)] text-ink-2" dir="auto">
                      {row.note}
                    </p>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </Card>
      {:else}
        <!-- Files (M8): the one tab with a backend behind it. The empty state is the designed
             one; the list is real, and every row's actions go through a command. -->
        <Card>
          <div class="flex flex-wrap items-center justify-between gap-[var(--sp-3)]">
            {@render sectionTitle(t("book.files.title"))}
            <Button variant="secondary" loading={picking} onclick={() => void attachPicked()}>
              {#snippet iconStart()}
                <Paperclip class="size-4 no-mirror" strokeWidth={1.75} />
              {/snippet}
              {t("book.files.add")}
            </Button>
          </div>

          {#if filesLoading}
            <ul class="mt-[var(--sp-4)] flex flex-col gap-2">
              {#each [0, 1] as i (i)}
                <li class="h-10 animate-pulse rounded-[var(--r-md)] border border-border-1 bg-surface-2"></li>
              {/each}
            </ul>
          {:else if filesFailed}
            <p class="mt-[var(--sp-4)] text-[var(--fs-base)] text-ink-3">{t("book.files.empty")}</p>
          {:else if files.length === 0}
            <div class="mt-[var(--sp-4)]">
              <EmptyState title={t("book.files.empty")} body={t("book.files.body")} compact />
            </div>
          {:else}
            <ul class="mt-[var(--sp-4)] flex flex-col gap-2">
              {#each files as row (row.id)}
                <li
                  class="flex flex-wrap items-center gap-2 rounded-[var(--r-md)] border border-border-1 bg-surface-1 px-[var(--sp-3)] py-2"
                >
                  <FileText class="size-4 shrink-0 text-ink-3 no-mirror" strokeWidth={1.75} />
                  <span class="min-w-0 flex-1 truncate text-[var(--fs-base)] text-ink-1" dir="auto">
                    {row.original_name}
                  </span>
                  <span
                    class="rounded-[var(--r-full)] border border-border-2 bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2"
                  >
                    {kindLabel(row.kind)}
                  </span>
                  <span
                    class="rounded-[var(--r-full)] border border-border-2 bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2"
                  >
                    {row.storage_mode === "copy"
                      ? t("wizard.files.mode.copy")
                      : t("wizard.files.mode.reference")}
                  </span>

                  <div class="flex items-center gap-1">
                    <button
                      type="button"
                      aria-label={t("book.files.open")}
                      title={t("book.files.open")}
                      onclick={() => void openAttachment(row)}
                      class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:text-ink-1"
                    >
                      <ExternalLink class="size-4 no-mirror" strokeWidth={1.75} />
                    </button>
                    <button
                      type="button"
                      aria-label={t("book.files.reveal")}
                      title={t("book.files.reveal")}
                      onclick={() => void revealAttachment(row)}
                      class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:text-ink-1"
                    >
                      <FolderOpen class="size-4 no-mirror" strokeWidth={1.75} />
                    </button>
                    <button
                      type="button"
                      aria-label={t("common.delete")}
                      title={t("common.delete")}
                      disabled={deleting}
                      onclick={() => void removeFile(row)}
                      class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:border-danger/50 hover:text-danger disabled:opacity-50"
                    >
                      <Trash2 class="size-4 no-mirror" strokeWidth={1.75} />
                    </button>
                  </div>
                </li>
              {/each}
            </ul>
          {/if}

          <div class="mt-[var(--sp-5)]">
            <Field
              label={t("wizard.files.storageMode")}
              for="book-file-mode"
              hint={t("wizard.files.mode.hint")}
            >
              <select
                id="book-file-mode"
                value={fileMode}
                onchange={(e) => (fileMode = e.currentTarget.value as StorageMode)}
                class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
              >
                <option value="copy">{t("wizard.files.mode.copy")}</option>
                <option value="reference">{t("wizard.files.mode.reference")}</option>
              </select>
            </Field>
          </div>
        </Card>
      {/if}
    </div>
  {/if}
</div>

{#if detail !== null}
  <BookEdit
    open={editOpen}
    {detail}
    {authors}
    {publishers}
    {seriesList}
    {categories}
    {tags}
    onclose={() => (editOpen = false)}
    onsaved={() => void load()}
  />
{/if}

<!-- Quote add/edit lives on the page, not in the tab: an editor that closes when the
     *tab* switches would eat the user's work. `bookId` anchors a new quote to this book. -->
<QuoteEdit
  open={quoteEditorOpen}
  bookId={id}
  quote={quotesEditing}
  onclose={() => (quoteEditorOpen = false)}
  onsaved={() => {
    toast.success(t("toast.quoteSaved"));
    void reloadQuotes();
  }}
/>
