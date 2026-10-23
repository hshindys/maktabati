# AUDIT — maktabati (Phase 0 study, 2026-10-23)

Studied end-to-end: docs, crates, Tauri command surface, and every Svelte view/button. Baseline:
`cargo test --workspace` → **229 unit + 7 doc tests, all pass**; `npm test` (vitest) → **8 files / 89 tests, all pass**.

## 1. Architecture

Rust-first per ADR-0001: all logic lives in workspace crates; the webview is presentation only (`src/`), talking to Rust through one typed IPC surface.

```
src-tauri/  Tauri v2 shell — only wiring: window, tray, logging, plugins, IPC commands
  commands/ mod.rs (diagnostics+settings), catalogue.rs (CRUD), files.rs (attachments), import.rs (Goodreads preview)
  error.rs  AppErrorDto { code, message, i18n_key, details } — every failure crosses IPC as one shape
  state.rs  AppState { db: Arc<Db>, data_dir, app_version }

crates/
  domain/   entities + validation (Book, BookInput, Isbn, Arabic normalize …)
  storage/  rusqlite Db, checksummed migrations, seed, typed repo/* per aggregate
  metadata/ only Goodreads CSV parser today (planned: providers)
  files/    attachment copy/reference + sha256 dedupe (used by file_* commands)
  search|notes|stats|backup: empty scaffolds for later milestones

src/
  lib/api.ts     the only importer of src/bindings.ts (tauri-specta generated)
  lib/views/     Dashboard, Library, BookPage, AddBook (wizard modal), Import, Settings, Gallery(dev), Placeholder
  lib/i18n/      flat dotted ar/en catalogues; missing key = TS error
  lib/router.ts  hash router, 16 routes
```

Command count: **6 diagnostics/settings + 44 catalogue + 5 files + 1 import = 56**, all registered through `collect_commands!` (a command cannot exist untyped on either side; ADR-0002). Bindings drift gate: `bindings_are_up_to_date` test.

Data at `~/.local/share/com.maktabati.desktop/`: `library.db` (SQLite WAL), `covers/`, `files/`, `backups/` (auto pre-migration), `logs/maktabati.log`.

## 2. Commands (IPC surface)

| Group | Commands |
|---|---|
| Diagnostics/settings | `app_info`, `library_health`, `library_summary`, `settings_all`, `settings_get`, `settings_set` |
| Authors | `author_{create,get,list,update,delete,search}` |
| Books | `book_{create,get,detail,list,update,delete,find_by_isbn,find_duplicates}` |
| Categories | `category_{create,get,list,children,update,delete}` |
| Languages | `language_{create,get,list,list_active,update,delete}` |
| Publishers | `publisher_{create,get,list,update,delete,search}` |
| Series | `series_{create,get,list,update,delete,search}` |
| Tags | `tag_{create,get,list,update,delete,search}` |
| Files | `file_{attach,list,delete,open,reveal}` |
| Import | `goodreads_parse(text) → GoodreadsRow[]` (pure parse, no writes) |

Notable: `book_update` replaces credits/categories/tags wholesale in one transaction — a full-book edit form is feasible with one `BookInput`. `repo::book::delete` cascades cycles/links/notes/files/quotes/search rows via FKs. There is **no quote/review command** today even though both tables exist in the schema.

## 3. Views & routes

| Route | View | State |
|---|---|---|
| `/` | Dashboard (Kpis, recent, empty state) | works |
| `/library` | Library (grid/list/table, sort) | works; rows link to book page |
| `/book/:id` | BookPage (7 tabs) | works; **no edit, no delete** |
| `/import` | Import (Goodreads) | *buggy, see §4* |
| `/settings` | Settings (appearance only) | works but bare |
| `/dev/gallery` | Gallery (DEV only) | works |
| `/reading /notes /quotes /authors /publishers /taxonomy /lending /goals /transfer /settings/backup /settings/diagnostics` | `Placeholder.svelte` | **stubs by design** |

All buttons in Dashboard / Library / BookPage-files / TitleBar / Sidebar / Gallery have handlers wired (verified). AddBook wizard steps 1–5 fully wired (draft in localStorage, inline entity create, duplicate warning, save+attach).

## 4. Dead buttons / dead ends (user pain #1 root cause)

