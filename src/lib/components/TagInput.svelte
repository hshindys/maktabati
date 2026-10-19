<script lang="ts">
  import { X } from "@lucide/svelte";

  import Field from "./Field.svelte";

  /**
   * TagInput — chips for the free-form tags of the wizard's classify step.
   *
   * Enter (or a comma, in either script) turns the typed text into a chip, duplicates are
   * dropped case-insensitively, and Backspace in an empty field removes the last chip:
   * the behaviour people already expect from every tag field they have used.
   */

  interface Props {
    label: string;
    value: string[];
    onchange: (next: string[]) => void;
    placeholder?: string;
    hint?: string | null;
    /** Accessible prefix for each chip's remove button ("Remove: تاريخ"). */
    removeLabel: string;
  }

  let { label, value, onchange, placeholder, hint = null, removeLabel }: Props = $props();

  const inputId = "tags-" + Math.random().toString(36).slice(2, 8);

  let text = $state("");

  function add() {
    const name = text.replace(/[,،]$/, "").trim();
    if (!name) return;
    if (!value.some((t) => t.toLocaleLowerCase() === name.toLocaleLowerCase())) {
      onchange([...value, name]);
    }
    text = "";
  }

  function remove(name: string) {
    onchange(value.filter((t) => t !== name));
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === "," || e.key === "،") {
      e.preventDefault();
      add();
    } else if (e.key === "Backspace" && text === "" && value.length > 0) {
      onchange(value.slice(0, -1));
    }
  }
</script>

<Field {label} {hint} for={inputId}>
  <div
    class="flex min-h-9 flex-wrap items-center gap-1.5 rounded-[var(--r-md)] border border-border-2 bg-surface-1 px-2 py-1 focus-within:border-accent"
  >
    {#each value as name (name)}
      <span
        class="inline-flex items-center gap-1 rounded-full bg-accent/10 px-2 py-0.5 text-[var(--fs-sm)] text-accent"
      >
        {name}
        <button
          type="button"
          aria-label="{removeLabel}: {name}"
          class="rounded-full hover:bg-accent/20"
          onclick={() => remove(name)}
        >
          <X class="size-3 no-mirror" strokeWidth={2.5} />
        </button>
      </span>
    {/each}
    <input
      id={inputId}
      bind:value={text}
      {placeholder}
      class="min-w-20 flex-1 bg-transparent py-0.5 text-[var(--fs-base)] outline-none"
      onkeydown={onkeydown}
      onblur={add}
    />
  </div>
</Field>
