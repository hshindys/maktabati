<script lang="ts">
  import {
    ArrowLeftRight,
    BookOpen,
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
  import { isActive } from "../router";
  import { preferences, setPreferences } from "../settings";

  /**
   * Sidebar — the persistent navigation rail (docs/04-ux-design.md §1).
   *
   * It never disappears: past 1100 px it collapses to icons with tooltips, because a nav that
   * hides behind a hamburger makes a 13-screen app feel twice as deep.
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
    /** Rendered above the section break. */
    group: "primary" | "secondary";
  }

  const ITEMS: NavItem[] = [
    { href: "/", key: "nav.dashboard", icon: LayoutDashboard, group: "primary" },
    { href: "/library", key: "nav.library", icon: Library, group: "primary" },
    { href: "/reading", key: "nav.reading", icon: BookOpen, group: "primary" },
    { href: "/notes", key: "nav.notes", icon: StickyNote, group: "secondary" },
    { href: "/quotes", key: "nav.quotes", icon: Quote, group: "secondary" },
    { href: "/authors", key: "nav.authors", icon: Users, group: "secondary" },
    { href: "/publishers", key: "nav.publishers", icon: Store, group: "secondary" },
    { href: "/taxonomy", key: "nav.categories", icon: FolderTree, group: "secondary" },
    { href: "/lending", key: "nav.lending", icon: HandCoins, group: "secondary" },
    { href: "/goals", key: "nav.goals", icon: Target, group: "secondary" },
    { href: "/transfer", key: "nav.transfer", icon: ArrowLeftRight, group: "secondary" },
    { href: "/settings", key: "nav.settings", icon: Settings, group: "secondary" },
  ];

  let primary = $derived(ITEMS.filter((i) => i.group === "primary"));
  let secondary = $derived(ITEMS.filter((i) => i.group === "secondary"));

  function toggleCollapse(): void {
    setPreferences({ sidebarCollapsed: !collapsed });
  }
</script>

<nav
  class="z-20 flex h-full shrink-0 flex-col border-e border-border-1 bg-surface-1 transition-[inline-size] duration-[var(--dur-base)] ease-[var(--ease)]"
  style="inline-size: {collapsed ? 'var(--rail)' : 'var(--drawer)'}"
  aria-label={t("nav.library")}
>
  <div class="flex flex-1 flex-col gap-1 overflow-y-auto p-[var(--sp-2)]">
    {#each primary as item (item.href)}
      {@const Icon = item.icon}
      <a
        href="#{item.href}"
        title={collapsed ? t(item.key) : undefined}
        aria-current={isActive(item.href, currentPath) ? "page" : undefined}
        class="group flex items-center gap-[var(--sp-3)] rounded-[var(--r-md)] px-[var(--sp-3)] py-2 text-[var(--fs-base)] transition-colors {isActive(
          item.href,
          currentPath,
        )
          ? 'bg-accent-soft text-accent-soft-ink font-medium'
          : 'text-ink-2 hover:bg-surface-2 hover:text-ink-1'}"
      >
        <Icon class="size-4.5 shrink-0" strokeWidth={1.75} />
        {#if !collapsed}
          <span class="truncate">{t(item.key)}</span>
        {/if}
      </a>
    {/each}

    <div class="my-[var(--sp-2)] border-t border-border-1" role="separator"></div>

    {#each secondary as item (item.href)}
      {@const Icon = item.icon}
      <a
        href="#{item.href}"
        title={collapsed ? t(item.key) : undefined}
        aria-current={isActive(item.href, currentPath) ? "page" : undefined}
        class="flex items-center gap-[var(--sp-3)] rounded-[var(--r-md)] px-[var(--sp-3)] py-2 text-[var(--fs-base)] transition-colors {isActive(
          item.href,
          currentPath,
        )
          ? 'bg-accent-soft text-accent-soft-ink font-medium'
          : 'text-ink-2 hover:bg-surface-2 hover:text-ink-1'}"
      >
        <Icon class="size-4.5 shrink-0" strokeWidth={1.75} />
        {#if !collapsed}
          <span class="truncate">{t(item.key)}</span>
        {/if}
      </a>
    {/each}
  </div>

  <div class="border-t border-border-1 p-[var(--sp-2)]">
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
