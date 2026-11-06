<script lang="ts">
  import { onMount } from "svelte";

  import { createLoan, deleteLoan, listBooks, listLoans, returnLoan } from "../api";
  import Button from "../components/Button.svelte";
  import Card from "../components/Card.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import { locale, translatorFor } from "../i18n";
  import { messageFor } from "../errors";
  import { formatNumber, type DigitStyle } from "../format";
  import { preferences } from "../settings";
  import { toast } from "../toast";
  import type { AppErrorDto, Book, Loan, LoanInput } from "@/bindings";

  let t = $derived(translatorFor($locale));
  let digits = $derived<DigitStyle>($preferences.digits);
  const num = (value: number) => formatNumber(value, { digits });
  const todayKey = () => {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  };

  let loans = $state<Loan[]>([]);
  let books = $state<Book[]>([]);
  let loading = $state(true);
  let saving = $state(false);
  let loadError = $state<AppErrorDto | null>(null);
  let showReturned = $state(false);
  let bookId = $state("");
  let borrower = $state("");
  let phone = $state("");
  let borrowedAt = $state(todayKey());
  let dueDate = $state("");
  let notes = $state("");

  let activeCount = $derived(loans.filter((loan) => loan.returned_at === null).length);
  let visibleLoans = $derived(loans.filter((loan) => showReturned || loan.returned_at === null));

  async function load(): Promise<void> {
    loading = true;
    const [loanResult, bookResult] = await Promise.all([listLoans(), listBooks()]);
    const error = loanResult.status === "error" && loanResult.error.code !== "shell.unavailable"
      ? loanResult.error
      : bookResult.status === "error" && bookResult.error.code !== "shell.unavailable"
        ? bookResult.error
        : null;
    if (error !== null) {
      loadError = error;
    } else {
      loadError = null;
      loans = loanResult.status === "ok" ? loanResult.data : [];
      books = bookResult.status === "ok" ? bookResult.data : [];
      if (bookId === "" && books.length > 0) bookId = books[0].id;
    }
    loading = false;
  }

  async function submitLoan(): Promise<void> {
    if (saving) return;
    if (bookId === "" || borrower.trim() === "" || !/^\d{4}-\d{2}-\d{2}$/.test(borrowedAt) ||
        (dueDate !== "" && !/^\d{4}-\d{2}-\d{2}$/.test(dueDate))) {
      toast.error(t("lending.validation"));
      return;
    }
    const input: LoanInput = {
      book_id: bookId,
      borrower: borrower.trim(),
      phone: phone.trim() || null,
      borrowed_at: borrowedAt,
      due_date: dueDate || null,
      notes: notes.trim() || null,
    };
    saving = true;
    try {
      const result = await createLoan(input);
      if (result.status === "ok") {
        borrower = "";
        phone = "";
        dueDate = "";
        notes = "";
        toast.success(t("lending.saved"));
        await load();
      } else {
        toast.error(messageFor(result.error, t));
      }
    } finally {
      saving = false;
    }
  }

  async function markReturned(loan: Loan): Promise<void> {
    const result = await returnLoan(loan.id, todayKey());
    if (result.status === "ok") {
      loans = loans.map((item) => item.id === loan.id ? { ...item, returned_at: todayKey() } : item);
      toast.success(t("lending.returned"));
    } else {
      toast.error(messageFor(result.error, t));
    }
  }

  async function remove(loan: Loan): Promise<void> {
    const result = await deleteLoan(loan.id);
    if (result.status === "ok") {
      loans = loans.filter((item) => item.id !== loan.id);
      toast.success(t("lending.deleted"));
    } else {
      toast.error(messageFor(result.error, t));
    }
  }

  function isOverdue(loan: Loan): boolean {
    return loan.returned_at === null && loan.due_date !== null && loan.due_date < todayKey();
  }

  onMount(() => void load());
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-[var(--sp-5)] px-[var(--sp-6)] py-[var(--sp-5)]">
  <header class="flex flex-wrap items-end justify-between gap-[var(--sp-3)]">
    <div>
      <h1 class="text-[var(--fs-2xl)] font-semibold">{t("lending.title")}</h1>
      <p class="mt-1 text-[var(--fs-base)] text-ink-2">{t("lending.intro")} · {t("lending.activeCount", { n: num(activeCount) })}</p>
    </div>
    <label class="flex items-center gap-2 text-[var(--fs-sm)] text-ink-2">
      <input type="checkbox" bind:checked={showReturned} />
      {t("lending.showReturned")}
    </label>
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
      <h2 class="mb-[var(--sp-4)] text-[var(--fs-lg)] font-semibold">{t("lending.newLoan")}</h2>
      {#if books.length === 0}
        <p class="text-[var(--fs-sm)] text-ink-3">{t("lending.noBooks")}</p>
      {:else}
        <form class="grid gap-[var(--sp-3)] sm:grid-cols-2 lg:grid-cols-3" onsubmit={(event) => { event.preventDefault(); void submitLoan(); }}>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("lending.book")}
            <select bind:value={bookId} class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1">
              {#each books as book (book.id)}<option value={book.id}>{book.title}</option>{/each}
            </select>
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("lending.borrower")}
            <input type="text" bind:value={borrower} required class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("lending.phone")}
            <input type="text" bind:value={phone} class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("lending.borrowedAt")}
            <input type="date" bind:value={borrowedAt} required dir="ltr" class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("lending.dueDate")}
            <input type="date" bind:value={dueDate} dir="ltr" class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <label class="flex flex-col gap-1 text-[var(--fs-sm)] text-ink-2">
            {t("lending.notes")}
            <input type="text" bind:value={notes} class="h-9 rounded-[var(--r-md)] border border-border-2 bg-surface-0 px-3 text-ink-1" />
          </label>
          <div class="flex items-end"><Button type="submit" variant="primary" loading={saving}>{t("lending.save")}</Button></div>
        </form>
      {/if}
    </Card>

    {#if visibleLoans.length === 0}
      <Card><EmptyState title={t("lending.empty.title")} body={t("lending.empty.body")} compact /></Card>
    {:else}
      <section class="grid gap-[var(--sp-3)]" aria-label={t("lending.list")}>
        {#each visibleLoans as loan (loan.id)}
          <Card>
            <div class="flex flex-wrap items-start justify-between gap-[var(--sp-4)]">
              <div class="min-w-0">
                <h2 class="truncate text-[var(--fs-lg)] font-semibold">{loan.book_title}</h2>
                <p class="mt-1 text-[var(--fs-base)] text-ink-2">{t("lending.borrowedBy", { name: loan.borrower })}</p>
                <p class="mt-1 text-[var(--fs-sm)] text-ink-3">
                  {t("lending.borrowedAt")}: {loan.borrowed_at}
                  {#if loan.due_date} · {t("lending.dueDate")}: {loan.due_date}{/if}
                  {#if loan.phone} · {loan.phone}{/if}
                </p>
                {#if loan.notes}<p class="mt-2 text-[var(--fs-sm)] text-ink-2">{loan.notes}</p>{/if}
                {#if loan.returned_at}
                  <span class="mt-2 inline-block text-[var(--fs-sm)] text-accent">{t("lending.returnedOn", { date: loan.returned_at })}</span>
                {:else if isOverdue(loan)}
                  <span class="mt-2 inline-block text-[var(--fs-sm)] font-medium text-danger">{t("lending.overdue")}</span>
                {:else}
                  <span class="mt-2 inline-block text-[var(--fs-sm)] text-ink-3">{t("lending.open")}</span>
                {/if}
              </div>
              <div class="flex gap-2">
                {#if loan.returned_at === null}
                  <Button size="sm" variant="primary" onclick={() => void markReturned(loan)}>{t("lending.markReturned")}</Button>
                {/if}
                <Button size="sm" variant="danger" onclick={() => void remove(loan)}>{t("common.delete")}</Button>
              </div>
            </div>
          </Card>
        {/each}
      </section>
    {/if}
  {/if}
</div>
