# Maktabati · مكتبتي

**A local-first personal library manager.** Your books, notes, quotes and reading history stay on your own machine — no account, no cloud, no telemetry.

> مكتبتي — مدير مكتبة شخصية. كل بياناتك محفوظة على جهازك وحده، دون حسابات أو تتبع.

---

## Status

In active development. Milestones `M0`–`M2` are complete (research, specs, domain + storage + shell); the screens and importers are landing milestone by milestone. See [`docs/05-roadmap.md`](docs/05-roadmap.md) for what is done and what is next.

## Why

Existing library managers are either cloud services, or desktop apps that assume a Latin-first, single-script world. Maktabati is built the other way round:

- **Arabic-first, not Arabic-compatible.** Arabic is the default locale, the default layout direction, and the default script. English is a first-class second locale. Switching is instant and live.
- **Local-only by construction.** There is no network code path except the metadata lookup you explicitly trigger. The capability allowlist in [`src-tauri/capabilities/default.json`](src-tauri/capabilities/default.json) is short enough to read in one sitting.
- **A real rubric, not one number.** You rate a book on content depth, writing style, editing, print quality, translation, value for money, structure and references — the way a collector actually thinks. The rubric is user-editable.
- **A book is a physical object.** Room, shelf and box are first-class fields, because "where is it?" is a question a personal library has to answer.

## Architecture

All behaviour lives in Rust. The webview is a presentation layer with no domain logic and no direct database access.

```
src-tauri/            Tauri shell: window, tray, plugins, IPC commands
  ├── commands/       the IPC surface — registered once, exported to TypeScript
crates/
  ├── domain/         entities, validation, ISBN, Arabic-aware text normalization
  ├── storage/        SQLite: pool, checksummed migrations, seed, repositories
  ├── search/         FTS5 index + Arabic normalization
  ├── metadata/       Open Library / Google Books, rate-limited, cache-first
  ├── files/          attachment import, SHA-256 dedupe, cover extraction
  ├── notes/          block-based notes, backlinks, Markdown export
  ├── stats/          aggregates, heatmap, goals
  └── backup/         snapshot, verify, restore
src/                  Svelte 5 + TS + Tailwind v4 (no domain logic)
```

Key decisions are recorded as ADRs in [`docs/adr/`](docs/adr/):

| | |
|---|---|
| [ADR-0001](docs/adr/0001-tauri-2-svelte-5-webview.md) | Tauri 2 + Svelte 5 webview over a native UI toolkit |
| [ADR-0003](docs/adr/0003-sqlite-storage.md) | SQLite in OS app data, WAL, checksummed transactional migrations |
| [ADR-0004](docs/adr/0004-full-text-search.md) | SQLite FTS5 with write-time Arabic normalization |
| [ADR-0005](docs/adr/0005-synchronous-rusqlite.md) | `rusqlite` over `sqlx`/`SeaORM` |
| [ADR-0009](docs/adr/0009-frontend-stack.md) | Svelte 5, Tailwind v4, bundled fonts, Western digits in storage |

## Development

### Prerequisites

- **Rust** 1.80+ (`rustup` recommended)
- **Node** 22+
- **Linux only:** the WebKitGTK stack

  ```bash
  sudo apt-get install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### Run

```bash
npm install
npm run dev          # starts Vite on :1420
cargo tauri dev      # in another shell — starts the Rust side and opens the window
```

`cargo tauri dev` requires `tauri-cli` (`cargo install tauri-cli --version "^2"`), or use `npm run tauri dev` once the `@tauri-apps/cli` dev dependency is added.

### Quality gates

Every one of these must pass before a commit. CI runs the same set on Linux and Windows.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check advisories bans sources

npm run typecheck     # svelte-check
npm test              # vitest
npm run lint:logical-css   # RTL guard: no physical left/right in src/
npm run build
```

### Regenerating the TypeScript contract

Commands are declared once in Rust and exported to `src/bindings.ts`. After changing any `#[tauri::command]`:

```bash
MAKTABATI_UPDATE_BINDINGS=1 cargo test -p maktabati-app bindings_are_up_to_date
```

Then commit `src/bindings.ts`. CI fails on drift.

## Data location

The library lives in the OS application-data directory for `com.maktabati.desktop`:

| OS | Path |
|---|---|
| Linux | `~/.local/share/com.maktabati.desktop/` |
| macOS | `~/Library/Application Support/com.maktabati.desktop/` |
| Windows | `%APPDATA%\com.maktabati.desktop\` |

Inside: `library.db` (SQLite, WAL), `covers/`, `files/`, `backups/`, `logs/`.

A backup is taken automatically **before every schema migration** and the migration is aborted if that backup fails.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). In short: conventional commits, tests with code, no physical `left/right` CSS, and any new error code needs an Arabic string.

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT licence ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Bundled fonts (Inter, Cairo, Tajawal, IBM Plex Sans Arabic, Amiri) are all SIL Open Font License 1.1.
