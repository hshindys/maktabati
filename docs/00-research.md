# Phase 0 — Research: Existing Book-Management Systems, Metadata Sources & Rust/Desktop Stack

**Project:** Maktabati / مكتبتي — Personal Library Manager
**Date:** 2026-10-18
**Status:** Complete (Phase 0 deliverable)

> **Method & honesty note.** Everything below was collected with live web search and live page
> fetches during Phase 0 on 2026-10-18. Where a claim could **not** be verified from a primary
> source, it is explicitly marked *"unverified"* or *"not found"*. No API, crate, licence or
> benchmark below is invented; the "Sources" section at the end lists every URL actually
> consulted. Where a search returned no primary documentation (e.g. an official API for a
> specific Arabic bookseller), that absence is itself recorded as a research finding.

---

## 1. Systems researched

### 1.1 Comparison table — library managers / catalogue tools

| System | Type / stack | Licence | Core model | Strengths | Weaknesses | UX highlights |
|---|---|---|---|---|---|---|
| **Calibre** (9.x) | Desktop, Python + Qt | GPL-3.0 | One SQLite DB = "library"; books as rows with *metadata fields*, **custom columns**, **formats** (multiple files per book), **virtual libraries** (saved filters), templates | Deepest metadata model of anything researched: custom columns of many types, template language, saved searches, virtual libraries, powerful query syntax (`tag:`, `author:`, `rating:>=3`, `vl:`, `#custom_col`, templates) | Desktop UI is dated; heavy; Python; single-library-at-a-time concept is confusing; file layout on disk is unusual | Book list (table) + search bar with an expressive query language; "Preferences → Add your own columns"; virtual libraries switch the whole view |
| **Calibre-Web** | Self-hosted web (Flask) | GPL-3.0 | Reads a *Calibre* library in place | Web UI over an existing Calibre DB; OPDS; send-to-Kindle; Kobo sync; multi-user | **Requires an existing Calibre library** — cannot build one from scratch; slows past ~20k books; bulk import is weak (independent mid-2026 benchmark: ~1,100 of 10,000 books in 91 min before being stopped) | Familiar shelf/folder browsing; admin-managed per-shelf Kobo sync toggles |
| **Calibre-Web-Automated (CWA)** | Fork of the above + Calibre bundled | GPL-3.0 (donation-funded) | Same, plus an ingest folder watcher | Creates a library if absent; watches an ingest folder and converts on the way in; KOReader progress sync; OIDC; Hardcover metadata; EPUB fixer | Same bulk-import weakness as parent; adds operational complexity | Auto-ingest removes the manual "add book" step entirely |
| **Kavita** | Self-hosted server (C#/.NET) | GPL-3.0 (core free; optional **Kavita+** $4/mo) | Folder + series-oriented library, metadata-managed | No Calibre dependency; ebooks+comics+manga+light novels; single/dual/webtoon reader modes; reading-progress sync (incl. KOReader); full-text search; smart filters; **least memory of any app up to 100k books** in the mid-2026 benchmark; fastest-but-one on import | No audiobooks; no Kobo reader sync; extra features behind optional subscription | Progressive-series navigation; per-library reading progress; "smart filters" as saved queries |
| **Komga** | Self-hosted (Kotlin/Spring Boot) | MIT | Comics/manga-first, folder + metadata | Mature (v1.28), rock-solid, OPDS v1.2+v2.0, Kobo sync, KOReader sync, strong metadata management, single JAR | Comics-first; ebooks secondary; not a personal catalogue (no ratings/rubrics/reviews of your own) | Clean, fast, server-grade browsing; two-way sync |
| **Audiobookshelf** | Self-hosted (Node) | GPL-3.0 | Audiobooks/podcasts first | Streams audio without transcoding; chapters; own Android/iOS apps; scrapes **Google Books + Open Library**; per-user RSS; send-to-Kindle | Ebook support is basic; **no OPDS** | Player-first UX; per-user progress |
| **BookLore → Grimmory** | Self-hosted successor | (see repo) | Books + audiobooks | Kobo + KOReader sync, OPDS, Kindle email, speaks Komga's API | New/renaming; licence has additional naming terms on BookOrbit | — |
| **BookOrbit** | Self-hosted (NestJS/Vue/PostgreSQL) | AGPL-3.0 + additional terms | Library + reading server | Two-way sync with Kobo + KOReader + web reader; pushes to Hardcover, The StoryGraph, Readwise; official iPhone app | Newest; licence requires a visible "Powered by BookOrbit" notice | Strongest *sync* story in the category |
| **Stump** | Self-hosted (Rust) | (see repo) | Server library | Kobo + KOReader sync, OIDC, EPUB highlights/annotations, OPDS v1.2+v2.0, React UI | Younger; smaller feature set | Clean React UI |
| **LibraryThing** | Web, cloud | Free since 2020 (site is proprietary) | Catalogue of *owned* books | Library-quality cataloguing, no book limit and no fee, barcode scanning via mobile apps, detailed charts/stats about your library | Cloud-only, no local-first desktop app, no local notes workspace | Catalogue-first, "home for your books" |
| **Libib** | Web + mobile | Free (Pro $9/mo or $99/yr) | Multi-media catalogue | Free up to **5,000 items / 100 collections**; ISBN+UPC scan; CSV export; **lending & patron management, custom fields, barcode management** are Pro | Lending and custom fields paywalled; cloud | Scan → auto-fill; collections |
| **CLZ Books / BookBuddy** | Paid mobile apps | Commercial | Owned-book catalogue | Condition fields (incl. dust-cover), custom fields; camera/Bluetooth scanner; CSV/PDF/HTML export | Paid; mobile-first | Scanner-first capture |
| **Goodreads** | Web, Amazon | Proprietary (free, ads) | Shelves + reviews + social | Largest community; Kindle integration; whole-star ratings; reading challenge | Whole stars only; ads; Amazon lock-in; no API for developers | Shelves, annual reading challenge |
| **The StoryGraph** | Web | Free (Plus $4.99/mo) | Shelves + **mood/pace/content-warnings** | **Half & quarter-star ratings**; mood/pace/genre tracking; detailed charts (books+pages per month, mood/pace pies, rating bars); reading journal; buddy reads with progress-locked spoilers; Goodreads/CSV import incl. DNF; **deliberately no generative AI** | Bugs; quieter social side; no Kindle auto-sync | Stats page is the best in class; DNF as a first-class shelf |
| **Hardcover** | Web | Free (Supporter $4.99/mo) | Shelves + statuses | **Four reading statuses incl. DNF**; half-star ratings; per-book privacy; custom lists; year-end "Wrapped"; **free public GraphQL API** | Catalogue smaller than Goodreads/Goodreads; smaller team → rough edges | Open API, "For You" feed |
| **Fable / Bookmory / Screvi** | Web/mobile | Freemium | Trackers | Fable: book clubs; Bookmory: reading sessions → streaks; Screvi: highlights + spaced repetition | Paid tiers; mobile-first | Session-based stats; SRS of highlights |

### 1.2 Comparison table — readers

| System | Stack | Formats | Reading UX | Notes / annotations | Themes & typography |
|---|---|---|---|---|---|
| **Readest** | **Next.js + Tauri v2** (desktop) | EPUB, PDF, MOBI, KF8/AZW3, FB2, CBZ, TXT, HTML, MD | Paginated **or** scrolling; split-screen "parallel read" up to 4 books (iPad); full-text search in book **and** across shelf; reading ruler; paragraph mode; speed reading; e-ink tuning | Highlights, bookmarks, structured notes; **the Notebook is a full writing workspace that autosaves and links back to the passage**; export annotations; copy a highlight with a deep link to the passage | Custom fonts, layout, margins, spacing, columns, theme mode + theme colours; code syntax highlighting; **vertical CJK**; PDFs blended into the reading theme with smart image inversion |
| **Foliate** (+ **foliate-js**) | GTK + GJS, WebKitGTK for rendering | EPUB (primary) | Two-page & scrolling; **paginator uses the same CSS multi-column strategy as Epub.js**; supports >2 columns; switching scrolled↔paginated **without reload**; anchors on `Range` objects with bisecting for accurate visible range; running heads/feet | Copy, highlight, find, dictionary, Wikipedia, translate | GTK themes |
| **Koodo Reader** | Electron (AGPL-3.0) | 15+ formats incl. EPUB, TXT, PDF, MOBI, AZW3, comics | Cross-platform + web; sync of library/progress/bookmarks/shelves/highlights/notes | Bookmarks, notes, highlights | Clean modern UI |
| **Calibre viewer** | Qt | EPUB/PDF/others | Fine but utilitarian | Annotations | Basic |

**Key reader insight:** `foliate-js`'s author explicitly documents that its paginator shares
Epub.js's CSS-multi-column limitations ("it's slow, some CSS styles do not work as expected, and
other bugs"). That is a direct, citable warning for our own EPUB pagination choice.

### 1.3 Comparison table — notes / knowledge tools

| System | Unit of thought | Storage | Linking | What we take |
|---|---|---|---|---|
| **Notion** | The **block** | Proprietary (cloud) | Page-level relations | Block editor UX: slash menu, callouts, toggles, tables, nested pages, templates, autosave |
| **Obsidian** | The **file (page)** | Plain Markdown files in a folder you own | `[[wikilinks]]` + backlinks panel + unlinked mentions | Local-first, plain-text portability, backlinks panel, templates |
| **Logseq** | The **block** | Markdown/Org | **Block-level references** — reference any bullet from anywhere; backlinks are granular to the bullet | Granular backlinks; outliner structure |

Obsidian and Logseq both store plain Markdown; Logseq is fully open source while Obsidian is
closed-source but does not lock up your files. Neither stores *structured* blocks in a database —
which is exactly the trade-off our ADR-0006 has to resolve (block DB vs Markdown files).

### 1.4 Metadata sources

| Source | Auth | Rate limit (documented) | Arabic coverage | Terms / notes |
|---|---|---|---|---|
| **Open Library** (`openlibrary.org`) | None (but a `User-Agent` with app name + email is **required** for regular use) | **Anonymous: 1 req/s. Identified (UA + email): 3 req/s (3× limit)** | Has an **Arabic language filter** in Search API and an Arabic UI locale; coverage exists but is community-curated and thinner than English | Official *"Do / Do Not"* list: **do** cache responses, identify your app, make time-sensitive requests on behalf of human users; **do not** scrape HTML, distribute requests across IPs, make hundreds of single-book requests (use `search.json` for batches), or use OL as a backend for high-traffic services. Bulk → use monthly **data dumps**. Registration form for your use-case encouraged. Violations → aggressive rate limiting or blocking. |
| **Google Books API v1** (`developers.google.com/books`) | API key | **Default quota ≈ 1,000 requests/day per project** (project-based quota covers all users of your key); exceed → HTTP 429 | Decent for Arabic-edition ISBNs; often thinner than Western | Key managed in Google Cloud Console → APIs → quota tab; can request increase |
| **Wikidata / WDQS** (`query.wikidata.org`) | None | **60 s processing time per client per 60 s; up to 30 error queries/min (burst 60); ~5 parallel queries per IP**; 60 s hard query timeout; generic/missing User-Agent rejected; honour `Retry-After` on 429/503 | Rich Arabic-author/translator/wikidata items; CC0 | WDQS migrating to a new backend in **2026–2027** with new endpoint URLs — client must be URL-configurable. Bulk → use dumps, not live crawling. |
| **WorldCat (OCLC)** — **Arabic Discovery Catalog** | Commercial/partner API keys | Not publicly documented | **One of the most comprehensive bibliographic databases of Arabic culture**, incl. electronic and open-access resources | OCLC Connect; requires partnership — **not suitable for a personal offline app** |
| **الاتحاد العربي (ARUC / Arab Union Catalogue)** — `aruc.org` | **No public API found** | — | Native Arabic union catalogue | Website only; no documented machine interface discovered in this research |
| **Bibliography of Arabic Books Online (BABO)** | Database via library subscription | — | Books published in Arabic **before 1960** | Guided by U. Alberta library; subscription-bound |
| **Neelwafurat** (`neelwafurat.com`) | **No official public API found** | — | Excellent Arabic book/edition coverage (cited in WIPO publishing docs and academic literature as a major Arabic e-book platform) | Referenced by WIPO and NINO Leiden as a discovery source; scraping would violate the spirit of OL-style ToS and is **out of scope** |
| **Jamalon** | **No public API documentation found** | — | Arabic ebook store | Not usable as a sanctioned integration |
| **Noor Book (`noor-book.com`)** | Third-party wrapper API (marketplace, key required) | Third-party managed | Large Arabic + English catalogue | **Unofficial** scraper-backed marketplace API — rejected for a local-first app (no SLA, unclear terms) |
| **Hardcover** (`docs.hardcover.app`) | Personal API token from account settings | GraphQL-based | Western-weighted catalogue | Open GraphQL API; could be an optional **future** source, not core |
| **ISBNdb / RapidAPI "Arabic Books Library"** | Paid / key | Paid | Mixed | Commercial; rejected for core |

**Arabic metadata conclusion:** there is **no sanctioned, free, high-coverage Arabic book metadata
API**. Our strategy must be: (a) **Open Library + Google Books** as the two sanctioned sources,
(b) **ISBN-first** entry (Arabic editions increasingly carry ISBN-13), (c) **file-embedded
metadata** (EPUB OPF / PDF XMP) as the primary source for Arabic books lacking online records,
(d) local-first **manual + CSV/JSON import** with a mapping step, and (e) an adapter trait so a
future sanctioned Arabic source can be dropped in.

---

## 2. Rust crates & desktop stack — verification

Every crate below was verified against the **live crates.io index** via `cargo search` /
`cargo info` on 2026-10-18 (version numbers are real, not recalled).

| Concern | Candidate | Latest verified | Verdict |
|---|---|---|---|
| Desktop shell | `tauri` | **2.x is the current stable line** (`tauri = "2"` resolves; `3.0.0-alpha.5` exists but is alpha) | ✅ **Tauri 2**, pinned to `^2` |
| Tauri plugins | `tauri-plugin-{dialog,fs,opener,window-state,single-instance,global-shortcut,updater,store,log}` | all **2.x stable** verified present | ✅ Adopt (all verified resolvable at `@2`) |
| DB access | `sqlx` | 0.9.0 | ⚠️ See ADR-0005 — rejected for this app |
| DB access | `sea-orm` | (2.0 line) | ⚠️ Rejected (see ADR-0005) |
| DB access | **`rusqlite`** | **0.40.2** | ✅ **Chosen** — sync, bundled SQLite, native FTS5 |
| Types → TS | `specta` / `tauri-specta` | **2.0.0-rc.25** (both) | ✅ Chosen; RC status noted as a risk (mitigated by a snapshot test on generated bindings) |
| Errors | `thiserror` 2.0.21 / `anyhow` 1.0.104 | verified | ✅ thiserror in libs, anyhow at app boundary |
| Logging | `tracing` 0.1.44 | verified | ✅ |
| HTTP | `reqwest` 0.13.5 | verified | ✅ with `rustls-tls` |
| EPUB read | `epub` 2.1.5 | verified | ✅ metadata/cover extraction (not rendering) |
| PDF metadata | `lopdf` 0.45.0 | verified | ✅ pure-Rust metadata/structure (not rendering) |
| PDF render (native) | `pdfium-render` 0.8.27 | verified exists | ⛔ Not needed — we render PDF with **pdf.js in the webview** (ADR-0007) |
| Images | `image` 0.25.10, `fast_image_resize` 6.1.0 | verified | ✅ covers/thumbnails |
| ISBN | **`isbn` 0.8.20261018** | verified | ✅ over `isbn2` — actively dated releases |
| Zip / walk / watch | `zip` 9.0.5, `walkdir` 2.5.0, `notify` 9.0.0-rc.6 | verified | ✅ (`notify` pin: prefer the latest 8.x stable if rc churns) |
| Normalization | `unicode-normalization` 0.1.26, `unicode-segmentation` 1.13.3 | verified | ✅ Arabic NFD + tashkeel stripping |
| Lock | `argon2` 0.6.0 | verified | ✅ optional app lock |
| Dialogs | `rfd` 0.17.2 | verified | ✅ native file dialogs |
| Hash | `sha2` 0.11.1 | verified | ✅ attachment dedupe |
| Dates | `chrono` 0.4.45 | verified | ✅ |
| IDs | `uuid` 1.28.0 | verified | ✅ |
| Tests | `proptest` 1.12.0, `insta` 1.49.0, `mockall` 0.15.0, `wiremock` 0.6.5 | verified | ✅ |
| Search | `tantivy` 0.27.0 | verified | ⛔ Rejected — see ADR-0004 (FTS5 chosen) |
| Arabic shaping | `rustybuzz` (HarfBuzz port; upstream `harfrust` is the current HarfBuzz-in-Rust effort) | verified via repo + docs.rs | ⛔ **Not needed** — we render in the webview, which already ships full HarfBuzz shaping |
| UI headless | **Bits UI for Svelte 5** (`bits-ui.com`) | verified site | ✅ headless component library |
| Reader JS | `foliate-js` / `epub.js` | verified repos | ✅ EPUB (foliate-js preferred: standalone, Range-based, better anchoring) + **pdf.js** for PDF |

**Frontend choice rationale (confirmed by research):** Readest — one of the best modern open-source
readers — is built on **Tauri v2 with a webview frontend**, which is direct evidence that the
webview route reaches production quality for e-book reading. Bits UI is explicitly built for
Svelte 5 with accessibility "baked in".

---

## 3. Ideas to adopt (with justification)

1. **Calibre's *custom columns* (user-defined fields)** — the single most-cited power feature.
   *Justification:* the brief explicitly asks for it, and it is what makes a catalogue adapt to a
   collector instead of the reverse. We adopt typed custom columns (`text/integer/float/bool/date/enum/rating`)
   with lookup-name semantics like Calibre's `#col`.
2. **Calibre's *virtual libraries* == our smart shelves** — saved filters that switch the whole
   browsing context. *Justification:* proven for 15 years; maps directly onto our saved-filter requirement.
3. **Calibre's expressive query syntax** — `field:value`, `field:>=3`, `#custom:...`, quoting for
   multi-word values. *Justification:* users who learn one query language should reuse it in the
   filter bar, the command palette and saved shelves.
4. **Kavita's "smart filters" + memory efficiency** — Kavita was the least memory-hungry app up to
   100k books in the mid-2026 benchmark. *Justification:* reinforces our 50k-book performance target
   and the requirement to virtualize the grid.
5. **StoryGraph's half-star ratings and mood/pace stats** — *Justification:* half stars are
   explicitly required; their chart set (books+pages per month, rating histogram) is a proven dashboard vocabulary.
6. **Hardcover's four reading statuses (incl. DNF)** — *Justification:* our brief already says
   to-read/reading/finished/paused/abandoned; Hard Cover validates DNF as first-class rather than an afterthought.
7. **Libib's lending tracker & custom fields being product-critical** — Libib puts lending, patron
   management and custom fields behind Pro. *Justification:* they are genuinely wanted by
   physical-collection owners (our brief includes lending), so we ship them **free and local**.
8. **Readest's "Notebook as a writing workspace" that autosaves and deep-links back to the passage**
   — *Justification:* directly matches our notes + highlight→quote + reader-position requirement.
9. **foliate-js's `Range`-based anchoring with bisecting** — more accurate position than Epub.js's
   fraction/CFI approach and it survives resize. *Justification:* our reader must restore last
   position reliably; we take foliate-js's approach for EPUB.
10. **Obsidian's `[[wikilink]]` + backlinks panel** and **Logseq's block-granular backlinks** —
    *Justification:* our brief wants `[[backlinks]]` between books/notes; block-granular gives the
    more useful "which paragraph linked here".
11. **Notion's slash menu + block types** (headings, lists, checkboxes, quotes, code, callouts,
    toggles, tables) — *Justification:* explicitly in the brief; validated by three independent systems.
