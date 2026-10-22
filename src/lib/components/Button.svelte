<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  /**
   * Button — the one primitive every action goes through.
   *
   * Variants map to intent, not to colour: `primary` is the single thing to do on a screen,
   * `secondary` supports it, `ghost` is for dense toolbars, `danger` is destructive and always
   * requires `confirm` handling by the caller.
   */

  type Variant = "primary" | "secondary" | "ghost" | "danger";
  type Size = "sm" | "md" | "lg";

  interface Props extends HTMLButtonAttributes {
    variant?: Variant;
    size?: Size;
    loading?: boolean;
    iconStart?: Snippet;
    iconEnd?: Snippet;
    children?: Snippet;
  }

  let {
    variant = "secondary",
    size = "md",
    loading = false,
    iconStart,
    iconEnd,
    children,
    type = "button",
    disabled = false,
    ...rest
  }: Props = $props();

  const VARIANTS: Record<Variant, string> = {
    primary:
      "bg-accent text-accent-fg hover:bg-accent-hover active:bg-accent-hover border border-transparent shadow-1",
    secondary:
      "bg-surface-1 text-ink-1 border border-border-2 hover:bg-surface-2 active:bg-surface-3",
    ghost:
      "bg-transparent text-ink-2 border border-transparent hover:bg-surface-2 hover:text-ink-1 active:bg-surface-3",
    danger: "bg-danger text-white border border-transparent hover:opacity-90 active:opacity-75",
  };

  const SIZES: Record<Size, string> = {
    sm: "h-7 px-2.5 text-[var(--fs-sm)] gap-1.5",
    md: "h-9 px-3.5 text-[var(--fs-base)] gap-2",
    lg: "h-11 px-5 text-[var(--fs-md)] gap-2.5",
  };

  let classes = $derived(
    [
      "inline-flex items-center justify-center rounded-[var(--r-md)] font-medium",
      "transition-colors duration-[var(--dur-fast)] ease-[var(--ease)]",
      "whitespace-nowrap select-none",
      "disabled:opacity-45 disabled:pointer-events-none",
      VARIANTS[variant],
      SIZES[size],
      rest.class ?? "",
    ]
      .filter(Boolean)
      .join(" "),
  );
</script>

<button
  {type}
  class={classes}
  disabled={disabled || loading}
  aria-busy={loading ? "true" : undefined}
  {...rest}
>
  {#if loading}
    <span
      class="inline-block size-[1em] animate-spin rounded-full border-[1.5px] border-current border-t-transparent"
      aria-hidden="true"
    ></span>
  {:else if iconStart}
    {@render iconStart()}
  {/if}

  {#if children}
    {@render children()}
  {/if}

  {#if iconEnd && !loading}
    {@render iconEnd()}
  {/if}
</button>
