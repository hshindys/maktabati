# OpenCode brief — Maktabati: defects, required changes, and the design pass

You are working in `~/Projects/maktabati` (branch `master`, remote `origin` = github.com/hshindys/maktabati).

**Maktabati** is a local-first, Arabic-first personal library manager. Rust workspace + Tauri 2 shell + Svelte 5/TS/Tailwind v4 frontend + SQLite. All behaviour lives in Rust; the webview is presentation only and never touches the database directly.

Every claim below was verified on this machine. Reproduce before you change anything.

---

## 0. Ground truth (do not "fix" these — they are already correct)

- All gates are green as of this brief: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace` (212 tests), `npm run typecheck` (0 errors/warnings), `npm test` (86 tests / 7 files), `npm run lint:logical-css`, `npm run build`, `cargo deny check advisories bans sources`.
- The IPC surface is 50 `#[tauri::command]` functions: 6 in `src-tauri/src/commands/mod.rs` and 44 in `src-tauri/src/commands/catalogue.rs`. All 50 are present in the generated `src/bindings.ts`, and drift is already gated in CI (`cargo test -p maktabati-app bindings_are_up_to_date` + `git diff --exit-code -- src/bindings.ts`). **Do not hand-edit `src/bindings.ts`** — regenerate with `MAKTABATI_UPDATE_BINDINGS=1 cargo test -p maktabati-app bindings_are_up_to_date`.
- `Cargo.toml:96-98` already sets `[profile.dev] debug = 0`. Leave it.
- i18n parity holds: 190 keys in `src/lib/i18n/ar.ts` and 190 in `en.ts`.
- `design/` **is** tracked — 16 committed files (both `.figcraft` sources, `gen_design.py`, `gen_frames.py`, `icons.py`, `measure_spacing.py`, `frames.json`, `design-system.json`, and the 8 exported PNGs), landed in `f8822fb`, `41dfd8c`, `6680e2a`. `git status --short` is clean.
- **Both design tools are already built and current — do not rebuild them.** `~/Projects/vectorcraft/target/release/vectorcraft-cli` (0.10.0) and `~/Projects/figcraft/target/release/figcraft-cli` (0.7.0) both run, and no source file is newer than either binary. Each workspace's `target/` is ~30 GB; a rebuild buys you nothing. See §4 and §5 for the verified invocations.

## 1. How to actually run the app (broken and undocumented today — see P0-2)

A plain debug build does **not** serve the frontend:

```bash
cargo build -p maktabati-app && ./target/debug/maktabati-app   # → blank white window
```

Verified failure: the window renders **99.77 % pure `#FFFFFF`**, and OCR of the window's own buffer returns the WebKit error string `Could not connect to localhost: Connection refused`. Reason: `src-tauri/tauri.conf.json` `build.devUrl` is `http://localhost:1420`, and a debug profile points the webview there instead of at `build.frontendDist` (`../dist`). Nothing was listening.

With the dev server up, the same binary renders correctly. Verified colour histogram of the live window: `#FBF8F1` 59.84 %, `#FFFFFF` 34.90 %, `#F4EFE4` 2.30 %, `#E3EFE8` 0.58 %, `#2F6B4F` 0.32 % — i.e. the real design tokens, plus the Arabic sidebar (لوحة التحكم / المكتبة / أقرأ الآن / الملاحظات / الاقتباسات / المؤلفون / الناشرون / التصنيفات / الأهداف / استيراد-تصدير / الإعدادات).

The correct command is `npm run dev` **in one terminal** and `npx tauri dev` in another — but there is no `tauri` npm script and `CONTRIBUTING.md` never mentions how to run the app.

---

## 2. Defects — fix in this order

### P0-1 — CI never runs on push

`.github/workflows/ci.yml:4-6`:

```yaml
on:
  push:
    branches: [main]
```

The repository has **no `main` branch** — `git branch -a` shows only `master` and `remotes/origin/master`. So CI runs only on pull requests and `workflow_dispatch`. Every push to `master` is ungated. This is why the missing module entry shipped earlier.

**Fix:** change to `branches: [master]` (or rename the default branch to `main` and update `README.md`, `CONTRIBUTING.md`, and the remote default together). Then add a `workflow_dispatch` smoke that runs the gates on demand.

### P0-2 — no way to launch the desktop app

