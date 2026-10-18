# ADR-0010 — Error boundary & concurrency: one `AppError` DTO, sync SQLite on `spawn_blocking`

**Status:** Accepted · **Date:** 2026-10-18

## Decision

### Errors

1. Each library crate defines a **typed `thiserror` enum** with a stable, machine-readable code
   (e.g. `validation.isbn_invalid`, `storage.busy`, `metadata.rate_limited`, `notes.rev_conflict`).
2. `crates/app` defines `AppError` with `#[from]` for every library error and implements
   `Serialize` → `AppErrorDto { code: String, message: String, details: Option<serde_json::Value> }`.
3. The webview maps `code` → localized string from `errors.{code}`; `message` is a developer-facing
   fallback. Users never see a raw Rust `Display`.
4. `clippy.toml` sets `unwrap-used = "deny"`, `expect-used = "deny"` (allow-listed for tests and
   seed/demo generators).
5. Panics are reserved for programmer errors; a panic hook writes a crash record to the log so the
   diagnostic export captures it.

### Concurrency

- `storage`, `search`, `notes`, `stats`, `backup` are **fully synchronous** crates.
- `app` wraps every storage-touching command body in
  `tauri::async_runtime::spawn_blocking(...)` and awaits it.
- A small connection pool (2–4) with WAL allows concurrent readers and a single writer.
- `metadata` is the only async/network crate (`reqwest` + `rustls`), with a per-provider token bucket.
- **No lock is held across an `.await`.** State handles are `Arc<...>`; critical sections are short.
- Long operations (`import`, `scan`, `backup`) run on a dedicated blocking worker and emit
  progress **events** so the UI can render determinate progress and cancel.

## Consequences

- ✅ One place converts errors → IPC; localization stays in the webview where the strings live.
- ✅ No async tax on SQLite (ADR-0005), and the main thread never blocks → 60 fps UI.
- ⚠️ Extra ceremony per command (`spawn_blocking`). Mitigated by a `run_blocking!`-style helper
  in `app` so it is one line and consistent.
- ⚠️ Cancellation is cooperative (checked between batches), not preemptive.

## Alternatives considered

- **`anyhow` across the boundary** — loses machine-readable codes; used only *inside* `app` for
  non-user-facing internal failures.
- **A single global `Mutex<Connection>`** — simplest, but serializes reads and blocks the writer
  behind long report queries. Rejected.
- **Async SQLite (sqlx)** — rejected in ADR-0005 with evidence.
