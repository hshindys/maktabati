# 05 — Roadmap

**Project:** Maktabati / مكتبتي · **Last updated:** 2026-10-18
**Process:** 13 milestones (M0–M12), each **built, tested and committed** with conventional
commits before moving on. Effort is estimated in focused engineering days for one senior engineer
(+ design review where marked).

**Legend:** ✅ done · 🚧 in progress · ⬜ not started

| Gate (must pass to close a milestone) |
|---|
| `cargo fmt --check` · `cargo clippy --workspace --all-targets -- -D warnings` · all tests · frontend typecheck + unit tests · docs/ADR updated if a decision changed |

---

## Milestone summary

| # | Milestone | Effort | Status |
|---|---|---|---|
| **0** | Online research → `docs/00-research.md` | 1.5 d | ✅ |
| **1** | Planning & architecture docs + ADRs | 2 d | ✅ |
| **2** | Workspace scaffold, CI, design system, app shell, i18n/RTL, DB + migrations + seed | 4 d | ✅ |
| **3** | CRUD for books/authors/publishers/categories/tags + add-book wizard | 4 d | ✅ |
| **4** | Metadata lookup, duplicate detection, import/export | 3 d | ⬜ |
| **5** | Library browser, filters, shelves, command palette, Arabic-aware search | 4 d | ⬜ |
| **6** | Per-book page: overview, status/progress, reading sessions, related | 3 d | ⬜ |
| **7** | Notes workspace (blocks, sub-pages, templates, backlinks, autosave, export) | 5 d | ⬜ |
| **8** | Files, folder scanning, PDF/EPUB readers, highlights → quotes, position sync | 5 d | ⬜ |
| **9** | Rubric ratings, reviews, quotes, aggregates per author/publisher | 3 d | ⬜ |
| **10** | Dashboard, charts, goals, heatmap, lending tracker | 4 d | ⬜ |
| **11** | Backups/restore, app lock, settings, accessibility + performance pass | 3 d | ⬜ |
| **12** | Packaging, installers, auto-update, final docs, release candidate | 3 d | ⬜ |
| | **Total** | **~45 d** | |

---

## M0 — Research ✅

**Deliverable:** `docs/00-research.md`.

**Acceptance criteria**
- [x] Calibre, Calibre-Web/CWA, Kavita, Komga, BookLore/Grimmory, Audiobookshelf researched
- [x] Hardcover, StoryGraph, Goodreads, LibraryThing, Libib, Bookshelf-class apps researched
- [x] Readest, Koodo, Foliate researched for reader UX
- [x] Notion, Obsidian, Logseq researched for notes UX
- [x] Open Library, Google Books, Wikidata, WorldCat/LoC, Arabic sources evaluated **with terms
      and rate limits**, and the absence of a sanctioned Arabic API recorded honestly
- [x] Rust crates + Tauri patterns verified against live crates.io
- [x] Comparison tables, ideas-to-adopt, pitfalls, **every source linked**, MUST/SHOULD/COULD list

**Risks:** none (doc-only).

---

## M1 — Planning & architecture ✅

**Deliverables:** `01-product-spec.md`, `02-architecture.md`, `03-data-model.md`,
`04-ux-design.md`, `05-roadmap.md`, `docs/adr/0001–0012`.

**Acceptance criteria**
- [x] All five documents present and cross-linked (+ `docs/README.md` index)
- [x] ADRs cover every major choice *and* every deviation from the brief (UI framework, type
      contract, persistence, search, DB layer, notes format, readers, metadata, frontend stack,
      errors/concurrency, barcode, escape hatch)
- [x] ER diagram + full DDL present and internally consistent
- [x] **Self-review pass:** no contradictions between spec ↔ architecture ↔ data model ↔ UX;
      every MUST feature in `00-research.md` maps to a milestone
- [x] Committed

**Self-review findings (resolved during M1)**
1. `search_document` declared `fts_book_norm` over columns (`tags_norm`,
   `description_norm`) that did not exist on the projection → **added** the normalized mirror
   columns to `search_document`.