`package.json` `scripts` has `dev`, `build`, `preview`, `typecheck`, `test`, `test:watch`, `lint:logical-css`, `check` — but **no `tauri` script**, although `@tauri-apps/cli@^2.12.3` is a devDependency. `CONTRIBUTING.md` documents commands, migrations, tests and errors, and never once says how to start the app.

**Fix:**
1. Add `"tauri": "tauri"` (and optionally `"desktop": "tauri dev"`) to `package.json` scripts.
2. Add a `## Running the app` section to `CONTRIBUTING.md`: `npm install` → `npm run tauri dev`; note that `tauri dev` starts Vite itself, that `npm run dev` alone only gives the browser preview (which degrades to empty states — see P0-5), and that `cargo run` on a debug build shows a blank window because it targets `devUrl`.
3. Make the failure loud instead of blank: when the webview loads a non-`tauri:` origin that fails, show a real error surface. At minimum log it — currently the app writes `database ready` to `~/.local/share/com.maktabati.desktop/logs/maktabati.log` and nothing about the failed page load.

### P0-3 — dark and sepia themes have no accent tokens

`src/app.css` defines accent tokens **once per `[data-accent="…"]`** at lines 136-187, with light-only values (`--accent: #2f6b4f`, `--accent-soft: #e3efe8`, `--accent-soft-ink: #24523c`, …). `[data-theme="dark"]` (`src/app.css:96`) and `[data-theme="sepia"]` (`src/app.css:117`) override surfaces and text only. Likewise `--success-soft: #e2f2e9`, `--warning-soft: #fbf0d8`, `--danger-soft: #fbe7e5`, `--info-soft: #e4eef7` (`src/app.css:190-197`) are declared once, light-only.

Measured WCAG 2.1 contrast, dark theme:

| pair | ratio | verdict |
|---|---|---|
| `--accent #2f6b4f` on `--surface-0 #12110f` | **3.00** | fails AA (4.5) for text |
| `--accent #2f6b4f` on `--surface-1 #1a1917` | **2.79** | fails even AA-large (3.0) |
| `--accent-soft #e3efe8` on `--surface-0 #12110f` | **15.98** | a near-white mint pill on a near-black UI |
| `--success-soft` / `--warning-soft` / `--danger-soft` / `--info-soft` on dark `--surface-0` | 15.87 – 16.68 | glaring light patches |

`--accent-soft` is the background of the active-nav pill and `::selection` (`src/app.css:289-290`), so in dark mode the "you are here" indicator is the brightest thing on screen and the accent-coloured links (`src/app.css:268`) are below AA.

**Fix:** add dark and sepia overrides for `--accent`, `--accent-hover`, `--accent-soft`, `--accent-soft-ink`, and for the four `*-soft` status surfaces, per accent (6 accents × 3 themes). Add a contrast test that computes ratios from the CSS custom properties and fails under 4.5:1 for text pairs — otherwise this regresses the first time someone adds an accent.

### P0-4 — `--text-3` fails AA in every theme

Measured `--text-3` against each surface:

| theme | text-3 | on surface-0 | on surface-1 | on surface-2 | on surface-3 |
|---|---|---|---|---|---|
| light | `#948a79` | 3.21 | 3.40 | **2.97** | **2.67** |
| dark | `#7e7768` | 4.25 | 3.95 | 3.62 | 3.22 |
| sepia | `#958568` | **2.94** | 3.12 | **2.76** | **2.44** |

Never reaches 4.5:1. Sepia on surface-3 is 2.44:1.

**Fix:** darken `--text-3` per theme until every pair is ≥ 4.5:1, or keep `--text-3` for decorative/non-text use only and introduce a `--text-3-strong` that meets AA. Whichever you choose, encode it in the same contrast test as P0-3.

### P0-5 — a backend failure is indistinguishable from an empty library

`src/lib/api.ts:84, 96, 108, 122, 134`: the read helpers collapse a backend `{ status: "error" }` (and any thrown error) into `EMPTY_SUMMARY` or `null`. So a corrupt, locked or missing database renders exactly like a brand-new install: a clean empty state and zero KPI counters, with no error, no toast, no log line.

Note the catalogue write path in the same file is already correct — `run()` (`src/lib/api.ts:174-184`) returns a typed `NO_SHELL`/`Outcome<T>` instead of faking success. Apply the same discipline to the read path.

