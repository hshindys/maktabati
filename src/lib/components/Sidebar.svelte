<script lang="ts">
  import {
    ArrowLeftRight,
    BookOpen,
    Download,
    FolderTree,
    HandCoins,
    LayoutDashboard,
    Library,
    LibraryBig,
    PanelLeftClose,
    PanelLeftOpen,
    Quote,
    Settings,
    Store,
    StickyNote,
    Target,
    Users,
  } from "@lucide/svelte";

  import { locale, translatorFor, type MessageKey } from "../i18n";
  import { activeHref, isActive } from "../router";
  import { preferences, setPreferences } from "../settings";
  import Mark from "./Mark.svelte";

  /**
   * Sidebar — the persistent navigation rail (docs/04-ux-design.md §1).
   *
   * It never disappears: past 1100 px it collapses to icons with tooltips, because a nav that
   * hides behind a hamburger makes a 13-screen app feel twice as deep. The rail is grouped
   * (P1-10) into Library / Organize / Track / System so the twelve screens read as four
   * ideas rather than one flat list, and the active item carries an accent bar on the edge
   * facing the content — the "you are here" cue a soft pill alone never quite lands.
   *
   * The brand block on top is not decoration: it is the only place in the shell that names
   * the product, and the mark is the VectorCraft master inlined (`Mark.svelte`), so it
   * themes with the app — ink pages on paper, paper pages on ink.
   */

  interface Props {
    currentPath: string;
  }

  let { currentPath }: Props = $props();

  let t = $derived(translatorFor($locale));
  let collapsed = $derived($preferences.sidebarCollapsed);

  interface NavItem {
    href: string;
    key: MessageKey;
    icon: typeof Library;
  }

  interface NavGroup {
    labelKey: MessageKey;
    items: NavItem[];
  }

  const GROUPS: NavGroup[] = [
    {
      labelKey: "nav.group.library",
      items: [
        { href: "/", key: "nav.dashboard", icon: LayoutDashboard },
        { href: "/library", key: "nav.library", icon: Library },
        { href: "/import", key: "nav.import", icon: Download },
        { href: "/reading", key: "nav.reading", icon: BookOpen },
      ],
    },
    {
      labelKey: "nav.group.organize",
      items: [
        { href: "/notes", key: "nav.notes", icon: StickyNote },
        { href: "/quotes", key: "nav.quotes", icon: Quote },
        { href: "/authors", key: "nav.authors", icon: Users },
        { href: "/publishers", key: "nav.publishers", icon: Store },
        { href: "/taxonomy", key: "nav.categories", icon: FolderTree },
      ],
    },
    {
      labelKey: "nav.group.track",
      items: [
        { href: "/lending", key: "nav.lending", icon: HandCoins },
        { href: "/goals", key: "nav.goals", icon: Target },
        { href: "/transfer", key: "nav.transfer", icon: ArrowLeftRight },
      ],
    },
    {
      labelKey: "nav.group.system",
      items: [{ href: "/settings", key: "nav.settings", icon: Settings }],
    },
  ];

  function toggleCollapse(): void {
    setPreferences({ sidebarCollapsed: !collapsed });
  }
</script>