12. **Open Library's identified User-Agent + aggressive local caching**, and `search.json` batch
    lookups instead of per-book requests — *Justification:* this is the documented way to stay
    inside their limits; violating it risks being blocked.
13. **Goodreads CSV column contract** — import must accept exactly
    `Book Id, Title, Author, Author l-f, Additional Authors, ISBN, ISBN13, My Rating, Average Rating,
    Publisher, Binding, Number of Pages, Year Published, Original Publication Year, Date Read,
    Date Added, Bookshelves, Bookshelves with positions, Exclusive Shelf, My Review, Spoiler,
    Private Notes, Read Count, Owned Copies` (with Excel-style `="..."` quoting on ISBN fields) and
    map `Exclusive Shelf` → our reading status. *Justification:* this is the migration path people
    actually use to leave Goodreads.
14. **StoryGraph/Hardcover CSV import + DNF shelf** — same reasoning for second-source imports.
15. **Calibre's `check_library`/FTS vacuum habit** — a maintenance command that verifies integrity
    and rebuilds the index. *Justification:* cheap insurance for a local-first app.
16. **Tauri's official plugin set** rather than hand-rolling: `window-state`, `single-instance`,
    `dialog`, `opener`, `global-shortcut`, `updater`, `store`, `log` — all verified 2.x stable.

