# ADR-0013 — Dynamic JSON and 64-bit integers on the IPC contract

**Status:** Accepted · **Date:** 2026-10-19 · **Supersedes part of:** ADR-0002

## Context

ADR-0002 generates the TypeScript contract with `specta`/`tauri-specta` at `=2.0.0-rc.25`.
Building that contract surfaced two hard limits in the toolchain, both of which stop the export
outright rather than degrading it.

### `serde_json::Value` cannot be exported

`specta`'s built-in impl is hand-written and expands structurally at every use site
(`specta-2.0.0-rc.25/src/type/legacy_impls.rs`):

```rust
serde_json::Value as SerdeValue = inline;
// …inside SerdeValue::definition:
("Array",  … Field::new(Vec::<Value>::definition(types)))
("Object", … Field::new(Map::<String, Value>::definition(types)))
```

`Value` contains `Value`, so that call re-enters itself without bound. Each pass also pushes type
entries into the `&mut Types` argument, so the exporter allocates as it recurses. Observed
directly: the bindings export ran past 90 s of CPU and past 3 GB of RSS with no sign of
finishing, and a `MemoryHigh` cap simply stalled it (≈90 000 cgroup throttle events) instead of
letting it terminate. `specta-typescript` documents the same dead end: this "doesn't work for
external implementations like `serde_json::Value` … you don't control the definition."

`Settings::details`, `settings_get`, `settings_set` and `settings_all` all carried `Value`.

### 64-bit integers are refused

`specta-typescript` raises `bigint-forbidden` for `usize`, `isize`, `i64`, `u64`, `i128`, `u128`
(`specta-typescript-0.0.12/src/primitives.rs`), unconditionally — there is no exporter setting
that relaxes it. `AppInfo`, `LibraryHealth` and `LibrarySummary` use ~24 such fields.

## Decision

### 1. `maktabati_domain::Json` — a derived named type, not `serde_json::Value`

A six-variant `#[derive(specta::Type)]` enum (`Json`) with `#[serde(untagged)]`, plus a
`JsonNumber` newtype for the numeric case. Three properties make it work:

* **It is a *derived named* type**, so `specta::Types` inserts a placeholder while resolving it
  and a recursive reference resolves back to that placeholder instead of re-entering. This is the
  machinery the hand-written `SerdeValue` impl bypasses — `Types`' own docs describe exactly this
  placeholder behaviour.
* **`JsonNumber` defines `Type` itself**, emitting `Primitive::f64`. `serde_json::Number` inlines
  as `{ f64 } | { i64 } | { u64 }`, and the `i64` arm trips the BigInt guard; controlling the
  definition is the remedy the upstream docs prescribe.
* **`JsonNull` is a unit *struct*, not a unit *variant*.** `enum Json { Null, … }` looks
  equivalent but is wrong: `specta_serde::rewrite_enum_repr_for_phase` does the unit→empty-tuple
  transform (which renders `null`) only for *variant-level* `untagged`, and early-returns
  `if matches!(repr, EnumRepr::Untagged)` for *container-level* `untagged` — which is what
  `#[serde(untagged)]` on the enum is. The variant therefore kept `Fields::Unit`, which renders as
  its name literal, and the contract claimed settings could be the string `"Null"` while the wire
  emitted `null`. A unit struct reaches `Fields::Unit` → `null` through the struct path, which is
  also what serde serializes it as, so both sides agree.

`#[serde(transparent)]` on `JsonNumber` and `untagged` on `Json` keep the wire format
byte-identical to `serde_json::Value`, asserted by `domain::json::tests::round_trips_through_serde_json_value`.

**Known, deliberate imprecision:** `JsonNumber` exports as `number | null` rather than `number`.
The `null` is not reachable — nothing but `Json::Null` deserializes from `null` — but the
direction is the safe one: the contract admits *more* inputs than reality, so frontend code is
nudged into a null check rather than talked out of one. The opposite direction, which the unit
variant exhibited, is the dangerous one, because it names a value that can never arrive and so
lets `v === "Null"` type-check while never matching at runtime.

### 2. `dangerously_cast_bigints_to_number()` on the command builder

One call on `tauri_specta::Builder` rather than ~24 per-field attributes, so the trade-off is
stated once and cannot be forgotten on the next field.

It is correct here rather than merely convenient:

* **The transport is JSON.** Tauri serialises the payload as JSON and the webview runs
  `JSON.parse`, which folds any integer past 2⁵³ into an `f64` before the frontend can see it.
  Exporting `bigint` would therefore describe a value the webview never receives; `number`
  describes what it actually holds.
* **No shipped value approaches 2⁵³.** Schema versions and migration counts are single digits;
  byte counts are local-disk sizes (2⁵³ ≈ 9 PB); `checked_at_ms` is a millisecond timestamp
  (2⁵³ ms ≈ 285 million years).

## Consequences

* `Json` is the only sanctioned way to put a dynamic payload on the command surface. A concrete
  type should still be preferred whenever the shape is actually known.
* `domain::json::tests::resolving_the_definition_terminates` guards the recursion — it re-resolves
  16× and asserts the registration count does not grow. A regression shows up in milliseconds at
  the domain layer instead of as a hung multi-gigabyte export in the app crate.
* The `dangerously_…` switch is global to the command surface. A future command that must carry an
  integer losslessly needs its own representation (or a string encoding), not reliance on this
  flag; the doc comment on `specta_builder()` records that.
* If ADR-0012's escape hatch is ever taken, both decisions are re-examined: a different generator
  may accept `serde_json::Value` directly.
