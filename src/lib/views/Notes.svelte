<script lang="ts">
  import { Pencil, Pin, PinOff, Plus, StickyNote, Trash2, TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import {
    createNotePage,
    deleteNotePage,
    listNotePages,
    updateNotePage,
  } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { messageFor } from "../errors";
  import { toast } from "../toast";
  import { go } from "../router";
  import type { AppErrorDto, NotePage, NotePageInput } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  const num = (v: number) => formatNumber(v, { digits });

  /**
   * Notes screen — the pages of notes (الملاحظات), docs/04-ux-design.md §4.
   *
   * A page is one document; the editor lives at `/notes/:pageId` and this list is the
   * front door. Pinned pages float up (a reading-plan page earns its pin), pages anchor
   * to a book at creation and show the anchor here — the tab, not the screen, is where
   * the book link was made. The row's primary click opens the editor: a list that
   * required a menu tap to read notes would be one hop too many.
   */

  let pages = $state<NotePage[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let working = $state(false);
  let query = $state("");

  /** The displayed name falls back on `title_norm`'s owner being an untitled page. */
  const titleOf = (page: NotePage): string =>
    page.title.trim().length > 0 ? page.title : t("notes.untitled");

  let filtered = $derived(
    query.trim().length === 0
      ? pages
      : pages.filter((p) => {
          const needle = query.trim().toLowerCase();
          return (
            p.title.toLowerCase().includes(needle) ||
            p.title_norm.includes(needle)
          );
        }),
  );

  async function load(): Promise<void> {
    loading = true;
    const res = await listNotePages();
    if (res.status === "ok") {
      pages = res.data;
      loadError = null;
    } else if (res.error.code === "shell.unavailable") {
      // Browser preview: no backend, so the designed empty state is the honest one.
      pages = [];
      loadError = null;
    } else {
      pages = [];
      loadError = res.error;
    }
    loading = false;
  }

  onMount(() => void load());

  // ── Create + edit ─────────────────────────────────────────────────────────────

  let editing = $state<NotePage | null>(null);
  let editorOpen = $state(false);
  let saving = $state(false);
  let formError = $state<string | null>(null);
  let title = $state("");

  function openEditor(row: NotePage | null): void {
    editing = row;
    editorOpen = true;
  }

  $effect(() => {
    if (!editorOpen) return;
    title = editing?.title ?? "";
    formError = null;
  });

  async function save(): Promise<void> {
    if (saving) return;
    const input: NotePageInput = {
      title: title.trim(),
      icon: editing?.icon ?? null,
      book_id: editing?.book_id ?? null,
      parent_page_id: editing?.parent_page_id ?? null,
      is_pinned: editing?.is_pinned ?? false,
    };
    saving = true;
    formError = null;
    const res =
      editing === null ? await createNotePage(input) : await updateNotePage(editing.id, input);
    saving = false;
    // Creating from this screen is the moment the user already said where they want to
    // be: land straight in the editor on the new page.
    if (res.status === "ok" && editing === null) {
      editorOpen = false;
      toast.success(t("toast.saved"));
      go(`/notes/${res.data.id}`);
      return;
    }
    if (res.status === "ok") {
      editorOpen = false;
      toast.success(t("toast.saved"));
      await load();
      return;
    }
    formError = messageFor(res.error, t);
  }

  // ── Pin + delete ──────────────────────────────────────────────────────────────

  async function togglePin(row: NotePage): Promise<void> {
    if (working) return;
    working = true;
    try {
      const res = await updateNotePage(row.id, {
        title: row.title,
        icon: row.icon,
        book_id: row.book_id,
        parent_page_id: row.parent_page_id,
        is_pinned: !row.is_pinned,
      });
      if (res.status !== "ok") toast.error(messageFor(res.error, t));
      await load();
    } finally {
      working = false;
    }
  }

  async function removePage(row: NotePage): Promise<void> {
    if (working) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("notes.deleteConfirm", { name: titleOf(row) }), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deleteNotePage(row.id);
      toast[res.status === "ok" ? "success" : "error"](
        res.status === "ok" ? t("toast.deleted") : messageFor(res.error, t),
      );
      await load();
    } finally {
      working = false;
    }
  }
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-4)] px-[var(--sp-6)] py-[var(--sp-5)]">
  <header class="flex items-baseline gap-[var(--sp-3)]">
    <h1 class="text-[var(--fs-xl)] font-semibold text-ink-1">{t("nav.notes")}</h1>
    {#if pages.length > 0}
      <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
        {num(pages.length)} {t("notes.count")}
      </span>
    {/if}
    <span class="ms-auto">
      <Button variant="primary" size="sm" onclick={() => openEditor(null)}>
        {#snippet iconStart()}
          <Plus class="size-4 no-mirror" strokeWidth={1.75} />
        {/snippet}
        {t("notes.add")}
      </Button>
    </span>
  </header>

  {#if loading}
    <div class="flex flex-col gap-[var(--sp-3)]">
      {#each [0, 1, 2] as i (i)}
        <div class="h-16 animate-pulse rounded-[var(--r-lg)] border border-border-1 bg-surface-2"></div>
      {/each}
    </div>
  {:else if loadError}
    <Card>
      <div role="alert" class="flex flex-col items-start gap-[var(--sp-3)]">
        <div class="flex items-center gap-2 text-[var(--fs-md)] font-semibold text-danger">
          <TriangleAlert class="size-5 no-mirror" strokeWidth={2} />
          {t("notes.loadFailed")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else if pages.length === 0}
    <Card>
      <EmptyState title={t("notes.empty.title")} body={t("notes.empty.body")} />
    </Card>
  {:else}
    <!-- The search box hides once the list is empty of everything to search. -->
    <input
      type="search"
      bind:value={query}
      placeholder={t("notes.search")}
      autocomplete="off"
      class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent sm:max-w-sm"
    />
    <ul class="flex flex-col gap-[var(--sp-2)]">
      {#each filtered as row (row.id)}
        <li>
          <Card>
            <span class="flex items-center gap-[var(--sp-3)]">
              <!-- The whole identity block is the link; the edge buttons stay per-action. -->
              <button
                type="button"
                onclick={() => go(`/notes/${row.id}`)}
                class="flex min-w-0 flex-1 items-center gap-[var(--sp-3)] text-start"
              >
                <StickyNote class="size-5 shrink-0 text-ink-3 no-mirror" strokeWidth={1.5} aria-hidden="true" />
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="bidi-auto truncate text-[var(--fs-base)] font-medium text-ink-1" dir="auto">
                    {titleOf(row)}
                  </span>
                  {#if row.book_id !== null}
                    <span class="bidi-auto truncate text-[var(--fs-sm)] text-ink-3" dir="auto">
                      {t("notes.anchored")}
                    </span>
                  {/if}
                </span>
                {#if row.is_pinned}
                  <Pin class="size-4 shrink-0 text-accent no-mirror" strokeWidth={1.75} aria-label={t("notes.pinned")} />
                {/if}
              </button>
              <span class="flex shrink-0 items-center gap-1">
                <button
                  type="button"
                  aria-label={row.is_pinned ? t("notes.unpin") : t("notes.pin")}
                  title={row.is_pinned ? t("notes.unpin") : t("notes.pin")}
                  disabled={working}
                  onclick={() => void togglePin(row)}
                  class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 transition-colors {row.is_pinned
                    ? 'text-accent'
                    : 'text-ink-2 hover:text-ink-1'} disabled:opacity-50"
                >
                  {#if row.is_pinned}
                    <PinOff class="size-3.5 no-mirror" strokeWidth={1.75} />
                  {:else}
                    <Pin class="size-3.5 no-mirror" strokeWidth={1.75} />
                  {/if}
                </button>
                <button
                  type="button"
                  aria-label={t("common.edit")}
                  title={t("common.edit")}
                  onclick={() => openEditor(row)}
                  class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:text-ink-1"
                >
                  <Pencil class="size-3.5 no-mirror" strokeWidth={1.75} />
                </button>
                <button
                  type="button"
                  aria-label={t("common.delete")}
                  title={t("common.delete")}
                  disabled={working}
                  onclick={() => void removePage(row)}
                  class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:border-danger/50 hover:text-danger disabled:opacity-50"
                >
                  <Trash2 class="size-3.5 no-mirror" strokeWidth={1.75} />
                </button>
              </span>
            </span>
          </Card>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<Modal open={editorOpen} onclose={() => (saving ? undefined : (editorOpen = false))} labelledBy="note-form-title">
  <h2 id="note-form-title" class="border-b border-border-1 px-6 py-4 text-[var(--fs-lg)] font-semibold text-ink-1">
    {editing === null ? t("notes.form.add") : t("notes.form.edit")}
  </h2>

  <form class="flex flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5" onsubmit={(e) => { e.preventDefault(); void save(); }}>
    <Field label={t("notes.title")} for="note-title" hint={t("notes.title.hint")}>
      <input
        id="note-title"
        bind:value={title}
        maxlength={300}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    {#if formError !== null}
      <p role="alert" class="bidi-auto rounded-[var(--r-md)] bg-danger-soft px-[var(--sp-3)] py-[var(--sp-2)] text-[var(--fs-sm)] text-danger" dir="auto">
        {formError}
      </p>
    {/if}

    <div class="flex items-center justify-end gap-[var(--sp-2)] border-t border-border-1 pt-[var(--sp-4)]">
      <Button variant="ghost" disabled={saving} onclick={() => (editorOpen = false)}>{t("common.cancel")}</Button>
      <Button type="submit" variant="primary" loading={saving}>
        {saving ? t("common.save") : (editing === null ? t("notes.form.create") : t("common.save"))}
      </Button>
    </div>
  </form>
</Modal>