---

## 4. Pitfalls to avoid

1. **Don't scrape HTML or hammer per-record endpoints.** Open Library publishes an explicit
   "Do Not" list (no HTML scraping, no distributed IPs, no hundreds of single-book requests);
   violations get you rate-limited or blocked.
2. **Don't assume Google Books is unlimited.** ~1,000 requests/day per project by default; the
   web is full of 429 reports. Need per-source rate limiting + caching + graceful offline fallback.
3. **Don't build on WDQS as a hard dependency.** 60 s/60 s budget, 5 parallel queries/IP, 30
   error queries/min, **and a 2026–2027 endpoint migration**. Use it as a best-effort enrichment
   source with a configurable endpoint.
4. **Don't assume an Arabic metadata API exists.** Neelwafurat, Jamalon and ARUC have **no
   documented public API**; OCLC's Arabic Discovery Catalog and BABO are partnership/subscription
   gated; Noor Book's API is an unofficial third-party scraper. Design for ISBN + embedded file
   metadata + manual/CSV entry instead.
5. **Don't run async SQLite for a desktop app.** Research consensus (multiple independent 2026
   comparisons) is that *all* async SQLite crates perform worse than sync ones because SQLite has
   no async API. Also, sqlx's `query!` needs a live DB or a committed `.sqlx` cache at build time —
   an avoidable CI trap. (ADR-0005.)
