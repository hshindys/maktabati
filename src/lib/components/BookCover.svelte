<script lang="ts">
  /**
   * BookCover — a book's real cover when it has one, the generated placeholder otherwise.
   *
   * This wraps CoverPlaceholder: the band of generated spines stays the fallback for every
   * holed book, and the real image the user picked slots in front of it (`cover_set`,
   * M9). The `src` is built via the asset protocol in `cover.ts`, so no IPC call per
   * render — a grid of 50k books asks about the data dir once per session.
   *
   * A stored cover whose file went missing from disk (a backup restore, a hand-delete)
   * fails the image load rather than lying: `onerror` drops back to the placeholder the
   * same way a null path does.
   */
  import CoverPlaceholder from "./CoverPlaceholder.svelte";
  import { coverSrc } from "../cover";

  interface Props {
    title: string;
    author?: string;
    /** Language code, used only to pick the default text direction of the title. */
    language?: string;
    /** Row-relative cover path (`covers/<book_id>.<ext>`) or `null` when the book has none. */
    coverPath: string | null;
    /** Height of the rendered cover in px; width follows the 2:3 trade paperback ratio. */
    height?: number;
    class?: string;
  }

  let {
    title,
    author,
    language = "ar",
    coverPath,
    height = 240,
    class: klass = "",
  }: Props = $props();

  let src = $state<string | null>(null);

  $effect(() => {
    const path = coverPath;
    if (path === null) {
      src = null;
      return;
    }
    let cancelled = false;
    coverSrc(path).then((s) => {
      if (!cancelled) src = s;
    });
    return () => {
      cancelled = true;
    };
  });
</script>

{#if src !== null}
  <!-- `block-size` matches the generated cover's box, so the layout never reflows when a
       cover arrives or is removed. -->
  <img
    {src}
    alt={title}
    class="rounded-[var(--r-sm)] object-cover shadow-1 {klass}"
    style="inline-size: {height * (2 / 3)}px; block-size: {height}px;"
    loading="lazy"
    onerror={() => (src = null)}
  />
{:else}
  <CoverPlaceholder {title} {author} {language} {height} class={klass} />
{/if}
