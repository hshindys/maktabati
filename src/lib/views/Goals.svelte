<script lang="ts">
  import { onMount } from "svelte";

  import {
    createReadingSession,
    deleteReadingGoal,
    listBooks,
    listReadingGoals,
    listReadingSessions,
    saveReadingGoal,
  } from "../api";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { toast } from "../toast";
  import type {
    AppErrorDto,
    Book,
    ReadingGoal,
    ReadingGoalInput,
    ReadingSession,
    ReadingSessionInput,
  } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  const num = (value: number) => formatNumber(value, { digits });
  const monthLabel = (value: number) =>
    new Intl.DateTimeFormat($locale === "ar" ? "ar" : "en", { month: "long", timeZone: "UTC" })
      .format(new Date(Date.UTC(2020, value - 1, 1)));

  function todayKey(): string {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  }

  let goals = $state<ReadingGoal[]>([]);
  let sessions = $state<ReadingSession[]>([]);
  let books = $state<Book[]>([]);
  let loadError = $state<AppErrorDto | null>(null);
  let loading = $state(true);
  let savingGoal = $state(false);
  let savingSession = $state(false);
  let editingGoal = $state<ReadingGoal | null>(null);
  let scope = $state<"year" | "month">("year");
  let year = $state(String(new Date().getFullYear()));
  let month = $state(String(new Date().getMonth() + 1));
  let targetBooks = $state("");
  let targetPages = $state("");
  let sessionBook = $state("");
  let sessionDate = $state(todayKey());
  let sessionPages = $state("");
  let sessionMinutes = $state("");
  let sessionNote = $state("");

  function parseOptionalPositive(raw: string): number | null {
    if (raw.trim() === "") return null;
    const value = Number(raw);
    return Number.isInteger(value) && value > 0 ? value : Number.NaN;
  }

  async function load(): Promise<void> {
    loading = true;
    const [goalResult, sessionResult, bookResult] = await Promise.all([
      listReadingGoals(),
      listReadingSessions(),
      listBooks(),
    ]);
    const failure = [goalResult, sessionResult, bookResult].find(
      (result) => result.status === "error" && result.error.code !== "shell.unavailable",
    );
    if (failure?.status === "error") {
      loadError = failure.error;
    } else {
      loadError = null;
      goals = goalResult.status === "ok" ? goalResult.data : [];
      sessions = sessionResult.status === "ok" ? sessionResult.data : [];
      books = bookResult.status === "ok" ? bookResult.data : [];
      if (sessionBook === "" && books.length > 0) sessionBook = books[0].id;
    }
    loading = false;
  }

  function edit(goal: ReadingGoal): void {
    editingGoal = goal;
    scope = goal.scope === "month" ? "month" : "year";
    year = String(goal.year);
    month = goal.month === null ? String(new Date().getMonth() + 1) : String(goal.month);
    targetBooks = goal.target_books === null ? "" : String(goal.target_books);
    targetPages = goal.target_pages === null ? "" : String(goal.target_pages);
  }

  function resetGoalForm(): void {
    editingGoal = null;
    scope = "year";
    year = String(new Date().getFullYear());
    month = String(new Date().getMonth() + 1);
    targetBooks = "";
    targetPages = "";
  }

  async function submitGoal(): Promise<void> {
    if (savingGoal) return;
    const parsedYear = Number(year);
    const parsedMonth = Number(month);
    const booksTarget = parseOptionalPositive(targetBooks);
    const pagesTarget = parseOptionalPositive(targetPages);
    if (
      !Number.isInteger(parsedYear) ||
      parsedYear < 1 ||
      parsedYear > 9999 ||
      (scope === "month" && (!Number.isInteger(parsedMonth) || parsedMonth < 1 || parsedMonth > 12)) ||
      Number.isNaN(booksTarget) ||
      Number.isNaN(pagesTarget) ||
      (booksTarget === null && pagesTarget === null)
    ) {
      toast.error(t("goals.validation"));
      return;
    }
    const input: ReadingGoalInput = {
      scope,
      year: parsedYear,
      month: scope === "month" ? parsedMonth : null,
      target_books: booksTarget,
      target_pages: pagesTarget,
    };
    savingGoal = true;
    try {
      const result = await saveReadingGoal(input);
      if (result.status === "ok") {
        toast.success(t("goals.saved"));
        resetGoalForm();
        await load();
      } else {
        toast.error(messageFor(result.error, t));
      }
    } finally {
      savingGoal = false;
    }
  }

  async function removeGoal(goal: ReadingGoal): Promise<void> {
    const result = await deleteReadingGoal(goal.id);
    if (result.status === "ok") {
      goals = goals.filter((item) => item.id !== goal.id);
      if (editingGoal?.id === goal.id) resetGoalForm();
      toast.success(t("goals.deleted"));
    } else {
      toast.error(messageFor(result.error, t));
    }
  }

  async function submitSession(): Promise<void> {
    if (savingSession) return;
    const pages = sessionPages.trim() === "" ? null : Number(sessionPages);
    const minutes = sessionMinutes.trim() === "" ? null : Number(sessionMinutes);
    if (
      sessionBook === "" ||
      !/^\d{4}-\d{2}-\d{2}$/.test(sessionDate) ||
      (pages === null && minutes === null) ||
      (pages !== null && (!Number.isInteger(pages) || pages < 0)) ||
      (minutes !== null && (!Number.isInteger(minutes) || minutes < 0))
    ) {
      toast.error(t("goals.session.validation"));
      return;
    }
    const input: ReadingSessionInput = {
      book_id: sessionBook,
      session_date: sessionDate,
      pages_read: pages,
      minutes,
      note: sessionNote.trim() || null,
    };
    savingSession = true;
    try {
      const result = await createReadingSession(input);
      if (result.status === "ok") {
        sessionPages = "";
        sessionMinutes = "";
        sessionNote = "";
        toast.success(t("goals.session.saved"));
        await load();
      } else {
        toast.error(messageFor(result.error, t));
      }
    } finally {
      savingSession = false;
    }
  }

  const percent = (actual: number, target: number) =>
    Math.min(100, Math.round((actual / target) * 100));

  onMount(() => void load());
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-5)] px-[var(--sp-6)] py-[var(--sp-5)]">
  <header>
    <h1 class="text-[var(--fs-2xl)] font-semibold">{t("goals.title")}</h1>
    <p class="mt-1 text-[var(--fs-base)] text-ink-2">{t("goals.intro")}</p>
  </header>

  {#if loading}
    <div class="h-28 animate-pulse rounded-[var(--r-lg)] border border-border-1 bg-surface-2"></div>
  {:else if loadError}
    <Card>
      <div role="alert" class="flex flex-col gap-[var(--sp-3)]">
        <p class="text-danger">{messageFor(loadError, t)}</p>
        <Button onclick={() => void load()}>{t("common.retry")}</Button>
      </div>
    </Card>
  {:else}
    <Card>
      <h2 class="mb-[var(--sp-4)] text-[var(--fs-lg)] font-semibold">{t("goals.form.title")}</h2>
      <form class="grid gap-[var(--sp-3)] sm:grid-cols-2 lg:grid-cols-3" onsubmit={(event) => { event.preventDefault(); void submitGoal(); }}>
        <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
          {t("goals.period")}
          <select bind:value={scope} class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1">
            <option value="year">{t("goals.yearly")}</option>
            <option value="month">{t("goals.monthly")}</option>
          </select>
        </label>
        <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
          {t("goals.year")}
          <input type="text" inputmode="numeric" bind:value={year} dir="ltr" class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
        </label>
        {#if scope === "month"}
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("goals.month")}
            <select bind:value={month} class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1">
              {#each Array.from({ length: 12 }, (_, i) => i + 1) as value (value)}
                <option value={String(value)}>{monthLabel(value)}</option>
              {/each}
            </select>
          </label>
        {/if}
        <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
          {t("goals.target.books")}
          <input type="text" inputmode="numeric" bind:value={targetBooks} dir="ltr" class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
        </label>
        <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
          {t("goals.target.pages")}
          <input type="text" inputmode="numeric" bind:value={targetPages} dir="ltr" class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
        </label>
        <div class="flex items-end gap-[var(--sp-2)]">
          <Button type="submit" variant="primary" loading={savingGoal}>{editingGoal ? t("goals.update") : t("goals.save")}</Button>
          {#if editingGoal}<Button type="button" onclick={resetGoalForm}>{t("common.cancel")}</Button>{/if}
        </div>
      </form>
    </Card>

    {#if goals.length === 0}
      <Card><EmptyState title={t("goals.empty.title")} body={t("goals.empty.body")} compact /></Card>
    {:else}
      <section class="grid gap-[var(--sp-4)] md:grid-cols-2" aria-label={t("goals.list")}>
        {#each goals as goal (goal.id)}
          <Card>
            <div class="mb-[var(--sp-4)] flex items-start justify-between gap-2">
              <div>
                <h2 class="text-[var(--fs-lg)] font-semibold">{goal.scope === "year" ? t("goals.yearlyTitle", { year: num(goal.year) }) : t("goals.monthlyTitle", { month: monthLabel(goal.month ?? 1), year: num(goal.year) })}</h2>
              </div>
              <div class="flex gap-2">
                <Button size="sm" onclick={() => edit(goal)}>{t("common.edit")}</Button>
                <Button size="sm" variant="danger" onclick={() => void removeGoal(goal)}>{t("common.delete")}</Button>
              </div>
            </div>
            {#if goal.target_books !== null}
              <ProgressBar value={percent(goal.books_completed, goal.target_books)} label="{t("goals.target.books")}: {num(goal.books_completed)} / {num(goal.target_books)}" showValue />
            {/if}
            {#if goal.target_pages !== null}
              <div class="mt-[var(--sp-3)]">
                <ProgressBar value={percent(goal.pages_read, goal.target_pages)} label="{t("goals.target.pages")}: {num(goal.pages_read)} / {num(goal.target_pages)}" showValue />
              </div>
            {/if}
          </Card>
        {/each}
      </section>
    {/if}

    <Card>
      <h2 class="mb-[var(--sp-4)] text-[var(--fs-lg)] font-semibold">{t("goals.session.title")}</h2>
      {#if books.length === 0}
        <p class="text-[var(--fs-sm)] text-ink-3">{t("goals.session.noBooks")}</p>
      {:else}
        <form class="grid gap-[var(--sp-3)] sm:grid-cols-2 lg:grid-cols-3" onsubmit={(event) => { event.preventDefault(); void submitSession(); }}>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("goals.session.book")}
            <select bind:value={sessionBook} class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1">
              {#each books as book (book.id)}<option value={book.id}>{book.title}</option>{/each}
            </select>
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("goals.session.date")}
            <input type="date" bind:value={sessionDate} dir="ltr" class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("goals.session.pages")}
            <input type="text" inputmode="numeric" bind:value={sessionPages} dir="ltr" class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("goals.session.minutes")}
            <input type="text" inputmode="numeric" bind:value={sessionMinutes} dir="ltr" class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("goals.session.note")}
            <input type="text" bind:value={sessionNote} class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <div class="flex items-end"><Button type="submit" variant="primary" loading={savingSession}>{t("goals.session.save")}</Button></div>
        </form>
      {/if}
      {#if sessions.length > 0}
        <h3 class="mb-2 mt-[var(--sp-5)] font-semibold">{t("goals.session.recent")}</h3>
        <ul class="divide-y divide-border-1">
          {#each sessions.slice(0, 10) as session (session.id)}
            <li class="flex flex-wrap justify-between gap-2 py-2 text-[var(--fs-sm)]">
              <span>{session.book_title} · {session.session_date}</span>
              <span class="text-ink-3">{session.pages_read === null ? "" : `${num(session.pages_read)} ${t("goals.session.pages")}`} {session.minutes === null ? "" : `· ${num(session.minutes)} ${t("goals.session.minutes")}`}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </Card>
  {/if}
</div>