{#snippet navLink(item: NavItem)}
  {@const Icon = item.icon}
  {@const active = isActive(item.href, activeHref(currentPath))}
  <a
    href="#{item.href}"
    title={collapsed ? t(item.key) : undefined}
    aria-current={active ? "page" : undefined}
    class="relative flex items-center gap-[var(--sp-3)] rounded-[var(--r-md)] px-[var(--sp-3)] py-2 text-[var(--fs-base)] transition-colors {active
      ? 'bg-accent-soft font-medium text-accent-soft-ink'
      : 'text-ink-2 hover:bg-surface-2 hover:text-ink-1 active:bg-surface-3'}"
  >
    {#if active}
      <!-- The "you are here" bar: flush to the rail edge, accent-filled. -->
      <span class="absolute inset-y-1.5 end-0 w-[3px] rounded-full bg-accent" aria-hidden="true"></span>
    {/if}
    <Icon class="size-4.5 shrink-0" strokeWidth={1.75} />
    {#if !collapsed}
      <span class="truncate">{t(item.key)}</span>
    {/if}
  </a>
{/snippet}

<nav
  class="z-20 flex h-full shrink-0 flex-col border-s border-border-1 bg-surface-1 transition-[inline-size] duration-[var(--dur-base)] ease-[var(--ease)]"
  style="inline-size: {collapsed ? 'var(--rail)' : 'var(--drawer)'}"
  aria-label={t("app.name")}
>
  <!-- Brand. When the rail is 3.75rem wide the wordmark cannot fit, so only the mark shows,
       centred — the same mark the title bar and the empty states carry. -->
  <div
    class="flex shrink-0 items-center gap-[var(--sp-3)] px-[var(--sp-3)] pt-[var(--sp-4)] pb-[var(--sp-3)] {collapsed
      ? 'justify-center border-b border-border-1'
      : ''}"
  >
    <Mark class="size-7 shrink-0" title={t("app.name")} />
    {#if !collapsed}
      <span class="truncate text-[var(--fs-md)] font-semibold text-ink-1">{t("app.name")}</span>
    {/if}
  </div>

  <div class="flex flex-1 flex-col overflow-y-auto p-[var(--sp-2)]">
    {#each GROUPS as group (group.labelKey)}
      <div class="flex flex-col gap-0.5">
        {#if !collapsed}
          <div
            class="px-[var(--sp-3)] pt-[var(--sp-3)] pb-1 text-[10px] font-semibold text-ink-3"
            aria-hidden="true"
          >
            {t(group.labelKey)}
          </div>
        {:else}
          <div class="my-2 border-t border-border-1" role="separator"></div>
        {/if}
        {#each group.items as item (item.href)}
          {@render navLink(item)}
        {/each}
      </div>
    {/each}
  </div>

  <div class="border-t border-border-1 p-[var(--sp-2)]">
    {#if import.meta.env.DEV}
      <!-- Dev-only component gallery; kept out of the shipped rail (P1-10). -->
      <a
        href="#/dev/gallery"
        aria-current={isActive("/dev/gallery", currentPath) ? "page" : undefined}
        title={t("nav.gallery")}
        class="flex items-center gap-[var(--sp-3)] rounded-[var(--r-md)] px-[var(--sp-3)] py-2 text-[var(--fs-sm)] text-ink-3 transition-colors hover:bg-surface-2 hover:text-ink-2"
      >
        <LibraryBig class="size-4.5 shrink-0" strokeWidth={1.75} />
        {#if !collapsed}
          <span class="truncate">{t("nav.gallery")}</span>
        {/if}
      </a>
    {/if}

    <button
      type="button"
      class="mt-1 flex w-full items-center gap-[var(--sp-3)] rounded-[var(--r-md)] px-[var(--sp-3)] py-2 text-[var(--fs-sm)] text-ink-3 transition-colors hover:bg-surface-2 hover:text-ink-2"
      onclick={toggleCollapse}
      aria-expanded={!collapsed}
      title={collapsed ? t("shell.expand") : t("shell.collapse")}
    >
      {#if collapsed}
        <PanelLeftOpen class="size-4.5 shrink-0" strokeWidth={1.75} />
        <span class="sr-only">{t("shell.expand")}</span>
      {:else}
        <PanelLeftClose class="size-4.5 shrink-0" strokeWidth={1.75} />
        <span class="truncate">{t("shell.collapse")}</span>
      {/if}
    </button>
  </div>
</nav>

<style>
  /* The two widths the rail can take. Declared here rather than as arbitrary Tailwind values
   * because the collapsed width must be an exact square for the icon centring to read well. */
  nav {
    --rail: 3.75rem;
    --drawer: 15rem;
  }
</style>
