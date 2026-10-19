<script lang="ts">
  import type { Snippet } from "svelte";
  import Button from "./Button.svelte";

  /**
   * EmptyState — illustrated, with one sentence and one primary action
   * (docs/04-ux-design.md §5.5).
   *
   * Every empty surface in the app uses this; a bare "no results" string is a dead end.
   */

  interface Props {
    title: string;
    body?: string;
    actionLabel?: string;
    onaction?: () => void;
    /** Inline SVG illustration; a generic open book is used when omitted. */
    illustration?: Snippet;
    compact?: boolean;
  }

  let { title, body, actionLabel, onaction, illustration, compact = false }: Props = $props();
</script>

<div
  class="flex flex-col items-center justify-center text-center {compact
    ? 'gap-[var(--sp-3)] py-[var(--sp-6)]'
    : 'gap-[var(--sp-4)] py-[var(--sp-8)]'}"
>
  <div class="text-ink-3" aria-hidden="true">
    {#if illustration}
      {@render illustration()}
    {:else}
      <!-- Decorative open book. Inline so it inherits `currentColor` and needs no asset. -->
      <svg
        viewBox="0 0 64 48"
        class={compact ? "size-16" : "size-24"}
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M32 12c-4-3-10-4-16-4H8v32h8c6 0 12 1 16 4" />
        <path d="M32 12c4-3 10-4 16-4h8v32h-8c-6 0-12 1-16 4" />
        <path d="M32 12v32" />
        <path d="M14 16h8M14 24h8M42 16h8M42 24h8" opacity=".5" />
      </svg>
    {/if}
  </div>

  <h3 class="text-[var(--fs-md)] font-semibold text-ink-1">{title}</h3>
  {#if body}
    <p class="max-w-[42ch] text-[var(--fs-base)] text-ink-2">{body}</p>
  {/if}

  {#if actionLabel && onaction}
    <Button variant="primary" size="md" onclick={onaction}>{actionLabel}</Button>
  {/if}
</div>
