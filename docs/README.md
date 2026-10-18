# Maktabati / مكتبتي — Documentation

Personal Library Manager · local-first desktop app · Rust workspace + Tauri 2 + Svelte 5.

## Read in this order

| # | Document | What it answers |
|---|---|---|
| 0 | [Research](00-research.md) | What exists, what to steal, what to avoid, which sources and crates are real |
| 1 | [Product spec](01-product-spec.md) | Vision, user, scope, non-goals, user stories, success criteria |
| 2 | [Architecture](02-architecture.md) | Layers, crate graph, data flow, IPC boundary, errors, concurrency, security, packaging |
| 3 | [Data model](03-data-model.md) | ER diagram, full DDL, indexes, FTS design, Arabic normalization, migrations |
| 4 | [UX design](04-ux-design.md) | IA, screens, wireframes, design system, themes, RTL rules, a11y, shortcuts |
| 5 | [Roadmap](05-roadmap.md) | 13 milestones with acceptance criteria, effort, risks |

## Decision records — [`adr/`](adr/)

| ADR | Decision |
|---|---|
| [0001](adr/0001-ui-framework.md) | Tauri 2 + Svelte 5 + Tailwind + Bits UI (webview) over Leptos |
| [0002](adr/0002-type-contract.md) | Generate the TS↔Rust contract with specta / tauri-specta |
| [0003](adr/0003-persistence.md) | SQLite layout, WAL, checksummed migrations + pre-migration backup |
| [0004](adr/0004-search.md) | SQLite FTS5 over tantivy; Arabic normalization at write time |
| [0005](adr/0005-db-access-layer.md) | **`rusqlite` instead of sqlx/SeaORM** (deviation, justified) |
| [0006](adr/0006-notes-storage.md) | Notes as structured blocks in SQLite + Markdown projection |
| [0007](adr/0007-readers.md) | pdf.js + foliate-js in the webview; Rust reads files, never renders |
| [0008](adr/0008-metadata-providers.md) | Open Library + Google Books only, rate-limited and cached, adapter seam |
| [0009](adr/0009-frontend-stack.md) | Svelte 5 + TS + Tailwind v4 + Vite + Bits UI; fonts bundled |
| [0010](adr/0010-error-and-concurrency.md) | One `AppError` DTO boundary; sync SQLite on `spawn_blocking` |
| [0011](adr/0011-barcode-scanning.md) | Barcode scan is COULD; typed/paste (USB scanners work) is guaranteed |
| [0012](adr/0012-type-contract-escape-hatch.md) | Contingency if specta cannot ship |

## Conventions

- **ADRs are append-only.** To change a decision, supersede the ADR with a new one; never rewrite history.
- **Documents live with the milestone.** A milestone is not closed until the docs it changed are updated.
- **Nothing is claimed that was not run.** Test results, versions and benchmarks are recorded only
  after actually executing them.
