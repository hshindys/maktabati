<script lang="ts">
  /**
   * CoverPlaceholder — the generated cover used when a book has no image.
   *
   * This is the "generated Arabic placeholder" from the spec: a deterministic, warm-toned
   * book-spine look with the real title set in the book's own language, so a shelf of
   * untranslated Arabic titles still reads as Arabic at a glance.
   *
   * The colour is a pure function of the title, which means the same book always looks the same
   * — important when scanning a 50k grid, where a random colour per render would be noise.
   */

  interface Props {
    title: string;
    author?: string;
    /** Language code, used only to pick the default text direction of the title. */
    language?: string;
    /** Height of the rendered cover in px; width follows the 2:3 trade paperback ratio. */
    height?: number;
    class?: string;
  }

  let { title, author, language = "ar", height = 240, class: klass = "" }: Props = $props();

  /** A small, stable hash — enough to spread titles across the palette. */
  function hash(text: string): number {
    let h = 2166136261;
    for (let i = 0; i < text.length; i += 1) {
      h ^= text.charCodeAt(i);
      h = Math.imul(h, 16777619);
    }
    return Math.abs(h);
  }

  /** Six bookbinding-ish tones; all AA against the cream page colour they carry. */
  const PALETTES = [
    { cloth: "#2F6B4F", page: "#F2EDE1", ink: "#1D4231" },
    { cloth: "#7A3B69", page: "#F6EDF3", ink: "#4E2642" },
    { cloth: "#8A6A1F", page: "#F7F1DF", ink: "#564012" },
    { cloth: "#1F6A8A", page: "#E9F2F6", ink: "#13485E" },
    { cloth: "#B4552F", page: "#F8ECE4", ink: "#6E3419" },
    { cloth: "#3B4CCA", page: "#EBEDF9", ink: "#26307F" },
  ];

  let palette = $derived(PALETTES[hash(title || "?") % PALETTES.length]);
  let width = $derived(Math.round(height * (2 / 3)));
  /** Arabic and other RTL scripts read right-to-left; Latin reads left-to-right. */
  let dir: "ltr" | "rtl" = $derived(/^[a-z]/i.test((language || "ar").slice(0, 1)) && language !== "ar" ? "ltr" : "rtl");
  let initials = $derived(
    (title || "")
      .trim()
      .split(/\s+/)
      .slice(0, 2)
      .map((w) => Array.from(w)[0] ?? "")
      .join(""),
  );
</script>

<div
  class="relative overflow-hidden rounded-[var(--r-sm)] shadow-1 {klass}"
  style="inline-size: {width}px; block-size: {height}px; background: {palette.page};"
  aria-hidden={title ? undefined : "true"}
>
  <!-- The cloth spine, on the inline start so it mirrors with the reading direction. -->
  <div
    class="absolute inset-y-0 start-0 w-[9%]"
    style="background: {palette.cloth}"
  ></div>
  <div
    class="absolute inset-y-0 start-[9%] w-[1px] opacity-40"
    style="background: {palette.cloth}"
  ></div>

  <!-- Rule-of-thirds hairline, book-cover convention. -->
  <div
    class="absolute inset-x-[18%] top-[26%] h-px opacity-30"
    style="background: {palette.cloth}"
  ></div>

  <div
    class="absolute inset-x-[18%] top-[34%] flex flex-col gap-[6px] text-start"
    dir={dir}
    lang={language}
    style="color: {palette.ink}"
  >
    <span
      class="bidi-auto line-clamp-4 break-words font-semibold leading-[1.45]"
      style="font-size: {Math.max(11, Math.round(height * 0.072))}px"
    >
      {title || initials || "—"}
    </span>
    {#if author}
      <span
        class="bidi-auto line-clamp-2 opacity-75"
        style="font-size: {Math.max(9, Math.round(height * 0.05))}px"
      >
        {author}
      </span>
    {/if}
  </div>
</div>