6. **Don't add tantivy/ES for search.** A second index means a second source of truth, a rebuild
   step, and drift. FTS5 lives in the same transaction as the data. (ADR-0004.)
7. **Don't forget Arabic text is not searchable as stored.** Alef/hamza variants, ya vs
   alef-maqsura, ta-marbuta, tashkeel and tatweel must be normalized **at write time** into a
   stored normalized column, and the same normalization applied to the query. Missing this is the
   #1 reason Arabic search "doesn't work".
8. **Don't use a native UI toolkit and expect free Arabic shaping.** Research shows the reliable
   path to correct Arabic shaping + bidi + rich editing + epub.js/pdf.js is a **webview** (which is
   why Readest, Foliate and Koodo all render book content in a webview). The reverse — a pure-Rust
   renderer — would require us to ship our own shaping/bidi stack.
9. **Don't hand-write the TS↔Rust contract.** It drifts within a week. Generate with specta and
   fail CI if bindings are stale.
10. **Don't paginate EPUB with naive Epub.js-style CFI fractions alone.** foliate-js's own README
    documents CSS-multi-column limitations (slowness, styles not applying, other bugs).
11. **Don't rely on network at startup.** Cold start target is < 2 s; metadata lookup must be a
    user action or an explicitly enabled setting, never a launch-time call.
