<script lang="ts">
  /**
   * ProgressBar — reading progress, goals, import progress.
   *
   * Follows logical direction (design rule 7.7): the fill grows from the inline start, so it
   * runs left-to-right in English and right-to-left in Arabic with no extra CSS. The ARIA
   * values are the raw 0–100 numbers regardless of the numeral rendering style.
   */

  interface Props {
    /** 0–100. Values outside the range are clamped rather than rejected. */
    value: number;
    /** Accessible name, e.g. "Reading progress". */
    label?: string;
    size?: "sm" | "md" | "lg";
    tone?: "accent" | "success" | "warning";
    /** Render the percentage next to the bar. */
    showValue?: boolean;
    /** Formatted value to display; falls back to `value` with a `%`. */
    displayValue?: string;
  }

  let {
    value,
    label = "",
    size = "md",
    tone = "accent",
    showValue = false,
    displayValue,
  }: Props = $props();

  let clamped = $derived(Math.min(100, Math.max(0, Number.isFinite(value) ? value : 0)));
  let pct = $derived(Math.round(clamped));

  const HEIGHTS = { sm: "h-1", md: "h-2", lg: "h-3" } as const;
  const TONES = {
    accent: "bg-accent",
    success: "bg-success",
    warning: "bg-warning",
  } as const;
</script>

<div class="flex items-center gap-[var(--sp-3)]">
  <div
    class="relative flex-1 overflow-hidden rounded-[var(--r-full)] bg-surface-3 {HEIGHTS[size]}"
    role="progressbar"
    aria-label={label}
    aria-valuenow={pct}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuetext="{pct}%"
  >
    <div
      class="h-full rounded-[var(--r-full)] {TONES[tone]} transition-[inline-size] duration-[var(--dur-base)] ease-[var(--ease)]"
      style="inline-size: {pct}%"
    ></div>
  </div>

  {#if showValue}
    <span class="tnum text-[var(--fs-sm)] text-ink-2 min-w-[3ch] text-end">
      {displayValue ?? `${pct}%`}
    </span>
  {/if}
</div>
