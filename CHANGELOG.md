# Changelog

All notable changes to Maktabati are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions are not yet tagged.

## [Unreleased] — 2026-10-23

Everything below is one working session across the OPENCODE-BRIEF phase list
(Phases 0–5). Left uncommitted on purpose.

### Phase 1 — Dead buttons fixed

- **BookPage**: real **edit** (`book_update` via a new `BookEdit` modal) and **delete**
  (`book_delete` with a native confirm that names the cascade) on the book itself.
- **Dashboard** empty-state CTA no longer lands on a placeholder screen.
- **Settings** gained three live cards: **Storage** (backup-now through `backup_library`,
  DB size / data-dir facts), **Hermes** (below), and an **About** section fed by real
  `app_info` (version, schema/migrations, platform, data dir) instead of i18n keys that
  nothing rendered.

### Phase 2 — Goodreads import completed

- **Parser** (`crates/metadata/src/goodreads.rs`): reads the `Publisher` and `Binding`
  columns; `binding_to_format()` maps paper→print, Kindle/NOOK/PDF→ebook/pdf,
  audio→audiobook; all 14 metadata tests green including table-driven binding fixtures.
- **Import view** (`src/lib/views/Import.svelte`) fully rewritten: correct
  `GoodreadsRow` field access (the old view read non-existent `year`/`pages`/`isbn`
  columns, silently dropping them), status/rating/publisher/binding/tags mapping,
  i18n for every string (ar + en), a preview table capped at 25 rows, warnings panel,
  and honest summary cards for **added / skipped / failed**.
- **Dedup**: rows whose canonical ISBN already exists in the library are *skipped* and
  counted separately from failures — a re-run must not invent second copies.
- **Find-or-create** for authors, publishers and tags with per-run caches.
- **Series, description and cover stay null**: they are not columns in a Goodreads
  export, so nothing is guessed on their behalf (documented in the view).
- **Review text** is shown in the preview but dropped on commit — the reviews record
  is a later milestone; staging it on books would be a lie.
- Mapping helpers extracted to `src/lib/goodreads.ts` (`STATUS_MAP`,
  `bindingToFormat`, `ratingToHalfStars`, `cleanIsbn`) — tested against the same
  fixtures as the Rust parser by `src/lib/goodreads.test.ts`.

### Phase 3 — Quotes (الاقتباسات)

- **Backend**: `domain::quote` + `repo::quote` (CRUD + `list_all` with owning-book
  join), four `quote_*` IPC commands, bindings regenerated (+232 lines).
- **Frontend**: `QuoteEdit` modal (add on a book page, edit/delete anywhere),
  BookPage quotes tab showing a real list with colour dots per `QuoteColor`,
  page/chapter/note, and a global **`/quotes`** screen (`quote_list_all`, newest
  first, link to the owning book). Read-only + edit/delete by design — an "add"
  button there would have to invent a book; creation lives on book pages.
- Quote highlight palette added to `app.css` (`--quote-*` × 6, bridged into
  Tailwind via `@theme`).

### Phase 4 — Hermes (Open Library enrich + loopback HTTP)

- **Pure parser** `crates/metadata/src/openlibrary.rs`: `EnrichedBook` +
  `parse_openlibrary_data` (data/details shapes) and `parse_openlibrary_search`
  (search.json docs); 8 tests. No I/O — the network layer stays thin and testable.
- **IPC**: `metadata_enrich(isbn)` / `metadata_enrich_search(title, author)`.
  **No command writes** — an enrichment is a *suggestion* the user is asked about
  (import / merge / discard).
- **Hermes HTTP API** (`src-tauri/src/commands/hermes.rs`): optional axum server on
  **127.0.0.1:17321 only**, endpoints `GET /health`, `GET /books?query=` (backed by a
  new `repo::book::search` over the normalized mirrors + exact ISBN — works on any DB
  because FTS sync doesn't exist yet), `POST /books`, `GET /books/{id}`,
  `PATCH /books/{id}` (whole-input replace, same semantics as IPC `book_update`), and
  `POST /enrich` (suggestion only, `{"suggestion": null}` on a miss — HTTP 200, not an
  error). Status-code map mirrors the IPC error codes.
- **Lifecycle**: `hermes_start/stop/status` commands for the Settings card, graceful
  shutdown via a `watch` channel in `AppState.hermes`, bind-before-mark-running, the
  std mutex is never held across an await, and opt-in **auto-start** at setup reads
  the `hermes.server.enabled` setting keyed toggle.
- **CLI**: `maktabati enrich <isbn>` runs headless before any Tauri app: exit 0 prints
  pretty JSON (or `null` when Open Library has no record), 1 on failure, 2 on usage
  error (stderr kept clean so stdout stays pipeable).
- Dependencies: axum 0.8 workspace dep; reqwest 0.13 features renamed to
  `rustls` + `query`; tokio gains `net`.

### Phase 5 — Tests & gates

- New suites: goodreads mapping (11), hermes HTTP router (7 → app crate now 25).
- Full gates green: `cargo test --workspace` (all crates), 
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all -- --check`, `npm test` → **100 tests / 9 files**,
  `npm run typecheck`, `npm run lint:logical-css`.
- **i18n**: every user-facing string added to **both** catalogues (ar-first), new
  `error.enrich.*` / `error.hermes.*` codes registered in the i18n drift test's
  Rust-code allow-list; `import.col.isbn` allow-listed as legitimately identical
  (ISBN stays Latin in Arabic bibliography).

### Known gaps (deliberate)

- Restore-into-DB is not built (needs a tested snapshot flow first).
- Goodreads series/description/cover and reviews remain null/unstored (data not in
  the CSV / milestone not reached).
- `/notes`, reader, lending, goals screens keep their honest placeholders.
- FTS `search_document` sync still pending; Hermes search uses the norm mirrors.