2. Both a `translators` table and `book_authors.role='translator'` existed → **removed** the
   table; translators are `book_authors` rows (one source of truth).
3. `crates/app` vs `src-tauri` naming was ambiguous → **resolved**: `src-tauri` *is* the app
   crate (package `maktabati-app`); documented in `02-architecture.md` §3.1.
4. Roadmap gate mentioned `cargo nextest`, which is not installed on the build host → gate now
   reads "nextest **or** `cargo test` if nextest is unavailable"; nextest installation is an M2 task.
5. Digit policy was stated in UX (`04`) but not in storage conventions → **added** to
   `03-data-model.md` §1 (store Western/ASCII, format at display time).

**Risks & mitigations**

| Risk | Mitigation |
|---|---|
| Docs drift from code as milestones land | Docs are updated *within* the milestone commit; ADRs are append-only |
| Over-specifying before learning | Non-goals + open questions (§7 of spec) explicitly reserve judgment |

---

## M2 — Foundation ✅

**Scope**
1. Cargo workspace: `crates/{domain,storage,search,metadata,files,notes,stats,backup,app}` +
   `src-tauri` shell + `src/` frontend.
2. CI (GitHub Actions): fmt, clippy `-D warnings`, nextest, cargo-deny, frontend typecheck/test,
   build on **Windows + Linux**.
3. **Design system** first: tokens (light/dark/sepia/accent), typography with bundled fonts,
   base components + dev gallery page.
4. **App shell:** custom title bar, sidebar, router, window-state persistence, tray, toasts,
   command-palette stub.
5. **i18n/RTL foundation:** `ar`/`en` resources, live switching, `dir` handling, digit setting,
   Hijri/Gregorian formatter, RTL lint + key-parity tests.
6. **DB:** `rusqlite` pool, PRAGMAs, migrator with checksums + pre-migration backup hook,
   `0001_init.sql` (full schema from `03-data-model.md`), `0002_seed_taxonomy.sql`, FTS5 smoke test.
7. **Seed script** with ~40 demo books (Arabic + English), authors, publishers, categories,
   sessions, quotes, notes — used by every later test.
8. specta bindings export + drift gate.

**Acceptance criteria**
- [ ] `cargo build --workspace` and `cargo test --workspace` green from a clean clone
- [ ] CI green on Windows **and** Linux
- [ ] App launches, shows shell, switches language live, mirrors layout, persists window state
- [ ] Fresh DB migrates to latest schema and seeds; re-running is idempotent
- [ ] FTS5 available (runtime `CREATE VIRTUAL TABLE` test)
- [ ] Dev gallery renders every base component in 2 themes × 2 directions
- [ ] `bindings.ts` generated and CI-verified
- [ ] All gates pass

**Risks & mitigations**

| Risk | Mitigation |
|---|---|
| Linux build host missing webkit2gtk/tray deps | Document install step in README; CI installs them explicitly |
| FTS5 not compiled into bundled SQLite | Hard CI assertion in `storage` tests |
| Fonts too large (offline bundle) | Subset to needed glyphs; only 4 Arabic + 1 Latin face; verify bundle size budget (< 3 MB) |
| specta RC churn | Exact pin (ADR-0002/0012) |

---

## M3 — Catalogue CRUD ✅

**Scope:** books / authors / publishers / categories / tags / languages / series CRUD with
validation (ISBN, required fields, enum checks), inline entity creation + autocomplete in forms,
the **sectioned add-book wizard**. Cover ingest moved to M8 — see the note under the criteria.

**Acceptance criteria**
- [x] Every entity: create, read, update, delete, with repository integration tests
- [x] Wizard: 4 steps, inline create, validation summaries, cancel/restore
- [x] ISBN-10/13 validated (checksum) with clear localized errors; property tests for validation
- [x] Duplicate check at save time (ISBN + `title_norm|author_norm`) warns but allows override
- [x] `fmt`/`clippy`/tests green

