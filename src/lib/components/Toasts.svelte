<script lang="ts">
  import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "@lucide/svelte";

  import { dismissToast, toasts, type ToastTone } from "../toast";

  /**
   * Toasts — bottom-**inline-start**, which is bottom-right in English and bottom-left in
   * Arabic, so the stack never sits on top of the OS clock or the tray corner.
   *
   * `role="status"` (not `alert`) so a confirmation does not interrupt a screen reader
   * mid-sentence; error toasts do use `alert` because they carry an action.
   */

  const ICONS = {
    success: CheckCircle2,
    error: XCircle,
    warning: AlertTriangle,
    info: Info,
  } as const;

  const TONES: Record<ToastTone, string> = {
    success: "text-success border-success/30 bg-success-soft",
    error: "text-danger border-danger/30 bg-danger-soft",
    warning: "text-warning border-warning/30 bg-warning-soft",
    info: "text-info border-info/30 bg-info-soft",
  };
</script>

{#if $toasts.length > 0}
  <div
    class="pointer-events-none fixed bottom-[var(--sp-5)] start-[var(--sp-5)] z-50 flex w-[min(24rem,calc(100vw-2rem))] flex-col gap-[var(--sp-2)]"
    aria-live="polite"
  >
    {#each $toasts as toast (toast.id)}
      {@const Icon = ICONS[toast.tone]}
      <div
        class="pointer-events-auto flex items-start gap-[var(--sp-3)] rounded-[var(--r-md)] border px-[var(--sp-4)] py-[var(--sp-3)] shadow-2 {TONES[toast.tone]}"
        role={toast.tone === "error" ? "alert" : "status"}
        data-tone={toast.tone}
      >
        <Icon class="mt-0.5 size-4.5 shrink-0 no-mirror" strokeWidth={1.75} />
        <p class="bidi-auto flex-1 text-[var(--fs-base)] text-ink-1">{toast.message}</p>
        <button
          type="button"
          class="shrink-0 rounded-[var(--r-sm)] p-0.5 text-ink-3 transition-colors hover:text-ink-1 hover:bg-surface-3"
          aria-label="Dismiss"
          onclick={() => dismissToast(toast.id)}
        >
          <X class="size-4 no-mirror" strokeWidth={1.75} />
        </button>
      </div>
    {/each}
  </div>
{/if}
