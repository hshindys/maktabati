<script lang="ts">
  /**
   * Mark — the Maktabati vector mark, inlined so it themes with the app.
   *
   * The four paths come from the VectorCraft master (`design/brand/maktabati-mark.svg`, kept
   * in sync by `scripts/make-brand.py`). Colours are CSS custom properties, not literals: the
   * book takes `--text-1` and the ribbon takes `--accent`, so the same markup renders
   * ink-on-paper in the light theme and paper-on-ink in the dark theme — no second asset.
   */
  interface Props {
    class?: string;
    /** Decorative by default (sits beside the visible name); set for standalone use. */
    title?: string;
  }
  let { class: className = "size-6", title }: Props = $props();
</script>

<svg
  viewBox="0 0 512 512"
  class={className}
  role={title ? "img" : undefined}
  aria-hidden={title ? undefined : "true"}
  aria-label={title}
  fill="none"
>
  <!-- left page -->
  <path d="M224 184 C168 180 120 196 72 208 L72 376 C140 380 190 364 224 352 Z" fill="var(--text-1)" />
  <!-- right page -->
  <path d="M288 184 C344 180 392 196 440 208 L440 376 C372 380 322 364 288 352 Z" fill="var(--text-1)" />
  <!-- meem-loop ring (bookmark pull) -->
  <path
    d="M230 104 C230 89.641 241.641 78 256 78 C270.359 78 282 89.641 282 104 C282 118.359 270.359 130 256 130 C241.641 130 230 118.359 230 104 Z"
    stroke="var(--accent)"
    stroke-width="16"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
  <!-- bookmark ribbon -->
  <path d="M232 128 L280 128 L280 404 L256 376 L232 404 Z" fill="var(--accent)" />
</svg>
