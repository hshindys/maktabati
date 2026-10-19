//! Numbered, checksummed, transactional migrations (ADR-0003).
//!
//! * Migration files are embedded with [`include_str!`] and applied in numeric order.
//! * Each runs **inside a single transaction**; a failure leaves the schema untouched.
//! * Every applied migration's SHA-256 is recorded and **re-verified on every startup**, so a
//!   modified applied migration is a hard, actionable error rather than a silent divergence.
//! * Before applying a migration that changes the schema version of an **existing** database, an
//!   automatic **pre-migration backup** hook is invoked; if it fails, the migration is aborted.
//!   A brand-new database skips it — there is nothing yet worth snapshotting.
//!
//! There are no down-migrations: recovery is *restore a backup* (ADR-0003).

use sha2::{Digest, Sha256};

use crate::conn::Db;
use crate::{Result, StorageError};

/// One embedded migration file.
#[derive(Clone)]
pub struct MigrationFile {
    /// Numeric prefix, unique and gapless from 1.
    pub version: i64,
    /// Descriptive name from the filename (`0001_init.sql` → `init`).
    pub name: String,
    /// Raw SQL.
    pub sql: &'static str,
    /// SHA-256 of `sql`, lower-case hex.
    pub checksum: String,
}

/// Every migration embedded in the binary, in application order.
pub fn embedded() -> Vec<MigrationFile> {
    let files: [(&str, i64, &str, &'static str); 1] =
        [("init", 1, "init", crate::migrations::M0001_INIT)];
    let mut out: Vec<MigrationFile> = files
        .into_iter()
        .map(|(_, version, name, sql)| MigrationFile {
            version,
            name: name.to_owned(),
            sql,
            checksum: sha256_hex(sql),
        })
        .collect();
    out.sort_by_key(|m| m.version);
    out
}

/// Records which migrations have been applied.
pub const MIGRATIONS_TABLE: &str = "CREATE TABLE IF NOT EXISTS _migrations (
    version    INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    checksum   TEXT NOT NULL,
    applied_at INTEGER NOT NULL
)";

/// SHA-256 of a string as lower-case hex.
#[must_use]
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

/// The highest applied migration version, or 0 for a fresh database.
///
/// # Errors
///
/// [`StorageError::Sqlite`] when `_migrations` cannot be created or queried.
pub fn current_version(db: &Db) -> Result<i64> {
    db.write(|conn| {
        conn.execute_batch(MIGRATIONS_TABLE)?;
        let v: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |r| r.get(0),
        )?;
        Ok(v)
    })
}

/// `true` when embedded migrations exist that are not yet applied.
///
/// # Errors
///
/// [`StorageError::Sqlite`] when the metadata table cannot be read.
pub fn pending(db: &Db) -> Result<bool> {
    Ok(current_version(db)? < embedded().last().map_or(0, |m| m.version))
}

/// Verify that every applied migration still matches its embedded file.
///
/// # Errors
///
/// * [`StorageError::MigrationChecksumMismatch`] if a recorded checksum differs.
/// * [`StorageError::Sqlite`] on query failure.
pub fn verify_checksums(db: &Db) -> Result<()> {
    let embedded_files = embedded();
    db.read(|conn| {
        let has_table: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_migrations'",
            [],
            |r| r.get(0),
        )?;
        if has_table == 0 {
            return Ok(());
        }
        let mut stmt =
            conn.prepare("SELECT version, name, checksum FROM _migrations ORDER BY version")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (version, name, db_checksum) = row?;
            match embedded_files.iter().find(|m| m.version == version) {
                None => {
                    return Err(StorageError::MigrationChecksumMismatch {
                        version,
                        name: name.clone(),
                        db: db_checksum,
                        file: "<not present in this build>".to_owned(),
                    })
                }
                Some(file) if file.checksum != db_checksum => {
                    return Err(StorageError::MigrationChecksumMismatch {
                        version,
                        name: name.clone(),
                        db: db_checksum,
                        file: file.checksum.clone(),
                    })
                }
                Some(_) => {}
            }
        }
        Ok(())
    })
}

