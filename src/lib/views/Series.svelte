<script lang="ts">
  import { BookCopy, Pencil, Trash2, TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import { createSeries, deleteSeries, listSeries, updateSeries } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { messageFor } from "../errors";
  import { toast } from "../toast";
  import type { AppErrorDto, Series, SeriesInput } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  const num = (v: number) => formatNumber(v, { digits });

  /**
   * Series screen — the collections a book belongs to (`السلسلة`).
   *
   * The wizard creates series inline, but a series lives longer than any one capture:
   * names get typos, descriptions move in later, and whole series get added before the
   * first book lands. Same three commands (`series_create`/`series_update`/`series_delete`),
   * a list of their own. Deleting a series only nulls `books.series_id` (`ON DELETE
   * SET NULL`) — the books keep their `series_order`, which the confirm says plainly.
   */

  let seriesList = $state<Series[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let working = $state(false);
  let query = $state("");

  /** Case-insensitive display-name filter; `name` is the name the user wrote. */
  let filtered = $derived(
    query.trim().length === 0
      ? seriesList
      : seriesList.filter((s) => s.name.toLowerCase().includes(query.trim().toLowerCase())),
  );

  async function load(): Promise<void> {
    loading = true;
    const res = await listSeries();
    if (res.status === "ok") {
      seriesList = res.data;
      loadError = null;
    } else if (res.error.code === "shell.unavailable") {
      // Browser preview: no backend, so the designed empty state is the honest one.
      seriesList = [];
      loadError = null;
    } else {
      seriesList = [];
      loadError = res.error;
    }
    loading = false;
  }

  onMount(() => void load());

  // ── Create + edit ─────────────────────────────────────────────────────────────

  /** `null` means creating; a row means editing that series. */
  let editing = $state<Series | null>(null);
  let editorOpen = $state(false);
  let saving = $state(false);
  let formError = $state<string | null>(null);

  let name = $state("");
  let description = $state("");

  function openEditor(row: Series | null): void {
    editing = row;
    editorOpen = true;
  }

  $effect(() => {
    if (!editorOpen) return;
    name = editing?.name ?? "";
    description = editing?.description ?? "";
    formError = null;
  });

  async function save(): Promise<void> {
    if (saving) return;
    if (name.trim().length === 0) {
      formError = t("series.error.emptyName");
      return;
    }
    const input: SeriesInput = {
      name: name.trim(),
      description: description.trim().length === 0 ? null : description.trim(),
    };
    saving = true;
    formError = null;
    const res = editing === null ? await createSeries(input) : await updateSeries(editing.id, input);
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

  async function removeSeries(row: Series): Promise<void> {
    if (working) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("series.deleteConfirm"), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deleteSeries(row.id);
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
    <h1 class="text-[var(--fs-xl)] font-semibold text-ink-1">{t("nav.series")}</h1>
    {#if seriesList.length > 0}
      <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
        {num(seriesList.length)} {t("series.count")}
      </span>
    {/if}
    <span class="ms-auto">
      <Button variant="primary" size="sm" onclick={() => openEditor(null)}>
        {#snippet iconStart()}
          <BookCopy class="size-4 no-mirror" strokeWidth={1.75} />
        {/snippet}
        {t("series.form.add")}
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
          {t("series.loadFailed")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else if seriesList.length === 0}
    <Card>
      <EmptyState title={t("series.empty.title")} body={t("series.empty.body")} />
    </Card>
  {:else}
    <input
      type="search"
      bind:value={query}
      placeholder={t("series.search")}
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
                {#if row.description !== null}
                  <span class="bidi-auto truncate text-[var(--fs-sm)] text-ink-3" dir="auto">
                    {row.description}
                  </span>
                {/if}
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
                  onclick={() => void removeSeries(row)}
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

<Modal open={editorOpen} onclose={() => (saving ? undefined : (editorOpen = false))} labelledBy="series-form-title">
  <h2 id="series-form-title" class="border-b border-border-1 px-6 py-4 text-[var(--fs-lg)] font-semibold text-ink-1">
    {editing === null ? t("series.form.add") : t("series.form.edit")}
  </h2>

  <form class="flex flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5" onsubmit={(e) => { e.preventDefault(); void save(); }}>
    <Field label={t("series.name")} for="series-name">
      <input
        id="series-name"
        bind:value={name}
        required
        maxlength={200}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <Field label={t("series.description")} for="series-description">
      <textarea
        id="series-description"
        bind:value={description}
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
