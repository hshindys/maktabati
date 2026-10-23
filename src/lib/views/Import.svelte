<script lang="ts">
  import { commands } from "@/bindings";
  import { createBook } from "../api";

  type Row = any;

  let csvText = $state("");
  let rows = $state<Row[]>([]);
  let warnings = $state<string[]>([]);
  let parsing = $state(false);
  let committing = $state(false);
  let progress = $state(0);
  let error = $state<string | null>(null);
  let done = $state(0);

  async function onFile(e: Event) {
    const input = e.target as HTMLInputElement;
    const f = input.files?.[0];
    if (!f) return;
    csvText = await f.text();
  }

  async function parse() {
    error = null; warnings = []; rows = [];
    parsing = true;
    try {
      const res = await commands.goodreadsParse(csvText);
      if (res.status === "ok") { rows = res.data; }
      else { error = res.error?.message ?? "parse failed"; }
    } catch (e) { error = String(e); }
    finally { parsing = false; }
  }

  async function commit() {
    committing = true; progress = 0; done = 0;
    for (const r of rows) {
      try {
        await createBook({
          title: r.title,
          subtitle: null, original_title: null, language_code: null,
          isbn10: r.isbn?.length === 10 ? r.isbn : null,
          isbn13: r.isbn?.length === 13 ? r.isbn : null,
          edition: null, publication_year: r.year, page_count: r.pages,
          format: null, description: null, series_id: null, series_order: null,
          room: null, shelf: null, box_name: null, acquired_date: null,
          acquired_from: "goodreads", price_minor: null, price_currency: null,
          cover_path: null, cover_source: null, source: "goodreads",
          external_ids: null, authors: [], publisher_id: null,
          categories: [], tags: [],
        });
        done++;
      } catch (e) { /* skip */ }
      progress = Math.round(((done) / rows.length) * 100);
    }
    committing = false;
  }
</script>

<div class="p-6 space-y-4">
  <h1 class="text-xl font-semibold">استيراد من Goodreads</h1>
  <p class="text-sm text-ink-2">صدّر CSV من Goodreads (My Books → Import/Export → Export Library) ثم ارفعه هنا.</p>

  <input type="file" accept=".csv,text/csv" onchange={onFile} />

  <textarea class="w-full h-32 border rounded p-2 font-mono text-xs" bind:value={csvText} placeholder="أو الصق محتوى CSV هنا..."></textarea>

  <div class="flex gap-2">
    <button class="btn" onclick={parse} disabled={parsing || !csvText.trim()}>
      {parsing ? "جارٍ التحليل..." : "تحليل"}
    </button>
    {#if rows.length}
      <button class="btn btn-primary" onclick={commit} disabled={committing}>
        {committing ? `استيراد ${done}/${rows.length}...` : `استيراد ${rows.length} كتاب`}
      </button>
    {/if}
  </div>

  {#if error}<p class="text-red-500 text-sm">{error}</p>{/if}
  {#if warnings.length}
    <ul class="text-amber-600 text-xs">{#each warnings as w}<li>{w}</li>{/each}</ul>
  {/if}

  {#if rows.length}
    <div class="text-sm text-ink-2">معاينة: {rows.length} كتاب</div>
    <table class="w-full text-sm border-collapse">
      <thead><tr class="border-b"><th class="p-1 text-start">العنوان</th><th class="p-1 text-start">المؤلف</th><th class="p-1 text-start">ISBN</th><th class="p-1 text-start">الحالة</th><th class="p-1 text-start">التقييم</th></tr></thead>
      <tbody>
        {#each rows.slice(0, 50) as r}
          <tr class="border-b"><td class="p-1">{r.title}</td><td class="p-1">{r.author}</td><td class="p-1">{r.isbn ?? ""}</td><td class="p-1">{r.status}</td><td class="p-1">{r.rating ?? ""}</td></tr>
        {/each}
      </tbody>
    </table>
    {#if rows.length > 50}<p class="text-xs text-ink-2">... و {rows.length - 50} more</p>{/if}
  {/if}
</div>