**Fix:** keep the browser-preview fallback (`commands() === null` → empty is right), but when the command *exists* and returns `status: "error"`, propagate the `AppErrorDto` to the caller so the view can render an error state and the error code can be toasted. A `library_summary` failure must not look like a fresh library.

### P1-6 — 9 of 12 sidebar links, and `/book/:id`, are dead ends

`src/lib/router.ts:27-44` declares 16 routes. `src/App.svelte` renders a real view for only four: `/`, `/library`, `/settings`, `/dev/gallery`. Everything else falls through to `src/lib/views/Placeholder.svelte` (30 lines: an `EmptyState` plus the raw path string).

Affected: `/reading`, `/notes`, `/quotes`, `/authors`, `/publishers`, `/taxonomy`, `/lending`, `/goals`, `/transfer`, `/settings/backup`, `/settings/diagnostics`, and `/book/:id`.

`/book/:id` is the worst of these: `book_detail` already exists in the backend (`src-tauri/src/commands/catalogue.rs`), and a FigCraft frame for the page already exists at `design/export/3 _ صفحة الكتاب _ Book page.png` — a 7-tab layout (نظرة عامة / ملاحظات / اقتباسات / سجل القراءة / تقييم / ملفات / مرتبط) with cover, progress bar, star rating, tag chips and actions. The design is done; only the implementation is missing.

### P1-7 — the UI advertises shortcuts that do not exist

`src/lib/components/TitleBar.svelte:109-123` renders a global search entry with the i18n hint `shell.searchHint: "Ctrl+K"`, and its own comments admit "The palette itself lands with the library milestone". `docs/04-ux-design.md:454-479` documents 15+ shortcuts and claims they are "generated from one shared definition so they never drift".

Grepping all of `src/` for keydown handlers finds only `Stars.svelte:88`, `Modal.svelte:40`, `Combobox.svelte:99`, `TagInput.svelte:43` and `App.svelte:35` (`Ctrl+N` only). `Ctrl+K`, `Ctrl+F`, `Ctrl+P`, `Ctrl+,`, `Alt+←/→`, `?` and the `G`-then-`D/L/B` sequences are all unimplemented.

`docs/04-ux-design.md:479` also points at `docs/shortcuts.md`, which **does not exist**.

**Fix:** implement the command palette and the global shortcut layer, or remove the advertised affordance until it lands. Either way create `docs/shortcuts.md` and generate the `?` overlay and the doc from one shared table.

### P1-8 — M3 is marked done but covers are not built

`docs/05-roadmap.md:132-149` lists M3 acceptance criteria including "Cover: upload, drag-drop, thumbnail regeneration, placeholder renders Arabic correctly", and M3 is marked ✅ while `README.md` states M0–M3 are complete.

Reality: `crates/files/src/lib.rs` is a **1-line stub**, and none of the 50 IPC commands deals with files or covers. Only `src/lib/components/CoverPlaceholder.svelte` exists — a generated placeholder, no ingest path, no thumbnail cache.

**Fix:** either implement the cover pipeline (`crates/files`: import, SHA-256 dedupe, downscale on ingest, thumbnail cache; `book_file` table already exists — 50 tables are live, migrations v1 `init` and v2 `book_publisher` applied) or move that criterion out of M3 and stop marking M3 complete. Do not leave a ✅ that overstates the build.

### P1-9 — six crates are placeholders

`crates/search`, `crates/metadata`, `crates/files`, `crates/notes`, `crates/stats`, `crates/backup` — each `src/lib.rs` is 1 line. They are declared in `Cargo.toml` and documented in `README.md` as if they exist. Implement them milestone by milestone (M4 metadata, M5 search, M7 notes, M8 files, M10 stats, M11 backup) or mark them explicitly as unimplemented in `README.md`.

### P1-10 — sidebar and dashboard presentation

`src/lib/components/Sidebar.svelte:48-61` renders 12 flat items in one list with no grouping, and `Sidebar.svelte:122` puts a `/dev/gallery` link in the user-facing rail. Independent visual review of the built app called the result "competent but uninspired": a 14-item ungrouped rail, a main region ~85 % empty with a small floating empty-state card, a weak active-nav indicator, flat surfaces with no elevation, and a duplicate "Add" affordance (toolbar button plus empty-state button).

