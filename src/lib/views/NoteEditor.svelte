<script lang="ts">
  import {
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Pin,
    PinOff,
    Plus,
    StickyNote,
    Trash2,
    TriangleAlert,
  } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Modal from "../components/Modal.svelte";
  import {
    deleteNoteBlock,
    deleteNotePage,
    getNotePage,
    insertNoteBlock,
    listNoteBlocks,
    moveNoteBlock,
    updateNoteBlock,
    updateNotePage,
  } from "../api";
  import type { NoteBlockType } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { toast } from "../toast";
  import { go, route } from "../router";
  import type { AppErrorDto, NoteBlock, NotePage } from "@/bindings";

  let currentPath = $derived($route.path);
  let pageId = $derived($route.params.pageId ?? "");
  let t = $derived(translatorFor($locale));
  let isRtl = $derived($locale === "ar");

  /**
   * Note editor — one page, docs/04-ux-design.md §4.
   *
   * Blocks render in content order as editable controls: text kinds are textareas that
   * save on blur (the page is a document being written, not a form being submitted),
   * a todo keeps its checkbox in the payload, a divider is just a rule. The v1 kinds are
   * the text subset of the schema's sixteen; the richer ones (table, image…) light up
   * when a reader exists, which is where they earn their UI.
   *
   * Reordering uses the two rails beside each block because drag-and-drop for a sequence
   * of a dozen blocks is a hand-trap on touch and an a11y hole on keyboard; rails are
   * explicit and reversible.
   */

  // The block kinds the editor offers. Cut across the schema enum: the rest of the
  // sixteen stay legal on the wire (read + edit as unknown payload), just not creatable.
  const KINDS: NoteBlockType[] = [
    "paragraph",
    "heading_1",
    "heading_2",
    "heading_3",
    "bulleted_list",
    "numbered_list",
    "todo",
    "quote",
    "code",
    "divider",
  ];

  const textOf = (block: NoteBlock): string => {
    const p = block.payload as { text?: unknown } | null;
    return typeof p?.text === "string" ? p.text : "";
  };

  const checkedOf = (block: NoteBlock): boolean => {
    const p = block.payload as { checked?: unknown } | null;
    return p?.checked === true;
  };

  let page = $state<NotePage | null>(null);
  let blocks = $state<NoteBlock[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let working = $state(false);

  const titleOf = (): string =>
    page !== null && page.title.trim().length > 0 ? page.title : t("notes.untitled");

  async function load(): Promise<void> {
    if (pageId.length === 0) return;
    loading = true;
    const res = await getNotePage(pageId);
    if (res.status === "ok") {
      page = res.data;
      const blockRes = await listNoteBlocks(pageId);
      if (blockRes.status === "ok") {
        blocks = blockRes.data;
        loadError = null;
      } else {
        loadError = blockRes.error;
      }
    } else {
      // No page, no editor: `note_page_get` 404s and this screen says so honestly.
      page = null;
    }
    loading = false;
  }

  onMount(() => void load());

  /** Re-read the sequence after a structural write (insert/move/delete). */
  async function reloadBlocks(): Promise<void> {
    if (pageId.length === 0) return;
    const res = await listNoteBlocks(pageId);
    if (res.status === "ok") blocks = res.data;
  }

  // ── Editing a block ───────────────────────────────────────────────────────────

  /** The last text/checked state we know on disk, so blur without change costs nothing. */
  const savedText = new Map<string, string>();

  async function saveBlock(block: NoteBlock, text: string, checked: boolean = checkedOf(block)): Promise<void> {
    const prevChecked = checkedOf(block);
    const prev = savedText.get(block.id) ?? textOf(block);
    if (prev === text && checked === prevChecked) return;
    savedText.set(block.id, text);
    // Every payload member the block already carries rides along (forward-compatible
    // kinds), with `text`/`checked` rewritten from what the control showed.
    const nextPayload: Record<string, unknown> = { text };
    for (const [key, value] of Object.entries(block.payload as Record<string, unknown>)) {
      if (key !== "text" && key !== "checked") nextPayload[key] = value;
    }
    if (block.block_type === "todo") nextPayload.checked = checked;
    const res = await updateNoteBlock(block.id, {
      block_type: block.block_type,
      payload: nextPayload as unknown as import("@/bindings").Json,
    });
    if (res.status !== "ok") {
      // The typed text stays in the control; the error explains itself in the toast.
      toast.error(messageFor(res.error, t));
      savedText.delete(block.id);
      return;
    }
  }

  function blockSaveHandler(block: NoteBlock): (e: Event) => void {
    return (e: Event) => {
      if (!(e.currentTarget instanceof HTMLTextAreaElement)) return;
      void saveBlock(block, e.currentTarget.value);
    };
  }

  async function toggleChecked(block: NoteBlock): Promise<void> {
    if (working) return;
    working = true;
    try {
      const next = !checkedOf(block);
      const payload: Record<string, unknown> = { text: textOf(block) };
      for (const [key, value] of Object.entries(block.payload as Record<string, unknown>)) {
        if (key !== "text" && key !== "checked") payload[key] = value;
      }
      payload.checked = next;
      const res = await updateNoteBlock(block.id, {
        block_type: block.block_type,
        payload: payload as unknown as import("@/bindings").Json,
      });
      if (res.status === "ok") {
        const updated = res.data;
        const index = blocks.findIndex((b) => b.id === updated.id);
        if (index >= 0) blocks[index] = updated;
      } else {
        toast.error(messageFor(res.error, t));
      }
    } finally {
      working = false;
    }
  }

  // ── Insert / move / delete a block ─────────────────────────────────────────────

  /** Which slot the add-menu targets (`null` = append at the end). */
  let insertAfterId = $state<string | null>(null);
  let addMenuOpen = $state(false);
  let adding = $state(false);

  function openAddMenu(afterId: string | null): void {
    insertAfterId = afterId;
    addMenuOpen = true;
  }

  async function addBlock(kind: NoteBlockType): Promise<void> {
    if (page === null || adding) return;
    adding = true;
    try {
      const res = await insertNoteBlock(page.id, insertAfterId, {
        block_type: kind,
        payload: { text: "" } as unknown as import("@/bindings").Json,
      });
      if (res.status === "ok") {
        addMenuOpen = false;
        await reloadBlocks();
      } else {
        toast.error(messageFor(res.error, t));
      }
    } finally {
      adding = false;
    }
  }

  /** Moving up: land before the previous-previous sibling (`null` = very first). */
  async function moveUp(index: number): Promise<void> {
    if (working || index === 0) return;
    const block = blocks[index];
    const anchor = index >= 2 ? blocks[index - 2].id : null;
    await doMove(block, anchor);
  }

  /** Moving down: land after the current successor. */
  async function moveDown(index: number): Promise<void> {
    if (working || index === blocks.length - 1) return;
    const block = blocks[index];
    await doMove(block, blocks[index + 1].id);
  }

  async function doMove(block: NoteBlock, afterId: string | null): Promise<void> {
    working = true;
    try {
      const res = await moveNoteBlock(block.id, afterId);
      if (res.status === "ok") await reloadBlocks();
      else toast.error(messageFor(res.error, t));
    } finally {
      working = false;
    }
  }

  async function removeBlock(block: NoteBlock): Promise<void> {
    if (working) return;
    let confirmed = false;
    try {
      // Text blocks lose their words, so the confirm names the block kind; the rules
      // (divider) are cheap to redraw, but one consistent dialog keeps the habit true.
      confirmed = await ask(
        t("notes.block.deleteConfirm", {
          kind: t(`notes.block.${block.block_type}` as Parameters<typeof t>[0]),
        }),
        { kind: "warning" },
      );
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deleteNoteBlock(block.id);
      if (res.status === "ok") await reloadBlocks();
      else toast.error(messageFor(res.error, t));
    } finally {
      working = false;
    }
  }

  // ── The page itself ───────────────────────────────────────────────────────────

  let editingTitle = $state(false);
  let titleDraft = $state("");

  function startTitleEdit(): void {
    if (page === null) return;
    titleDraft = page.title;
    editingTitle = true;
  }

  async function commitTitle(): Promise<void> {
    editingTitle = false;
    if (page === null) return;
    const trimmed = titleDraft.trim();
    if (trimmed === page.title) return;
    working = true;
    try {
      const res = await updateNotePage(page.id, {
        title: trimmed,
        icon: page.icon,
        book_id: page.book_id,
        parent_page_id: page.parent_page_id,
        is_pinned: page.is_pinned,
      });
      if (res.status === "ok") {
        page = res.data;
        toast.success(t("toast.saved"));
      } else {
        toast.error(messageFor(res.error, t));
      }
    } finally {
      working = false;
    }
  }

  async function togglePinned(): Promise<void> {
    if (page === null || working) return;
    working = true;
    try {
      const res = await updateNotePage(page.id, {
        title: page.title,
        icon: page.icon,
        book_id: page.book_id,
        parent_page_id: page.parent_page_id,
        is_pinned: !page.is_pinned,
      });
      if (res.status === "ok") page = res.data;
      else toast.error(messageFor(res.error, t));
    } finally {
      working = false;
    }
  }

  async function removePage(): Promise<void> {
    if (page === null || working) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("notes.deleteConfirm", { name: titleOf() }), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deleteNotePage(page.id);
      if (res.status === "ok") {
        toast.success(t("toast.deleted"));
        go("/notes");
      } else {
        toast.error(messageFor(res.error, t));
      }
    } finally {
      working = false;
    }
  }
