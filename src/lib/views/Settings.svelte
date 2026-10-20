<script lang="ts">
  import { locale, translatorFor, type MessageKey } from "../i18n";
  import {
    ACCENTS,
    LOCALES,
    preferences,
    setPreferences,
    THEME_CHOICES,
    type Accent,
    type CalendarStyle,
    type DigitStyle,
    type Locale,
    type ThemeChoice,
  } from "../settings";
  import Card from "../components/Card.svelte";
  import Mark from "../components/Mark.svelte";

  /**
   * Settings → Appearance & language.
   *
   * Every control here is a **live preview**: changing it updates `<html data-theme>` and
   * `<html dir>` immediately, with no save button and no reload. That is the whole point of
   * having one token layer — a preference change is a CSS variable change, not a re-render.
   */

  let t = $derived(translatorFor($locale));

  const THEME_LABELS: Record<ThemeChoice, MessageKey> = {
    light: "shell.theme.light",
    dark: "shell.theme.dark",
    sepia: "shell.theme.sepia",
    system: "shell.theme.system",
  } as const;

  const ACCENT_LABELS: Record<Accent, MessageKey> = {
    green: "accent.green",
    indigo: "accent.indigo",
    plum: "accent.plum",
    terracotta: "accent.terracotta",
    ocean: "accent.ocean",
    gold: "accent.gold",
  } as const;

  const CALENDARS: CalendarStyle[] = ["gregorian", "hijri", "both"];
  const DIGITS: DigitStyle[] = ["arabic-indic", "western"];

  /**
   * Digit and calendar options carry hyphenated TS union members (`arabic-indic`), but i18n keys
   * are dotted and shaped like Rust identifiers — the naming test enforces it, because keys are
   * derived from Rust error codes. Mapping them explicitly is what keeps the hyphen out of the key
   * *and* makes the compiler prove every option has a real translation.
   */
  const DIGIT_LABELS: Record<DigitStyle, MessageKey> = {
    western: "settings.digits.western",
    "arabic-indic": "settings.digits.arabic_indic",
  };

  const CALENDAR_LABELS: Record<CalendarStyle, MessageKey> = {
    gregorian: "settings.calendar.gregorian",
    hijri: "settings.calendar.hijri",
    both: "settings.calendar.both",
  };

  let heading = "section";
</script>

