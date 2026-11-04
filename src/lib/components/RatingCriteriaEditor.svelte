<script lang="ts">
  import { onMount } from "svelte";

  import Button from "./Button.svelte";
  import Card from "./Card.svelte";
  import {
    createRatingCriterion,
    listRatingCriteria,
    updateRatingCriterion,
    type RatingCriterion,
    type RatingCriterionInput,
  } from "../api";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { toast } from "../toast";
  import type { AppErrorDto } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let criteria = $state<RatingCriterion[]>([]);
  let drafts = $state<Record<string, RatingCriterionInput>>({});
  let loading = $state(true);
  let loadError = $state<AppErrorDto | null>(null);
  let savingId = $state<string | null>(null);
  let creating = $state(false);
  let newNameAr = $state("");
  let newNameEn = $state("");

  const toInput = (criterion: RatingCriterion): RatingCriterionInput => ({
    name_ar: criterion.name_ar,
    name_en: criterion.name_en,
    description_ar: criterion.description_ar,
    description_en: criterion.description_en,
    weight: criterion.weight,
    position: criterion.position,
    is_active: criterion.is_active,
  });

  async function load(): Promise<void> {
    loading = true;
    const result = await listRatingCriteria();
    if (result.status === "ok") {
      criteria = result.data;
      drafts = Object.fromEntries(result.data.map((criterion) => [criterion.id, toInput(criterion)]));
      loadError = null;
    } else if (result.error.code === "shell.unavailable") {
      criteria = [];
      drafts = {};
      loadError = null;
    } else {
      loadError = result.error;
    }
    loading = false;
  }

  onMount(() => void load());

  function updateDraft(id: string, patch: Partial<RatingCriterionInput>): void {
    const current = drafts[id];
    if (current !== undefined) drafts = { ...drafts, [id]: { ...current, ...patch } };
  }

  async function save(criterion: RatingCriterion): Promise<void> {
    if (savingId !== null) return;
    savingId = criterion.id;
    const result = await updateRatingCriterion(criterion.id, drafts[criterion.id]);
    if (result.status === "ok") {
      toast.success(t("toast.saved"));
      await load();
    } else {
      toast.error(messageFor(result.error, t));
    }
    savingId = null;
  }

  async function add(): Promise<void> {
    if (creating) return;
    creating = true;
    const input: RatingCriterionInput = {
      name_ar: newNameAr,
      name_en: newNameEn,
      description_ar: null,
      description_en: null,
      weight: 1,
      position: criteria.length,
      is_active: true,
    };
    const result = await createRatingCriterion(input);
    if (result.status === "ok") {
      newNameAr = "";
      newNameEn = "";
      toast.success(t("toast.saved"));
      await load();
    } else {
      toast.error(messageFor(result.error, t));
    }
    creating = false;
  }
</script>

<Card>
  <h2 class="text-[var(--fs-md)] font-semibold text-ink-1">{t("evaluation.criteria.title")}</h2>
  <p class="mt-1 text-[var(--fs-sm)] text-ink-3">{t("evaluation.criteria.description")}</p>

  {#if loading}
    <p class="mt-[var(--sp-3)] text-[var(--fs-sm)] text-ink-3">{t("common.loading")}</p>
  {:else if loadError !== null}
    <p role="alert" class="mt-[var(--sp-3)] text-[var(--fs-sm)] text-danger">
      {messageFor(loadError, t)}
    </p>
  {:else}
    <ul class="mt-[var(--sp-4)] flex flex-col gap-[var(--sp-3)]">
      {#each criteria as criterion (criterion.id)}
        {@const draft = drafts[criterion.id]}
        <li class="rounded-[var(--r-md)] border border-border-1 bg-surface-1 p-[var(--sp-3)]">
          <div class="grid gap-[var(--sp-3)] sm:grid-cols-2">
            <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
              {t("evaluation.criteria.nameAr")}
              <input
                value={draft.name_ar}
                oninput={(event) => updateDraft(criterion.id, { name_ar: event.currentTarget.value })}
                dir="rtl"
                class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
              />
            </label>
            <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
              {t("evaluation.criteria.nameEn")}
              <input
                value={draft.name_en}
                oninput={(event) => updateDraft(criterion.id, { name_en: event.currentTarget.value })}
                dir="ltr"
                class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
              />
            </label>
            <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
              {t("evaluation.criteria.descriptionAr")}
              <input
                value={draft.description_ar ?? ""}
                oninput={(event) =>
                  updateDraft(criterion.id, {
                    description_ar: event.currentTarget.value.length === 0 ? null : event.currentTarget.value,
                  })}
                dir="rtl"
                class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
              />
            </label>
            <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
              {t("evaluation.criteria.descriptionEn")}
              <input
                value={draft.description_en ?? ""}
                oninput={(event) =>
                  updateDraft(criterion.id, {
                    description_en: event.currentTarget.value.length === 0 ? null : event.currentTarget.value,
                  })}
                dir="ltr"
                class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
              />
            </label>
            <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
              {t("evaluation.criteria.weight")}
              <input
                type="number"
                min="0"
                max="100"
                step="0.1"
                value={draft.weight}
                oninput={(event) =>
                  updateDraft(criterion.id, { weight: Number(event.currentTarget.value) })}
                class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
              />
            </label>
            <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
              {t("evaluation.criteria.position")}
              <input
                type="number"
                min="0"
                step="1"
                value={draft.position}
                oninput={(event) =>
                  updateDraft(criterion.id, { position: Number(event.currentTarget.value) })}
                class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
              />
            </label>
          </div>
          <div class="mt-[var(--sp-3)] flex flex-wrap items-center justify-between gap-[var(--sp-3)]">
            <label class="flex items-center gap-2 text-[var(--fs-sm)] text-ink-2">
              <input
                type="checkbox"
                checked={draft.is_active}
                onchange={(event) =>
                  updateDraft(criterion.id, { is_active: event.currentTarget.checked })}
                class="size-4 accent-[var(--accent)]"
              />
              {t("evaluation.criteria.active")}
            </label>
            <Button
              variant="secondary"
              loading={savingId === criterion.id}
              disabled={savingId !== null || creating}
              onclick={() => void save(criterion)}
            >
              {t("common.save")}
            </Button>
          </div>
        </li>
      {/each}
    </ul>

    <form
      class="mt-[var(--sp-4)] flex flex-col gap-[var(--sp-3)] border-t border-border-1 pt-[var(--sp-4)]"
      onsubmit={(event) => {
        event.preventDefault();
        void add();
      }}
    >
      <h3 class="text-[var(--fs-base)] font-medium text-ink-1">{t("evaluation.criteria.addTitle")}</h3>
      <div class="grid gap-[var(--sp-3)] sm:grid-cols-2">
        <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
          {t("evaluation.criteria.nameAr")}
          <input
            bind:value={newNameAr}
            required
            dir="rtl"
            class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
          />
        </label>
        <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
          {t("evaluation.criteria.nameEn")}
          <input
            bind:value={newNameEn}
            required
            dir="ltr"
            class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-[var(--sp-3)] text-ink-1 outline-none focus:border-accent"
          />
        </label>
      </div>
      <div>
        <Button type="submit" variant="primary" loading={creating} disabled={savingId !== null}>
          {t("evaluation.criteria.add")}
        </Button>
      </div>
    </form>
  {/if}
</Card>
