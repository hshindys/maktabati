# ADR-0006 — Notes storage: structured blocks in SQLite with a Markdown projection

**Status:** Accepted · **Date:** 2026-10-18

## Context

The brief requires a Notion-style block editor per book: headings, lists, checkboxes, quotes,
code, callouts, toggles, tables; slash menu; autosave; **nested sub-pages**; **templates**;
**`[[backlinks]]`**; full-text search; and **Markdown export**. It also says: *"Store notes as
structured blocks in SQLite with Markdown export; design the format to be future-proof (ADR)."*

Options:

| | A. Structured blocks in SQLite | B. One Markdown file per page on disk | C. Single JSON document per page |
|---|---|---|---|
| Slash menu / block ops | ✅ natural (block = row) | ❌ must re-parse the whole file each keystroke | ⚠️ whole-document rewrite |
| Autosave & crash safety | ✅ row-level transaction | ⚠️ file rewrite races | ⚠️ whole-document rewrite |
| Backlinks at **block** granularity (Logseq lesson) | ✅ queryable | ⚠️ needs a scan/parse | ❌ |
| Nested sub-pages | ✅ parent_id | ⚠️ folders/links only | ⚠️ nesting in one doc |
| Search | ✅ FTS5 row per block | ✅ but must reindex files | ✅ |
| Interop / "your data in 50 years" | ⚠️ needs export (Markdown) | ✅ excellent | ⚠️ |
| Future block types | ✅ schema-free JSON payload per block | ❌ syntax is the schema | ✅ |

## Decision

**A. Structured blocks in SQLite**, with:

1. `notes_page` — a page belongs to a book *or* is a standalone/sub-page (`parent_id`),
   carries a title, template provenance, and timestamps.
2. `note_block` — `id`, `page_id`, `parent_block_id`, `position` (fractional index — see below),
   `type` (enum), `payload` (JSON per type), `created_at`, `updated_at`, `rev`.
3. **Block types (v1):** `paragraph`, `heading_1..3`, `bulleted_list`, `numbered_list`,
   `todo` (with `checked`), `quote`, `code` (with language), `callout` (icon + tone), `toggle`,
   `table` (rows/cols + cells), `divider`, `image` (attachment ref), `page_link` (→ another page),
   `bookmark` (→ a reader position / quote).
4. **Fractional positioning** (`position` = sortable string between neighbours) so drag-reorder
   never rewrites sibling positions; avoids O(n) updates on every move.
5. **Autosave** = debounced block-level patch (`notes::apply_patch { block_id, payload, rev }`)
   with optimistic-concurrency `rev` check; each patch is one transaction. Crash safety follows
   from SQLite, not from the editor.
6. **Backlinks** are *extracted*, not stored as truth: a `note_link(from_block, to_page_or_book,
   raw_text)` table is maintained on save, giving Logseq-style block-granular backlinks with an
   Obsidian-style `[[wikilink]]` authoring experience. Renames update links by id, not by string.
7. **Markdown projection** (`notes::export_markdown`, and import) is a pure codec in
   `crates/notes` — a lossless round-trip for supported block types, with `[[wikilinks]]` and
   fenced code preserved. This is the interop escape hatch: users can always get their data out.
8. **Templates** are ordinary pages flagged `is_template`; "new book notes" instantiates the
   designated default template by deep-copying its blocks.

### Why not B (files on disk)

The editor would re-parse and re-serialize the document constantly; block-granular backlinks and
`rev`-based conflict handling become ad-hoc; and a torn write loses a chapter. Obsidian/Logseq
work as files because their unit is the *page* and they accept re-parse cost; our unit is the
*block* with autosave-on-every-keystroke semantics. We still guarantee portability via export.

### Future-proofing

- Block `type` is a string enum with an `unknown` fallback: a newer version reading an older DB
  never drops data — unknown blocks render as read-only and round-trip untouched.
- `payload` is JSON validated per type on write (`serde` untagged enums), so schema evolution is
  additive.
- `rev` + `updated_at` give an audit trail and enable a future conflict/merge story.

## Consequences

- ✅ Block ops, drag-reorder, block-granular backlinks and search are all cheap SQL.
- ✅ Crash-safe autosave for free.
- ✅ Markdown export guarantees user data ownership (a stated product value).
- ⚠️ Markdown is a *projection*, not the source of truth — importing an externally edited file
  must be an explicit user action (Import/Replace), never an automatic watch, or we risk
  clobbering blocks.
- ⚠️ Requires a `note_link` maintenance step on save — covered by tests that assert links update
  when a `[[page]]` is renamed.

## Alternatives considered

- **B: one `.md` per page** — best interop, worst editing/backlink/autosave semantics. Rejected as
  primary storage; adopted as *export* format.
- **C: single JSON doc per page** — smaller change from B, but backlinks/search need parsing and
  every keystroke rewrites the whole document.
- **CRDT (automerge/yjs)** — overkill: single user, single device, local-first; adds heavy deps.
