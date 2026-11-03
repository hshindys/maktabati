# ADR-0004 — Search: SQLite FTS5 with Arabic-aware projections

**Status:** Accepted · **Date:** 2026-10-18

## Context

Search must work offline over books, authors, notes, and quotes; handle Arabic and English in the
same query; and stay transactionally consistent with the SQLite library. A separate index such
as tantivy would add a second durable store and a synchronization/rebuild failure mode.

## Decision

Use SQLite FTS5 in the library database. The schema lives in `crates/storage/migrations`; typed
projections are written by `crates/storage::repo::search_index`; `crates/search` owns query
construction and cross-entity results.

### Projection and consistency

- Books use `search_document` and `fts_book` / `fts_book_norm`. `sync_book` writes normalized
  Arabic/Latin text in the same transaction as book create/update. Relation triggers keep author,
  publisher, series, tag, and category fields current; a status trigger updates status/timestamp.
- Notes have one projection per page, including its title and all block text. Note-page/block
  repository writes refresh it in the same transaction.
- Quotes use `text_norm` and `note_norm`; quote-table triggers update their FTS index.
- Authors are indexed from the existing normalized name mirrors.
- FTS5 external-content triggers mirror projection inserts, updates, and deletes into the
  virtual tables in the same SQLite transaction. Foreign-key cascades remove deleted documents.
- Migration 0004 creates the schema. After migrations and seed data, startup performs one
  versioned Rust backfill through `reindex_if_needed`; it applies `domain::text::normalize` to
  legacy records and rebuilds all FTS tables before marking the backfill complete.

### Normalization and queries

Both stored projections and user input use `domain::text::normalize`: it removes tashkeel and
tatweel, folds alef/yaa/taa-marbuta variants, collapses whitespace, and lowercases Latin. FTS5's
`unicode61 remove_diacritics 2` tokenizer remains a second line of defense.

The current query grammar normalizes input, quotes each token, appends the FTS5 prefix operator,
and joins tokens with `AND`. This avoids interpreting user text as FTS operators while supporting
incremental word-prefix matching. The global palette calls the server-side `search` command after
a 150 ms debounce and requests at most eight results; the command caps results at 50. Empty
queries return recent books. Hermes `GET /books?query=` shares the same book search and preserves
its existing `Book[]` response shape.

`bm25()` ranks results within each FTS table. The current palette returns books, notes, quotes,
and authors with keyboard navigation. Advanced field syntax, boolean expressions, publisher
entity results, snippets, and a 50k-book benchmark remain future M5 work.

## Why not tantivy

- **Two sources of truth.** A separate index directory needs a bridge and can drift from library
  writes. FTS5 is in the same transaction as the rows.
- **Operational weight.** There is no second index directory to ship, corrupt, or repair.
- **No ranking advantage at this scale.** FTS5's BM25 is sufficient for the local library.
- Backup/restore stays within the SQLite file.

## Consequences

- ✅ Search state is backed up and restored with the library.
- ✅ Projection and FTS updates share the source write transaction.
- ✅ Tests cover Arabic normalization, mixed Arabic/English, books, notes, quotes, authors, and a
  5,000-book query below 100 ms.
- ⚠️ FTS5 must be present in bundled SQLite; `rusqlite`'s `bundled` feature provides it.
- ⚠️ Any normalization contract change needs a versioned projection reindex.
- ⚠️ Advanced search syntax and query snippets are not implemented yet.

## Alternatives considered

| Option | Why rejected |
|---|---|
| **tantivy** | Extra durable index and drift risk. |
| **SQLite FTS4** | FTS5 provides BM25 and better snippets when those are added. |
| **Trigram/`LIKE`** | No ranking and scales linearly for each keystroke. |
| **External search service** | Violates local-first operation. |

## References

Kadri & Nie, *Effective stemming for Arabic information retrieval* (ACL 2006); Arabic
normalization pitfalls — listed in `00-research.md` §6.