1. **BookPage has no edit / no delete for the book itself** — the CRUD commands exist and are tested but nothing in the UI calls `book_update` / `book_delete`. This is exactly the perceived "edit/delete missing".
2. **Sidebar → nine Placeholder screens** (reading, notes, quotes, authors, publishers, taxonomy, lending, goals, transfer). Honest "later" screens, but the dashboards empty-state CTA "استيراد/تصدير" lands on one — a dead end users click on day one.
3. **Import.svelte is broken on its own data**: it reads `r.year`, `r.pages`, `r.isbn`, which do not exist on `GoodreadsRow` (fields are `publication_year`, `page_count`, `isbn10/isbn13`) → **year/pages/ISBN are silently never imported**. It also ignores `status`, `rating`, `tags`, `review`, `date_read`; maps 0–5 rating nowhere; creates no publisher; hard-codes Arabic strings with zero i18n keys; and casts everything to `any`.
4. **Goodreads parser is thinner than needed**: no `Publisher`, `Binding`, or `Owned Copies` columns read; shelf tags and `My Review` are parsed (`tags`, `review`) but unused by the commit path.
5. **Settings page** renders only appearance; `about.*` and `health.*` i18n keys exist but nothing uses them (no About / storage-health display), and there is no export/import.
6. Quotes notes tabs on BookPage are designed empty states ("يصل في المرحلة التالية") — the `quotes` table + FTS index already exist in `0001_init.sql`.

## 5. DB schema (from migrations)

- `0001_init.sql`: languages, publishers, authors (+name_norm unique), series, categories (tree, seeded 51), tags, **books** (both ISBNs unique-when-present, denormalized reading status/rating/progress, room/shelf/box, publisher link is `0002`), junctions `book_authors` (role), `book_categories`, `book_tags`, reading_cycles/sessions, loans, goals, activity_log, reviews/rubric, notes_page/note_block/note_link, **quotes** (text+text_norm, note, page, chapter, color enum, source enum, FK file), book_files (sha256 unique in copy mode), shelves, custom columns, app_settings, metadata_cache, search_document + `fts_book`/`fts_book_norm`/`fts_note`/`fts_quote` (FTS5, Arabic diacritics-insensitive).
- `0002_book_publisher.sql`: `books.publisher_id` FK.
- Quote colors are a seeded enum set (`QuoteColor::ALL`); schema-conformance tests pin every Rust enum value to a DDL CHECK.

## 6. i18n audit

266 keys defined in ar.ts (en.ts type-forced to a full mirror; test also asserts identical key sets). Static `t("…")` usage audit: **no used-but-undefined keys**. Gaps: ~120 defined keys unused (health/about/library.empty… — will be consumed by Settings/storage screens), while **Import.svelte defines no keys at all** (literal Arabic strings). Any new screen work must add keys to both catalogues.

## 7. Test status

| Suite | Result |
|---|---|
| `cargo test --workspace` | ✅ 229 unit (app 18, domain 102, files 5, metadata 4, storage 100) + 7 doc-tests |
| `npm test` (vitest) | ✅ 89 tests, 8 files |

## 8. Recommended fix order (matches OPENCODE-BRIEF phases)

1. **P1 BookPage edit + delete** (backend ready — pure frontend) + Settings storage/about card (uses existing commands).
2. **P2 Goodreads completeness**: fix the wrong-field-name bugs, extend parser (publisher/binding), commit tags/review/status/rating-published, rewrite Import.svelte with i18n + correct types, parser tests.
3. **P3 Quotes**: `quotes` table already exists → domain entity + `repo::quote` + 4 commands + bindings + api.ts + BookPage quotes tab + global `/quotes` screen.
4. **P4 Hermes**: OpenLibrary enrichment in `crates/metadata` (reqwest already a workspace dep) + optional axum HTTP API on 127.0.0.1:17321 (settings toggle) + `enrich <isbn>` CLI through the existing binary.
5. **P5 Tests** for every new command/parser/UI path; keep the bindings drift gate green.

*Deferred / deliberately not built now*: restore-into-DB (risky without a tested snapshot flow), per-row edit/delete in Library (BookPage actions cover the flow), reader/notes screens beyond the honest placeholders.

---