/// Apply every pending migration.
///
/// `before_schema_change` runs **before** the first pending migration is applied and must
/// produce a verified backup; returning `Err` aborts the whole run (this is the automatic
/// pre-migration backup hook from ADR-0003).
///
/// It is **not** called when the database has never been migrated, because an empty database
/// has nothing to lose and a snapshot of it would be dead weight in `backups/`.
///
/// Returns the version reached.
///
/// # Errors
///
/// * Whatever `before_schema_change` returns (aborts the run).
/// * [`StorageError::MigrationChecksumMismatch`] from [`verify_checksums`].
/// * [`StorageError::Migration`] when a file has no valid version prefix or is a duplicate.
/// * [`StorageError::Sqlite`] when SQL fails (the transaction is rolled back).
pub fn apply(db: &Db, before_schema_change: impl FnMut() -> Result<()>) -> Result<i64> {
    let files = embedded();
    apply_from(db, &files, before_schema_change)
}

/// [`apply`] with the migration list supplied rather than [`embedded`].
///
/// The seam exists because the backup hook is unreachable through [`apply`] alone while the
/// shipped set holds exactly one migration: a fresh database has `start == 0` and so has nothing
/// to protect, and once 0001 is recorded nothing is pending — the state that can invoke the hook
/// (`start > 0` *and* something pending) requires a list running past version 1. Tests build that
/// state here; nothing else needs to.
pub(crate) fn apply_from(
    db: &Db,
    files: &[MigrationFile],
    mut before_schema_change: impl FnMut() -> Result<()>,
) -> Result<i64> {
    for w in files.windows(2) {
        if w[0].version >= w[1].version {
            return Err(StorageError::Migration(format!(
                "migrations out of order: {} >= {}",
                w[0].version, w[1].version
            )));
        }
    }

    verify_checksums(db)?;

    let start = current_version(db)?;
    let pending: Vec<&MigrationFile> = files.iter().filter(|m| m.version > start).collect();
    if pending.is_empty() {
        return Ok(start);
    }

    // A brand-new database has no schema and no user data, so there is nothing to protect:
    // running the hook would write a snapshot that can never be useful, once per install.
    // `start == 0` is the exact signal, because migration 0001 is what creates the schema from
    // nothing — any database that has ever been migrated reports a version above zero.
    if start > 0 {
        before_schema_change()?;
    }

    let mut version = start;
    for file in pending {
        db.transact(|conn| {
            conn.execute_batch(file.sql)?;
            conn.execute(
                "INSERT INTO _migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    file.version,
                    file.name,
                    file.checksum,
                    crate::conn::now_ms()
                ],
            )?;
            Ok(())
        })?;
        version = file.version;
        tracing::info!(version = file.version, name = %file.name, "migration applied");
    }

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn test_db() -> Db {
        Db::open_with(
            std::path::Path::new(":memory:"),
            crate::conn::OpenOptions {
                in_memory: true,
                pool_size: 1,
            },
        )
        .unwrap()
    }

    #[test]
    fn embedded_migrations_are_ordered_and_uniquely_versioned() {
        let files = embedded();
        assert!(!files.is_empty());
        let mut seen = std::collections::BTreeSet::new();
        let mut last = 0;
        for f in &files {
            assert!(f.version > last, "versions must strictly increase");
            last = f.version;
            assert!(seen.insert(f.version), "duplicate version {}", f.version);
            assert!(!f.name.is_empty());
            assert_eq!(f.checksum.len(), 64, "sha-256 hex is 64 chars");
        }
    }

    #[test]
    fn checksum_is_stable_and_content_sensitive() {
        assert_eq!(sha256_hex("abc"), sha256_hex("abc"));
        assert_ne!(sha256_hex("abc"), sha256_hex("abd"));
        assert_eq!(sha256_hex("").len(), 64);
    }

    #[test]
    fn applies_all_migrations_then_is_idempotent() {
        let db = test_db();
        assert!(pending(&db).unwrap());

        let v1 = apply(&db, || Ok(())).unwrap();
        assert_eq!(v1, embedded().last().unwrap().version);
        assert!(!pending(&db).unwrap());

        // Re-running is a no-op and does not call the backup hook again.
        let mut called = 0;
        let v2 = apply(&db, || {
            called += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(v2, v1);
        assert_eq!(called, 0, "no backup hook when nothing is pending");
    }

    #[test]
    fn fresh_database_skips_the_backup_hook() {
        let db = test_db();
        // An empty database has no schema and no user data, so there is nothing to protect — the
        // hook must not run even when it is one that would fail.
        let v = apply(&db, || Err(StorageError::Migration("backup failed".into()))).unwrap();
        assert_eq!(v, embedded().last().unwrap().version);
        assert!(!pending(&db).unwrap());
    }

    #[test]
    fn backup_hook_failure_aborts_migration() {
        // The hook fires only when a schema already exists (`start > 0`) *and* something is
        // pending — a state one shipped migration cannot produce, so `apply_from` supplies a
        // second one to make the branch reachable.
        let db = test_db();
        let init = embedded().into_iter().find(|m| m.version == 1).unwrap();

        // Record 0001 as already applied, with its real checksum so `verify_checksums` is happy.
        db.write(|conn| {
            conn.execute_batch(MIGRATIONS_TABLE)?;
            conn.execute(
                "INSERT INTO _migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    init.version,
                    init.name,
                    init.checksum,
                    crate::conn::now_ms()
                ],
            )?;
            Ok(())
        })
        .unwrap();

        let files = [
            init.clone(),
            MigrationFile {
                version: 2,
                name: "never".into(),
                sql: "CREATE TABLE never_applied (id INTEGER PRIMARY KEY);",
                checksum: sha256_hex("never"),
            },
        ];

        let err = apply_from(&db, &files, || {
            Err(StorageError::Migration("backup failed".into()))
        });
        assert!(err.is_err(), "a failed backup must abort the migration");

        // Nothing past version 1 was applied.
        assert_eq!(current_version(&db).unwrap(), 1);
        db.read(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'never_applied'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(n, 0, "the pending migration must not have run");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn checksum_mismatch_is_detected() {
        let db = test_db();
        apply(&db, || Ok(())).unwrap();
        // Tamper with the recorded checksum, as if someone edited an applied migration file.
        db.write(|c| {
            c.execute(
                "UPDATE _migrations SET checksum = 'tampered' WHERE version = 1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let err = verify_checksums(&db).unwrap_err();
        match err {
            StorageError::MigrationChecksumMismatch { version, .. } => assert_eq!(version, 1),
            other => panic!("expected checksum mismatch, got {other}"),
        }
    }

    #[test]
    fn real_schema_applies_cleanly_and_has_expected_tables() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("lib.db")).unwrap();
        apply(&db, || Ok(())).unwrap();

        db.read(|c| {
            for table in [
                "books",
                "authors",
                "publishers",
                "categories",
                "tags",
                "series",
                "reading_cycles",
                "reading_sessions",
                "reviews",
                "rating_criteria",
                "rubric_scores",
                "notes_page",
                "note_block",
                "note_link",
                "quotes",
                "book_files",
                "shelves",
                "custom_column_def",
                "app_settings",
                "metadata_cache",
                "search_document",
                "fts_book",
                "fts_book_norm",
                "fts_note",
                "fts_quote",
            ] {
                let n: i64 = c.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                    [table],
                    |r| r.get(0),
                )?;
                assert_eq!(n, 1, "table `{table}` missing after migration");
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn fts5_is_available_in_the_bundled_sqlite() {
        // Hard gate from ADR-0004: if the bundled SQLite lacks FTS5, search must fail loudly
        // here rather than at runtime for users.
        let db = test_db();
        db.write(|c| {
            c.execute_batch(
                "CREATE VIRTUAL TABLE fts_smoke USING fts5(a, b, tokenize=\"unicode61 remove_diacritics 2\")",
            )?;
            c.execute("INSERT INTO fts_smoke (a, b) VALUES ('القراءة', 'hello')", [])?;
            let n: i64 = c.query_row(
                "SELECT COUNT(*) FROM fts_smoke WHERE fts_smoke MATCH 'القراءة'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(n, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn foreign_keys_are_enforced_against_the_real_schema() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("lib.db")).unwrap();
        apply(&db, || Ok(())).unwrap();
        let err = db.write(|c| {
            c.execute(
                "INSERT INTO books (id, title, title_norm, language_code, reading_status,
                                    progress_pct, source, created_at, updated_at)
                 VALUES ('b1','t','t','no_such_lang','to_read',0,'manual',1,1)",
                [],
            )?;
            Ok(())
        });
        assert!(err.is_err(), "unknown language code must be rejected");
    }
}
