# ADR-0003 — Persistence layout, WAL, and migrations with automatic pre-migration backup

**Status:** Accepted · **Date:** 2026-10-18

## Context

Local-first means the SQLite file **is** the user's data. Losing or corrupting it is the worst
possible failure. The brief requires WAL, transactional writes, scheduled + pre-migration
backups, and manual backup/restore with integrity verification.

## Decision

### Layout

All data lives in one OS-app-data root resolved with the `directories` crate:

```
{data}/maktabati/
├── library.db            # SQLite, WAL
├── covers/               # originals + thumbnails
├── files/                # copied attachments addressed by SHA-256
├── backups/              # zip archives + .sha256 manifests
├── logs/maktabati.log    # size-rotated tracing output
└── settings.json         # non-secret preferences
```

Secrets (app-lock hash) live in the DB, not in `settings.json`.

### PRAGMAs (applied on every connection)

```sql
PRAGMA journal_mode = WAL;        -- persistent setting
PRAGMA synchronous  = NORMAL;     -- safe with WAL, much faster
PRAGMA foreign_keys = ON;         -- SQLite default is OFF!
PRAGMA busy_timeout = 5000;
PRAGMA temp_store   = MEMORY;
PRAGMA mmap_size    = 268435456;
```

### Migrations

- Numbered SQL files (`0001_init.sql`, …) embedded with `include_str!` and applied **in a single
  transaction** each, recorded in `_migrations(version, name, checksum, applied_at)`.
- **Checksum verification:** on startup the recorded checksum of every applied migration is
  compared with the embedded file; a mismatch is a hard, logged error (never a silent rewrite).
- **Automatic backup before any migration** that changes schema version; if the backup fails the
  migration is aborted. The backup is pruned to keep the newest N.
- Down-migrations are **not** supported (data-loss risk). Rollforward + restore-from-backup is the
  recovery path, which is also what users understand.

### Integrity

- `PRAGMA quick_check` runs on startup after a crash (detected by WAL presence + unclean flag)
  and in Settings → "Verify database".
- Backups are zip archives containing `manifest.json` (file list, sizes, SHA-256 per entry +
  DB `user_version` + schema version) and are verified after writing.

## Consequences

- ✅ Crash-safe writes; readers never block the writer; a restore is verifiable, not hopeful.
- ✅ Automatic pre-migration backup makes schema evolution safe for non-technical users.
- ⚠️ WAL creates `-wal`/`-shm` files — the backup code must use SQLite's **online backup**
  (`VACUUM INTO` or `Connection::backup`) rather than copying `library.db` naively.
- ⚠️ `synchronous=NORMAL` can lose the last transactions on power loss (not corruption). Accepted;
  can be raised to `FULL` via a setting.

## Alternatives considered

| Option | Why rejected |
|---|---|
| **Copy file while open** | Produces torn backups under WAL. Rejected outright. |
| **SQLx migrations** | Tied to the sqlx runtime we are not using (ADR-0005). |
| **`refinery`/`rusqlite_migration`** | Viable; our own ~100-line migrator gives checksum + pre-migration-backup hooks for free and avoids a dependency whose semantics we'd still have to wrap. |
| **Down migrations** | Risky and rarely used; restore-from-backup is safer. |
