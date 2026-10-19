<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  /**
   * Card — the standard content container.
   *
   * Two densities: `padded` for interactive panels (KPIs, widgets) and `flush` for list rows
   * that manage their own internal spacing (the book grid).
   */

  interface Props extends HTMLAttributes<HTMLDivElement> {
    /** Remove the internal padding — the caller supplies its own. */
    flush?: boolean;
    /** Adds hover affordance for cards that are actually buttons/links. */
    interactive?: boolean;
    children?: Snippet;
  }

  let { flush = false, interactive = false, children, class: klass = "", ...rest }: Props =
    $props();

  let classes = $derived(
    [
      "bg-surface-1 border border-border-1 rounded-[var(--r-lg)]",
      flush ? "" : "p-[var(--sp-5)]",
      interactive
        ? "transition-[transform,box-shadow] duration-[var(--dur-base)] ease-[var(--ease)] hover:shadow-2 hover:-translate-y-0.5 cursor-pointer"
        : "",
      klass,
    ]
      .filter(Boolean)
      .join(" "),
  );
</script>

<div class={classes} {...rest}>
  {#if children}
    {@render children()}
  {/if}
</div>