**Fix:** group the rail (Library / Management / System), move `/dev/gallery` into Settings → Diagnostics and gate it behind `import.meta.env.DEV`, strengthen the active indicator, give surfaces a real elevation scale, and remove one of the two "Add" buttons.

---

## 3. Enhancements to land with the fixes

1. **A contrast gate in CI.** Compute ratios from `src/app.css` custom properties for a fixed list of token pairs; fail under 4.5:1. This is the only thing that stops P0-3/P0-4 from recurring.
2. **An end-to-end smoke test of the real shell.** Today nothing proves the webview renders inside Tauri: the browser preview always degrades to empty states (`src/lib/api.ts:61-67`), and the frontend tests are pure-logic vitest under a `node` environment. A CI step that builds the app, boots it against a temporary data dir, and asserts the window is not a single flat colour would have caught P0-2.
3. **A `?` shortcut overlay generated from one table**, with `docs/shortcuts.md` emitted from the same table.
4. **Keyboard-first library browsing** (arrows / Enter / Space / E / Shift+arrows) per `docs/04-ux-design.md:454-479`.
5. **Cover pipeline** per P1-8, plus drag-and-drop onto the library grid.
6. **Empty-state craft.** The dashboard's empty state is the first thing every user sees; it should teach the product (add a book, scan an ISBN, import Goodreads) rather than sit as a small centred card.
7. **A second locale switch that actually flips direction** (`dir="rtl"` ↔ `ltr`) with a snapshot test in both directions — `docs/04-ux-design.md:413` calls RTL non-negotiable, and the guard `npm run lint:logical-css` already protects the CSS side.

---

## 4. Task A — design the logo in VectorCraft

The repo has **no vector master** for its mark. `src-tauri/icons/{icon.png,32x32.png,128x128.png,128x128@2x.png,icon.ico,icon.icns}` are rasterised by `scripts/make-icons.py` — Pillow polygons drawing a warm paper plate, an open book in ink, three text rules, and a green bookmark ribbon. It works, but it is drawn in code, has no SVG, no brand sheet, and no variants.

VectorCraft lives at `~/Projects/vectorcraft` (pure-Rust, clean-room Illustrator reimplementation). **The CLI is already built and current — do not rebuild it.** Verified on this machine:

- `~/Projects/vectorcraft/target/release/vectorcraft-cli` → `vectorcraft-cli 0.10.0`, built 2026-10-12, and **no `.rs` file under `crates/` or `apps/` is newer than it** (checked with `find crates apps -name '*.rs' -newer …`). The GUI binary `target/release/vectorcraft` is newer still (2026-10-15).
- End-to-end smoke test that passed:

```bash
cd ~/Projects/vectorcraft
./target/release/vectorcraft-cli run \
  --cmd file.new   --params '{"width":512,"height":512,"units":"Pixels"}' \
  --cmd shape.polygon --params '{"cx":256,"cy":256,"radius":200,"sides":6}' \
  --cmd shape.ellipse --params '{"x":156,"y":156,"width":200,"height":200}' \
  --export /tmp/vc-smoke.svg
# → {"bytes":605,"format":"svg","path":"/tmp/vc-smoke.svg","warnings":[]}  (valid SVG, one <path> per shape)
```

`commands` reports **743 commands**, including `path.create` (`{anchors:[{x,y,in?,out?}],closed?,d?}`), `shape.rectangle|ellipse|polygon|star|line|arc|spiral`, `text.create`, `object.transform`, `recolor.*` and `document.export` (`format: svg|svgz|pdf|png|…`). That is everything Task A needs — drive it headlessly with `run`, and only fall back to the GUI control server if you want to look at the art while drawing it.

```bash
# optional, interactive: GUI with the control server on 127.0.0.1:7979
cargo run --release -p vectorcraft -- --control 7979 design/brand/maktabati-mark.vectorcraft
```

The control server speaks JSON lines on `127.0.0.1:7979`; method definitions are in `crates/ui-egui/src/control.rs`. Shape:

```json
{"id":1,"method":"engine.execute","params":{"command":"shape.rectangle","params":{"x":10,"y":10,"width":100,"height":50}}}
{"id":2,"method":"ui.screenshot","params":{"path":"/tmp/shot.png"}}
```

