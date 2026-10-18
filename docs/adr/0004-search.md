# ADR-0004 — Search: SQLite FTS5 (not tantivy), with Arabic normalization at write time

**Status:** Accepted · **Date:** 2026-10-18

## Context

Requirements: Arabic-aware full-text search over books, authors, publishers, notes and quotes;
mixed Arabic/English queries; < 100 ms for 50,000 books; ranking with snippets; and it must be
consistent with the data (no index drift).

Two credible options: **SQLite FTS5** (in-process, same file) or **tantivy** (a separate index
directory).

## Decision

Use **SQLite FTS5**, with a dedicated `crates/search` crate that owns:

1. **The FTS5 schema** — external-content virtual tables over a denormalized search projection,
   plus triggers so inserts/updates/deletes keep the index consistent *in the same transaction*.
2. **Arabic normalization applied at write time** into stored normalized columns
   (`title_norm`, `author_norm`, …) *and* to the query string:
   - Unicode **NFD**, then strip combining marks → removes tashkeel (fatha/damma/kasra/sukun/
     shadda/tanwin) and other diacritics;
   - strip **tatweel** (U+0640) and control/format characters;
   - fold **أ إ آ ٱ → ا**, **ى → ي**, **ة → ه** (configurable fold set), plus optional
     `ؤ → و`, `ئ → ي`;
   - collapse whitespace, trim, lowercase Latin;
   - optional **light stemming** (prefix/suffix trimming à la Larkey) behind a setting, default
     **off** for precision (research: light stemming can *reduce* precision if too aggressive).
   - **Idempotence is a property test:** `normalize(normalize(x)) == normalize(x)`.
3. **A query parser** supporting Calibre-style syntax (`title:…`, `author:…`, `rating:>=3`,
   quoted multi-word, `#custom:…`, boolean AND/OR/NOT) → SQL + FTS5 MATCH.
4. **Ranking & snippets** via FTS5 `bm25()` and `snippet()`, with a tie-break that prefers exact
   normalized matches and ISBN matches.
5. **Tokenizer:** FTS5 `unicode61` with `remove_diacritics 2` on the *raw* text column as a
   second line of defense, plus our own normalized columns as the primary path.

### Why not tantivy

- **Two sources of truth.** A separate index directory means a rebuild step, an async/sync bridge,
  and the classic failure "I added a book but search doesn't see it". FTS5 lives in the same
  transaction as the row.
- **Operational weight:** an index to ship, to corrupt, to repair (`calibredb check_library`-style
  maintenance is exactly the pitfall research flagged).
- **No ranking advantage at our scale.** BM25 in FTS5 is plenty for 50k docs; tantivy's strengths
  (analyzers, facets, distributed search) are not needed.
- Backup/restore becomes trivial: one file.

### Why not LIKE/`instr`

Cannot rank, cannot snippet, and is O(n) per keystroke — fails the 100 ms target at 50k rows.

## Consequences

- ✅ Single-file durability; search correctness inherits SQLite's transactional guarantees.
- ✅ `crates/search` is fully testable with in-memory `:memory:` databases.
- ⚠️ FTS5 must be available in the bundled SQLite. `rusqlite` with the `bundled` feature ships
  FTS5 enabled — verified at scaffold time by a runtime `CREATE VIRTUAL TABLE` smoke test in CI.
- ⚠️ Our normalization is *our* contract. Because raw text is also stored, we can re-normalize
  retroactively by re-running the projection (a maintenance command ships for this).

## Alternatives considered

| Option | Why rejected |
|---|---|
| **tantivy** (0.27.0 verified) | Drift risk + extra artifact; see above. |
| **SQLite FTS4** | No `bm25()` ranking or `snippet()` of comparable quality; FTS5 chosen. |
| **Trigram/`LIKE`** | No ranking/snippets; slower. |
| **External search service** | Violates local-first. |

## References

Kadri & Nie, *Effective stemming for Arabic information retrieval* (ACL 2006); Larkey's light
stemmer; Arabic normalization pitfalls — all listed in `00-research.md` §6.
