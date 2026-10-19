<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * KpiCard — one number with context, used across the dashboard and goals.
   *
   * `value` is already formatted by the caller (so it arrives with the right numeral system and
   * grouping); this component only places it.
   */

  interface Props {
    label: string;
    /** Pre-formatted, e.g. "١٬٢٨٤". Rendered as-is inside a bidi-isolated span. */
    value: string;
    caption?: string;
    icon?: Snippet;
    /** Optional progress bar slot, e.g. the goal counter. */
    footer?: Snippet;
    tone?: "default" | "accent" | "success" | "warning";
  }

  let { label, value, caption, icon, footer, tone = "default" }: Props = $props();

  const TONES = {
    default: "text-ink-1",
    accent: "text-accent",
    success: "text-success",
    warning: "text-warning",
  } as const;
</script>

<div
  class="flex min-w-0 flex-col gap-[var(--sp-2)] rounded-[var(--r-lg)] border border-border-1 bg-surface-1 p-[var(--sp-5)]"
>
  <div class="flex items-center justify-between gap-[var(--sp-3)]">
    <span class="truncate text-[var(--fs-sm)] text-ink-2">{label}</span>
    {#if icon}
      <span class="shrink-0 text-ink-3" aria-hidden="true">
        {@render icon()}
      </span>
    {/if}
  </div>

  <!-- Numbers are direction-agnostic: isolate so "1,284" never picks up RTL reordering. -->
  <span class="tnum bidi-ltr text-[var(--fs-3xl)] font-semibold leading-none {TONES[tone]}">
    {value}
  </span>

  {#if caption}
    <span class="truncate text-[var(--fs-xs)] text-ink-3">{caption}</span>
  {/if}

  {#if footer}
    <div class="mt-[var(--sp-1)]">{@render footer()}</div>
  {/if}
</div>