# Post-fix status (same session, 2026-10-23)

All five phases were executed. The sections above are left as the **baseline
record**; the numbers below supersede them.

## New command surface (56 → 66)

`quote_{create,list,list_all,update,delete}` (5), `metadata_enrich`,
`metadata_enrich_search`, `hermes_start`, `hermes_stop`, `hermes_status` (5).
All registered in `collect_commands!` **and** pinned by the
`assert_command_surface` snapshot test; `src/bindings.ts` regenerated (+232
lines) and the drift gate passes.

## Fixes applied, phase by phase

- **P1** BookPage book edit (`BookEdit` modal → `book_update`) and delete
  (native confirm → `book_delete`); Settings gains Storage (backup +
  size/integrity facts), Hermes, and About cards backed by real `app_info` /
  `library_health`; Dashboard CTA no longer dead-ends.
- **P2** Parser reads `Publisher` + `Binding` (`binding_to_format`). Import
  rewritten: correct `GoodreadsRow` fields (§4.3's silent drops fixed),
  status/rating/publisher/binding/tags mapping, find-or-create with per-run
  caches, ISBN-dedup **skips** counted apart from failures, full i18n,
  preview capped at 25 rows. Series/description/cover stay null (not CSV
  columns); review text visible in preview but dropped (reviews milestone).
- **P3** `domain::quote` + `repo::quote` + `quote_*` commands; `QuoteEdit`
  modal; BookPage quotes tab is a real list (colour dot, page/chapter, note);
  global `/quotes` route registered in App.svelte — read-only + edit/delete
  (creation lives on a book page, which owns the id).
- **P4** `metadata::openlibrary` pure parsers (`EnrichedBook`,
  `parse_openlibrary_data` for data/details, `parse_openlibrary_search`);
  src-tauri `commands/enrich.rs` (reqwest rustls; `/enrich` **never writes**)
  and `commands/hermes.rs` — axum 0.8 on loopback `127.0.0.1:17321`
  (`/health`, `GET /books?query=` via new `repo::book::search` over the norm
  mirrors + exact ISBN, `POST /books`, `GET|PATCH /books/{id}`,
  `POST /enrich`), graceful shutdown through `AppState.hermes`, opt-in
  autostart on `hermes.server.enabled`; CLI `maktabati enrich <isbn>`
  (0 = JSON or `null`, 1 = error, 2 = usage).
- **P5** worker suites below; bindings drift gate green; clippy `-D warnings`
  and `cargo fmt --check` clean (two pre-existing lint/possession drifts in
  `domain/file.rs` and one new-lint in `goodreads.rs` fixed en route).

## Final gate results

| Gate | Result |
|---|---|
| `cargo test --workspace` | ✅ all crates green (app 25 incl. 7 hermes HTTP, domain 102, files 5, metadata 14, storage 107, +doc-tests) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | ✅ clean |
| `cargo fmt --all -- --check` | ✅ clean |
| `npm test` (vitest) | ✅ 100 tests / 9 files (`goodreads.test.ts` +11, `i18n.test.ts` extended) |
| `npm run typecheck` | ✅ 0 errors |
| `npm run lint:logical-css` | ✅ clean |
| bindings drift (`bindings_are_up_to_date`) | ✅ up to date |
| `npm run tauri build` | ✅ DEB + AppImage bundles produced |

## i18n additions

ar.ts / en.ts extended in lockstep: `quote.*`, `quotes.*`, `book.quotes.*`
(real state, not empty-state copy), `import.*` (the view previously defined
**zero** keys), `settings.health.*`, `settings.hermes.*`,
`settings.about.*`, `toast.quoteSaved/quoteDeleted/imported/backupDone/
hermesStarted/hermesStopped`, and `error.enrich.*` / `error.hermes.*`
(registered in the i18n drift test's Rust-code allow-list;
`import.col.isbn` allow-listed as legitimately identical — ISBN stays Latin
in Arabic bibliography).

## Still open (deliberate)

- FTS `search_document` sync is untouched; Hermes search uses the norm
  mirrors until that milestone lands (also why `repo::book::search` is LIKE,
  not FTS — it must work on any DB).
- Reviews record, restore-into-DB, and reader/lending/goals screens unchanged.
- All changes remain **uncommitted**.