12. **Don't ship a monolithic `books` table with JSON blobs for everything.** It kills filtering,
    sorting and aggregation. Normalize (see `03-data-model.md`) but keep a narrow denormalized
    search projection.
13. **Don't make ratings a single number.** Every serious tracker now offers half/quarter stars
    (StoryGraph, Hardcover) and the brief demands per-criterion rubrics; a single float can't
    answer "which publisher consistently prints well *by my criteria*".
14. **Don't ignore the mid-2026 import benchmark.** Calibre-Web processed only ~1,100/10,000 books
    in 91 minutes. Our folder scanner and CSV importer must be batched, transactional, and show
    progress — and must be benchmarked against 50k rows in CI-as-a-test (not just "it works").
15. **Don't conflate licence with usability.** Several strong servers (BookOrbit) carry *additional*
    licence terms (visible attribution, name restrictions) even though they're "open source".
16. **Don't put arbitrary file paths into the DB and re-open them later.** Validate, hash (SHA-256),
    store relative to a known root, and re-verify on read.

---

## 5. Prioritized feature list (MUST / SHOULD / COULD)

### MUST (the app is not shippable without these)
- M1. Local-first storage: SQLite (WAL) in the OS app-data folder; no account, no telemetry.
- M2. Full Arabic + English UI, **live switching**, default **Arabic**, whole-layout RTL/LTR mirroring,
      per-field auto direction (`dir="auto"` + bidi isolation), bundled Arabic + Latin fonts.