</script>

{#snippet blockControls(block: NoteBlock, index: number)}
  <button type="button" aria-label={t("notes.block.addAfter")} onclick={() => openAddMenu(block.id)} class="p-0.5 text-ink-3 transition-colors hover:text-ink-1">
    <Plus class="size-3.5 no-mirror" strokeWidth={1.75} />
  </button>
  <button type="button" aria-label={t("notes.block.moveUp")} disabled={index === 0 || working} onclick={() => void moveUp(index)} class="p-0.5 text-ink-3 transition-colors hover:text-ink-1 disabled:opacity-40">
    <ArrowUp class="size-3.5 no-mirror" strokeWidth={1.75} />
  </button>
  <button type="button" aria-label={t("notes.block.moveDown")} disabled={index === blocks.length - 1 || working} onclick={() => void moveDown(index)} class="p-0.5 text-ink-3 transition-colors hover:text-ink-1 disabled:opacity-40">
    <ArrowDown class="size-3.5 no-mirror" strokeWidth={1.75} />
  </button>
  <button type="button" aria-label={t("common.delete")} onclick={() => void removeBlock(block)} class="p-0.5 text-ink-3 transition-colors hover:text-danger">
    <Trash2 class="size-3.5 no-mirror" strokeWidth={1.75} />
  </button>
{/snippet}

<!-- Keyed on the concrete path so editor → editor (back to /notes flips through here
     only transiently) remounts cleanly instead of carrying a skeleton state. -->
{#key currentPath}
  <div class="mx-auto flex max-w-3xl flex-col gap-[var(--sp-4)] px-[var(--sp-6)] py-[var(--sp-5)]">
    <header class="flex items-center gap-[var(--sp-3)]">
      <button
        type="button"
        onclick={() => go("/notes")}
        class="inline-flex items-center gap-1.5 rounded-[var(--r-md)] text-[var(--fs-sm)] text-ink-2 transition-colors hover:text-ink-1"
      >
        {#if isRtl}
          <ArrowRight class="size-4 no-mirror" strokeWidth={1.75} />
        {:else}
          <ArrowLeft class="size-4 no-mirror" strokeWidth={1.75} />
        {/if}
        {t("notes.editor.back")}
      </button>
      <span class="ms-auto flex items-center gap-1">
        {#if page !== null}
          <button
            type="button"
            aria-label={page.is_pinned ? t("notes.unpin") : t("notes.pin")}
            title={page.is_pinned ? t("notes.unpin") : t("notes.pin")}
            disabled={working}
            onclick={() => void togglePinned()}
            class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 transition-colors {page.is_pinned
              ? 'text-accent'
              : 'text-ink-2 hover:text-ink-1'} disabled:opacity-50"
          >
            {#if page.is_pinned}
              <PinOff class="size-4 no-mirror" strokeWidth={1.75} />
            {:else}
              <Pin class="size-4 no-mirror" strokeWidth={1.75} />
            {/if}
          </button>
          <button
            type="button"
            aria-label={t("common.delete")}
            title={t("common.delete")}
            disabled={working}
            onclick={() => void removePage()}
            class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:border-danger/50 hover:text-danger disabled:opacity-50"
          >
            <Trash2 class="size-4 no-mirror" strokeWidth={1.75} />
          </button>
        {/if}
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
    {:else if page === null}
      <Card>
        <EmptyState title={t("notes.editor.missing.title")} body={t("notes.editor.missing.body")} />
        <div class="mt-[var(--sp-3)] flex justify-center">
          <Button onclick={() => go("/notes")}>{t("notes.editor.back")}</Button>
        </div>
      </Card>
    {:else}
      {#if editingTitle}
        <input
          type="text"
          bind:value={titleDraft}
          onblur={commitTitle}
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void commitTitle();
            }
            if (e.key === "Escape") editingTitle = false;
          }}
          aria-label={t("notes.title")}
          maxlength={300}
          class="bidi-auto w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] py-[var(--sp-2)] text-[var(--fs-lg)] font-semibold text-ink-1 outline-none focus:border-accent"
        />
      {:else}
        <button
          type="button"
          onclick={startTitleEdit}
          class="bidi-auto flex w-full items-baseline gap-2 rounded-[var(--r-md)] px-[var(--sp-3)] py-[var(--sp-2)] text-start transition-colors hover:bg-surface-2"
        >
          {#if page.title.trim().length > 0}
            <span class="text-[var(--fs-lg)] font-semibold text-ink-1">{page.title}</span>
          {:else}
            <span class="text-[var(--fs-lg)] font-semibold text-ink-3">{t("notes.untitled")}</span>
          {/if}
          <StickyNote class="ms-auto size-4 shrink-0 text-ink-3 no-mirror" strokeWidth={1.5} aria-hidden="true" />
        </button>
      {/if}

      {#if blocks.length === 0}
        <Card>
          <EmptyState title={t("notes.editor.empty.title")} body={t("notes.editor.empty.body")} />
          <div class="mt-[var(--sp-3)] flex justify-center">
            <Button variant="primary" size="sm" onclick={() => openAddMenu(null)}>
              {#snippet iconStart()}
                <Plus class="size-4 no-mirror" strokeWidth={1.75} />
              {/snippet}
              {t("notes.block.add")}
            </Button>
          </div>
        </Card>
      {:else}
        <ul class="flex flex-col gap-1">
          {#each blocks as block, index (block.id)}
            <li class="group rounded-[var(--r-md)]" data-block={block.block_type}>
              {#if block.block_type === "divider"}
                <div class="flex items-center gap-[var(--sp-2)] py-[var(--sp-2)]">
                  <span class="h-px flex-1 border-t border-border-2" aria-hidden="true"></span>
                  <span class="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
                    {@render blockControls(block, index)}
                  </span>
                </div>
              {:else}
                <div class="flex items-start gap-[var(--sp-1)] py-0.5">
                  {#if block.block_type === "todo"}
                    <button
                      type="button"
                      aria-label={checkedOf(block) ? t("notes.block.untick") : t("notes.block.tick")}
                      onclick={() => void toggleChecked(block)}
                      class="mt-[7px] flex size-4.5 shrink-0 items-center justify-center rounded-full border transition-colors {checkedOf(block)
                        ? 'border-accent bg-accent text-surface-0'
                        : 'border-border-2 bg-surface-1 hover:border-ink-2'}"
                    >
                      {#if checkedOf(block)}
                        <svg viewBox="0 0 12 12" class="size-3" aria-hidden="true"><path d="M2.5 6.2 5 8.6 9.5 3.6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
                      {/if}
                    </button>
                    <textarea
                      value={textOf(block)}
                      onblur={blockSaveHandler(block)}
                      rows={1}
                      dir="auto"
                      class="bidi-auto min-h-6 flex-1 resize-none bg-transparent px-1 py-[3px] text-[var(--fs-base)] outline-none {checkedOf(block) ? 'text-ink-3 line-through' : 'text-ink-1'}"
                      aria-label={t(`notes.block.${block.block_type}` as Parameters<typeof t>[0])}
                    ></textarea>
                  {:else}
                    <textarea
                      value={textOf(block)}
                      onblur={blockSaveHandler(block)}
                      rows={Math.max(1, textOf(block).split("\n").length)}
                      dir="auto"
                      class="bidi-auto w-full flex-1 resize-none bg-transparent px-1 py-[3px] text-ink-1 outline-none {block.block_type === 'heading_1'
                        ? 'text-[var(--fs-lg)] font-semibold'
                        : block.block_type === 'heading_2'
                          ? 'text-[var(--fs-md)] font-semibold'
                          : block.block_type === 'heading_3'
                            ? 'text-[var(--fs-base)] font-semibold'
                            : block.block_type === 'code'
                              ? 'bidi-ltr font-mono text-[var(--fs-sm)] text-ink-1'
                              : block.block_type === 'quote'
                                ? 'border-s-2 border-accent/60 ps-2 text-[var(--fs-base)] text-ink-2'
                                : block.block_type === 'bulleted_list'
                                  ? 'ps-4 text-[var(--fs-base)]'
                                  : block.block_type === 'numbered_list'
                                    ? 'ps-4 text-[var(--fs-base)]'
                                    : 'text-[var(--fs-base)]'}"
                      placeholder={index === 0 && textOf(block).length === 0 && page.title.trim().length === 0 ? t("notes.editor.firstBlockHint") : ""}
                      aria-label={t(`notes.block.${block.block_type}` as Parameters<typeof t>[0])}
                    ></textarea>
                  {/if}

                  <!-- The rails: explicit, keyboard reachable, reversible. -->
                  <span class="flex shrink-0 items-center gap-0.5 py-1 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
                    {@render blockControls(block, index)}
                  </span>
                </div>
              {/if}
            </li>
          {/each}
        </ul>

        <span class="flex justify-start">
          <Button variant="ghost" size="sm" onclick={() => openAddMenu(null)}>
            {#snippet iconStart()}
              <Plus class="size-4 no-mirror" strokeWidth={1.75} />
            {/snippet}
            {t("notes.block.add")}
          </Button>
        </span>
      {/if}
    {/if}
  </div>
{/key}

<Modal open={addMenuOpen} onclose={() => (adding ? undefined : (addMenuOpen = false))} labelledBy="add-block-title">
  <h2 id="add-block-title" class="border-b border-border-1 px-6 py-4 text-[var(--fs-lg)] font-semibold text-ink-1">
    {t("notes.block.addMenu")}
  </h2>
  <ul class="grid grid-cols-2 gap-1 px-4 py-4">
    {#each KINDS as kind (kind)}
      <li>
        <button
          type="button"
          disabled={adding}
          onclick={() => void addBlock(kind)}
          class="w-full rounded-[var(--r-md)] border border-border-1 bg-surface-1 px-[var(--sp-3)] py-[var(--sp-2)] text-start text-[var(--fs-base)] text-ink-2 transition-colors hover:border-accent hover:text-ink-1 disabled:opacity-50"
        >
          {t(`notes.block.${kind}` as Parameters<typeof t>[0])}
        </button>
      </li>
    {/each}
  </ul>
</Modal>
