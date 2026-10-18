# ADR-0005 — Database access layer: `rusqlite` (deviation from the brief's sqlx / SeaORM)

**Status:** Accepted · **Date:** 2026-10-18 · **Type:** Deviation from the suggested stack
(allowed by the brief: *"justify deviations in an ADR"*)

## Context

The brief suggests *"sqlx with compile-time checked queries, or SeaORM — choose and justify"*.
Our workload is: **one embedded SQLite file, single user, local-first desktop app**, with FTS5,
JSON columns, CTE-heavy reporting queries, and a large number of read paths with dynamic filters.

## Research findings used (`00-research.md` §2, §6)

Three independent 2026 Rust-ORM comparisons converge on:

> *"Is the database SQLite, embedded in a CLI or desktop app? Use **rusqlite**."*
> *"All async SQLite crates perform considerably worse than their sync counterparts. This is
> likely due to SQLite not providing an async API at all."*

Plus concrete operational facts:

- sqlx's `query!` macro needs a **live database at compile time** or a committed `.sqlx` cache —
  a documented, recurring CI trap (`DATABASE_URL` missing → build fails). The cache must be
  regenerated on every query/schema change (`cargo sqlx prepare`), which fights our
  migration-heavy development loop.
- SeaORM is an ORM with runtime-checked queries; we want full control of SQL (dynamic filters,
  FTS5 MATCH construction, reporting CTEs) and *explicit* nullability, which an ORM obscures.
- sqlx cannot infer `NULL`ability of a left-joined column (documented limitation) — exactly the
  shape our list/detail queries have.

## Decision

Use **`rusqlite` 0.40.2** with the `bundled` feature (so FTS5 and a known SQLite version ship
inside the binary, no system dependency), wrapped in our own thin layer:

```
crates/storage/src/
├── conn.rs        # open(), PRAGMAs (ADR-0003), pooled handle
├── migrations.rs  # numbered, checksummed, transactional, pre-migration backup hook
├── rows.rs        # helpers: get_optional<T: FromSql>, parse JSON columns, bool<->i64
├── repo/          # book.rs, author.rs, publisher.rs, category.rs, reading.rs, ...
└── tx.rs          # with_tx(|tx| ...) closure helper
```

- **No async in `storage`.** The crate is fully synchronous and `Send`.
- `app` calls it through `tauri::async_runtime::spawn_blocking`.
- A tiny **pool** (2–4 `Connection`s) with `busy_timeout` gives 1 writer + readers under WAL.
- **Typed query builders are hand-written, not macro-generated.** Compile-time safety comes from:
  1. typed row decoders (`FromSql` impls on domain value objects),
  2. **integration tests that execute every repository query** against a temp DB (so a schema
     mismatch fails in CI, not in production),
  3. `cargo sqlx`-style drift is impossible because there is no second artifact.

### Consequences

- ✅ Fastest option for embedded SQLite; no runtime/async overhead; sync transactions are natural.
- ✅ Zero build-time DB requirement → fast, deterministic CI.
- ✅ FTS5 + JSON1 + `VACUUM INTO` all available with no extra wiring.
- ⚠️ We lose `query!`'s compile-time SQL checking. **Mitigation is mandatory and non-negotiable:**
  every repository function has an integration test that runs it against a migrated temp database
  (`storage` test harness), and clippy forbids raw string-concatenated SQL (only `&str` literals
  or `sqlx`-free parameter binding).
- ⚠️ Sync API could block if misused. **Mitigation:** `storage` is only reachable from `app`
  through `spawn_blocking`; a debug assertion detects being called on the async runtime thread.

## Alternatives considered

| Option | Why rejected |
|---|---|
| **sqlx 0.9** | Async-without-async (measured slower per three 2026 comparisons); compile-time DB requirement; nullability inference gaps on LEFT JOIN; offline `.sqlx` cache churn. |
| **SeaORM 2.0** | ORM layer we don't want; runtime-checked (weaker than compile-time, stronger than nothing — but adds a model/ActiveModel layer over SQL we want to own); N+1 risks in relationship loading. |
| **Diesel** | Strong guarantees but a DSL to learn; its SQLite async story adds `diesel-async` complexity; SQL readability for a personal project favors plain SQL. |
| **tokio-rusqlite / r2d2 + spawn_blocking** | Rejected *as a dependency*: our pool is ~60 lines and lets us place WAL/busy-timeout/backup hooks precisely. |

## Migration note

If the project later needs Postgres (it does not — local-first is a product value), this decision
would be revisited; the `repo/` layer is the single seam that would change.
