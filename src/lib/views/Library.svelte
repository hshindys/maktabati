<script lang="ts">
  import { Grid3x3, List, Rows3 } from "@lucide/svelte";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import { locale, translatorFor } from "../i18n";
  import { formatNumber } from "../format";
  import { preferences } from "../settings";

  /**
   * Library browser — the screen the app is judged on.
   *
   * Layout and controls are the real thing; the result grid is populated from the book query
   * once CRUD lands in the next milestone, so this screen already establishes the three view
   * modes, the filter/sort bar and the empty state that the 50k-book list will drop into.
   */

  type ViewMode = "grid" | "list" | "table";
  const VIEWS: Array<{ mode: ViewMode; icon: typeof Grid3x3; key: "library.view.grid" | "library.view.list" | "library.view.table" }> = [
    { mode: "grid", icon: Grid3x3, key: "library.view.grid" },
    { mode: "list", icon: List, key: "library.view.list" },
    { mode: "table", icon: Rows3, key: "library.view.table" },
  ];

  let t = $derived(translatorFor($locale));
  let view = $state<ViewMode>("grid");
  let sort = $state("library.sort.recent");

  const SORTS = [
    "library.sort.recent",
    "library.sort.title",
    "library.sort.author",
    "library.sort.year",
    "library.sort.rating",
    "library.sort.progress",
  ] as const;

  let count = $derived(formatNumber(0, { digits: $preferences.digits }));
</script>

<div class="flex min-h-full flex-col">
  <div class="flex flex-wrap items-center gap-[var(--sp-3)] border-b border-border-1 bg-surface-1 px-[var(--sp-5)] py-[var(--sp-3)]">
    <h1 class="text-[var(--fs-lg)] font-semibold">{t("library.title")}</h1>
    <span class="tnum bidi-ltr rounded-[var(--r-full)] bg-surface-2 px-2 py-0.5 text-[var(--fs-sm)] text-ink-2">
      {count} {t("library.count")}
    </span>

    <div class="ms-auto flex items-center gap-[var(--sp-3)]">
      <!-- Sort -->
      <label class="flex items-center gap-[var(--sp-2)]">
        <span class="text-[var(--fs-sm)] text-ink-3">{t("library.sort")}</span>
        <select
          bind:value={sort}
          class="h-8 rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-2)] text-[var(--fs-sm)] text-ink-1"
        >
          {#each SORTS as key (key)}
            <option value={key}>{t(key)}</option>
          {/each}
        </select>
      </label>

      <!-- View mode -->
      <div class="flex rounded-[var(--r-md)] border border-border-1 bg-surface-1 p-0.5" role="radiogroup" aria-label={t("library.view.grid")}>
        {#each VIEWS as item (item.mode)}
          {@const Icon = item.icon}
          <button
            type="button"
            role="radio"
            aria-checked={view === item.mode}
            aria-label={t(item.key)}
            title={t(item.key)}
            onclick={() => (view = item.mode)}
            class="grid size-7 place-items-center rounded-[var(--r-sm)] transition-colors {view === item.mode
              ? 'bg-accent text-accent-fg'
              : 'text-ink-2 hover:bg-surface-2'}"
          >
            <Icon class="size-4 no-mirror" strokeWidth={1.75} />
          </button>
        {/each}
      </div>

      <Button variant="primary" size="sm">{t("common.add")}</Button>
    </div>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto px-[var(--sp-5)] py-[var(--sp-5)]">
    <Card>
      <EmptyState
        title={t("library.empty.title")}
        body={t("library.empty.body")}
        actionLabel={t("common.add")}
        onaction={() => undefined}
      />
    </Card>
  </div>
</div>
