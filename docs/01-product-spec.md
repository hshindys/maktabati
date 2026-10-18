# 01 — Product Specification

**Product:** Maktabati / مكتبتي — Personal Library Manager
**Version:** 1.0.0-rc (spec)
**Last updated:** 2026-10-18
**Status:** Approved for implementation (Phase 1)

---

## 1. Vision

> A calm, fast, private desktop home for a personal book collection — one that treats Arabic
> books as a first-class citizen, not a localization afterthought.

Maktabati is a **local-first desktop application** for cataloguing, reading, annotating and
understanding a personal book collection. Everything lives on the user's machine: a SQLite
database, cover images, book files and backups. There is no account, no server, no telemetry.

The product's three differentiators versus everything researched in `00-research.md`:

1. **Bilingual by design.** Arabic is the default language and the layout, search, typography,
   date display and digit style all work natively in Arabic — and switch live to English.
2. **My criteria, not a crowd's.** A user-defined rating *rubric* (معايير التقييم) lets the owner
   score books on their own dimensions, and roll those scores up **per author and per publisher**,
   so they learn who is consistently good *by their own measure*.
3. **Catalogue + notes + reader in one place.** A Calibre-grade catalogue, a Notion-style per-book
   notes workspace, and an in-app EPUB/PDF reader that feeds highlights straight into quotes —
   without any cloud.

---

## 2. Target user

**Primary persona — "Hatem"**

| Attribute | Value |
|---|---|
| Role | Individual reader / collector, single user, single machine (plus backups) |
| Languages | Reads **Arabic** and **English**; UI defaults to **Arabic** |
| Collection | Physical books **and** e-book files (EPUB/PDF/MOBI/TXT/audio), hundreds to tens of thousands |
| Devices | Windows primarily; also Linux and macOS |
| Motivation | Wants to *know* their library: what they own, what they're reading, what they thought, who publishes well |
| Pain today | Goodreads/StoryGraph are English-centric and cloud-bound; Calibre is powerful but dated and not Arabic-friendly; no tool combines catalogue + notes + rubric + reader locally |
| Technical level | Comfortable with a desktop app; does not want to run a server |

**Secondary persona — "the collector."** Catalogues a physical shelf (room/shelf/box), cares about
condition, acquisition price, lending out books, and printing/paper quality. (Drives: physical
location, condition, lending tracker, rubric dimensions like print quality and value for price.)

**Non-persona:** libraries, multi-user organizations, social/networked readers, teams.

---

## 3. Scope

### 3.1 In scope

| Area | Summary |
|---|---|
| Catalogue | Books, authors (multi-role), publishers, hierarchical categories, tags, languages, series, physical location, condition, acquisition, custom columns |
| Reading | Status, multiple reading cycles, sessions, progress %, goals, streaks, activity log |
| Evaluation | Overall half-star rating, user-defined rubric dimensions, reviews, dated comments, aggregates per author/publisher |
| Knowledge | Per-book block notes with sub-pages, templates, backlinks, Markdown export; quotes & highlights |
| Files | Attach by copy/reference, SHA-256 dedupe, cover extraction, folder scanning, in-app EPUB/PDF reader |
| Discovery | ISBN/title metadata lookup (Open Library, Google Books), cover fetch, duplicate detection & merge |
| Browsing | Grid/list/table, virtualized to 50k+, filters, sorting, saved shelves (static + smart), command palette |
| Insights | Dashboard with breakdowns, charts, heatmap, goals, top-rated |
| Data | CSV/JSON import/export with column mapping, Goodreads & Calibre import, backup/restore |
| Platform | Windows (primary), Linux, macOS; system tray; auto-updater; optional app lock |

### 3.2 Non-goals (explicit)

- ❌ **No accounts, cloud sync, social features, sharing, or telemetry.** Local-first is a
  product value, not a limitation.
- ❌ **No DRM removal or format conversion.** We read what the user legally owns; we do not
  crack or transcode DRM'd content.
- ❌ **No server/self-host mode** and no multi-user access control.
- ❌ **No book-buying, price comparison, or store integration.**
- ❌ **No recommendation engine that phones home.** "Similar books" uses local data only.
- ❌ **No OCR of scanned PDFs** in v1 (may read embedded text only).
- ❌ **No note collaboration / real-time co-editing.**
- ❌ **No mobile apps** in v1.
- ❌ **Not a general PKM** (it is book-scoped; it does not replace Obsidian for life notes).
- ❌ **No scraping of HTML sites** for metadata (explicitly avoided; see research §4.1).

---

## 4. User stories

### 4.1 Catalogue

- **US-01** As a reader, I can add a book by typing an ISBN so the metadata is pre-filled for me
  to review before saving.
- **US-02** As a reader, I can add a book manually with any subset of fields, because Arabic books
  often have no ISBN record anywhere.