> **Cover criterion moved to M8.** Upload, drag-drop, thumbnail regeneration and the Arabic
> placeholder all need `crates/files` (ingest, SHA-256 dedupe, downscale, WebP thumbnail
> cache), which is the M8 files crate. The wizard's cover drop-zone stays absent until that
> backend exists rather than shipping a control that cannot store a cover. The `CoverPlaceholder`
> component (which renders Arabic correctly) already exists.

**Risks:** cover formats (AVIF/HEIC) — accept JPEG/PNG/WebP only, documented; large images → downscale on ingest.

---

## M4 — Metadata, dedupe & import/export

**Scope:** `crates/metadata` (OL + Google Books adapters, token bucket, cache, merge policy,
cover fetch), duplicate detection + **merge flow**, folder scan from embedded metadata,
CSV/JSON import with **column-mapping step** + preview, Goodreads + Calibre import, export.

**Acceptance criteria**
- [ ] `metadata` unit tests pass against **wiremock** (success, 404, 429 + `Retry-After`, offline,
      malformed JSON, cache hit, negative-cache TTL)
- [ ] Rate limiter proven by a test that fires N requests and asserts the schedule
- [ ] Wizard pre-fills from lookup and marks the source of each field
- [ ] Duplicate detection returns candidates; merge keeps union of relations and non-empty fields
- [ ] CSV import: mapping UI, preview, transactional commit, **50k-row benchmark test** with
      progress events
- [ ] Goodreads import maps `Exclusive Shelf` → status, ratings, `Date Read`, shelves, review text
- [ ] Folder scan reads EPUB OPF + PDF XMP and creates candidates for review
- [ ] All gates pass

**Risks:** Google's 1,000/day quota → OL first, cache-first, configurable budget (ADR-0008).

---

## M5 — Library browser, shelves, palette, search

**Scope:** `crates/search` (schema, projection, triggers, query parser, ranking, snippets,
reindex), virtualized grid/list/table, filter bar, sorting, saved static + **smart shelves**,
command palette, global search across books/authors/publishers/notes/quotes.

**Acceptance criteria**
- [ ] **Property tests:** normalization idempotence; alef/hamza/ya/ta-marbuta/tashkeel/tatweel
      variants match; mixed Arabic+English queries; 40+ Arabic fixtures
- [ ] Query parser tests: `title:`, `rating:>=3`, `tag:x`, `#custom:`, quotes, AND/OR/NOT
- [ ] `search` integration: 50k seeded books → **p95 < 100 ms** benchmark asserted in CI
- [ ] Grid scrolls at 60 fps with 50k rows (virtualized; only visible page crosses IPC)
- [ ] Saved shelves round-trip (`query_json` v1) and re-evaluate live
- [ ] Palette returns merged, categorized results with keyboard navigation
- [ ] `search::reindex_all()` maintenance path tested after stemming toggle
- [ ] All gates pass

**Risks:** FTS5 prefix-token latency while typing → only the final token is prefixed; debounce 150 ms.

---

## M6 — Per-book page (structure)

**Scope:** the 7-tab page shell; **Overview** (metadata, custom columns, status, progress, quick
rating, activity); status/progress + **reading cycles** + **reading sessions** + charts;
**Related** (same author/publisher/series/category + local similarity).

**Acceptance criteria**
- [ ] Tab routing is deep-linkable; `1..7` shortcuts work
- [ ] Status transitions create/close reading cycles correctly (domain rule tests)
- [ ] Progress % computed from page count and current page; capped/validated
- [ ] Sessions CRUD + a per-book mini chart (SVG, token-themed)
- [ ] Related queries return correct sets; "similar books" uses local co-category/co-tag overlap
      (documented scoring in `domain`)
- [ ] All gates pass

---

## M7 — Notes workspace

**Scope:** `crates/notes` (block model, fractional indexing, Markdown codec, templates,
link extraction), block editor UI (all v1 block types, slash menu, drag reorder, nested sub-pages),
autosave with `rev` conflict handling, backlinks panel, notes search, Markdown export/import.

