<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * Modal — a centred dialog over a dimmed backdrop (docs/04-ux-design.md §4.5).
   *
   * Escape and the backdrop both call `onclose`, and the wizard decides what closing
   * means — its cancel keeps the draft, so dismissal is never destructive. Focus moves
   * into the panel on open and returns to whatever held it before, which is the small
   * amount of focus management a four-step form needs without a dialog dependency.
   */

  interface Props {
    open: boolean;
    onclose: () => void;
    /** `id` of the element that titles the dialog (for `aria-labelledby`). */
    labelledBy: string;
    children: Snippet;
  }

  let { open, onclose, labelledBy, children }: Props = $props();

  let panel: HTMLDivElement | undefined = $state();
  let opener: HTMLElement | null = null;

  $effect(() => {
    if (open) {
      opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      // Land on the first control inside, so a keyboard user starts in the form.
      queueMicrotask(() => {
        const first = panel?.querySelector<HTMLElement>("input, select, textarea, button");
        (first ?? panel)?.focus();
      });
    } else if (opener) {
      opener.focus();
      opener = null;
    }
  });

  function onkeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

{#if open}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-[var(--sp-4)]">
    <button
      type="button"
      class="absolute inset-0 cursor-default bg-black/50"
      tabindex="-1"
      aria-hidden="true"
      onclick={onclose}
    ></button>
    <div
      bind:this={panel}
      role="dialog"
      aria-modal="true"
      aria-labelledby={labelledBy}
      tabindex="-1"
      class="relative flex max-h-[92vh] w-[min(860px,94vw)] flex-col overflow-hidden rounded-[var(--r-xl)] border border-border-1 bg-surface-1 shadow-3 outline-none"
    >
      {@render children()}
    </div>
  </div>
{/if}