- **US-03** As a collector, I can record *where* a book physically is (room → shelf → box) and its
  condition, so I can find and value it.
- **US-04** As a reader, I can create an author/publisher/category inline from the add-book form,
  so I am never blocked by a missing entity.
- **US-05** As a power user, I can define my own custom columns (text/number/date/bool/enum/rating)
  and use them everywhere filtering and sorting works.
- **US-06** As a reader with duplicates, I can detect them (by ISBN and by normalized
  title+author) and **merge** two records, keeping the richer data.
- **US-07** As a user, I can drag-and-drop a cover image onto a book to set it.

### 4.2 Reading & progress

- **US-08** As a reader, I can set a book's status to to-read / reading / finished / paused /
  abandoned and see it everywhere (filters, dashboard, shelves).
- **US-09** As a re-reader, I can have multiple reading cycles per book, each with its own
  start/finish dates, without losing history.
- **US-10** As a reader, my current page updates the progress % automatically, and I can log a
  session (date, pages, minutes) by hand.
- **US-11** As a reader, I can set a yearly reading goal and see my progress and streak.
- **US-12** As a lender, I can record who borrowed a book, when it is due, and get a reminder.

### 4.3 Per-book page

- **US-13** As a reader, I open a book's dedicated page and see, in tabs: Overview, Notes,
  Quotes, Reading log, Reviews, Files, Related.
- **US-14** As a reader, on Overview I see a large cover, full metadata, categories/tags, status,
  progress bar, rating and quick actions at a glance.
- **US-15** As a reader, on Related I see books by the same author, publisher, series and
  category, plus local "similar books" suggestions.

### 4.4 Notes

- **US-16** As a note-taker, I write in a block editor with headings, lists, checkboxes, quotes,
  code, callouts, toggles and tables, and I insert blocks from a **slash menu**.
- **US-17** As a note-taker, my work **autosaves** — I never press save and never lose text,
  even on a crash.
- **US-18** As a note-taker, I create **nested sub-pages** (e.g. Chapter summaries, Ideas,
  Questions, Vocabulary) inside a book's workspace.
- **US-19** As a note-taker, I start from a **customizable default template** for new book notes.
- **US-20** As a note-taker, I type `[[` to link to another book or note, and I can see
  **backlinks** pointing at the current note — at block granularity.
- **US-21** As a note-taker, I can export a book's notes to Markdown and read them in any editor.
- **US-22** As a note-taker, I search across all notes and quotes from the command palette.

### 4.5 Reader & quotes

- **US-23** As a reader, I open an attached EPUB or PDF **inside the app** with themes (incl.
  sepia), font size/family controls, margins, RTL support and bookmarks.
- **US-24** As a reader, selecting text lets me highlight it with a colour; a highlight can carry
  my comment and becomes a **Quote** linked to that reader position.
- **US-25** As a reader, when I close the reader my position is remembered and restored — and it
  drives `current_page` / progress.
- **US-26** As a reader, I can jump from a quote back to the exact reader position.

### 4.6 Evaluation

- **US-27** As a critical reader, I rate a book 0–5 in **half stars**.
- **US-28** As a critical reader, I define my own rating criteria (e.g. عمق المحتوى، أسلوب
  الكتابة، التدقيق والتحرير، جودة الطباعة والورق، جودة الترجمة، جودة مقابل السعر) and score each
  book on them.
- **US-29** As a collector, I see **average rubric scores per author and per publisher**, so I
  can tell who is consistently good by my own standards.
- **US-30** As a reader, I write an overall review plus multiple dated comments over time.

### 4.7 Browsing & search

- **US-31** As a user with 50,000 books, I browse a cover-wall grid that scrolls at 60 fps.
- **US-32** As a user, I filter by status, language, category, author, publisher, rating, format,
  year, tags, has-file, and rubric scores — and combine filters.
- **US-33** As a user, I save a filter as a **shelf** (static or smart) and pin it in the sidebar.
- **US-34** As a user, I press `Ctrl/Cmd+K` and search books, authors, publishers, notes and
  quotes at once, in Arabic **or** English, and jump straight to a result.
- **US-35** As a user searching in Arabic, I find a book when I mistype أ/إ/آ, ي/ى, ة, or when I
  omit tashkeel — and my query may mix Arabic and English.

### 4.8 Dashboard

- **US-36** As a user, my dashboard shows totals and breakdowns by status, language, category,
  publisher, author and format.
- **US-37** As a user, I see books finished and pages read per month/year, my reading streak, and
  yearly goal progress.
- **US-38** As a user, I see a GitHub-style heatmap of my reading activity.
- **US-39** As a user, I see widgets for currently reading, recently added, recently finished,
  top rated, and my to-read queue.

### 4.9 Import / export / backup

- **US-40** As a migrator, I import a **Goodreads CSV** and get shelves, ratings, dates and
  reviews mapped correctly.
