<script lang="ts">
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TitleBar from "./lib/components/TitleBar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import { route } from "./lib/router";

  import Dashboard from "./lib/views/Dashboard.svelte";
  import Gallery from "./lib/views/Gallery.svelte";
  import LibraryView from "./lib/views/Library.svelte";
  import SettingsView from "./lib/views/Settings.svelte";
  import Placeholder from "./lib/views/Placeholder.svelte";

  /**
   * App shell — title bar, sidebar, routed main region, toasts.
   *
   * The shell is deliberately the only place that knows about layout; views render into
   * `<main>` and never manage their own chrome, which is what keeps the RTL mirroring
   * consistent across all thirteen screens.
   */

  let currentPath = $derived($route.path);
</script>

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
          {:else}
            <Placeholder path={currentPath} pattern={$route.pattern} />
          {/if}
        </div>
      {/key}
    </main>
  </div>

  <Toasts />
</div>