**Acceptance criteria**
- [ ] Every block type create/edit/delete/move with unit tests
- [ ] **Markdown round-trip property test:** `parse(render(blocks)) == blocks` for supported types
- [ ] Autosave: debounced patch; **crash-safety test** (kill mid-save → no lost committed block)
- [ ] Backlinks update when a `[[page]]` is renamed; block-granular source shown
- [ ] Templates instantiate by deep copy; default-template setting works
- [ ] Export produces valid Markdown readable by Obsidian (fixture snapshot)
- [ ] Slash menu, `[[` autocomplete, undo/redo all keyboard-operable
- [ ] All gates pass

**Risks:** editor complexity → Bits UI primitives + a single `Block` component with a type switch;
large pages (1,000+ blocks) → render only the viewport window (virtualize by block).

---

## M8 — Files, scanning & readers

**Scope:** `crates/files` (attach copy/reference, SHA-256 dedupe, EPUB/PDF metadata + cover
extraction, chunked safe read, folder watcher), reader overlay (pdf.js + foliate-js vendored),
themes/fonts/RTL, bookmarks, **highlight → quote**, position sync to `reading_cycles`/progress.

**Acceptance criteria**
- [ ] Cover pipeline: ingest (JPEG/PNG/WebP only), downscale on ingest, WebP thumbnail cache at
      `{book_id}_{200,400,800}.webp`, EXIF stripped, max 4096px; wizard drop-zone + drag-drop onto
      the library grid; `cover_set` / `cover_remove` / `cover_regenerate` commands; Arabic
      placeholder renders correctly at every size
- [ ] Attach: copy vs reference; dedupe proven by a test attaching the same bytes twice
- [ ] Path traversal impossible (tests attempt `../../etc/passwd` and absolute escapes)
- [ ] Type/size validation (magic bytes + allowlist) rejects mismatched content
- [ ] EPUB/PDF metadata extraction tests on fixture files incl. an Arabic EPUB
- [ ] Reader restores last position on open and writes position back (debounced) → progress updates
- [ ] Highlight creates a quote with page/chapter/colour/comment; `jump_to_position` re-opens it
- [ ] Reader themes (light/dark/sepia), font size/family, margins, RTL pagination verified with a
      fixture Arabic EPUB
- [ ] All gates pass

**Risks:** large PDF memory → render viewport ± N pages (pdf.js built-in); webview asset scope for
reading bytes must be id-addressed, never path-addressed (security §8).

---

## M9 — Ratings, reviews, quotes, aggregates

**Scope:** overall half-star rating, **rubric criteria CRUD** + weights, per-book rubric scoring,
reviews + dated comments, quotes management screen, **aggregate scores per author and per publisher**.

**Acceptance criteria**
- [ ] Rating domain value object rejects out-of-range and non-half-star values (property tests)
- [ ] Rubric scoring UI: sliders with keyboard support, per-criterion note
- [ ] Aggregate query returns weighted mean + count; `min_books` gate enforced (tested: 1 perfect
      book does not beat 20 books at 4.2)
- [ ] Author/publisher detail pages show their aggregate + breakdown by criterion
- [ ] Quotes screen: filter by book/colour/page, edit note, jump to reader
- [ ] All gates pass

---

## M10 — Dashboard, charts, goals, heatmap, lending

**Scope:** `crates/stats` aggregates; dashboard KPIs + breakdowns; books/pages per month/year
charts; streak; GitHub-style heatmap (53×7); yearly/monthly goals; widgets; lending tracker with
due/overdue reminders; activity log surface.

**Acceptance criteria**
- [ ] All aggregates computed in SQL with a `stats_version` memoization invalidation test
- [ ] Heatmap correct across locale week-start (Sunday vs Monday) and both digit styles
- [ ] Goals progress reflects finished books/pages within scope; editable targets
- [ ] Lending: open/overdue/returned, `reminder_at`, and a dashboard badge
- [ ] Charts are SVG, themed by tokens, RTL-correct, and render with fixture data snapshots
- [ ] Dashboard renders < 300 ms with the 50k seed (measured in the benchmark test)
- [ ] All gates pass

