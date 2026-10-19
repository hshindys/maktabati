<script lang="ts">
  import { locale, translatorFor } from "../i18n";
  import EmptyState from "../components/EmptyState.svelte";

  /**
   * Placeholder — the "screen is coming in a later milestone" state.
   *
   * Shipping a real, honest empty state beats a blank region: it tells the user which part of
   * the roadmap they are looking at, and it keeps the navigation fully clickable so the shell's
   * layout can be reviewed end to end.
   */

  interface Props {
    path: string;
    pattern: string;
  }

  let { path, pattern }: Props = $props();
  let t = $derived(translatorFor($locale));
</script>

<div class="mx-auto flex max-w-3xl flex-col items-center px-[var(--sp-6)] py-[var(--sp-8)]">
  <EmptyState
    title={pattern}
    body={t("app.tagline")}
  />
  <p class="bidi-ltr mt-[var(--sp-6)] rounded-[var(--r-md)] border border-border-1 bg-surface-2 px-[var(--sp-3)] py-[var(--sp-2)] font-mono text-[var(--fs-xs)] text-ink-3">
    {path}
  </p>
</div>
