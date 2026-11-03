<script lang="ts">
  import { ask, open, save } from "@tauri-apps/plugin-dialog";

  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import { exportLibraryCsv, exportLibraryJson, exportNotesMarkdown, importLibraryJson } from "../api";
  import { messageFor } from "../errors";
  import { locale, translatorFor } from "../i18n";
  import { toast } from "../toast";

  let t = $derived(translatorFor($locale));
  let busy = $state<"json" | "csv" | "markdown" | "import" | null>(null);

  async function exportJson(): Promise<void> {
    let path: string | null;
    try {
      path = await save({
        title: t("transfer.export.json"),
        defaultPath: "maktabati-library.json",
        filters: [{ name: t("transfer.dialog.json"), extensions: ["json"] }],
      });
    } catch (error) {
      console.error("JSON export file picker failed", error);
      toast.error(t("transfer.dialog.failed"));
      return;
    }
    if (path === null) return;
    busy = "json";
    const result = await exportLibraryJson(path);
    if (result.status === "ok") toast.success(t("transfer.export.done", { path: result.data }));
    else toast.error(messageFor(result.error, t));
    busy = null;
  }

  async function exportCsv(): Promise<void> {
    let parent: string | string[] | null;
    try {
      parent = await open({
        title: t("transfer.export.csv"),
        directory: true,
        multiple: false,
      });
    } catch (error) {
      console.error("CSV export folder picker failed", error);
      toast.error(t("transfer.dialog.failed"));
      return;
    }
    if (typeof parent !== "string") return;
    busy = "csv";
    const result = await exportLibraryCsv(parent);
    if (result.status === "ok") toast.success(t("transfer.export.done", { path: result.data }));
    else toast.error(messageFor(result.error, t));
    busy = null;
  }

  async function exportMarkdown(): Promise<void> {
    let path: string | null;
    try {
      path = await save({
        title: t("transfer.export.markdown"),
        defaultPath: "maktabati-notes.md",
        filters: [{ name: t("transfer.dialog.markdown"), extensions: ["md"] }],
      });
    } catch (error) {
      console.error("Markdown export file picker failed", error);
      toast.error(t("transfer.dialog.failed"));
      return;
    }
    if (path === null) return;
    busy = "markdown";
    const result = await exportNotesMarkdown(path);
    if (result.status === "ok") toast.success(t("transfer.export.done", { path: result.data }));
    else toast.error(messageFor(result.error, t));
    busy = null;
  }

  async function importJson(): Promise<void> {
    let path: string | string[] | null;
    try {
      path = await open({
        title: t("transfer.import.json"),
        multiple: false,
        filters: [{ name: t("transfer.dialog.json"), extensions: ["json"] }],
      });
    } catch (error) {
      console.error("JSON import file picker failed", error);
      toast.error(t("transfer.dialog.failed"));
      return;
    }
    if (typeof path !== "string") return;
    let confirmed = false;
    try {
      confirmed = await ask(t("transfer.import.confirm"), { kind: "warning" });
    } catch (error) {
      console.error("JSON import confirmation dialog failed", error);
      toast.error(t("transfer.dialog.failed"));
      return;
    }
    if (!confirmed) return;

    busy = "import";
    const result = await importLibraryJson(path);
    if (result.status === "ok") toast.success(t("transfer.import.done"));
    else toast.error(messageFor(result.error, t));
    busy = null;
  }
</script>

<div class="mx-auto flex max-w-3xl flex-col gap-[var(--sp-6)] px-[var(--sp-6)] py-[var(--sp-6)]">
  <header>
    <h1 class="text-[var(--fs-2xl)] font-semibold">{t("transfer.title")}</h1>
    <p class="mt-[var(--sp-2)] max-w-2xl text-[var(--fs-sm)] text-ink-3">{t("transfer.description")}</p>
  </header>

  <Card>
    <h2 class="mb-[var(--sp-2)] text-[var(--fs-md)] font-semibold">{t("transfer.export.title")}</h2>
    <div class="flex flex-wrap gap-[var(--sp-3)]">
      <Button variant="primary" loading={busy === "json"} disabled={busy !== null} onclick={() => void exportJson()}>
        {t("transfer.export.json")}
      </Button>
      <Button variant="secondary" loading={busy === "csv"} disabled={busy !== null} onclick={() => void exportCsv()}>
        {t("transfer.export.csv")}
      </Button>
      <Button
        variant="secondary"
        loading={busy === "markdown"}
        disabled={busy !== null}
        onclick={() => void exportMarkdown()}
      >
        {t("transfer.export.markdown")}
      </Button>
    </div>
  </Card>

  <Card>
    <h2 class="text-[var(--fs-md)] font-semibold">{t("transfer.import.title")}</h2>
    <p class="mt-[var(--sp-2)] mb-[var(--sp-4)] text-[var(--fs-sm)] text-ink-3">
      {t("transfer.import.description")}
    </p>
    <Button variant="danger" loading={busy === "import"} disabled={busy !== null} onclick={() => void importJson()}>
      {t("transfer.import.json")}
    </Button>
  </Card>
</div>
