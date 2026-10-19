<script lang="ts">
  import { Plus, X } from "@lucide/svelte";

  import Field from "./Field.svelte";
  import type { EntityRef } from "../draft";

  /**
   * Combobox — autocomplete over existing catalogue entries with inline create.
   *
   * The pattern from docs/04 §4.5: type to filter, pick a match, or press the create row
   * to turn the typed name into a new entity on the spot. Options are filtered locally
   * from a list the wizard loads once — a personal library is a few hundred rows, and
   * local matching keeps Arabic honest instead of inheriting a server collation. A name
   * that was created but whose write failed stays in the draft as `{ id: null, name }`,
   * so nothing the user typed is ever lost.
   */

  interface Props {
    label: string;
    required?: boolean;
    error?: string | null;
    hint?: string | null;
    value: EntityRef | null;
    onchange: (next: EntityRef | null) => void;
    options: EntityRef[];
    placeholder?: string;
    /** Label of the inline-create row (e.g. "إضافة مؤلف"); omit to make the field pick-only. */
    createLabel?: string;
    /** Create the entity now and return it; the wizard toasts and returns `{ id: null }` on failure. */
    oncreate?: (name: string) => Promise<EntityRef>;
    removeLabel: string;
  }

  let {
    label,
    required = false,
    error = null,
    hint = null,
    value,
    onchange,
    options,
    placeholder,
    createLabel,
    oncreate,
    removeLabel,
  }: Props = $props();

  const inputId = "cb-" + Math.random().toString(36).slice(2, 8);
  const listId = `${inputId}-list`;

  let input: HTMLInputElement | undefined = $state();
  let query = $state("");
  let open = $state(false);
  let active = $state(0);

  /**
   * The input is deliberately uncontrolled while the user types — a `value={...}` bound to
   * derived state would snap back to the chosen name on every empty keystroke. The effect
   * only rewrites the DOM value when the selection *changes from outside* (pick or clear).
   */
  $effect(() => {
    if (!open && input) input.value = value?.name ?? "";
  });

  let filtered = $derived.by(() => {
    const q = query.trim().toLocaleLowerCase();
    if (!q) return options;
    return options.filter((o) => o.name.toLocaleLowerCase().includes(q));
  });

  let canCreate = $derived(
    !!oncreate &&
      !!createLabel &&
      query.trim() !== "" &&
      !options.some((o) => o.name.toLocaleLowerCase() === query.trim().toLocaleLowerCase()),
  );

  let maxIndex = $derived(filtered.length - 1 + (canCreate ? 1 : 0));

  function pick(i: number) {
    if (i >= 0 && i < filtered.length) {
      const chosen = filtered[i];
      onchange({ id: chosen.id, name: chosen.name });
    } else if (canCreate && i === filtered.length && oncreate) {
      void oncreate(query.trim()).then((created) => onchange(created));
      return;
    } else {
      return;
    }
    query = "";
    open = false;
  }

  function clear() {
    onchange(null);
    query = "";
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      // Open dropdown first; let the second Escape reach the modal.
      if (open) {
        e.stopPropagation();
        open = false;
      }
      return;
    }
    if (!open && (e.key === "ArrowDown" || e.key === "Enter")) {
      open = true;
      active = 0;
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      open = true;
      active = Math.min(active + 1, maxIndex);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.max(active - 1, 0);
    } else if (e.key === "Enter" && open) {
      e.preventDefault();
      pick(active);
    }
  }

  function oninput() {
    query = input?.value ?? "";
    open = true;
    active = 0;
  }
</script>

<Field {label} {required} {error} {hint} for={inputId}>
  <div class="relative">
    <div
      class="flex h-9 items-center gap-1.5 rounded-[var(--r-md)] border bg-surface-1 px-[var(--sp-3)] focus-within:border-accent {error
        ? 'border-danger/60'
        : 'border-border-2'}"
    >
      <input
        id={inputId}
        bind:this={input}
        {placeholder}
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        autocomplete="off"
        class="min-w-0 flex-1 bg-transparent text-[var(--fs-base)] outline-none"
        oninput={oninput}
        onfocus={() => (open = true)}
        onkeydown={onkeydown}
        onblur={() => (open = false)}
      />
      {#if value}
        <button
          type="button"
          aria-label="{removeLabel}: {value.name}"
          class="rounded-full p-0.5 text-ink-3 hover:bg-surface-2 hover:text-ink-1"
          onmousedown={(e) => e.preventDefault()}
          onclick={clear}
        >
          <X class="size-3.5 no-mirror" strokeWidth={2.5} />
        </button>
      {/if}
    </div>

    {#if open && (filtered.length > 0 || canCreate)}
      <ul
        id={listId}
        role="listbox"
        class="absolute inset-x-0 top-full z-10 mt-1 max-h-56 overflow-auto rounded-[var(--r-md)] border border-border-2 bg-surface-1 py-1 shadow-2"
      >
        {#each filtered as option, i (option.id ?? option.name)}
          <li>
            <button
              type="button"
              role="option"
              aria-selected={value?.id === option.id}
              class="block w-full px-3 py-1.5 text-start text-[var(--fs-base)] {i === active
                ? 'bg-surface-2'
                : ''}"
              onmousedown={(e) => e.preventDefault()}
              onmouseenter={() => (active = i)}
              onclick={() => pick(i)}
            >
              {option.name}
            </button>
          </li>
        {/each}
        {#if canCreate}
          <li>
            <button
              type="button"
              class="flex w-full items-center gap-2 px-3 py-1.5 text-start text-[var(--fs-base)] text-accent {active ===
              filtered.length
                ? 'bg-surface-2'
                : ''}"
              onmousedown={(e) => e.preventDefault()}
              onmouseenter={() => (active = filtered.length)}
              onclick={() => pick(filtered.length)}
            >
              <Plus class="size-4 no-mirror" strokeWidth={2} />
              {createLabel} «{query.trim()}»
            </button>
          </li>
        {/if}
      </ul>
    {/if}
  </div>
</Field>
