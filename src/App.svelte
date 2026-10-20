<script lang="ts">
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TitleBar from "./lib/components/TitleBar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import { route } from "./lib/router";
  import { openAddBook } from "./lib/wizard";

  import Dashboard from "./lib/views/Dashboard.svelte";
  import Gallery from "./lib/views/Gallery.svelte";
  import LibraryView from "./lib/views/Library.svelte";
  import SettingsView from "./lib/views/Settings.svelte";
  import BookPage from "./lib/views/BookPage.svelte";
  import Placeholder from "./lib/views/Placeholder.svelte";
  import AddBook from "./lib/views/AddBook.svelte";

  /**
   * App shell — title bar, sidebar, routed main region, toasts.
   *
   * The shell is deliberately the only place that knows about layout; views render into
   * `<main>` and never manage their own chrome, which is what keeps the RTL mirroring
   * consistent across all thirteen screens. It also owns the two app-wide affordances
   * that belong to no route: the toasts and the add-book wizard (`Ctrl+N`, docs/04 §4.5).
   */

  let currentPath = $derived($route.path);

  function onKeydown(e: KeyboardEvent) {
    // Ctrl/Cmd+N opens the wizard from anywhere — the shortcut the design gives it.
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "n") {
      e.preventDefault();
      openAddBook();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="flex h-screen flex-col overflow-hidden bg-surface-0 text-ink-1">
  <TitleBar />

  <div class="flex min-h-0 flex-1">
    <Sidebar {currentPath} />

    <main class="min-w-0 flex-1 overflow-y-auto" id="main">
      <!-- `key` forces a remount on route change so the enter animation replays; the
           transition itself is capped at 120 ms (docs/04-ux-design.md §5.5). -->
      {#key currentPath}
        <div class="route-enter">
          {#if currentPath === "/"}
            <Dashboard />
          {:else if currentPath === "/library"}
            <LibraryView />
          {:else if currentPath === "/settings"}
            <SettingsView />
          {:else if currentPath === "/dev/gallery"}
            <Gallery />
          {:else if currentPath.startsWith("/book/")}
            <!-- A detail page, not a section: `/book/:id` keeps المكتبة lit in the rail via
                 `activeHref`, so it is matched before the placeholder fallback. -->
            <BookPage />
          {:else}
            <Placeholder path={currentPath} pattern={$route.pattern} />
          {/if}
        </div>
      {/key}
    </main>
  </div>

  <Toasts />

  <!-- The wizard is a modal, not a route: one instance for the whole app. -->
  <AddBook />
</div>