- M3. Books CRUD with the full brief data model (title/subtitle/original title/language/translators/
      ISBN-10+13 validated/edition/year/pages/format/description/series+order/physical location/
      acquisition/condition/cover/timestamps/custom fields).
- M4. Authors (M:N with role), Publishers, hierarchical Categories + Tags, seeded bilingual taxonomy.
- M5. Reading status + multiple reading cycles + ReadingSession log + auto progress %.
- M6. Per-book page with the 7 tabs (Overview, Notes, Quotes, Reading log, Reviews/rubric, Files, Related).
- M7. Arabic-aware search (FTS5 + normalization + light stemming) across books/authors/publishers/
      notes/quotes, with mixed Arabic/English queries and a command palette (Ctrl/Cmd+K).
- M8. Library browser: grid (cover wall), list, table views; **virtualized for 50,000+ books**;
      filters, sorting, saved filters/shelves (static + smart).
- M9. Covers: upload, drag-and-drop, fetch by ISBN/title, extract from EPUB/PDF, generated Arabic
      placeholder, multi-size thumbnail cache.
- M10. Add-book wizard with inline entity creation + ISBN lookup that pre-fills for review.
- M11. Block-based Notes: required block types, slash menu, autosave, nested sub-pages, templates,
       `[[backlinks]]`, full-text search, Markdown export.
- M12. Rubric rating criteria (user-defined), overall half-star rating, reviews + dated comments,
       **aggregates per author and per publisher**.
- M13. Quotes/highlights with page, chapter, colour, comment, link back to reader position.
- M14. Attachments (PDF/EPUB/MOBI/TXT/audio) by copy or reference with SHA-256 dedupe;
       in-app **EPUB + PDF readers** with themes, font controls, RTL, bookmarks, highlight→quote,
       last-position memory.
- M15. Dashboard: totals, breakdowns, books/pages per month, streak, yearly goal, GitHub-style
       heatmap, widgets (currently reading / recently added / recently finished / top rated / to-read).
- M16. Backups: scheduled + pre-migration, manual backup/restore with integrity verification.
- M17. Settings incl. language, theme, digits (Arabic-Indic vs Western), Hijri+Gregorian dates,
       accent colour; **accessibility pass (focus rings, ARIA, AA contrast)**.
- M18. Security: strict Tauri capability allowlist, CSP, validated paths, type/size checks;
       structured logging + log viewer.
- M19. Quality gates: `cargo fmt`, `cargo clippy -D warnings`, tests (unit/integration/property/
       HTTP-mock), CI on Windows + Linux.
- M20. Import/export: CSV/JSON with column-mapping; Goodreads + Calibre export import;
       duplicate detection (ISBN + normalized title+author) with merge.

### SHOULD (strongly expected, small risk)
- S1. Folder scan of e-books → auto-create entries from embedded metadata + folder watcher.
- S2. Webcam/barcode ISBN scanning where feasible (falls back to typed ISBN).
- S3. Reading goals (yearly/monthly), lending tracker with reminders, activity log.
- S4. Collections/Shelves incl. saved-filter smart shelves; "similar books" from local data.
- S5. App lock (PIN/password, argon2); auto-updater channel; system tray; custom title bar.
- S6. Management screens for authors/publishers/categories/tags/languages/criteria with
      **merge-duplicates** and bulk edit.
- S7. Diagnostics: log viewer in settings, diagnostic export.
- S8. Storybook-like component gallery (dev only) + screenshot visual checks.
- S9. Native barcode scan → if the platform API is not viable, degrade to "type or paste ISBN".
- S10. Wikidata as optional enrichment (configurable endpoint, best-effort).

### COULD (nice-to-have, cut first under pressure)
- C1. Audiobook file support with a player and progress.
- C2. Recommendations based on local data + optional external source.
- C3. QR/labels & printable catalogue cards; shelf-label PDF export.
- C4. Plugin/script hooks (custom templates, template language à la Calibre).
- C5. Highlight spaced-repetition review (Screvi-style) of quotes.
- C6. Multi-window / separate reader window.
- C7. Reading-session auto-timer with idle detection.
- C8. Hardcover/StoryGraph sync (would require network + accounts → conflicts with local-first default).

---

## 6. Sources consulted (every URL used)

**Calibre**
- https://manual.calibre-ebook.com/gui.html — search & sort, virtual libraries, `vl:` field
- https://manual.calibre-ebook.com/template_lang.html — template language, custom column lookup names
- https://manual.calibre-ebook.com/sub_groups.html — managing subgroups/genres
- https://manual.calibre-ebook.com/generated/en/calibredb.html — CLI: custom columns, fts_search, check_library

