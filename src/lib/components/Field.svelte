<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * Field — label + control + error/hint, the one way a form label is rendered.
   *
   * The error line sits exactly where docs/04 §5.2 puts it: `text-danger` below the
   * control, with a `text-ink-3` hint when there is no error. `for` links a plain control;
   * self-managing controls (Combobox, TagInput) pass the id they mint internally, so the
   * label always points at the thing that receives typing.
   */

  interface Props {
    label: string;
    required?: boolean;
    error?: string | null;
    hint?: string | null;
    for?: string;
    children: Snippet;
  }

  let { label, required = false, error = null, hint = null, for: id, children }: Props = $props();
</script>

<div class="flex min-w-0 flex-col gap-1">
  <label for={id} class="text-[var(--fs-sm)] text-ink-2">
    {#if required}<span class="text-danger" aria-hidden="true">* </span>{/if}{label}
  </label>
  {@render children()}
  {#if error}
    <span class="text-[var(--fs-xs)] text-danger">{error}</span>
  {:else if hint}
    <span class="text-[var(--fs-xs)] text-ink-3">{hint}</span>
  {/if}
</div>
