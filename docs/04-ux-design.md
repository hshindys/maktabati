# 04 — UX Design

**Project:** Maktabati / مكتبتي · **Last updated:** 2026-10-18
**Design principle:** *calm, bookish, and unmistakably Arabic-first* — a warm paper surface in
light mode, a deep ink surface in dark mode, and typography that treats Arabic as the headline
script, not a fallback.

---

## 1. Information architecture

```
App shell
├── Title bar (custom) ── window controls · global search entry · language · theme · lock
├── Sidebar (collapsible, remembers width)
│   ├── Dashboard            لوحة التحكم
│   ├── Library              المكتبة
│   ├── Currently reading    أقرأ الآن
│   ├── Shelves ▾ (static + smart, pinned, favorites)
│   ├── Notes                الملاحظات
│   ├── Quotes               الاقتباسات
│   ├── Authors              المؤلفون
│   ├── Publishers           الناشرون
│   ├── Categories           التصنيفات
│   ├── Lending              الإعارات
│   ├── Goals                الأهداف
│   ├── Import / Export      استيراد / تصدير
│   └── Settings             الإعدادات
├── Main region (routed)
│   ├── Dashboard
│   ├── Library browser (grid / list / table)  + filter bar + shelf context
│   ├── Book page (7 tabs) ⭐ the most important screen
│   ├── Reader (EPUB/PDF) — full-screen overlay
│   ├── Management screens (authors/publishers/categories/tags/languages/criteria)
│   ├── Import wizard · Backup · Settings
│   └── Dev component gallery (debug builds only)
├── Command palette (Ctrl/Cmd+K) — overlay
├── Add-book wizard — modal, multi-step
└── Toasts / bottom status strip
```

---

## 2. Screen inventory

| # | Screen | Route | Purpose | Key elements |
|---|---|---|---|---|
| 1 | **Onboarding (first run)** | `/onboarding` | Get to 1 book in < 60 s | language pick, theme pick, "add your first book", "import a file", "start empty" |
| 2 | **Dashboard** | `/` | Understand my reading | KPI cards, breakdowns, charts, heatmap, widgets |
| 3 | **Library** | `/library` | Browse & find | grid/list/table toggle, filter bar, sort, shelf context, virtualized results |
| 4 | **Book page** | `/book/:id` | ⭐ Everything about one book | 7 tabs (Overview, Notes, Quotes, Reading log, Reviews, Files, Related) |
| 5 | **Reader** | `/read/:fileId` | Read in-app | pdf.js / foliate-js, themes, fonts, bookmarks, highlights |
| 6 | **Add-book wizard** | modal | Capture a book fast | 4 steps + duplicate check + source preview |
| 7 | **Authors** | `/authors` | Manage people | table, merge duplicates, per-author aggregate score |
| 8 | **Publishers** | `/publishers` | Manage publishers | table, merge, aggregate score |
| 9 | **Categories & tags** | `/taxonomy` | Manage tree + tags | tree editor, drag-reorder, bilingual names |
| 10 | **Languages** | `/settings/languages` | Manage language list | add/remove/reorder, direction |
| 11 | **Rating criteria** | `/settings/criteria` | Rubric editor | CRUD, weight, reorder, preview of its effect |
| 12 | **Shelves** | `/shelf/:id` | Static or smart shelf | same browser, shelf-specific actions |
| 13 | **Notes workspace** | `/notes`, `/notes/:pageId` | Global notes | page tree, backlinks, search |
| 14 | **Quotes** | `/quotes` | All highlights | filter by book/colour/page, jump to reader position |
| 15 | **Lending** | `/lending` | Who has my books | open/overdue/returned, reminders |
| 16 | **Goals** | `/goals` | Yearly/monthly targets | progress bars, edit targets |
| 17 | **Import/Export** | `/transfer` | Migrate | Goodreads/Calibre/CSV/JSON, column-mapping step, folder scan, export |
| 18 | **Backup & restore** | `/settings/backup` | Safety | create, verify, restore, auto-backup schedule |
| 19 | **Settings** | `/settings` | Preferences | general, appearance, language/digits/dates, search, metadata, security, logs |
| 20 | **Command palette** | `Ctrl/Cmd+K` | Jump anywhere | unified search + commands |
| 21 | **Component gallery** | `#/dev/gallery` (debug) | Design-system QA | every component in every state, light/dark/RTL |
| 22 | **Log viewer / diagnostics** | `/settings/diagnostics` | Support | tail log, export bundle |