---

## M11 — Backups, lock, settings, accessibility & performance pass

**Scope:** backup create/restore/verify (+ schedule + pre-migration integration), app lock
(argon2id), full settings surface, log viewer + diagnostic export, accessibility audit, performance
audit.

**Acceptance criteria**
- [ ] Backup round-trip test: create → corrupt a byte → verify **fails** → restore refuses;
      create → restore into a clean dir → data identical (row-count + checksums)
- [ ] Pre-migration backup fires automatically (test: migration on a populated DB creates an archive)
- [ ] App lock: correct/incorrect PIN, lock on startup/after idle, unlock gate on commands
- [ ] Settings: language, theme, accent, digits, dates, search options, metadata providers,
      backup schedule, log level — all persisted and applied live
- [ ] **Accessibility audit checklist completed:** contrast test green, focus visible everywhere,
      ARIA labels on icon buttons, keyboard-only pass over all critical flows, `prefers-reduced-motion`
- [ ] **Performance audit:** cold start < 2 s, search p95 < 100 ms @ 50k, 60 fps grid — measured
- [ ] All gates pass

**Risks:** accessibility regressions → the checklist is a CI-runnable script where possible, a
documented manual pass otherwise.

---

## M12 — Packaging, installers, auto-update, release candidate

**Scope:** app icon + branding, Windows **MSI + NSIS**, Linux **AppImage + deb**, macOS **dmg**,
updater signing (disabled until keys exist), file associations (optional), README/CONTRIBUTING/
user guide/shortcuts/CHANGELOG, rustdoc on public APIs, release dry-run.

**Acceptance criteria**
- [ ] `tauri build` produces installers on all three OS targets (CI artifacts)
- [ ] Fresh install on Windows: installs, runs, tray works, first-run onboarding completes
- [ ] Fresh install on Linux: AppImage runs on a clean VM
- [ ] Updater config present and **disabled by default**; enabling documented (ADR/spec Q3)
- [ ] README: setup, build, run, package; CONTRIBUTING; `docs/user-guide.md`;
      `docs/shortcuts.md` generated from the shared shortcut definition; CHANGELOG for 1.0.0
- [ ] rustdoc builds without warnings; `cargo doc --workspace` clean
- [ ] `cargo deny check` clean (licenses + advisories + bans)
- [ ] All gates pass; tagged `v1.0.0-rc.1`

**Risks:** Windows code signing (OV/EV cert) is a procurement step outside code → unsigned RC
documented as a known limitation; signing is a release-engineering task.

---

## Cross-milestone risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Scope creep pushes past M12 | High | High | MUST/SHOULD/COULD list in `00-research.md`; COULD items are explicitly cut first |
| Arabic shaping/RTL regressions | Medium | High | RTL lint + snapshot tests in CI from M2; dev gallery in both directions |
| 50k-row performance slips | Medium | High | Benchmark tests asserted in CI from M5, not "tested by eye" |
| Webview differences Win/Linux | Medium | Medium | CI builds both; target WebView2 evergreen on Windows |
| specta/tauri-specta RC breakage | Low | Medium | Exact pin + ADR-0012 escape hatch |
| Google Books quota exhaustion | Medium | Low | OL first, cache-first, configurable budget |
| Reader memory on huge PDFs | Medium | Medium | Lazy page rendering, chunked byte delivery |
| Single-developer throughput | High | Medium | Milestones are ordered so each is independently demoable; docs-first keeps decisions cheap to revisit |

---

## Definition of done for the whole project

1. Every MUST item in `00-research.md` §5 is implemented and covered by a test.
2. All quality gates green on Windows and Linux CI.
3. Performance targets measured and met (not asserted by eye).
4. Accessibility checklist completed and contrast/RTL lint green.
5. Installers produced and manually verified on a clean Windows and Linux machine.
6. Documentation complete: README, CONTRIBUTING, user guide, shortcuts, CHANGELOG, rustdoc.
7. `v1.0.0-rc.1` tagged; release notes written.