MCP mode is also available: `vectorcraft-cli mcp` (see `docs/mcp.md` in that repo). House rules from its `AGENTS.md`: never `unwrap`/`expect`/`panic`/`todo` in shipped code (workspace clippy denies them outside tests); everything is a command under `crates/engine/src/cmd/*`; `cargo xtask ci` runs fmt + clippy + tests + layers + wasm; parallel agents must use separate `CARGO_TARGET_DIR` values (each grows ~30 GB); in that shell `mv`/`cp` are aliased interactive — use `/bin/mv -f` and `/bin/cp -f`.

**Deliverable:**

1. `design/brand/maktabati-mark.svg` — an **original** vector mark. Constraints: it must read at 16 px; it must sit on the `#FBF8F1` paper plate and survive on `#12110f`; it must not trace, embed or approximate any third-party logo, icon set or font; and it must be licensable under MIT OR Apache-2.0. A bookish idea that also suggests the Arabic «م» (or a shelf, or a page-turn) is welcome — restraint over cleverness.
2. Variants exported from VectorCraft into `design/brand/`: `maktabati-mark.svg` (master), `maktabati-mark-mono.svg`, `maktabati-wordmark.svg` (mark + «مكتبتي» + `Maktabati`), and PNG renders at 1024 / 512 / 256 / 128 / 32.
3. Rewire the icon pipeline so it is reproducible from the vector master: `scripts/make-icons.py` (or a replacement) should rasterise from the SVG. `rsvg-convert` **is installed** at `/usr/sbin/rsvg-convert` and librsvg is already a Tauri Linux dependency — use it, keep the same output filenames in `src-tauri/icons/`, and keep a documented fallback. Run `python3 scripts/make-icons.py` and commit the regenerated set.
4. Wire the mark into the app: the brand block in `src/lib/components/TitleBar.svelte`, the Settings → About panel, and `src-tauri/tauri.conf.json` (the window icon set). The title bar currently draws a placeholder glyph.
5. Commit `design/brand/` alongside the SVG master.

## 5. Task B — the GUI pass in FigCraft

FigCraft lives at `~/Projects/figcraft` (pure-Rust, clean-room Figma reimplementation). **Its CLI is already built and current — do not rebuild it.** Verified on this machine:

- `~/Projects/figcraft/target/release/figcraft-cli` → `figcraft-cli 0.7.0`, built 2026-10-20, newer than every `.rs` file under `crates/` and `apps/`.
- Both of this project's design files open cleanly in it:

```bash
cd ~/Projects/maktabati/design
~/Projects/figcraft/target/release/figcraft-cli info design-system.figcraft --json
# → 298 nodes, 1 frame "Maktabati · Design System" (1440×3568), 0 missing fonts, 0 images
~/Projects/figcraft/target/release/figcraft-cli info maktabati.figcraft --json
# → 1279 nodes, 9 frames — Design System + 1·Dashboard + 2·Library + 3·Book page +
#   4/5/6·Wizard + … , 0 missing fonts, 0 images
```

Useful subcommands: `run <script.json> [--in FILE] [--out FILE]` (headless, JSON lines of `{"command": id, "params": {…}}`, one JSON result per step), `export <file> -o <dir> --format png|svg|pdf|fig [--node ID|NAME]`, `render`, `convert`, `commands [--filter TEXT]`, `mcp`. Pass `--no-system-fonts` for renders that must match on every machine.

Hard rules from that repo's `AGENTS.md`, which you must not break: clean-room only, and **never add, copy, trace, redraw or embed any third-party product iconography, image, font, template or UI kit** — every asset under `assets/`, `docs/brand/` or `docs/images/` needs an `ATTRIBUTION.md` row or `cargo xtask assets` (part of `cargo xtask ci`) fails. FigCraft's own UI icons are drawn in code at `crates/ui-egui/src/icons.rs` from simple primitives. Parallel agents need separate `CARGO_TARGET_DIR`.

**Input files:** `~/Projects/maktabati/design/design-system.figcraft` and `~/Projects/maktabati/design/maktabati.figcraft`. Existing exported frames live in `~/Projects/maktabati/design/export/` and cover the dashboard, library, book page and the four wizard steps.

**Deliverable:**

