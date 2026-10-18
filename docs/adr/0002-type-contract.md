# ADR-0002 — Generate the TS↔Rust contract with specta / tauri-specta

**Status:** Accepted · **Date:** 2026-10-18

## Context

Rust owns all domain logic and the webview is a thin client over Tauri commands. The brief
requires that shared TypeScript types are generated from Rust automatically so the contract is
never hand-written twice. Without generation, DTO drift is near-guaranteed within a week and
shows up as runtime JSON failures rather than compile errors.

## Decision

Use **`specta` + `tauri-specta`** (verified `2.0.0-rc.25` on 2026-10-18):

1. Every command: `#[tauri::command] #[specta::specta]`.
2. Every DTO: `#[derive(Serialize, Deserialize, specta::Type)]`.
3. Bindings exported to `src/bindings.ts`:
   - from a `#[test] fn export_bindings()` — the canonical, CI-checked path;
   - from `main()` under `cfg(debug_assertions)` for local dev convenience.
4. **Drift gate:** CI runs the export test and fails if `git diff` shows `src/bindings.ts`
   changed. A stale contract is a broken build, not a surprise at runtime.
5. The webview imports `import * as commands from './bindings'` — no hand-written `invoke()`
   strings anywhere.

## Consequences

- ✅ One source of truth; refactors break the build in both languages simultaneously.
- ✅ Autocomplete and type errors in the editor for every command.
- ⚠️ **Risk:** specta is on an RC line (`2.0.0-rc.25`). Mitigation: it is pinned exactly
  (`=2.0.0-rc.25`), the generated file is committed, and the surface we use (`Type`, `collect_types`,
  `ts::export`) is small. ADR-0012 documents the escape hatch (`tyzen`, `ts-rs`, or a hand-rolled
  serde-schema exporter) if specta is abandoned.
- ⚠️ Tauri command namespacing must be kept flat (`books_list`) or handled by tauri-specta's
  collection convention — decided at scaffold time and recorded in code comments.

## Alternatives considered

| Option | Why rejected |
|---|---|
| **ts-rs** | Generates types but not typed command *functions*; we'd hand-write the invoke layer. |
| **tyzen** (`tauri-typegen`/`tyzen-macro`, verified present) | Newer, less battle-tested; kept as documented fallback. |
| **Hand-written `api.ts`** | Explicitly disallowed by the brief. |
| **JSON-schema mid layer** | Extra indirection, no benefit at this scale. |
