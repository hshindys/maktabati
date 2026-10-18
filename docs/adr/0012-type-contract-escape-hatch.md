# ADR-0012 — Escape hatch if specta/tauri-specta cannot ship

**Status:** Accepted (contingency) · **Date:** 2026-10-18

## Context

ADR-0002 pins `specta`/`tauri-specta` at `=2.0.0-rc.25`. Both are on a pre-1.0 release-candidate
line (verified 2026-10-18). If a breaking change or abandonment blocks us, we must not fall back
to hand-written types — that violates the brief.

## Decision

Pre-ordered fallback, evaluated only if ADR-0002 becomes untenable:

1. **`tyzen` / `tauri-typegen`** (verified to exist on crates.io) — zero-boilerplate Rust↔TS
   binding generator designed for the Tauri DX.
2. **`ts-rs`** for types + a 50-line script that emits typed command wrappers from a single Rust
   manifest of command names (still generated, never hand-written).
3. **Self-maintained exporter** using `serde_reflection`-style schema extraction.

**Rule:** whichever is used, the generated file remains committed and CI fails on drift — the
property that matters is *"the contract is generated and verified"*, not which tool generates it.

## Status

Not needed as of 2026-10-18. Recorded so the decision is not re-litigated under pressure.
