# Contributing to Maktabati

Thanks for helping. This project has a few hard rules that exist because the product is
Arabic-first and offline-first; everything else is negotiable in review.

## Ground rules

1. **No network without an explicit user action.** A background request is a bug. Metadata lookups
   go through `crates/metadata`, are rate-limited, and are only started by a click.
2. **Arabic is the reference locale.** A new string is written in `ar.ts` first and translated to
   `en.ts` second. A key added to `ar.ts` and missing from `en.ts` is a compile error — that is
   deliberate.
3. **Layout uses CSS logical properties.** `margin-inline-start`, `inset-inline-end`,
   `text-align: start`. `npm run lint:logical-css` fails the build on `margin-left`, `padding-right`,
   `text-align: right`, `float: right` and friends. If mirroring is genuinely wrong for one
   element, put `/* logical-css-allow: <reason> */` on the previous line and say why.
4. **No `unwrap`/`expect` outside tests.** Enforced by `[workspace.lints.clippy]` in `Cargo.toml`.
   A `let-else` or a typed error is nearly always the better answer anyway.
5. **Conventional commits.** `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, with an
   optional `scope` — `feat(search): …`.

## Commit format

```
<type>(<scope>): <imperative summary, ≤ 72 chars>

<body: why, not what>
```

`commitlint` checks this in CI on pull requests.

## Adding a Tauri command

1. Write it in `src-tauri/src/commands/`, returning `CmdResult<T>`.
2. Add it to the `commands!` macro in `src-tauri/src/lib.rs` — that single list both registers it
   with Tauri and exports its type to TypeScript, so a command cannot exist on one side only.
3. Regenerate the contract and commit it:

   ```bash
   MAKTABATI_UPDATE_BINDINGS=1 cargo test -p maktabati-app bindings_are_up_to_date
   ```

4. Never block. `rusqlite` is synchronous (ADR-0005), so use the `blocking()` helper in
   `commands/mod.rs`, which runs the closure on `spawn_blocking`.

## Adding a database migration

1. Add `crates/storage/migrations/NNNN_name.sql`. Never edit an applied file — the checksum
   recorded in `_migrations` is re-verified on every startup and a mismatch is a hard stop.
2. Register it in `crates/storage/src/migrations.rs` and `migrate::embedded()`. A missing file is
   a compile error, not a runtime surprise.
3. There are no down-migrations. Recovery is *restore the automatic pre-migration backup*.
4. If you add a variant to anything in `crates/domain/src/enums.rs`, add the matching value to the
   SQL `CHECK` constraint — `repo::schema_conformance::every_rust_enum_variant_exists_in_the_ddl`
   will fail otherwise.

## Testing

- Rust: `cargo test --workspace`. Tests live next to the code; `proptest` for invariants,
  `insta` for anything whose exact output is part of the contract.
- Frontend: `npm test` (vitest, node environment). Pure logic only — components are exercised
  through the dev gallery (`#/dev/gallery`) rather than through a DOM test harness.
- Every user-visible change needs a test, or a sentence in the PR explaining why one would be
  theatre.

## Errors

Every failure crossing IPC becomes an `AppErrorDto { code, message, i18n_key, details }`
(ADR-0010). A **new code needs a new key in both `ar.ts` and `en.ts`** under `error.<code>`, or the
UI shows a developer string to a user.

## Before you push

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
npm run check
```

CI runs the identical set on Linux and Windows.