**Self-hosted servers / catalogue apps**
- https://productimpossible.com/review/self-hosted-ebook-managers/ — 2026 comparison, licences, import benchmark, Kavita+ terms
- https://dev.to/selfhostingsh/kavita-vs-calibre-web-which-should-you-self-host-3p04
- https://www.reddit.com/r/selfhosted/comments/1hfdxfp/kavita_vs_ubooquity_vs_calibre/
- https://homelabcompass.com/alternatives/self-hosted-ebook-library
- https://www.xda-developers.com/best-self-hosted-services-for-bibliophiles/
- https://github.com/janeczku/calibre-web · https://github.com/crocodilestick/Calibre-Web-Automated
- https://www.kavitareader.com/ · https://komga.org/ · https://audiobookshelf.org/
- https://github.com/janeczku/calibre-web/issues/2122 — KOReader progress/highlight sync requests
- https://jccpalmer.com/posts/setting-up-kobo-sync-with-calibre-web/ — Calibre-Web Kobo sync UX
- https://www.reddit.com/r/koreader/comments/1pd64mf/calibreweb_kobo_synclike_function/ — "OPDS is one-way download"
- https://selfhosted.libhunt.com/compare/komga-vs-calibre-web
- https://www.libib.com/ · https://www.librarything.com/
- https://www.achriom.com/blog/best-personal-library-app/ — LibraryThing/Libib/CLZ/BookBuddy comparison
- https://ernawrites.com/2024/08/08/5-best-apps-to-manage-your-home-library/
- https://tidymalism.com/best-book-catalogue-apps/

**Trackers**
- https://thestorygraph.com/ — half/quarter stars, mood/pace, stats charts, reading journal
- https://www.achriom.com/blog/best-goodreads-alternatives/ — StoryGraph/LibraryThing/Libby pricing & features
- https://screvi.com/blog/best-book-tracking-apps-2026/ — Hardcover open GraphQL API, 4 statuses, half stars; StoryGraph; Bookmory; Screvi
- https://rosslibrary.org/clash-of-the-book-apps-goodreads-fable-storygraph-and-uncover/
- https://www.achriom.com/blog/best-goodreads-alternatives/
- https://makeheadway.com/blog/storygraph-vs-goodreads/

**Readers**
- https://readest.com/ · https://github.com/readest/readest — Tauri v2 + Next.js, feature table, Notebook workspace
- https://www.koodoreader.com/en · https://github.com/koodo-reader/koodo-reader
- https://github.com/johnfactotum/foliate-js — paginator, CSS multi-column limitations, Range anchoring
- https://itsfoss.com/news/readest-app/
- https://www.merge-json-files.com/blog/best-epub-reader-for-ubuntu
- https://intity.github.io/epub-js/ · https://github.com/futurepress/epub.js
- https://www.npmjs.com/package/react-reader — `isRTL` prop, annotation API

**Notes tools**
- https://dev.to/dev_tips/obsidian-notion-logseq-the-note-taking-stack-that-doesnt-suck-for-devs-2cf7
- https://fabric.so/comparison/obsidian-vs-logseq — unit of thought: file vs block
- https://www.obsibrain.com/blog/obsidian-vs-logseq/ — `[[wikilinks]]`, backlinks, block IDs
- https://www.outlinersoftware.com/topics/viewt/10610

**Metadata APIs & terms**
- https://openlibrary.org/developers/api — **fetched directly**: rate limits (1 req/s anon, 3 req/s identified), usage guidelines, Do/Do-Not list, registration
- https://openlibrary.org/dev/docs/api/search — Search API, language filter incl. Arabic, `search.json`
- https://openlibrary.org/dev/docs/api/books — Books API `jscmd=data` payload shape (publishers, identifiers, cover sizes, subjects)
- https://blog.openlibrary.org/2025/01/16/api-search-json-performance-tuning/
- https://developers.google.com/books/docs/v1/using — Google Books API v1
- https://support.google.com/books/partner/thread/455522095/ — default 1,000 requests/day
- https://stackoverflow.com/questions/72845450/what-are-the-rate-limits-for-the-google-books-api — project vs user quota, 429
- https://www.mobileread.com/forums/showthread.php?t=358307 — "Google books 1000/day, Open Library 100 per 5 min" (community-reported, older)
- https://www.wikidata.org/wiki/Wikidata:SPARQL_query_service/query_limits
- https://www.mediawiki.org/wiki/Wikidata_Query_Service/User_Manual — 60 s/60 s, 30 error queries/min, 5 parallel/IP
- https://apis.io/rate-limits/wikidata/wikidata-rate-limits/ — 429 + Retry-After policy transcription
- https://www.wikidata.org/wiki/Wikidata:SPARQL_query_service — 2026–2027 backend migration, new endpoint URLs
- https://connect.oclc.org/en/arabic-discovery-catalog — OCLC Arabic Discovery Catalog
- https://www.aruc.org/ — الفهرس العربي الموحد (Arab Union Catalogue)
- https://guides.library.ualberta.ca/az/bibliography-of-arabic-books-online — BABO (pre-1960 Arabic)
- https://publicapis.dev/category/books — community book-API index
- https://parse.bot/marketplace/8239180d-412d-485c-9927-f426a6dea355/noor-book-com-api — unofficial Noor Book API (rejected)
- https://www.wipo.int/documents/d/mobile-apps/docs-en-wipo-tool-mobile-apps-in-digital-publishing.pdf — WIPO citing Neelwafurat & Jarir
- https://www.nino-leiden.nl/download/4337 — NINO on Neel wa Furat as a discovery tool
- https://docs.hardcover.app/api/getting-started/ — Hardcover API token flow