- **US-41** As a Calibre user, I import a Calibre library/export including custom columns.
- **US-42** As a user, I import CSV or JSON through a **column-mapping step** and preview rows
  before committing.
- **US-43** As a user, I point the app at a folder of e-books and it creates entries from embedded
  metadata (EPUB OPF / PDF XMP), showing progress and letting me review before saving.
- **US-44** As a cautious user, I export the whole library to JSON/CSV at any time.
- **US-45** As a user, I get automatic backups (scheduled + before every migration) and can do a
  manual backup/restore with an **integrity check** that tells me if the archive is sound.

### 4.10 Settings, language, accessibility

- **US-46** As an Arabic reader, I see Arabic UI by default, and I can switch to English **live**
  with the whole layout mirroring — no restart.
- **US-47** As a user, I choose Arabic-Indic (٠١٢) or Western (012) digits.
- **US-48** As a user, I choose Gregorian, Hijri, or both date displays.
- **US-49** As a user, I pick a light "warm paper" theme, a deep dark theme, a sepia reading
  theme, and an accent colour.
- **US-50** As a keyboard-only user, I can reach every control with visible focus rings and use
  documented shortcuts; as a screen-reader user, controls are labelled and contrast meets AA.
- **US-51** As a privacy-conscious user, I can lock the app with a PIN/password (argon2).
- **US-52** As a user, I can read logs in Settings and export a diagnostic bundle when something
  breaks.

---

## 5. Success criteria

### 5.1 Performance (measured by automated benchmarks in CI, not by assertion)

| Metric | Target |
|---|---|
| Cold start (main window painted with shell) | **< 2 s** |
| Search results for 50,000 books | **< 100 ms** p95 |
| Grid scroll | **60 fps** (no long tasks > 50 ms during scroll) |
| Import 50,000 CSV rows | completes with progress, transactional, < 5 min on a mid-range machine |
| Memory at 50k books (idle browser) | < 600 MB RSS |

### 5.2 Reliability

- SQLite in **WAL** mode; all multi-statement writes in transactions.
- **Zero data loss** on crash during note autosave (crash-safety covered by a fault-injection test).
- Backup archive verifies with SHA-256 manifest; restore refuses corrupt archives.
- Migrations run inside a transaction with an automatic pre-migration backup.

### 5.3 Quality gates (must pass for every merge)

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace        # (or cargo test if nextest unavailable)
cargo deny check                     # licences + advisories + duplicates
```
Plus: TypeScript typecheck, frontend unit tests, and CI build on **Windows + Linux**.

### 5.4 Correctness of Arabic

- **Property tests** prove normalization is idempotent and that `normalize(q)` matching
  `normalize(doc)` behaves for alef/hamza/ya/ta-marbuta/tashkeel/tatweel variants.
- **Fixture tests** cover ≥ 40 real Arabic title/author strings incl. mixed-script strings.
- Snapshot tests prove RTL layout mirroring and that an Arabic title + English author render with
  correct per-field direction.

### 5.5 Usability

- A first-run user can add their first book (by ISBN or manually) in **< 60 seconds**.
- All keyboard shortcuts documented in `docs/shortcuts.md` and discoverable in-app.
- Every empty state has an illustration/heading and a primary action.

### 5.6 Definition of Done (per milestone)

1. Feature implemented behind a typed Tauri command.
2. Unit + integration tests written and passing.
3. `fmt`, `clippy -D warnings`, all tests green.
4. Docs/ADR updated if a decision changed.
5. Conventional commit(s) pushed.

---

## 6. Release channels

| Channel | Purpose |
|---|---|
| `nightly` | Built from `main` on every merge; auto-update enabled; may be broken |
| `beta` | Cut weekly from `main`; smoke-tested |
| `stable` | Tagged release; MSI/NSIS (Windows), AppImage + deb (Linux), dmg (macOS) |

---

## 7. Open questions (recorded, with defaults chosen)

| # | Question | Decision |
|---|---|---|
| Q1 | Barcode scanning via webcam? | **Default: attempt `rfd`/native camera only if trivially available; otherwise ship "type or paste ISBN"** and record as COULD. (ADR-0009) |
| Q2 | Hijri conversion library? | **Default: use `umalqura`-style table via a small vetted crate if available; otherwise implement Umm al-Qura table in `domain` with tests.** Fallback if blocked: Gregorian only, documented. |
| Q3 | Auto-updater signing keys? | **Default: configure the updater but ship it disabled until a signing key exists** (safe default; enabling is a release-engineering step, not a code change). |

---

## 8. Related documents

- `00-research.md` — evidence base and feature prioritization
- `02-architecture.md` — how it is built
- `03-data-model.md` — what is stored
- `04-ux-design.md` — how it looks and behaves
- `05-roadmap.md` — when it ships
