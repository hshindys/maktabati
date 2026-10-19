<script lang="ts">
  import { BookOpen, Search, Star } from "@lucide/svelte";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import Combobox from "../components/Combobox.svelte";
  import CoverPlaceholder from "../components/CoverPlaceholder.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import Field from "../components/Field.svelte";
  import KpiCard from "../components/KpiCard.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import Stars from "../components/Stars.svelte";
  import TagInput from "../components/TagInput.svelte";
  import { locale, translatorFor } from "../i18n";
  import { pushToast } from "../toast";
  import { ACCENTS, preferences, setPreferences, THEME_CHOICES } from "../settings";
  import type { EntityRef } from "../draft";

  /**
   * Component gallery — debug builds only (route `#/dev/gallery`).
   *
   * Design rule: *every* component ships every state — default, hover, focus-visible, active,
   * disabled, loading, error — in light, dark and sepia, in RTL and LTR. A grid is the only
   * honest way to verify that, so this screen is where a missing state shows up before a user
   * finds it.
   */

  let t = $derived(translatorFor($locale));
  let rating = $state(7);
  let progress = $state(62);

  const SECTIONS = [
    "buttons",
    "inputs",
    "forms",
    "feedback",
    "data",
    "navigation",
    "overlays",
  ] as const;

  // Form primitives need live state — a static screenshot cannot show a chip being added.
  let galleryAuthor = $state<EntityRef | null>(null);
  let galleryTags = $state<string[]>(["تاريخ", "حضارة"]);
</script>