1. Open `design-system.figcraft` and extend it with the tokens the app actually ships — the full light/dark/sepia × 6-accent matrix from `src/app.css`, the type scale (1.200 minor third on 16 px, `--fs-xs` … `--fs-3xl`, `--lh-read: 1.85` for Arabic), the spacing scale, radii, elevation, and the state matrix (hover / focus / disabled / loading / error) for every component in `src/lib/components/`.
2. Open `design/maktabati.figcraft` and **add frames for the screens that exist only as prose** in `docs/04-ux-design.md:44-71`: book page (all 7 tabs), library in grid/list/table with the filter bar, notes workspace with the page tree and backlinks, quotes, authors, publishers, taxonomy tree, lending, goals, import/export with the column-mapping step, backup & restore, diagnostics, command palette, and onboarding. Match the existing frames' visual language exactly — same palette, same radii, same RTL conventions.
3. **Revise** the dashboard and library frames to answer the critique in P1-10: grouped rail, real elevation, one "Add" affordance, an empty state that teaches the product.
4. Export every frame as PNG into `design/export/` with the existing naming convention (`N _ Arabic _ English.png`), and keep `gen_frames.py` / `gen_design.py` in sync so the frames are reproducible from code rather than hand-drawn once.
5. Then **implement** in the app, in this order: `/book/:id` (backend already supports it), the command palette + shortcut layer, `/authors`, `/publishers`, `/taxonomy`, `/reading`, `/notes`, `/quotes`, `/lending`, `/goals`, `/transfer`, `/settings/backup`, `/settings/diagnostics`. One route per commit; replace `Placeholder.svelte` usage as each lands and delete the fallback only when no route needs it.

---

## 6. Constraints — these are project law (`CONTRIBUTING.md`)

1. **No network without an explicit user action.** A background request is a bug. Metadata lookups go through `crates/metadata`, are rate-limited, and start only on a click.
2. **Arabic is the reference locale.** Write the new string in `src/lib/i18n/ar.ts` first, then `en.ts`. A key in `ar.ts` missing from `en.ts` is a compile error — deliberately.
3. **CSS logical properties only.** `margin-inline-start`, `inset-inline-end`, `text-align: start`. `npm run lint:logical-css` fails the build on `margin-left`, `padding-right`, `text-align: right`, `float: right` and friends. If mirroring is genuinely wrong for one element, put `/* logical-css-allow: <reason> */` on the previous line and say why.
4. **No `unwrap`/`expect` outside tests** — enforced by `[workspace.lints.clippy]` in `Cargo.toml:110+` (`unwrap_used = "deny"`). Use `let-else` or a typed error.
5. **Never block.** `rusqlite` is synchronous (ADR-0005) — use the `blocking()` helper in `src-tauri/src/commands/mod.rs`, which runs the closure on `spawn_blocking`.
6. **New Tauri command:** write it in `src-tauri/src/commands/` returning `CmdResult<T>`, register it in the `commands!` macro in `src-tauri/src/lib.rs` (that one list both registers it with Tauri and exports its type to TypeScript), then regenerate `src/bindings.ts` and commit it.
7. **New migration:** add `crates/storage/migrations/NNNN_name.sql`, never edit an applied file (the checksum in `_migrations` is re-verified at startup and a mismatch is a hard stop), and register it in `crates/storage/src/migrations.rs` + `migrate::embedded()`. No down-migrations.
8. **New enum variant** in `crates/domain/src/enums.rs` needs the matching SQL `CHECK` value — `repo::schema_conformance::every_rust_enum_variant_exists_in_the_ddl` fails otherwise.
9. **Every error crossing IPC** becomes `AppErrorDto { code, message, i18n_key, details }` (ADR-0010), and a new code needs `error.<code>` in **both** `ar.ts` and `en.ts`.
10. **Conventional commits**, `≤ 72` chars: `<type>(<scope>): <imperative summary>`, body explains *why*. `commitlint` enforces it on PRs.
11. Every user-visible change needs a test, or one sentence in the PR explaining why a test would be theatre.

## 7. Definition of done

Before you claim anything is finished, run and paste the output of:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check advisories bans sources
npm run check          # typecheck + test + lint:logical-css
npm run build          # then confirm dist/ has a real JS chunk
npm run tauri dev      # and confirm the window is NOT a blank page
```

For the last one, prove it rather than eyeball it: the window must not be a single flat colour. Capture it and check the histogram, or assert in a test that the built bundle mounts `#app` with a non-empty child count.

Work in the order given: **P0 first (CI trigger, run path, theme tokens, error propagation), then Task A, then Task B.** Do not start the GUI pass while the P0 defects are open — the theme-token fix changes the colours every frame depends on.