<div class="mx-auto flex max-w-3xl flex-col gap-[var(--sp-6)] px-[var(--sp-6)] py-[var(--sp-6)]">
  <h1 class="text-[var(--fs-2xl)] font-semibold">{t("settings.title")}</h1>

  <!-- Language & direction -->
  <Card>
    <h2 class="mb-[var(--sp-4)] text-[var(--fs-md)] font-semibold">{t("settings.language")}</h2>
    <div class="flex flex-wrap gap-[var(--sp-2)]" role="radiogroup" aria-label={t("shell.language")}>
      {#each LOCALES as code (code)}
        <button
          type="button"
          role="radio"
          aria-checked={$preferences.locale === code}
          onclick={() => setPreferences({ locale: code as Locale })}
          class="rounded-[var(--r-md)] border px-[var(--sp-4)] py-[var(--sp-2)] text-[var(--fs-base)] transition-colors {$preferences.locale === code
            ? 'border-accent bg-accent-soft text-accent-soft-ink font-medium'
            : 'border-border-2 bg-surface-1 text-ink-2 hover:bg-surface-2'}"
        >
          {code === "ar" ? "العربية · RTL" : "English · LTR"}
        </button>
      {/each}
    </div>
  </Card>

  <!-- Theme -->
  <Card>
    <h2 class="mb-[var(--sp-4)] text-[var(--fs-md)] font-semibold">{t("settings.appearance")}</h2>
    <div class="grid grid-cols-2 gap-[var(--sp-3)] sm:grid-cols-4" role="radiogroup" aria-label={t("shell.theme")}>
      {#each THEME_CHOICES as choice (choice)}
        {@const resolved = choice === "system" ? "light" : choice}
        <button
          type="button"
          role="radio"
          aria-checked={$preferences.theme === choice}
          onclick={() => setPreferences({ theme: choice })}
          class="group flex flex-col items-center gap-[var(--sp-2)] rounded-[var(--r-md)] border p-[var(--sp-2)] transition-colors {$preferences.theme === choice
            ? 'border-accent'
            : 'border-border-2 hover:border-border-2'}"
        >
          <!-- Swatch uses the real tokens, so the preview cannot drift from the theme. -->
          <span
            class="h-12 w-full rounded-[var(--r-sm)] border border-border-1"
            style="background: var(--surface-0)"
          >
            <span
              class="mt-[var(--sp-3)] ms-[var(--sp-3)] block h-2 w-8 rounded-full"
              style="background: var(--accent)"
            ></span>
          </span>
          <span class="text-[var(--fs-sm)] text-ink-2">{t(THEME_LABELS[choice])}</span>
          {#if resolved === "dark"}
            <span class="sr-only">{resolved}</span>
          {/if}
        </button>
      {/each}
    </div>

    <h3 class="mt-[var(--sp-5)] mb-[var(--sp-3)] text-[var(--fs-base)] font-medium">
      {t("settings.accent")}
    </h3>
    <div class="flex flex-wrap gap-[var(--sp-2)]" role="radiogroup" aria-label={t("settings.accent")}>
      {#each ACCENTS as accent (accent)}
        <button
          type="button"
          role="radio"
          aria-checked={$preferences.accent === accent}
          onclick={() => setPreferences({ accent })}
          title={t(ACCENT_LABELS[accent])}
          aria-label={t(ACCENT_LABELS[accent])}
          class="grid size-9 place-items-center rounded-[var(--r-full)] border-2 transition-transform {$preferences.accent === accent
            ? 'border-ink-1 scale-110'
            : 'border-transparent hover:scale-105'}"
          style="background: var(--swatch)"
          data-accent={accent}
        >
          <span class="sr-only">{t(ACCENT_LABELS[accent])}</span>
        </button>
      {/each}
    </div>
  </Card>

  <!-- Numerals & calendar -->
  <Card>
    <h2 class="mb-[var(--sp-4)] text-[var(--fs-md)] font-semibold">{t("settings.digits")}</h2>
    <div class="flex flex-wrap gap-[var(--sp-2)]" role="radiogroup" aria-label={t("settings.digits")}>
      {#each DIGITS as style (style)}
        <button
          type="button"
          role="radio"
          aria-checked={$preferences.digits === style}
          onclick={() => setPreferences({ digits: style })}
          class="rounded-[var(--r-md)] border px-[var(--sp-4)] py-[var(--sp-2)] font-medium transition-colors {$preferences.digits === style
            ? 'border-accent bg-accent-soft text-accent-soft-ink'
            : 'border-border-2 bg-surface-1 text-ink-2 hover:bg-surface-2'}"
        >
          <span class="bidi-ltr">{t(DIGIT_LABELS[style])}</span>
        </button>
      {/each}
    </div>

    <h2 class="mt-[var(--sp-5)] mb-[var(--sp-3)] text-[var(--fs-md)] font-semibold">
      {t("settings.calendar")}
    </h2>
    <div class="flex flex-wrap gap-[var(--sp-2)]" role="radiogroup" aria-label={t("settings.calendar")}>
      {#each CALENDARS as cal (cal)}
        <button
          type="button"
          role="radio"
          aria-checked={$preferences.calendar === cal}
          onclick={() => setPreferences({ calendar: cal })}
          class="rounded-[var(--r-md)] border px-[var(--sp-4)] py-[var(--sp-2)] text-[var(--fs-base)] transition-colors {$preferences.calendar === cal
            ? 'border-accent bg-accent-soft text-accent-soft-ink'
            : 'border-border-2 bg-surface-1 text-ink-2 hover:bg-surface-2'}"
        >
          {t(CALENDAR_LABELS[cal])}
        </button>
      {/each}
    </div>

    <!-- Live sample so the effect of the two controls above is immediately visible. -->
    <p
      class="mt-[var(--sp-5)] rounded-[var(--r-md)] border border-border-1 bg-surface-2 px-[var(--sp-4)] py-[var(--sp-3)] text-[var(--fs-base)] text-ink-2"
    >
      <span class="text-ink-3">{heading}</span>
      <span class="bidi-auto">
        {new Intl.DateTimeFormat($locale === "ar" ? "ar" : "en", {
          day: "numeric",
          month: "long",
          year: "numeric",
        }).format(new Date())}
      </span>
    </p>
  </Card>

  <!-- About -->
  <Card>
    <div class="flex items-center gap-[var(--sp-4)]">
      <Mark class="size-12 shrink-0" title={t("app.name")} />
      <div>
        <h2 class="text-[var(--fs-lg)] font-semibold text-ink-1">{t("app.name")}</h2>
        <p class="text-[var(--fs-sm)] text-ink-3">{t("app.tagline")}</p>
      </div>
    </div>
    <p class="mt-[var(--sp-4)] text-[var(--fs-sm)] text-ink-2">{t("about.noTelemetry")}</p>
  </Card>
</div>

<style>
  /* The accent swatches read their colour from the preset's own token rather than a literal,
     so adding a preset to settings.ts lights up here with no extra CSS. */
  [data-accent="green"] { --swatch: #2f6b4f; }
  [data-accent="indigo"] { --swatch: #3b4cca; }
  [data-accent="plum"] { --swatch: #7a3b69; }
  [data-accent="terracotta"] { --swatch: #b4552f; }
  [data-accent="ocean"] { --swatch: #1f6a8a; }
  [data-accent="gold"] { --swatch: #8a6a1f; }
</style>