<div class="mx-auto flex max-w-6xl flex-col gap-[var(--sp-7)] px-[var(--sp-6)] py-[var(--sp-6)]">
  <header class="flex flex-wrap items-center justify-between gap-[var(--sp-4)]">
    <div>
      <h1 class="text-[var(--fs-2xl)] font-semibold">{t("nav.gallery")}</h1>
      <p class="mt-1 text-[var(--fs-sm)] text-ink-3">
        Every component, every state — light / dark / sepia, RTL / LTR.
      </p>
    </div>

    <!-- Live switchers so a reviewer can flip context without leaving the page. -->
    <div class="flex flex-wrap items-center gap-[var(--sp-2)]">
      <div class="flex rounded-[var(--r-md)] border border-border-1 bg-surface-1 p-0.5">
        {#each THEME_CHOICES as theme (theme)}
          <button
            type="button"
            onclick={() => setPreferences({ theme })}
            aria-pressed={$preferences.theme === theme}
            class="rounded-[var(--r-sm)] px-[var(--sp-3)] py-1 text-[var(--fs-sm)] transition-colors {$preferences.theme === theme
              ? 'bg-accent text-accent-fg'
              : 'text-ink-2 hover:bg-surface-2'}"
          >
            {theme}
          </button>
        {/each}
      </div>
      <div class="flex rounded-[var(--r-md)] border border-border-1 bg-surface-1 p-0.5">
        {#each ["ar", "en"] as code (code)}
          <button
            type="button"
            onclick={() => setPreferences({ locale: code as "ar" | "en" })}
            aria-pressed={$preferences.locale === code}
            class="rounded-[var(--r-sm)] px-[var(--sp-3)] py-1 text-[var(--fs-sm)] uppercase transition-colors {$preferences.locale === code
              ? 'bg-accent text-accent-fg'
              : 'text-ink-2 hover:bg-surface-2'}"
          >
            {code}
          </button>
        {/each}
      </div>
    </div>
  </header>

  <!-- Accent strip: the six presets, live. -->
  <section>
    <h2 class="mb-[var(--sp-3)] text-[var(--fs-md)] font-semibold">{t("settings.accent")}</h2>
    <div class="flex flex-wrap gap-[var(--sp-2)]">
      {#each ACCENTS as accent (accent)}
        <button
          type="button"
          onclick={() => setPreferences({ accent })}
          aria-pressed={$preferences.accent === accent}
          class="h-10 w-24 rounded-[var(--r-md)] border-2 text-[var(--fs-sm)] font-medium transition-transform {$preferences.accent === accent
            ? 'border-ink-1 scale-105'
            : 'border-transparent'}"
          style="background: var(--color-accent)"
        >
          {accent}
        </button>
      {/each}
    </div>
  </section>

  {#each SECTIONS as section (section)}
    <section>
      <h2 class="mb-[var(--sp-4)] border-b border-border-1 pb-[var(--sp-2)] font-mono text-[var(--fs-sm)] uppercase tracking-wide text-ink-3">
        {section}
      </h2>

      {#if section === "buttons"}
        <Card>
          <div class="flex flex-wrap items-center gap-[var(--sp-3)]">
            <Button variant="primary">{t("common.save")}</Button>
            <Button variant="secondary">{t("common.cancel")}</Button>
            <Button variant="ghost">{t("common.edit")}</Button>
            <Button variant="danger">{t("common.delete")}</Button>
            <Button variant="primary" size="sm">sm</Button>
            <Button variant="primary" size="md">md</Button>
            <Button variant="primary" size="lg">lg</Button>
            <Button variant="primary" disabled>{t("common.save")}</Button>
            <Button variant="primary" loading>{t("common.loading")}</Button>
            <Button variant="secondary">
              {#snippet iconStart()}
                <Search class="size-4 no-mirror" strokeWidth={1.75} />
              {/snippet}
              {t("common.search")}
            </Button>
          </div>
        </Card>

      {:else if section === "inputs"}
        <Card>
          <div class="grid gap-[var(--sp-4)] sm:grid-cols-2">
            <label class="flex flex-col gap-1">
              <span class="text-[var(--fs-sm)] text-ink-2">{t("library.sort")}</span>
              <input
                class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] focus:border-accent"
                placeholder={t("common.search")}
              />
            </label>
            <label class="flex flex-col gap-1">
              <span class="text-[var(--fs-sm)] text-ink-2">ISBN</span>
              <input
                class="bidi-ltr h-9 rounded-[var(--r-md)] border border-danger/60 bg-surface-1 px-[var(--sp-3)] font-mono text-[var(--fs-base)]"
                value="978-0-306-40615-8"
                aria-invalid="true"
              />
              <span class="text-[var(--fs-xs)] text-danger">{t("error.validation.isbn_invalid")}</span>
            </label>
          </div>
        </Card>

      {:else if section === "forms"}
        <!-- The wizard's form primitives, live: combobox pick-or-create, tag chips, a
             labelled field with its error state (the states the wizard relies on). -->
        <Card>
          <div class="grid gap-[var(--sp-4)] sm:grid-cols-2">
            <Field
              label={t("wizard.field.title")}
              required
              for="gallery-field"
              hint={t("wizard.optional")}
            >
              <input
                id="gallery-field"
                class="h-9 w-full rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-[var(--sp-3)] text-[var(--fs-base)] outline-none focus:border-accent"
              />
            </Field>
            <Combobox
              label={t("wizard.field.authors")}
              value={galleryAuthor}
              onchange={(next) => (galleryAuthor = next)}
              options={[
                { id: "a1", name: "أحمد فكري" },
                { id: "a2", name: "طه حسين" },
                { id: "a3", name: "نجيب محفوظ" },
              ]}
              placeholder={t("common.search")}
              createLabel={t("wizard.add", { name: t("wizard.field.authors") })}
              oncreate={async (name) => ({ id: `new-${name}`, name })}
              removeLabel={t("common.delete")}
            />
          </div>
          <div class="mt-[var(--sp-4)]">
            <TagInput
              label={t("wizard.field.tags")}
              value={galleryTags}
              onchange={(next) => (galleryTags = next)}
              placeholder={t("wizard.tags.placeholder")}
              hint={t("wizard.tags.hint")}
              removeLabel={t("common.delete")}
            />
          </div>
        </Card>

      {:else if section === "feedback"}
        <div class="grid gap-[var(--sp-4)] lg:grid-cols-2">
          <Card>
            <h3 class="mb-[var(--sp-4)] text-[var(--fs-base)] font-medium">Toasts</h3>
            <div class="flex flex-wrap gap-[var(--sp-2)]">
              <Button size="sm" onclick={() => pushToast(t("toast.saved"), { tone: "success" })}>
                success
              </Button>
              <Button size="sm" onclick={() => pushToast(t("error.title"), { tone: "error" })}>
                error
              </Button>
              <Button
                size="sm"
                onclick={() => pushToast(t("health.pending"), { tone: "warning" })}
              >
                warning
              </Button>
              <Button size="sm" onclick={() => pushToast(t("common.loading"), { tone: "info" })}>
                info
              </Button>
            </div>
          </Card>

          <Card>
            <h3 class="mb-[var(--sp-4)] text-[var(--fs-base)] font-medium">Empty states</h3>
            <EmptyState
              compact
              title={t("library.empty.title")}
              body={t("library.empty.body")}
              actionLabel={t("common.retry")}
              onaction={() => undefined}
            />
          </Card>
        </div>

      {:else if section === "data"}
        <div class="grid gap-[var(--sp-4)] lg:grid-cols-3">
          <KpiCard label={t("dashboard.kpi.total")} value="1,284" caption={t("dashboard.kpi.books")}>
            {#snippet icon()}
              <BookOpen class="size-4.5" strokeWidth={1.75} />
            {/snippet}
          </KpiCard>

          <Card>
            <h3 class="mb-[var(--sp-4)] text-[var(--fs-base)] font-medium">{t("library.sort.rating")}</h3>
            <div class="flex flex-col gap-[var(--sp-3)]">
              <Stars value={rating} label="Rating" onchange={(v) => (rating = v)} size="lg" />
              <Stars value={8} readonly label="Read-only rating" />
              <Stars value={3} readonly size="sm" />
              <span class="tnum bidi-ltr text-[var(--fs-sm)] text-ink-3">{rating} / 10</span>
            </div>
          </Card>

          <Card>
            <h3 class="mb-[var(--sp-4)] text-[var(--fs-base)] font-medium">Progress</h3>
            <div class="flex flex-col gap-[var(--sp-4)]">
              <ProgressBar value={progress} label="Reading" showValue />
              <ProgressBar value={100} tone="success" label="Done" size="sm" showValue />
              <ProgressBar value={0} label="Not started" size="lg" showValue />
              <input
                type="range"
                min="0"
                max="100"
                bind:value={progress}
                class="accent-[var(--accent)]"
                aria-label="Preview progress"
              />
            </div>
          </Card>
        </div>

      {:else if section === "navigation"}
        <Card>
          <div class="flex flex-wrap items-end gap-[var(--sp-3)]">
            <CoverPlaceholder title="مقدمة في التاريخ الإسلامي" author="أحمد فكري" height={200} />
            <CoverPlaceholder title="A Brief History of Time" author="Stephen Hawking" language="en" height={200} />
            <CoverPlaceholder title="مقدمة في الحضارة الإسلامية" author="أحمد فكري" height={200} />
          </div>
          <p class="mt-[var(--sp-3)] text-[var(--fs-xs)] text-ink-3">
            Generated placeholders. The colour is a hash of the title, so the same book always
            looks the same across the grid.
          </p>
        </Card>

      {:else}
        <Card>
          <div class="flex flex-wrap gap-[var(--sp-3)]">
            <Button variant="secondary" onclick={() => pushToast(t("toast.saved"))}>
              <Star class="size-4 no-mirror" strokeWidth={1.75} />
              {t("common.save")}
            </Button>
            <Button variant="secondary">{t("common.copy")}</Button>
          </div>
          <p class="mt-[var(--sp-3)] text-[var(--fs-xs)] text-ink-3">
            Overlays (dialogs, popovers, menus) arrive with the library milestone.
          </p>
        </Card>
      {/if}
    </section>
  {/each}
</div>
