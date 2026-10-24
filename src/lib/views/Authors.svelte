<script lang="ts">
  import { Pencil, Trash2, TriangleAlert, Users } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import {
    createAuthor,
    deleteAuthor,
    listAuthors,
    updateAuthor,
  } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { messageFor } from "../errors";
  import { toast } from "../toast";
  import type { AppErrorDto, Author, AuthorInput } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  const num = (v: number) => formatNumber(v, { digits });

  /**
   * Authors screen — the people roster (docs/04-ux-design.md §4 screen 7).
   *
   * `author_create`/`author_update`/`author_delete` are the same commands the add-book
   * wizard and the book-page edit use; this screen puts a list, a search box and a form
   * in front of them so a book can be credited before its author exists. Deleting an
   * author only drops their credits (the junction rows) — the books stay, which the
   * confirm dialog says in one clause.
   *
   * Merge duplicates (the design's later step) is deliberately absent: merging is a
   * rewrite of junction rows by id that needs its own screen, and a button that does
   * nothing is worse than no button.
   */

  let authors = $state<Author[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let working = $state(false);
  let query = $state("");

  /** Case-insensitive display-name filter; `name` is the name the user wrote. */
  let filtered = $derived(
    query.trim().length === 0
      ? authors
      : authors.filter((a) => a.name.toLowerCase().includes(query.trim().toLowerCase())),
  );

  async function load(): Promise<void> {
    loading = true;
    const res = await listAuthors();
    if (res.status === "ok") {
      authors = res.data;
      loadError = null;
    } else if (res.error.code === "shell.unavailable") {
      // Browser preview: no backend, so the designed empty state is the honest one.
      authors = [];
      loadError = null;
    } else {
      authors = [];
      loadError = res.error;
    }
    loading = false;
  }

  onMount(() => void load());

  // ── Create + edit ─────────────────────────────────────────────────────────────

  /** `null` means creating; a row means editing that author. */
  let editing = $state<Author | null>(null);
  let editorOpen = $state(false);
  let saving = $state(false);
  let formError = $state<string | null>(null);

  let name = $state("");
  let nameOriginal = $state("");
  let nationality = $state("");
  let bio = $state("");
  let birthYear = $state("");
  let deathYear = $state("");

  function openEditor(row: Author | null): void {
    editing = row;
    editorOpen = true;
  }

  $effect(() => {
    if (!editorOpen) return;
    name = editing?.name ?? "";
    nameOriginal = editing?.name_original ?? "";
    nationality = editing?.nationality ?? "";
    bio = editing?.bio ?? "";
    birthYear = editing?.birth_year === null || editing?.birth_year === undefined ? "" : String(editing?.birth_year);
    deathYear = editing?.death_year === null || editing?.death_year === undefined ? "" : String(editing?.death_year);
    formError = null;
  });

  /** Years accept stars only; `""` means "unset" and one bad digit blocks the save. */
  function parseYear(value: string): number | null | string {
    const trimmed = value.trim();
    if (trimmed.length === 0) return null;
    const parsed = Number.parseInt(trimmed, 10);
    if (!Number.isInteger(parsed) || parsed <= 0) return "bad";
    return parsed;
  }

  async function save(): Promise<void> {
    if (saving) return;
    if (name.trim().length === 0) {
      formError = t("author.error.emptyName");
      return;
    }
    const birth = parseYear(birthYear);
    const death = parseYear(deathYear);
    if (birth === "bad" || death === "bad") {
      formError = t("author.error.years");
      return;
    }
    if (typeof birth === "number" && typeof death === "number" && death < birth) {
      formError = t("author.error.years");
      return;
    }
    const input: AuthorInput = {
      name: name.trim(),
      name_original: nameOriginal.trim().length === 0 ? null : nameOriginal.trim(),
      nationality: nationality.trim().length === 0 ? null : nationality.trim(),
      bio: bio.trim().length === 0 ? null : bio.trim(),
      birth_year: typeof birth === "number" ? birth : null,
      death_year: typeof death === "number" ? death : null,
      photo_path: editing?.photo_path ?? null,
    };
    saving = true;
    formError = null;
    const res =
      editing === null ? await createAuthor(input) : await updateAuthor(editing.id, input);
    saving = false;
    if (res.status === "ok") {
      editorOpen = false;
      toast.success(t("toast.saved"));
      await load();
      return;
    }
    formError = messageFor(res.error, t);
  }

  // ── Delete ────────────────────────────────────────────────────────────────────

  async function removeAuthor(row: Author): Promise<void> {
    if (working) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("authors.deleteConfirm"), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deleteAuthor(row.id);
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
    <h1 class="text-[var(--fs-xl)] font-semibold text-ink-1">{t("nav.authors")}</h1>
    {#if authors.length > 0}
      <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
        {num(authors.length)} {t("authors.count")}
      </span>
    {/if}
    <span class="ms-auto">
      <Button variant="primary" size="sm" onclick={() => openEditor(null)}>
        {#snippet iconStart()}
          <Users class="size-4 no-mirror" strokeWidth={1.75} />
        {/snippet}
        {t("author.form.add")}
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
          {t("authors.loadFailed")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else if authors.length === 0}
    <Card>
      <EmptyState title={t("authors.empty.title")} body={t("authors.empty.body")} />
    </Card>
  {:else}
    <!-- The search box hides once the list is empty of everything to search. -->
    <input
      type="search"
      bind:value={query}
      placeholder={t("authors.search")}
      autocomplete="off"
      class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent sm:max-w-sm"
    />
    <ul class="flex flex-col gap-[var(--sp-2)]">
      {#each filtered as row (row.id)}
        <li>
          <Card>
            <span class="flex items-center gap-[var(--sp-3)]">
              <span class="flex min-w-0 flex-1 flex-col">
                <span class="bidi-auto truncate text-[var(--fs-base)] font-medium text-ink-1" dir="auto">
                  {row.name}
                </span>
                <span class="bidi-auto truncate text-[var(--fs-sm)] text-ink-3" dir="auto">
                  {#if row.nationality !== null}
                    {row.nationality}{#if row.birth_year !== null}
                      · <span class="bidi-ltr tnum" dir="ltr">{num(row.birth_year)}{row.death_year === null ? "" : `–${num(row.death_year)}`}</span>
                    {/if}
                  {:else if row.birth_year !== null}
                    <span class="bidi-ltr tnum" dir="ltr">{num(row.birth_year)}{row.death_year === null ? "" : `–${num(row.death_year)}`}</span>
                  {/if}
                </span>
              </span>
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
                  onclick={() => void removeAuthor(row)}
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

<Modal open={editorOpen} onclose={() => (saving ? undefined : (editorOpen = false))} labelledBy="author-form-title">
  <h2 id="author-form-title" class="border-b border-border-1 px-6 py-4 text-[var(--fs-lg)] font-semibold text-ink-1">
    {editing === null ? t("author.form.add") : t("author.form.edit")}
  </h2>

  <form class="flex flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5" onsubmit={(e) => { e.preventDefault(); void save(); }}>
    <Field label={t("author.name")} for="author-name">
      <input
        id="author-name"
        bind:value={name}
        required
        maxlength={200}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <Field label={t("author.nameOriginal")} for="author-original" hint={t("author.nameOriginal.hint")}>
      <input
        id="author-original"
        bind:value={nameOriginal}
        maxlength={200}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <Field label={t("author.nationality")} for="author-nationality">
      <input
        id="author-nationality"
        bind:value={nationality}
        maxlength={100}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <div class="grid grid-cols-2 gap-[var(--sp-4)]">
      <Field label={t("author.birth")} for="author-birth">
        <input
          id="author-birth"
          inputmode="numeric"
          bind:value={birthYear}
          pattern="[0-9]*"
          class="tnum bidi-ltr h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        />
      </Field>
      <Field label={t("author.death")} for="author-death">
        <input
          id="author-death"
          inputmode="numeric"
          bind:value={deathYear}
          pattern="[0-9]*"
          class="tnum bidi-ltr h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        />
      </Field>
    </div>

    <Field label={t("author.bio")} for="author-bio">
      <textarea
        id="author-bio"
        bind:value={bio}
        rows={3}
        maxlength={4000}
        class="w-full resize-y rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] py-[var(--sp-2)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      ></textarea>
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
