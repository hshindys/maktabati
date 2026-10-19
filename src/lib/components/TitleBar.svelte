<script lang="ts">
  import { Languages, Lock, Maximize2, Minus, Minimize2, Moon, Search, Sun, X } from "@lucide/svelte";
  import { onMount } from "svelte";

  import { locale, translatorFor } from "../i18n";
  import { preferences, resolveTheme, setPreferences, THEME_CHOICES } from "../settings";

  /**
   * TitleBar — the custom window chrome (docs/04-ux-design.md §4.1).
   *
   * `decorations` is `false` in `tauri.conf.json`, so this component owns dragging, the window
   * buttons and the global search entry. Everything inside the drag region carries
   * `titlebar-no-drag` so it stays clickable.
   */

  let t = $derived(translatorFor($locale));

  let maximized = $state(false);
  /** `false` when not running inside Tauri (plain `npm run dev` in a browser). */
  let inTauri = $state(false);

  onMount(() => {
    // `onMount` must hand back its cleanup synchronously, so the listener is captured into a
    // variable rather than returned from the async body.
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      // Dynamic import: the Tauri IPC module throws when loaded outside the shell, and the
      // component gallery is also opened in a plain browser during design review.
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        if (disposed) return;
        inTauri = true;
        maximized = await win.isMaximized();
        unlisten = await win.onResized(async () => {
          maximized = await win.isMaximized();
        });
        // The window may have been torn down while the import resolved.
        if (disposed) unlisten();
      } catch {
        // Not running inside the Tauri shell — the buttons stay inert.
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  async function toggleMaximize(): Promise<void> {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().toggleMaximize();
    } catch {
      /* not running under Tauri */
    }
  }

  async function minimize(): Promise<void> {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().minimize();
    } catch {
      /* not running under Tauri */
    }
  }

  async function close(): Promise<void> {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    } catch {
      /* not running under Tauri */
    }
  }

  function toggleLocale(): void {
    setPreferences({ locale: $preferences.locale === "ar" ? "en" : "ar" });
  }

  function cycleTheme(): void {
    const i = THEME_CHOICES.indexOf($preferences.theme);
    setPreferences({ theme: THEME_CHOICES[(i + 1) % THEME_CHOICES.length] });
  }

  let ThemeIcon = $derived(
    resolveTheme($preferences.theme) === "dark" ? Moon : Sun,
  );
</script>

<header
  class="titlebar-drag relative z-30 flex h-11 shrink-0 items-center gap-[var(--sp-2)] border-b border-border-1 bg-surface-2 pe-1 ps-[var(--sp-3)]"
  data-tauri-drag-region
>
  <!-- App mark + name -->
  <div class="titlebar-no-drag flex items-center gap-[var(--sp-2)] pe-[var(--sp-2)]">
    <span
      class="grid size-6 place-items-center rounded-[var(--r-sm)] bg-accent text-[13px] font-bold text-accent-fg"
      aria-hidden="true"
    >
      م
    </span>
    <span class="text-[var(--fs-base)] font-semibold text-ink-1">{t("app.name")}</span>
  </div>

  <!-- Global search entry. The palette itself lands with the library milestone. -->
  <div class="titlebar-no-drag mx-auto w-full max-w-[34rem] px-[var(--sp-3)]">
    <button
      type="button"
      class="flex h-7 w-full items-center gap-[var(--sp-2)] rounded-[var(--r-md)] border border-border-1 bg-surface-1 px-[var(--sp-3)] text-start text-[var(--fs-sm)] text-ink-3 transition-colors hover:border-border-2 hover:text-ink-2"
      onclick={() => {
        /* opened by Ctrl/Cmd+K once the palette lands */
      }}
    >
      <Search class="size-3.5 shrink-0 no-mirror" strokeWidth={1.75} />
      <span class="flex-1 truncate">{t("shell.search")}</span>
      <kbd
        class="bidi-ltr rounded-[var(--r-sm)] border border-border-1 bg-surface-2 px-1.5 py-px text-[10px] font-medium text-ink-3"
      >
        {t("shell.searchHint")}
      </kbd>
    </button>
  </div>

  <!-- Trailing controls -->
  <div class="titlebar-no-drag flex items-center gap-0.5">
    <button
      type="button"
      class="grid size-8 place-items-center rounded-[var(--r-sm)] text-ink-2 transition-colors hover:bg-surface-3 hover:text-ink-1"
      onclick={toggleLocale}
      title={t("shell.language")}
      aria-label={t("shell.language")}
    >
      <Languages class="size-4 no-mirror" strokeWidth={1.75} />
    </button>

    <button
      type="button"
      class="grid size-8 place-items-center rounded-[var(--r-sm)] text-ink-2 transition-colors hover:bg-surface-3 hover:text-ink-1"
      onclick={cycleTheme}
      title={t("shell.theme")}
      aria-label="{t("shell.theme")}: {t(`shell.theme.${resolveTheme($preferences.theme)}`)}"
    >
      <ThemeIcon class="size-4 no-mirror" strokeWidth={1.75} />
    </button>

    <button
      type="button"
      class="grid size-8 place-items-center rounded-[var(--r-sm)] text-ink-2 transition-colors hover:bg-surface-3 hover:text-ink-1"
      title={t("shell.lock")}
      aria-label={t("shell.lock")}
      disabled
    >
      <Lock class="size-4 no-mirror" strokeWidth={1.75} />
    </button>

    <!-- Window controls. Only rendered under Tauri — a browser preview has its own chrome. -->
    {#if inTauri}
      <div class="ms-[var(--sp-2)] flex items-center">
        <button
          type="button"
          class="grid h-8 w-11 place-items-center rounded-[var(--r-sm)] text-ink-2 transition-colors hover:bg-surface-3 hover:text-ink-1"
          onclick={minimize}
          title={t("shell.minimize")}
          aria-label={t("shell.minimize")}
        >
          <Minus class="size-4 no-mirror" strokeWidth={1.75} />
        </button>
        <button
          type="button"
          class="grid h-8 w-11 place-items-center rounded-[var(--r-sm)] text-ink-2 transition-colors hover:bg-surface-3 hover:text-ink-1"
          onclick={toggleMaximize}
          title={maximized ? t("shell.restore") : t("shell.maximize")}
          aria-label={maximized ? t("shell.restore") : t("shell.maximize")}
        >
          {#if maximized}
            <Minimize2 class="size-4 no-mirror" strokeWidth={1.75} />
          {:else}
            <Maximize2 class="size-4 no-mirror" strokeWidth={1.75} />
          {/if}
        </button>
        <button
          type="button"
          class="grid h-8 w-11 place-items-center rounded-[var(--r-sm)] text-ink-2 transition-colors hover:bg-danger hover:text-white"
          onclick={close}
          title={t("shell.close")}
          aria-label={t("shell.close")}
        >
          <X class="size-4 no-mirror" strokeWidth={1.75} />
        </button>
      </div>
    {/if}
  </div>
</header>
