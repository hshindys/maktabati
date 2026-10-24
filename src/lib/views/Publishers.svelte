<script lang="ts">
  import { Pencil, Store, Trash2, TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Field from "../components/Field.svelte";
  import Modal from "../components/Modal.svelte";
  import {
    createPublisher,
    deletePublisher,
    listPublishers,
    updatePublisher,
  } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { messageFor } from "../errors";
  import { toast } from "../toast";
  import type { AppErrorDto, Publisher, PublisherInput } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  const num = (v: number) => formatNumber(v, { digits });

  /**
   * Publishers screen — the houses roster (docs/04-ux-design.md §4 screen 8).
   *
   * The same commands the add-book wizard reaches inline, given a list, a search box and
   * a form of their own. Deleting a publisher only nulls `books.publisher_id` (`ON DELETE
   * SET NULL`) — the confirm dialog carries that clause so the user is not deleting a
   * row they think their books depend on.
   */

  let publishers = $state<Publisher[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let working = $state(false);
  let query = $state("");

  /** Case-insensitive display-name filter; `name` is the name the user wrote. */
  let filtered = $derived(
    query.trim().length === 0
      ? publishers
      : publishers.filter((p) => p.name.toLowerCase().includes(query.trim().toLowerCase())),
  );

  async function load(): Promise<void> {
    loading = true;
    const res = await listPublishers();
    if (res.status === "ok") {
      publishers = res.data;
      loadError = null;
    } else if (res.error.code === "shell.unavailable") {
      // Browser preview: no backend, so the designed empty state is the honest one.
      publishers = [];
      loadError = null;
    } else {
      publishers = [];
      loadError = res.error;
    }
    loading = false;
  }

  onMount(() => void load());

  // ── Create + edit ─────────────────────────────────────────────────────────────

  /** `null` means creating; a row means editing that publisher. */
  let editing = $state<Publisher | null>(null);
  let editorOpen = $state(false);
  let saving = $state(false);
  let formError = $state<string | null>(null);

  let name = $state("");
  let country = $state("");
  let city = $state("");
  let website = $state("");
  let notes = $state("");

  function openEditor(row: Publisher | null): void {
    editing = row;
    editorOpen = true;
  }

  $effect(() => {
    if (!editorOpen) return;
    name = editing?.name ?? "";
    country = editing?.country ?? "";
    city = editing?.city ?? "";
    website = editing?.website ?? "";
    notes = editing?.notes ?? "";
    formError = null;
  });

  async function save(): Promise<void> {
    if (saving) return;
    if (name.trim().length === 0) {
      formError = t("publisher.error.emptyName");
      return;
    }
    const input: PublisherInput = {
      name: name.trim(),
      country: country.trim().length === 0 ? null : country.trim(),
      city: city.trim().length === 0 ? null : city.trim(),
      website: website.trim().length === 0 ? null : website.trim(),
      notes: notes.trim().length === 0 ? null : notes.trim(),
      cover_path: editing?.cover_path ?? null,
    };
    saving = true;
    formError = null;
    const res =
      editing === null ? await createPublisher(input) : await updatePublisher(editing.id, input);
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

  async function removePublisher(row: Publisher): Promise<void> {
    if (working) return;
    let confirmed = false;
    try {
      confirmed = await ask(t("publishers.deleteConfirm"), { kind: "warning" });
    } catch {
      return; // no shell — a preview has nothing to delete
    }
    if (!confirmed) return;
    working = true;
    try {
      const res = await deletePublisher(row.id);
      toast[res.status === "ok" ? "success" : "error"](
        res.status === "ok" ? t("toast.deleted") : messageFor(res.error, t),
      );
      await load();
    } finally {
      working = false;
    }
  }

  /** Only `http(s)` schemes link out; anything else is shown as text instead of trusted. */
  const safeLink = (url: string) => (/^https?:\/\//i.test(url) ? url : null);
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-4)] px-[var(--sp-6)] py-[var(--sp-5)]">
  <header class="flex items-baseline gap-[var(--sp-3)]">
    <h1 class="text-[var(--fs-xl)] font-semibold text-ink-1">{t("nav.publishers")}</h1>
    {#if publishers.length > 0}
      <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
        {num(publishers.length)} {t("publishers.count")}
      </span>
    {/if}
    <span class="ms-auto">
      <Button variant="primary" size="sm" onclick={() => openEditor(null)}>
        {#snippet iconStart()}
          <Store class="size-4 no-mirror" strokeWidth={1.75} />
        {/snippet}
        {t("publisher.form.add")}
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
          {t("publishers.loadFailed")}
        </div>
        <p class="text-[var(--fs-base)] text-ink-2" dir="auto">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else if publishers.length === 0}
    <Card>
      <EmptyState title={t("publishers.empty.title")} body={t("publishers.empty.body")} />
    </Card>
  {:else}
    <input
      type="search"
      bind:value={query}
      placeholder={t("publishers.search")}
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
                <span class="bidi-auto flex min-w-0 gap-1 text-[var(--fs-sm)] text-ink-3" dir="auto">
                  {#if row.country !== null || row.city !== null}
                    <span class="truncate">{[row.city, row.country].filter((v) => v !== null).join(" — ")}</span>
                  {/if}
                  {#if row.website !== null && (row.country !== null || row.city !== null)}
                    <span aria-hidden="true">·</span>
                  {/if}
                  {#if row.website !== null}
                    {@const url = safeLink(row.website)}
                    {#if url !== null}
                      <!-- The website is the one fact worth leaving the app for; the opener
                           plugin owns that hand-off, exactly like a book file. -->
                      <a
                        href={url}
                        target="_blank"
                        rel="noreferrer"
                        class="bidi-ltr truncate transition-colors hover:text-accent hover:underline"
                        dir="ltr"
                      >
                        {row.website}
                      </a>
                    {:else}
                      <span class="bidi-ltr truncate" dir="ltr">{row.website}</span>
                    {/if}
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
                  onclick={() => void removePublisher(row)}
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

<Modal open={editorOpen} onclose={() => (saving ? undefined : (editorOpen = false))} labelledBy="publisher-form-title">
  <h2 id="publisher-form-title" class="border-b border-border-1 px-6 py-4 text-[var(--fs-lg)] font-semibold text-ink-1">
    {editing === null ? t("publisher.form.add") : t("publisher.form.edit")}
  </h2>

  <form class="flex flex-col gap-[var(--sp-4)] overflow-y-auto px-6 py-5" onsubmit={(e) => { e.preventDefault(); void save(); }}>
    <Field label={t("publisher.name")} for="publisher-name">
      <input
        id="publisher-name"
        bind:value={name}
        required
        maxlength={200}
        class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <div class="grid grid-cols-2 gap-[var(--sp-4)]">
      <Field label={t("publisher.country")} for="publisher-country">
        <input
          id="publisher-country"
          bind:value={country}
          maxlength={100}
          class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        />
      </Field>
      <Field label={t("publisher.city")} for="publisher-city">
        <input
          id="publisher-city"
          bind:value={city}
          maxlength={100}
          class="bidi-auto h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
        />
      </Field>
    </div>

    <Field label={t("publisher.website")} for="publisher-website" hint={t("publisher.website.hint")}>
      <input
        id="publisher-website"
        inputmode="url"
        bind:value={website}
        maxlength={200}
        class="bidi-ltr h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] text-ink-1 outline-none focus:border-accent"
      />
    </Field>

    <Field label={t("publisher.notes")} for="publisher-notes">
      <textarea
        id="publisher-notes"
        bind:value={notes}
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