**Rust / desktop stack**
- https://www.rustfinity.com/blog/rust-orms — Diesel/SQLx/SeaORM/rusqlite 2026 comparison incl. "async SQLite is slower"
- https://rustify.rs/articles/rust-sqlx-vs-diesel-vs-seaorm-2026
- https://www.aarambhdevhub.medium.com/rust-orms-in-2026-...
- https://rustify.rs/articles/rust-tauri-v2-desktop-app-tutorial-2026 — Tauri v2 architecture diagram, WebView2
- https://tech-insider.org/tauri-vs-electron-desktop-app-framework-2026/
- https://blog.codercops.com/blog/tauri-2-desktop-mobile-apps-rust-web-2026 — Windows uses WebView2
- https://www.reddit.com/r/rust/comments/1d0l216/using_tauri_with_sqlite_database_orm_good_or_bad/
- https://v2.tauri.app/plugin/ — official Tauri feature/plugin index
- https://github.com/tauri-apps/plugins-workspace — official plugins + platform matrix
- https://v2.tauri.app/release/ — plugin release versions (2.x, Oct 2026)
- https://docs.rs/tauri-specta · https://tauri-specta .dev · https://specta.dev/ — type generation
- https://crates.io (via `cargo search` / `cargo info` on 2026-10-18) — verified versions for tauri, sqlx, rusqlite, specta, tauri-specta, epub, lopdf, isbn, reqwest, image, fast_image_resize, zip, walkdir, notify, unicode-normalization, unicode-segmentation, argon2, rfd, sha2, chrono, uuid, thiserror, anyhow, tracing, proptest, insta, mockall, wiremock, tantivy, and all `tauri-plugin-*`
- https://github.com/harfbuzz/rustybuzz · https://github.com/harfbuzz/harfrust — Arabic shaping in Rust
- https://crates.io/crates/pdfium-render · https://dev.to/hiyoyok/lopdf-vs-pdfium-in-rust-... — lopdf vs pdfium guidance
- https://www.bits-ui.com/ · https://www.bits-ui.com/docs — Svelte 5 headless, accessibility

**Arabic IR / normalization**
- https://aclanthology.org/2006.bcs-1.6.pdf — Kadri & Nie, *Effective stemming for Arabic information retrieval* (light vs linguistic stemming; light stemming can help or hurt)
- https://ir.cs.georgetown.edu/downloads/p340-aljlayl.pdf — two Arabic stemming algorithms for IR
- https://ciir.cs.umass.edu/pubfiles/ir-249.pdf — *Light Stemming and Co-occurrence Analysis*
- https://www.semanticscholar.org/paper/Light-Stemming-for-Arabic-Information-Retrieval-Larkey-Ballesteros/ — Larkey's light stemmer
- https://pmc.ncbi.nlm.nih.gov/articles/PMC7895730/ — Arabic social-media preprocessing (normalization steps)
- https://elon.io/grammar/arabic-levantine/writing-system/unicode-search — Unicode normalization + Arabic search pitfalls

**Import formats**
- https://www.goodreads.com/topic/show/419981-csv-import-export-format — Goodreads CSV columns
- https://www.goodreads.com/topic/show/22566592-what-s-in-an-export-file
- https://help.micro.blog/t/importing-book-metadata-non-goodreads/ — full Goodreads header + `="..."` quoting example
- https://leafed.app/blog/goodreads-export-guide.html
- https://www.reddit.com/r/Calibre/comments/1lh5pb0/import_goodreads_library_export_csv_instructions/

---

## 7. Outcome

Phase 0 is complete. The findings above directly drive:

- **ADR-0001** UI framework → Tauri 2 + Svelte 5 + Tailwind + Bits UI (webview justified by Readest/Foliate precedent and Arabic shaping).
- **ADR-0005** DB layer → `rusqlite` (deviation from the brief's sqlx/SeaORM suggestion, justified by research).
- **ADR-0004** Search → SQLite FTS5 with Arabic normalization at write time (tantivy rejected).
- **ADR-0006** Notes → structured blocks in SQLite with Markdown projection/export.
- **ADR-0007** Readers → pdf.js + foliate-js in the webview; Rust handles files, not rendering.
- **ADR-0008** Metadata → Open Library + Google Books only, with strict rate limiting, caching, and an adapter seam for a future sanctioned Arabic source.

→ Next: **Phase 1 — `docs/01-product-spec.md`, `02-architecture.md`, `03-data-model.md`,
`04-ux-design.md`, `05-roadmap.md` and the ADR set.**
