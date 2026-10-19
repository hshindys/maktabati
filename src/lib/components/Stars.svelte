<script lang="ts">
  import { Star } from "@lucide/svelte";

  /**
   * Stars — the half-star rating control.
   *
   * The domain stores ratings as `INTEGER 0..=10` half-star units (`maktabati-domain`), so this
   * component never deals in floats: `value` is already the stored integer. Keyboard: ←/→ move
   * by a half star, Home/End jump to the ends, per docs/04-ux-design.md §5.4.
   */

  interface Props {
    /** 0–10 in half-star units. */
    value: number;
    readonly?: boolean;
    /** Accessible label; without it the control is invisible to a screen reader. */
    label?: string;
    /** Called with the new 0–10 value. */
    onchange?: (next: number) => void;
    size?: "sm" | "md" | "lg";
  }

  let { value, readonly = false, label = "", onchange, size = "md" }: Props = $props();

  const MAX = 10;
  const STAR_COUNT = 5;

  let clamped = $derived(Math.min(MAX, Math.max(0, Math.round(value))));
  let hoverValue = $state<number | null>(null);
  let shown = $derived(hoverValue ?? clamped);

  /** Percent of each star that is filled, from the 0–10 value. */
  const fills = $derived(
    Array.from({ length: STAR_COUNT }, (_, i) => {
      const units = shown - i * 2;
      return Math.min(100, Math.max(0, units * 50));
    }),
  );

  const SIZES = { sm: "size-3.5", md: "size-4.5", lg: "size-6" } as const;
  const GAPS = { sm: "gap-0", md: "gap-0.5", lg: "gap-1" } as const;

  function unitsFromPointer(event: PointerEvent, starIndex: number): number {
    const el = event.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    // `x` is physical, so compute the fraction using the element's own direction: in RTL the
    // star's visual "start" is its right edge.
    const isRtl = getComputedStyle(el).direction === "rtl";
    const fraction = isRtl
      ? (rect.right - event.clientX) / rect.width
      : (event.clientX - rect.left) / rect.width;
    const half = fraction > 0.5 ? 1 : 0.5;
    return starIndex * 2 + half * 2;
  }

  function onKeydown(event: KeyboardEvent): void {
    if (readonly) return;
    const step = event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
    if (step !== 0) {
      event.preventDefault();
      onchange?.(Math.min(MAX, Math.max(0, clamped + step)));
      return;
    }
    if (event.key === "Home") {
      event.preventDefault();
      onchange?.(0);
    } else if (event.key === "End") {
      event.preventDefault();
      onchange?.(MAX);
    }
  }
</script>

<!-- `role` stays `slider` even when readonly. The conditional `img` alternative dropped
     `aria-valuetext`, so a readonly rating announced its label but never its value — and a
     conditional role leaves svelte-check unable to prove the tabindex below is safe
     (`a11y_no_noninteractive_tabindex`). `aria-readonly` carries the readonly state instead. -->
<div
  class="inline-flex items-center {GAPS[size]}"
  role="slider"
  aria-label={label || "Rating"}
  aria-valuenow={clamped}
  aria-valuemin={0}
  aria-valuemax={MAX}
  aria-valuetext="{clamped / 2} out of 5"
  aria-readonly={readonly}
  tabindex={readonly ? -1 : 0}
  onkeydown={onKeydown}
  onpointerleave={() => (hoverValue = null)}
>
  {#each fills as fill, i (i)}
    {@const Icon = Star}
    <button
      type="button"
      class="relative {SIZES[size]} p-0 border-0 bg-transparent leading-none"
      disabled={readonly}
      aria-hidden="true"
      tabindex="-1"
      onpointerenter={() => !readonly && (hoverValue = i * 2 + 2)}
      onclick={() => !readonly && onchange?.(i * 2 + 2)}
      onpointermove={(e) => !readonly && (hoverValue = unitsFromPointer(e, i))}
    >
      <Icon class="absolute inset-0 {SIZES[size]} text-star-empty" strokeWidth={1.75} />
      <span
        class="absolute inset-0 overflow-hidden text-star"
        style="inline-size: {fill}%"
        aria-hidden="true"
      >
        <Icon class="size-full text-star" strokeWidth={1.75} />
      </span>
    </button>
  {/each}
</div>