---

## 3. Navigation map

```mermaid
flowchart TD
    ONB[Onboarding] --> D[Dashboard]
    D --> LIB[Library]
    D --> CR[Currently reading]
    D --> GOAL[Goals]
    LIB --> BOOK[Book page]
    BOOK --> TAB_O[Overview]
    BOOK --> TAB_N[Notes]
    BOOK --> TAB_Q[Quotes]
    BOOK --> TAB_L[Reading log]
    BOOK --> TAB_R[Reviews & rubric]
    BOOK --> TAB_F[Files]
    BOOK --> TAB_X[Related]
    TAB_F --> READ[Reader]
    READ -->|highlight| TAB_Q
    READ -->|position saved| TAB_L
    TAB_N --> NPAGE[Note page]
    NPAGE -->|[[link]]| TAB_O
    NPAGE -->|[[link]]| NPAGE
    LIB -.->|smart shelf| SH[Shelf]
    CP[Command palette] --> LIB
    CP --> BOOK
    CP --> NPAGE
    CP --> AUTH[Authors]
    SIDE[Sidebar] --> MGMT[Management screens]
    SIDE --> TRANS[Import/Export]
    SIDE --> SET[Settings]
    SET --> DIAG[Diagnostics / logs]
    WIZ[Add-book wizard] -->|saved| BOOK
```

**Navigation rules**

- The sidebar never disappears in the main app (collapses to icons ≤ 1100 px).
- The book page is a **destination**, not a modal — it is bookmarkable and back/forward works.
- The reader is a **full-screen overlay** on top of the book page so closing returns to the exact tab.
- Command palette is global and works from any route.
- Browser-style back (Alt+←/→) works everywhere.

---

## 4. Wireframes

### 4.1 Shell + Dashboard

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ ── ▬▬▬   مكتبتي      [🔍 ابحث في كل شيء  Ctrl+K ]   ع   ☀  🔒      ─ □ ✕ │  ← custom title bar
├───────────────┬──────────────────────────────────────────────────────────────┤
│  لوحة التحكم  │  مرحباً حاتم          السبت ١٥ ربيع الآخر ١٤٤٨ · 2026-10-18│
│  المكتبة      │ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ │
│  أقرأ الآن    │ │ 1,284   │ │   47    │ │  18,302 │ │  6-day  │ │  38/50  │ │
│  الرفوف     ▾ │ │ كتاب    │ │ أقرأها │ │  صفحة   │ │ متتالية │ │ هدف ٢٠٢٦│ │
│   • كل الكتب  │ │  إجمالي │ │  الآن   │ │ هذا العام│ │         │ │  ▓▓▓▓░  │ │
│   • قراءة ٢٠٢٦│ └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘ │
│   • كلاسيكيات │                                                              │
│  الملاحظات    │  ┌─ أقرأ الآن ─────────────┐  ┌─ أضيف مؤخراً ─────────────┐ │
│  الاقتباسات   │  │ ▓▓▓▓▓▓▓░░ 73%           │  │ ▭ ▭ ▭ ▭ ▭  (cover strip)  │ │
│  المؤلفون     │  │ مقدمة في التاريخ الإسلامي│  │                            │ │
│  الناشرون     │  │ [متابعة القراءة]         │  │                            │ │
│  التصنيفات    │  └──────────────────────────┘  └────────────────────────────┘ │
│  الإعارات     │                                                              │
│  الأهداف      │  ┌─ أنهيت هذا الشهر (12) ─┐  ┌─ الأعلى تقييماً ──────────┐ │
│               │  │  ▮▮▮▮▮▮▮▮▮▮ books/month │  │ ★★★★½ …                    │ │
├───────────────┤  └─────────────────────────┘  └────────────────────────────┘ │
│ ⚙ الإعدادات   │  ┌─ خريطة النشاط (heatmap) ────────────────────────────────┐ │
└───────────────┘  │ ░░▒▒▓▓██ ░░▒▒▓▓ … (53×7)                               │ │
                   └──────────────────────────────────────────────────────────┘ │
