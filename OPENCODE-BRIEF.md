# OpenCode task: maktabati — audit + feature work

Working directory: /home/hatem/Projects/maktabati

## Context
maktabati = Arabic-first, local-first library manager (Rust+Tauri+Svelte). User is hatem/Shindy. App installed at ~/.local/opt/maktabati. Frontend is Svelte 5 (runes). Backend is Rust (tauri v2). Bindings auto-generated in src/bindings.ts; regenerate with MAKTABATI_UPDATE_BINDINGS=1. Tests: vitest (frontend), cargo test (backend). Build: npm run tauri build (6-7 min).

## Known pain points (reported by user)
1. Many buttons don't work / are unimplemented placeholders.
2. Goodreads CSV import missing: publisher, author, series, description (abstract). Currently only title/author/isbn/year/pages/rating. Author linking was just added but untested.
3. Wants "Hermes integration": when adding a book, auto-search online (by ISBN/title) to fill author, publisher, series, description, cover. Hermes is an AI agent with internet; we need a hook point — either an HTTP API exposed by the app, or a CLI subcommand, or a shared SQLite DB. Decide simplest.
4. Wants easy edit + delete (edit book, delete book) — verify flows.
5. Wants quotes feature (اقتباسات): store quotes per book, view/edit/delete.
6. Wants to study the whole program thoroughly first.

## Phase 0 — Study (do first, produce report)
- Read README.md, CLAUDE.md, AGENTS.md, .cursorrules if present.
- Map modules: crates/ (core, metadata, db, commands?), src-tauri/, src/lib/.
- List all commands exposed (grep for #[tauri::command] / invoke_handler).
- List all views and their routes in router.ts.
- List every button in each view; mark which have handlers vs dead.
- Identify DB schema (migrations).
- Identify i18n keys usage vs defined.
- Run existing tests: npm test, cargo test. Report pass/fail.

Output: write AUDIT.md at project root with sections: Architecture, Commands, Views/Routes, Dead buttons, DB schema, Test status, Recommended fix order.

## Phase 1 — Fix dead buttons
- For each dead button found in Phase 0, either implement or remove + note.
- Priority: Library (edit/delete book), BookPage (edit), Settings (export/import DB), Sidebar navigation.

## Phase 2 — Goodreads import completeness
- Extend crates/metadata goodreads parser to also extract: publisher (from "Publisher" column if present), series (from "Series" / "Original Publication Year"?), description (Goodreads CSV has no description column — skip; note). Actually Goodreads CSV columns: Book Id, Title, Author, Author l-f, Additional Authors, ISBN, ISBN13, My Rating, Average Rating, Publisher, Binding, Number of Pages, Year Published, Original Publication Year, Date Read, Date Added, Bookshelves, Bookshelves with positions, Exclusive Shelf, My Review, Spoiler, Private Notes, Read Count, Owned Copies.
- Map: publisher (find-or-create publisher like author), bookshelves -> tags, My Review -> notes, binding -> format, Owned Copies.
- Series: Goodreads CSV does NOT include series. Leave series null; document.
- Update Import.svelte preview table to show author/publisher/tags.
- Surface per-row errors (already done).

## Phase 3 — Quotes feature
- DB: new table quotes(id, book_id FK, text, page, chapter, created_at, updated_at).
- Rust: commands quote_create/list/update/delete.
- Bindings regenerate.
- Frontend: api.ts wrappers; BookPage shows quotes list + add quote modal; i18n keys ar/en.

## Phase 4 — Hermes integration (auto-enrich on add)
Decision: expose a small HTTP server inside the Tauri app (axum on 127.0.0.1:17321) with endpoints:
  GET  /health
  GET  /books?query=
  POST /books  (create)
  GET  /books/:id
  PATCH /books/:id
  POST /enrich  {isbn or title+author} -> returns suggested fields (author, publisher, series, description, cover_url, year, pages) WITHOUT writing; caller confirms.
Hermes then calls these via curl. Also add a CLI: `maktabati enrich <isbn>` prints JSON.

Alternative simpler: just document the SQLite DB path and schema so Hermes reads/writes directly. Prefer HTTP for safety + validation. Implement HTTP server as optional (settings toggle) to avoid port conflicts.

Enrichment source: use existing metadata crate providers if any; else add OpenLibrary (free, no key) lookup by ISBN/title. Implement in crates/metadata.

## Phase 5 — Tests
- Add tests for new commands, parser extensions, quote CRUD.
- Frontend: at least smoke tests for new views.

## Constraints
- Do not break existing tests.
- Keep Arabic RTL, i18n for every user-facing string.
- Fish shell for any terminal commands given to user.
- Commit granularly? User didn't ask for git; just edit files.

## Deliverable
PR-style: changes + updated AUDIT.md + a short CHANGELOG entry. Run npm run tauri build at end to verify.
