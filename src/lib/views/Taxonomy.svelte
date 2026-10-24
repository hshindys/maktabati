<script lang="ts">
  import { FolderTree, Pencil, Plus, Tag as TagIcon, Trash2, TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import {
    createCategory,
    createTag,
    deleteCategory,
    deleteTag,
    listCategories,
    listTags,
    updateCategory,
    updateTag,
  } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { messageFor } from "../errors";
  import { toast } from "../toast";
  import type { AppErrorDto, Category, CategoryInput, Tag, TagInput } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  const num = (v: number) => formatNumber(v, { digits });

  /**
   * Taxonomy screen — the categories tree and the tags (docs/04-ux-design.md §4 screen 9).
   *
   * The two halves share one screen because they answer one question — "what words do my
   * books answer to?" — with different shapes: categories are a nested tree (one book,
   * several ancestors) while tags are a flat bag (cutting across the tree). Both halves
   * drive the same commands the add-book wizard and book edit use, so a taxonomy built
   * here is instantly offered as autocomplete everywhere.
   *
   * The tree renders fully expanded, indented by `depth`: on a 50k-book machine the tree
   * is dozens wide, not thousands, and collapsing states add a second way for a category
   * to hide. System-seeded categories say so and refuse deletion in the backend — the
   * error surfaces in the toast rather than being pre-disabled here, so the database
   * stays the single authority on what may be deleted.
   *
   * A future "move a branch" control is deliberately absent: moving changes every
   * descendant's materialized path and the backend does not expose it yet, and a move
   * that lies about success would orphan every book link in the branch.
   */

  // ── Categories: load + tree ─────────────────────────────────────────────────────

  let categories = $state<Category[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let working = $state(false);
  let query = $state("");

  const nameOf = (c: Category) => ($locale === "ar" ? c.name_ar : c.name_en);
  const otherNameOf = (c: Category) => ($locale === "ar" ? c.name_en : c.name_ar);

  /** Search flattens the tree: the full materialized path tells a category apart. */
  let filtered = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    if (needle.length === 0) return null;
    return categories.filter(
      (c) => c.name_ar.toLowerCase().includes(needle) || c.name_en.toLowerCase().includes(needle) || c.path.includes(needle),
    );
  });

  /** Root categories in stable order, each with its descendants already beneath it. */
  let tree = $derived.by(() => {
    const byParent = new Map<string | null, Category[]>();
    for (const c of [...categories].sort((a, b) => a.position - b.position)) {
      const bucket = byParent.get(c.parent_id) ?? [];
      bucket.push(c);
      byParent.set(c.parent_id, bucket);
    }
    const flattened: Array<{ category: Category; indent: number }> = [];
    const walk = (parentId: string | null, indent: number): void => {
      for (const c of byParent.get(parentId) ?? []) {
        flattened.push({ category: c, indent });
        walk(c.id, indent + 1);
      }
    };
    walk(null, 0);
    return flattened;
  });

  async function load(): Promise<void> {
    loading = true;
    const results = await Promise.all([listCategories(), listTags()]);
    const cats = results[0];
    if (cats.status === "ok") {
      categories = cats.data;
    } else if (cats.error.code !== "shell.unavailable") {
      loadError = cats.error;
      loading = false;
      return;
    }
    const tgs = results[1];
    if (tgs.status === "ok") {
      tags = tgs.data;
    } else if (tgs.error.code !== "shell.unavailable") {
      loadError = tgs.error;
      loading = false;
      return;
    }
    // A preview (no shell) shows two honest empty sections, rejecting the loading state.
    loadError = null;
    loading = false;
  }

  onMount(() => void load());

  // ── Categories: create + edit ─────────────────────────────────────────────────

  /** `null` means creating; a row means editing (the parent and key stay fixed). */
  let editing = $state<Category | null>(null);
  let editorOpen = $state(false);
  let saving = $state(false);
  let formError = $state<string | null>(null);

  let parentId = $state<string | null>(null);
  let key = $state("");
  let nameAr = $state("");
  let nameEn = $state("");
  let color = $state("");

  function openEditor(row: Category | null): void {
    editing = row;
    editorOpen = true;
  }

  $effect(() => {
    if (!editorOpen) return;
    parentId = editing?.parent_id ?? null;
    key = editing?.key ?? "";
    nameAr = editing?.name_ar ?? "";
    nameEn = editing?.name_en ?? "";
    color = editing?.color ?? "";
    formError = null;
  });

  /** `#RRGGBB` or nothing; a colour half-typed is a colour half-drawn. */
  function parseColor(): string | null | string {
    const trimmed = color.trim();
    if (trimmed.length === 0) return null;
    return /^#[0-9a-fA-F]{6}$/.test(trimmed) ? trimmed.toLowerCase() : "bad";
  }

  async function saveCategory(): Promise<void> {
    if (saving) return;
    if (editing === null && key.trim().length === 0) {
      formError = t("category.error.emptyKey");
      return;
    }
    if (nameAr.trim().length === 0 || nameEn.trim().length === 0) {
      formError = t("category.error.emptyName");
      return;
    }
    const shade = parseColor();
    if (shade === "bad") {
      formError = t("category.error.color");
      return;
    }
    // On edit the repo keeps the stored key — the field is hidden, not lying.
    const input: CategoryInput = {
      key: key.trim(),
      name_ar: nameAr.trim(),
      name_en: nameEn.trim(),
      icon: null,
      color: shade,
    };
    saving = true;
    formError = null;
    const res =
      editing === null
        ? await createCategory(input, parentId)
        : await updateCategory(editing.id, input);
    saving = false;
    if (res.status === "ok") {
      editorOpen = false;
      toast.success(t("toast.saved"));
      await load();
      return;
    }
    formError = messageFor(res.error, t);
  }

  async function removeCategory(row: Category): Promise<void> {
    if (working) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("category.deleteConfirm", { name: nameOf(row) }), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deleteCategory(row.id);
      toast[res.status === "ok" ? "success" : "error"](
        res.status === "ok" ? t("toast.deleted") : messageFor(res.error, t),
      );
      await load();
    } finally {
      working = false;
    }
  }

  // ── Tags ────────────────────────────────────────────────────────────────────────

  let tags = $state<Tag[]>([]);
  let tagEditorOpen = $state(false);
  let editingTag = $state<Tag | null>(null);
  let savingTag = $state(false);
  let tagFormError = $state<string | null>(null);

  let tagName = $state("");
  let tagColor = $state("");

  let filteredTags = $derived(
    query.trim().length === 0
      ? tags
      : tags.filter((tag) => tag.name.toLowerCase().includes(query.trim().toLowerCase())),
  );

  function openTagEditor(row: Tag | null): void {
    editingTag = row;
    tagEditorOpen = true;
  }

  $effect(() => {
    if (!tagEditorOpen) return;
    tagName = editingTag?.name ?? "";
    tagColor = editingTag?.color ?? "";
    tagFormError = null;
  });

  async function saveTag(): Promise<void> {
    if (savingTag) return;
    if (tagName.trim().length === 0) {
      tagFormError = t("tag.error.emptyName");
      return;
    }
    const shade = parseColor();
    if (shade === "bad") {
      tagFormError = t("category.error.color");
      return;
    }
    const input: TagInput = { name: tagName.trim(), color: shade, };
    savingTag = true;
    tagFormError = null;
    const res =
      editingTag === null ? await createTag(input) : await updateTag(editingTag.id, input);
    savingTag = false;
    if (res.status === "ok") {
      tagEditorOpen = false;
      toast.success(t("toast.saved"));
      await load();
      return;
    }
    tagFormError = messageFor(res.error, t);
  }

  async function removeTag(row: Tag): Promise<void> {
    if (working) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("tag.deleteConfirm", { name: row.name }), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deleteTag(row.id);
      toast[res.status === "ok" ? "success" : "error"](
        res.status === "ok" ? t("toast.deleted") : messageFor(res.error, t),
      );
      await load();
    } finally {
      working = false;
    }
  }
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-6)] px-[var(--sp-6)] py-[var(--sp-5)]">
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
          {t("taxonomy.loadFailed")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else}
    <input
      type="search"
      bind:value={query}
      placeholder={t("taxonomy.search")}
      autocomplete="off"
      class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent sm:max-w-sm"
    />

    <!-- ── Categories ─────────────────────────────────────────────────────────────-- -->
    <section class="flex flex-col gap-[var(--sp-3)]" aria-labelledby="taxonomy-categories-title">
      <header class="flex items-baseline gap-[var(--sp-3)]">
        <h2 id="taxonomy-categories-title" class="flex items-center gap-2 text-[var(--fs-lg)] font-semibold text-ink-1">
          <FolderTree class="size-5 no-mirror" strokeWidth={1.75} />
          {t("nav.categories")}
        </h2>
        {#if categories.length > 0}
          <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
            {num(categories.length)} {t("categories.count")}
          </span>
        {/if}
        <span class="ms-auto">
          <Button variant="primary" size="sm" onclick={() => openEditor(null)}>
            {#snippet iconStart()}
              <Plus class="size-4 no-mirror" strokeWidth={1.75} />
            {/snippet}
            {t("category.form.add")}
          </Button>
        </span>
      </header>

      {#if categories.length === 0}
        <Card><EmptyState title={t("categories.empty.title")} body={t("categories.empty.body")} /></Card>
      {:else if filtered !== null && filtered.length === 0}
        <Card><EmptyState title={t("taxonomy.noMatches.title")} body={t("taxonomy.noMatches.body")} /></Card>
      {:else if filtered !== null}
        <ul class="flex flex-col gap-[var(--sp-2)]">
          {#each filtered as row (row.id)}
            <li>
              <Card>
                {@render categoryRow(row, row.depth, row.path)}
              </Card>
            </li>
          {/each}
        </ul>
      {:else}
        <ul class="flex flex-col gap-[var(--sp-2)]">
          {#each tree as node (node.category.id)}
            <li>
              <Card>
                {@render categoryRow(node.category, node.indent, node.category.path)}
              </Card>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <!-- ── Tags ─────────────────────────────────────────────────────────────────────-- -->
    <section class="flex flex-col gap-[var(--sp-3)]" aria-labelledby="taxonomy-tags-title">
      <header class="flex items-baseline gap-[var(--sp-3)]">
        <h2 id="taxonomy-tags-title" class="flex items-center gap-2 text-[var(--fs-lg)] font-semibold text-ink-1">
          <TagIcon class="size-5 no-mirror" strokeWidth={1.75} />
          {t("tags.title")}
        </h2>
        {#if tags.length > 0}
          <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
            {num(tags.length)} {t("tags.count")}
          </span>
        {/if}
        <span class="ms-auto">
          <Button variant="primary" size="sm" onclick={() => openTagEditor(null)}>
            {#snippet iconStart()}
              <Plus class="size-4 no-mirror" strokeWidth={1.75} />
            {/snippet}
            {t("tag.form.add")}
          </Button>
        </span>
      </header>

      {#if tags.length === 0}
        <Card><EmptyState title={t("tags.empty.title")} body={t("tags.empty.body")} /></Card>
      {:else}
        <ul class="flex flex-wrap gap-[var(--sp-2)]">
          {#each filteredTags as row (row.id)}
            <li class="flex items-center gap-1.5 rounded-[var(--r-full)] border border-border-2 bg-surface-1 py-1 ps-3 pe-1.5">
              {#if row.color !== null}
                <span class="size-3 rounded-full" style={`background: ${row.color};`} aria-hidden="true"></span>
              {/if}
              <span class="bidi-auto text-[var(--fs-base)] text-ink-1" dir="auto">{row.name}</span>
              <button
                type="button"
                aria-label={t("common.edit")}
                title={t("common.edit")}
                onclick={() => openTagEditor(row)}
                class="inline-flex items-center justify-center rounded-full p-1 text-ink-3 transition-colors hover:text-ink-1"
              >
                <Pencil class="size-3.5 no-mirror" strokeWidth={1.75} />
              </button>
              <button
                type="button"
                aria-label={t("common.delete")}
                title={t("common.delete")}
                disabled={working}
                onclick={() => void removeTag(row)}
                class="inline-flex items-center justify-center rounded-full p-1 text-ink-3 transition-colors hover:text-danger disabled:opacity-50"
              >
                <Trash2 class="size-3.5 no-mirror" strokeWidth={1.75} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
</div>

{#snippet categoryRow(row: Category, indent: number, path: string)}
  <span class="flex items-center gap-[var(--sp-2)]" style={`padding-inline-start: ${indent * 1.25}rem`}>
    {#if row.color !== null}
      <span class="size-3 shrink-0 rounded-full" style={`background: ${row.color};`} aria-hidden="true"></span>
    {/if}
    <span class="flex min-w-0 flex-1 flex-col">
      <span class="bidi-auto truncate text-[var(--fs-base)] font-medium text-ink-1" dir="auto">
        {nameOf(row)}{#if !row.is_system}<span class="text-ink-3"> · {otherNameOf(row)}</span>{/if}
      </span>
      <span class="bidi-ltr truncate text-[var(--fs-sm)] text-ink-3" dir="ltr">{path}</span>
    </span>
    {#if row.is_system}
      <span class="shrink-0 rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-3">
        {t("category.system")}
      </span>
    {:else}
      <span class="flex shrink-0 items-center gap-1">
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
          onclick={() => void removeCategory(row)}
          class="inline-flex items-center justify-center rounded-[var(--r-md)] border border-border-2 bg-surface-1 p-1.5 text-ink-2 transition-colors hover:border-danger/50 hover:text-danger disabled:opacity-50"
        >
          <Trash2 class="size-3.5 no-mirror" strokeWidth={1.75} />
        </button>
      </span>
    {/if}
  </span>
{/snippet}

<Modal open={editorOpen} onclose={() => (saving ? undefined : (editorOpen = false))} labelledBy="category-form-title">
  <h2 id="category-form-title" class="border-b border-border-1 px-6 py-4 text-[var(--fs-lg)] font-semibold text-ink-1">
    {editing === null ? t("category.form.add") : t("category.form.edit")}
  </h2>

  <form class="flex flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5" onsubmit={(e) => { e.preventDefault(); void saveCategory(); }}>
    {#if editing === null}
      <Field label={t("category.parent")} for="category-parent">
        <select
          id="category-parent"
          bind:value={parentId}
          class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        >
          <!-- `bind:value` keeps the select's `""` sentinel on the "top level" option,
               while every real option carries its id. -->
          <option value="">{t("category.parent.none")}</option>
          {#each tree as node (node.category.id)}
            <option value={node.category.id}>{nameOf(node.category)}</option>
          {/each}
        </select>
      </Field>

      <Field label={t("category.key")} for="category-key" hint={t("category.key.hint")}>
        <input
          id="category-key"
          bind:value={key}
          required
          maxlength={100}
          dir="ltr"
          class="bidi-ltr h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        />
      </Field>
    {/if}

    <Field label={t("category.nameAr")} for="category-name-ar">
      <input
        id="category-name-ar"
        bind:value={nameAr}
        required
        lang="ar"
        maxlength={200}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <Field label={t("category.nameEn")} for="category-name-en">
      <input
        id="category-name-en"
        bind:value={nameEn}
        required
        lang="en"
        maxlength={200}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <Field label={t("category.color")} for="category-color" hint={t("category.color.hint")}>
      <input
        id="category-color"
        bind:value={color}
        maxlength={7}
        dir="ltr"
        placeholder="#RRGGBB"
        class="bidi-ltr h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    {#if formError !== null}
      <p role="alert" class="bidi-auto rounded-[var(--r-md)] bg-danger-soft px-[var(--sp-3)] py-[var(--sp-2)] text-[var(--fs-sm)] text-danger" dir="auto">
        {formError}
      </p>
    {/if}

    <div class="flex items-center justify-end gap-[var(--sp-2)] border-t border-border-1 pt-[var(--sp-4)]">
      <Button variant="ghost" disabled={saving} onclick={() => (editorOpen = false)}>{t("common.cancel")}</Button>
      <Button type="submit" variant="primary" loading={saving}>{t("common.save")}</Button>
    </div>
  </form>
</Modal>

<Modal open={tagEditorOpen} onclose={() => (savingTag ? undefined : (tagEditorOpen = false))} labelledBy="tag-form-title">
  <h2 id="tag-form-title" class="border-b border-border-1 px-6 py-4 text-[var(--fs-lg)] font-semibold text-ink-1">
    {editingTag === null ? t("tag.form.add") : t("tag.form.edit")}
  </h2>

  <form class="flex flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5" onsubmit={(e) => { e.preventDefault(); void saveTag(); }}>
    <Field label={t("tag.name")} for="tag-name">
      <input
        id="tag-name"
        bind:value={tagName}
        required
        maxlength={100}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <Field label={t("tag.color")} for="tag-color" hint={t("category.color.hint")}>
      <input
        id="tag-color"
        bind:value={tagColor}
        maxlength={7}
        dir="ltr"
        placeholder="#RRGGBB"
        class="bidi-ltr h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    {#if tagFormError !== null}
      <p role="alert" class="bidi-auto rounded-[var(--r-md)] bg-danger-soft px-[var(--sp-3)] py-[var(--sp-2)] text-[var(--fs-sm)] text-danger" dir="auto">
        {tagFormError}
      </p>
    {/if}

    <div class="flex items-center justify-end gap-[var(--sp-2)] border-t border-border-1 pt-[var(--sp-4)]">
      <Button variant="ghost" disabled={savingTag} onclick={() => (tagEditorOpen = false)}>{t("common.cancel")}</Button>
      <Button type="submit" variant="primary" loading={savingTag}>{t("common.save")}</Button>
    </div>
  </form>
</Modal>