```

### 4.2 Library browser

```
┌──────────────┬────────────────────────────────────────────────────────────────┐
│ (sidebar)    │ المكتبة · كل الكتب            [شبكة ▾] [الأحدث ↓] [+ كتاب جديد]│
│              │ ┌ فلترة ─────────────────────────────────────────────────────┐ │
│              │ │ الحالة: [قراءة✓][أنهيت✓] اللغة:[عربي] التصنيف:[…] ★≥٤ …  │ │
│              │ │ نشط: [قراءة ×] [أنهيت ×]      [حفظ كرف] [مسح]            │ │
│              │ └────────────────────────────────────────────────────────────┘ │
│              │ 1,284 كتاب · 340 نتيجة                                       │
│              │ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐   ← virtualized cover wall │
│              │ │ ▭  │ │ ▭  │ │ ▭  │ │ ▭  │ │ ▭  │                            │
│              │ │مقدمة│ │الأسود│ │ …  │ │    │ │    │  hover: lift + actions  │
│              │ │في   │ │يت   │ │    │ │    │ │    │  dir="auto" titles       │
│              │ │التار│ │يم   │ │    │ │    │ │    │                            │
│              │ └────┘ └────┘ └────┘ └────┘ └────┘                            │
│              │  (list view: rows with cover · title · author · status · ★)  │
│              │  (table view: sortable columns incl. custom #columns)        │
└──────────────┴────────────────────────────────────────────────────────────────┘
```

### 4.3 Book page (the most important screen)

```
┌──────────────┬────────────────────────────────────────────────────────────────┐
│ (sidebar)    │ ← رجوع                                                         │
│              │ ┌────────┐  مقدمة في الحضارة الإسلامية                         │
│              │ │        │  أحمد فكري · ترجمة: محمد عبدالله      [عربي ⟲]     │
│              │ │ COVER  │  دار المعارف · ١٩٧٨ · ٤٢٤ صفحة · طبعة أولى         │
│              │ │ 320px  │  ▓▓▓▓▓▓░░░░ 62%  ·  صفحة ٢٦٣ من ٤٢٤               │
│              │ │        │  ★★★★☆ (٤)   #تاريخ #حضارة #إسلام                │
│              │ └────────┘  [▶ متابعة القراءة] [✎ ملاحظات] [⋯]                │
│              │ ┌────────┬────────┬─────────┬──────────┬─────────┬───────┬────┐│
│              │ │نظرة   │ملاحظات │اقتباسات │سجل       │تقييم    │ملفات  │مرتبط││
│              │ │عامة ①  │ ②      │ ③       │القراءة ④ │ ⑤       │ ⑥     │ ⑦  ││
│              │ ├────────┴────────┴─────────┴──────────┴─────────┴───────┴────┤│
│              │ │ (tab content)                                               ││
│              │ └──────────────────────────────────────────────────────────────┘│
└──────────────┴────────────────────────────────────────────────────────────────┘
```

**Tab ① Overview** — description, full metadata table (two-column, `dir="auto"` values), custom
columns, physical location, acquisition, status selector, quick rating, activity feed.

**Tab ② Notes (Notion-style)**

```
│ ملاحظات الكتاب · ٤ صفحات فرعية            [+ صفحة فرعية] [قالب ▾] [تصدير .md] │
│ ┌ شجرة ───────────┐ ┌───────────────────────────────────────────────────────┐ │
│ │ 📄 ملاحظاتي      │ │ فصل ١: السياق Historical context                    │ │
│ │  ├ ملخص الفصول   │ │ ════════════════════════════════════════            │ │
│ │  ├ أفكار        │ │ • نقطة أولى                                    ⋮    │ │
│ │  ├ أسئلة        │ │ • «نص مقتبس»                                   ⋮    │ │
│ │  └ مفردات       │ │   > اقتباس بخط عربي جميل                         │ │
│ │ 📄 أفكار عامة   │ │ ▸ قابل للتوسيع (toggle)                             │ │
│ │ 🔗 روابط عاكسة:  │ │ [ ] مهمة لم تُنجز                                  │ │
│ │  ← من "ملخص الفصول"│ │ /  ← slash menu opens here                        │ │
│ └─────────────────┘ └───────────────────────────────────────────────────────┘ │
│ autosaved: تم الحفظ · منذ لحظات                                              │
```

**Tab ⑤ Reviews & rubric**

```
│ التقييم العام   ★★★★☆  (نصف نجمة)                          [تعديل]           │
│ معاييري التقييم                                                          │
│   عمق المحتوى        ★★★★☆  ──────●──────                              │
│   أسلوب الكتابة      ★★★★★  ─────────●──                              │
│   التدقيق والتحرير   ★★★☆☆  ────●────────                              │
│   جودة الطباعة       ★★★★☆  ──────●──────                              │
│   جودة مقابل السعر   ★★★½☆  ────────●────                              │
│   (＋ إضافة معيار)                                                        │
│ متوسط الناشر في معاييري: ٤٫١ / ٥  (٢٣ كتاباً)   [عرض التفاصيل]            │
│ متوسط المؤلف: ٤٫٤ / ٥ (١١ كتاباً)                                        │
│ مراجعتي ────────────── [نص المراجعة…]                    [تاريخ]           │
│ تعليقات: ١٢ مارس ٢٠٢٦ "أعود إليها كل عام…"                              │
```

**Tab ⑥ Files**

```
│ ملفات الكتاب                          [＋ إرفاق] [فحص مجلد]               │
│ ┌──────────────────────────────────────────────────────────────────────┐ │
│ │ 📄 book.epub   2.4 MB  SHA-256 3f9a…  آخر قراءة: ٦٢٪   [فتح] [⋯]     │ │
│ │ 📄 scan.pdf    18 MB    مرجعي (خارج المكتبة)                [فتح] [⋯] │ │
│ └──────────────────────────────────────────────────────────────────────┘ │
```

**Reader overlay**

```
│ ← رجوع   مقدمة…  الفصل ٣/١٢         [أ−][أ+]  [خط ▾]  [سمة ▾: فاتح/داكن/سيبيا] │
│ ┌────────────────────────────────────────────────────────────────────────────┐ │
│ │                                                                            │ │
│ │                    ← paginated or scroll, dir from book language →         │ │
│ │                        نص عربي منسّق بشكل صحيح                              │ │
│ │                                                                            │ │
│ └────────────────────────────────────────────────────────────────────────────┘ │
│ ▓▓▓▓▓▓░░ 62% · صفحة ٢٦٣      [🔖 علامة]  [🔍 بحث داخل الكتاب]               │
```

### 4.4 Command palette

```
              ┌──────────────────────────────────────────────┐
              │  🔍  تاريخ مصر                          ⏎  │
              ├──────────────────────────────────────────────┤
              │  📚 كتب      مصر في عصر محمد علي   ★★★★☆    │
              │              تاريخ الحضارة المصرية  ★★★★☆    │
              │  👤 مؤلفون   عبد الرحمن الرافعي             │
              │  🏢 ناشرون   دار المعارف                     │
              │  📝 ملاحظات  …الفصل الثاني: الأسباب          │
              │  💬 اقتباسات  …كتاب التاريخ الإسلامي          │
              ├──────────────────────────────────────────────┤
              │  ⚡ أوامر   [+ كتاب جديد] [استيراد CSV]      │
              └──────────────────────────────────────────────┘
```

### 4.5 Add-book wizard

```
┌─ إضافة كتاب ─────────────────────────────── ─ □ ✕ ─┐
│  ① المصدر  ② البيانات  ③ التصنيفات  ④ المراجعة      │
│ ───────────────────────────────────────────────────  │
│  ISBN: [978-977-09-1234-٥ ] [🔎 بحث]                 │
│  ┌ مصدر: Open Library ─────────────────────────────┐ │
│  │ العنوان: مقدمة في الحضارة الإسلامية              │ │
│  │ المؤلف: [أحمد فكري        ] [+ إضافة مؤلف]       │ │
│  │ الناشر: [دار المعارف      ] [+ إضافة ناشر]       │ │
│  │ السنة: [1978]  الصفحات: [424]  اللغة: [عربي ▾]  │ │
│  │ التصنيف: [تاريخ ▾] [+ تصنيف فرعي]               │ │
│  └──────────────────────────────────────────────────┘ │
│  ⚠ قد يوجد كتاب متطابق: «مقدمة…» لـ أحمد فكري        │
│     [عرض التطابق]  [حفظ رغم ذلك]                      │
│                                          [إلغاء] [التالي →] │
└──────────────────────────────────────────────────────┘
```

---

## 5. Design system

### 5.1 Design tokens (CSS custom properties; single source of truth)

```css
:root {
  /* ── Light: "warm paper" ─────────────────────────────── */
  --surface-0: #FBF8F1;   /* app background — warm paper      */
  --surface-1: #FFFFFF;   /* cards                             */
  --surface-2: #F4EFE4;   /* subtle wells, table stripes       */
  --surface-3: #EAE3D5;   /* hover                             */
  --border-1:  #E3DBCB;
  --border-2:  #CFC4AE;
  --text-1:    #2A2621;   /* primary ink                       */
  --text-2:    #6B6153;   /* secondary                         */
  --text-3:    #948A79;   /* muted / placeholders              */

  /* ── Dark: "deep ink" ────────────────────────────────── */
  /* --surface-0: #12110F; --surface-1: #1A1917; --surface-2: #22211E;
     --surface-3: #2C2A26; --border-1: #302E2A; --border-2: #423F39;
     --text-1: #EDE7DB; --text-2: #ADA697; --text-3: #7E7768;         */

  /* ── Sepia (reading) ─────────────────────────────────── */
  /* --surface-0: #F3E7CF; --surface-1: #F7EEDC; --text-1: #463B2A;   */

  /* ── Accent (user-selectable; default "bookbinding green") ── */
  --accent:        #2F6B4F;
  --accent-hover:  #27593F;
  --accent-fg:     #FFFFFF;
  --accent-soft:   #E3EFE8;
  /* accent palette: green (default) · indigo #3B4CCA · plum #7A3B69
     · terracotta #B4552F · ocean #1F6A8A · gold #8A6A1F               */

  /* ── Semantic ────────────────────────────────────────── */
  --success: #2F7A4F;  --success-soft: #E2F2E9;
  --warning: #9A6B12;  --warning-soft: #FBF0D8;
  --danger:  #A93226;  --danger-soft:  #FBE7E5;
  --info:    #2A5F8F;  --info-soft:    #E4EEF7;

  /* ── Rating ──────────────────────────────────────────── */
  --star-filled: #C99A17;  --star-empty: #D8D0BE;

  /* ── Type scale (1.200 minor third, 16px base) ───────── */
  --fs-xs: 12px;  --fs-sm: 13.5px; --fs-base: 15px; --fs-md: 17px;
  --fs-lg: 20px;  --fs-xl: 24px;   --fs-2xl: 30px;  --fs-3xl: 38px;
  --lh-tight: 1.25; --lh-normal: 1.6;  /* 1.75–1.9 for Arabic body */

  /* ── Spacing (4px base) ──────────────────────────────── */
  --sp-1: 4px;  --sp-2: 8px;  --sp-3: 12px; --sp-4: 16px; --sp-5: 24px;
  --sp-6: 32px; --sp-7: 48px; --sp-8: 64px;

  /* ── Radii ───────────────────────────────────────────── */
  --r-sm: 6px; --r-md: 10px; --r-lg: 14px; --r-xl: 20px; --r-full: 999px;

  /* ── Elevation (soft, warm-tinted) ───────────────────── */
  --sh-1: 0 1px 2px rgba(42,38,33,.06);
  --sh-2: 0 2px 8px rgba(42,38,33,.10);
  --sh-3: 0 8px 24px rgba(42,38,33,.14);
  --sh-lift: 0 14px 34px rgba(42,38,33,.20);   /* cover hover */

  /* ── Motion ──────────────────────────────────────────── */
  --dur-fast: 120ms; --dur-base: 200ms; --dur-slow: 320ms;
  --ease: cubic-bezier(.2,.7,.3,1);
  --stagger: 30ms;
}
```

### 5.2 Typography

| Role | Latin | Arabic | Notes |
|---|---|---|---|
| UI | **Inter** | **Cairo** (primary), **Tajawal** (secondary/numeric) | `font-family: Inter, Cairo, …` so mixed strings pick the right face per script |
| Long-form reading / notes body | Inter | **IBM Plex Sans Arabic** | excellent screen legibility |
| Reader / quotes | Inter | **Amiri** (Naskh) | bookish, classical; user-switchable to Cairo/Plex |
| Numerals | Inter | per **digit setting** | `font-variant-numeric: tabular-nums` for tables |

All fonts are **bundled locally** (OFL) — no network fetch, works offline.
Body line-height in Arabic is **1.75–1.9** (looser than Latin's 1.6) — Arabic diacritics and
descenders need the room.

### 5.3 Iconography

**Lucide**, 1.5–1.75 px stroke, 16/20/24 px sizes. Icons inherit `currentColor`. Never used
without an accessible label (visible or `aria-label`).

### 5.4 Components catalog (Bits UI + our styles)

`Button` (primary/secondary/ghost/danger, sizes sm/md/lg, loading) · `IconButton` · `Input` ·
`Textarea` · `Select` · `Combobox` (autocomplete for authors/publishers/categories) · `Checkbox` ·
`Radio` · `Switch` · `Slider` (rubric) · `Stars` (half-star, keyboard ↑↓) · `Tabs` · `Accordion` ·
`Dialog` · `Drawer` · `Popover` · `Tooltip` · `DropdownMenu` · `ContextMenu` · `Toast` ·
`Breadcrumb` · `Pagination` · `Badge`/`Chip` · `Card` · `EmptyState` · `Skeleton` · `ProgressBar` ·
`VirtualGrid` · `VirtualTable` · `FilterBar` · `CommandPalette` · `DropZone` · `Tree` · `SplitPane`.

Every component ships **all** states: default, hover, focus-visible, active, disabled, loading,
error — in light, dark and sepia, in RTL and LTR. The dev gallery renders them in a grid precisely
so this is verifiable.

### 5.5 Micro-interactions

- **Skeletons** for every async surface (no spinners on first paint).
- **Cover hover:** lift (`--sh-lift`, `translateY(-4px)`), reveal action row, `160ms --ease`.
- **Toasts:** bottom-inline-start, slide+fade in `200ms`, auto-dismiss 4 s, `role="status"`.
- **Empty states:** illustrated (inline SVG), a heading, one sentence, one primary action.
- **Drag & drop:** covers/files show an accent dashed outline + scale(1.01); drop = instant action.
- **Route transition:** opacity 120 ms + 4 px translate — never longer, or it feels slow.
- **Staggered list entrance:** max 6 rows × 30 ms; disabled for virtualized lists (perf).
- `prefers-reduced-motion` → all transitions collapse to ≤ 1 ms opacity.

---

## 6. Themes

| Theme | Surface | Purpose |
|---|---|---|
| **Light — warm paper** (default) | `#FBF8F1` paper, ink text | all-day app use |
| **Dark — deep ink** | `#12110F`, high-contrast text | night use |
| **Sepia (reading)** | `#F3E7CF` | reading surfaces & quotes |
| **System** | follows OS preference | default option |

Plus a **user-selectable accent** (6 presets, see tokens) applied to buttons, links, focus rings,
progress bars, active nav, and the heatmap ramp (accent-tinted, 5 steps).

Themes switch **instantly** with no reload; `data-theme` + `data-accent` on `<html>`.

---

## 7. RTL / LTR rules (non-negotiable)

1. `<html lang dir>` is set from the locale: `ar → dir="rtl"`, `en → dir="ltr"`, updated live
   with **no restart**.
2. **All layout uses CSS logical properties**: `margin-inline-start/end`, `padding-inline`,
   `inset-inline-start/end`, `border-start-start-radius`, `text-align: start/end`.
   Hard-coded `left/right` is a lint failure (a CI style check greps for `margin-left`, `padding-right`,
   `text-align: right`, etc. in `src/`).
3. **Flexbox/Grid follow writing direction automatically** — never use `row-reverse` to fake RTL.
4. **Per-field direction:** any rendered user-authored string gets
   `<span dir="auto" style="unicode-bidi: isolate">`. This is the default on cards, table cells,
   list rows, note blocks and tooltips. An Arabic title + English author on the same card must both
   read correctly.
5. **Numbers, ISBNs, URLs, code** render LTR inside RTL context (`dir="ltr"` + `unicode-bidi: isolate`).
6. **Icons that imply direction** (back/forward/chevrons) are mirrored in RTL via `transform: scaleX(-1)`
   *except* universal symbols (play, search, plus).
7. **Progress bars, sliders, scrollbars** follow logical direction (`direction: inherit`).
8. **Heatmap / calendar** starts on the correct weekday edge for the locale.
9. **Keyboard shortcut display** mirrors modifier order for RTL (`Ctrl+K` stays, but glyphs
   representing arrows/chevrons mirror).
10. Dates: formatted per locale with an explicit calendar (Gregorian/Hijri), and **digits per the
    digit setting** applied at format time only.

---

## 8. Accessibility

| Requirement | Implementation |
|---|---|
| Contrast | WCAG **AA** for all text (≥ 4.5:1 body, ≥ 3:1 large/UI). Verified by an automated contrast test over the token matrix in CI. |
| Focus | Visible `:focus-visible` ring everywhere: `2px solid var(--accent)` + `2px offset`; never `outline: none` without a replacement. |
| Keyboard | Every action reachable; documented shortcut map (§9); roving tabindex in grids/menus/trees (Bits UI behaviour). |
| ARIA | Real elements first (`<button>`, `<nav>`, `<dialog>`); `aria-label` for icon-only controls; `aria-live="polite"` for toasts and async results; `aria-busy` on loading regions. |
| Grid | `role="grid"`/`gridcell` with correct `aria-rowindex` even when virtualized; arrow-key navigation + Home/End/PageUp/PageDown. |
| Reader | Selection, zoom, and font size all work with the keyboard; text stays real (selectable) text, never canvas-only. |
| Motion | `prefers-reduced-motion` respected. |
| Screen readers | Tested against NVDA/Orca basics: language attribute, heading order, form labels + `aria-describedby` errors. |
| Zoom | Layout holds at 200% browser zoom; window resizes down to **900 px** without overlap. |

---

## 9. Keyboard shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl/Cmd + K` | Command palette (global search + commands) |
| `Ctrl/Cmd + N` | New book (opens wizard) |
| `Ctrl/Cmd + F` | Search within current screen |
| `Ctrl/Cmd + Shift + F` / `/` | Focus global filter bar |
| `Ctrl/Cmd + P` | Quick switch: Library / Dashboard / Notes |
| `G` then `D` / `L` / `B` | Go to Dashboard / Library / Book (vim-style sequences) |
| `Alt + ←` / `Alt + →` | Back / forward |
| `Ctrl/Cmd + ,` | Settings |
| `Ctrl/Cmd + Shift + B` | Create backup |
| `Ctrl/Cmd + L` | Lock app (if app lock enabled) |
| `Ctrl/Cmd + Enter` | Save / confirm focused form |
| `Esc` | Close modal, palette, drawer, reader chrome |
| `?` | Shortcut help overlay |
| **In library** | `↑↓←→` navigate · `Enter` open · `Space` quick preview · `E` edit · `Shift+↑↓` multi-select · `Ctrl+C` copy title |
| **In book page** | `1..7` switch tab |
| **In reader** | `←/→` page (direction-aware) · `+/-` zoom · `T` theme cycle · `B` bookmark · `H` highlight selection · `Esc` close |
| **In editor** | `/` slash menu · `[[` link · `Ctrl+Z/Shift+Ctrl+Z` undo/redo · `Ctrl+B/I/U` · `Ctrl+Shift+1..3` headings |

All of the above appear in `docs/shortcuts.md` and in the in-app `?` overlay, generated from one
shared definition so they never drift.

---

## 10. Responsive-within-desktop rules

| Width | Behaviour |
|---|---|
| ≥ 1440 px | Sidebar expanded, 5–6 columns of covers, 3-column book page header |
| 1100–1440 px | Sidebar icons-only below 1200, 4 columns |
| **900–1100 px** | Sidebar drawer, 3 columns, book page header stacks, tabs scroll horizontally |
| < 900 px | Not supported (documented); app enforces a min window width of 900 px |

Window state (size, position, maximized, sidebar width, last view mode) persists via
`tauri-plugin-window-state` + a `app_settings` row.

---

## 11. Visual QA

- **`#/dev/gallery`** (debug builds only): every component × every state × theme × direction.
- **Snapshot screenshots** of key screens (dashboard, library grid, book page, reader, palette) in
  both themes and both directions — compared in CI where a headless webview is available; otherwise
  run as a documented pre-release checklist.
- **Contrast test** over the token matrix runs in CI.
- **RTL lint** (grep for physical CSS properties) runs in CI.
