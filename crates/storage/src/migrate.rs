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
    /// Apply with `PRAGMA foreign_keys` OFF (empty databases to rebuild a table that is a
    /// foreign-key parent; see `0003`'s header). The SQL itself still runs in exactly one
    /// transaction, which the runner verifies with `PRAGMA foreign_key_check` before
    /// restoring foreign keys.
    pub foreign_keys_off: bool,
}

/// Every migration embedded in the binary, in application order.
pub fn embedded() -> Vec<MigrationFile> {
    // The trailing flag marks migrations that must be applied with `PRAGMA foreign_keys`
    // switched OFF (a table rebuild's `DROP TABLE` runs an implicit FK-checked DELETE;
    // 0003 rebuilds `books`, parent of fifteen child tables). See the file's header.
    let files: [(&str, i64, &str, &'static str, bool); 3] = [
        ("init", 1, "init", crate::migrations::M0001_INIT, false),
        (
            "book_publisher",
            2,
            "book_publisher",
            crate::migrations::M0002_BOOK_PUBLISHER,
            false,
        ),
        (
            "cover_sources",
            3,
            "cover_sources",
            crate::migrations::M0003_COVER_SOURCES,
            true,
        ),
    ];
    let mut out: Vec<MigrationFile> = files
        .into_iter()
        .map(|(_, version, name, sql, foreign_keys_off)| MigrationFile {
            version,
            name: name.to_owned(),
            sql,
            checksum: sha256_hex(sql),
            foreign_keys_off,
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
        if file.foreign_keys_off {
            apply_with_foreign_keys_off(db, file)?;
            version = file.version;
            tracing::info!(version = file.version, name = %file.name, "migration applied");
            continue;
        }
        apply_in_transaction(db, file)?;
        version = file.version;
        tracing::info!(version = file.version, name = %file.name, "migration applied");
    }

    Ok(version)
}

/// Apply one ordinary migration: the batch plus the `_migrations` row in a single
/// transaction.
///
/// # Errors
///
/// [`StorageError::Sqlite`] when the batch or the metadata insert fails (rolled back).
fn apply_in_transaction(db: &Db, file: &MigrationFile) -> Result<()> {
    db.transact(|conn| {
        conn.execute_batch(file.sql)?;
        record_migration(conn, file)?;
        Ok(())
    })
}

/// Apply a migration that must run with foreign keys off — a table rebuild.
///
/// `PRAGMA foreign_keys` cannot be toggled inside a transaction, so the pragma is flipped
/// around the migration's own transaction, all on the one connection `Db::write` hands
/// out: OFF before it, ON after it. Inside the transaction (where the rebuild happens)
/// `PRAGMA foreign_key_check` must answer no violations before the batch commits — a
/// rebuild that leaves a dangling reference fails the migration instead of corrupting.
/// The connection must not go through `Db::transact`, which would take a second, unrelated
/// pooled connection.
///
/// # Errors
///
/// * [`StorageError::Sqlite`] for the pragma flips, the batch itself, the metadata insert
///   or a `foreign_key_check` that reports violations (all rolled back before the error).
fn apply_with_foreign_keys_off(db: &Db, file: &MigrationFile) -> Result<()> {
    db.write(|conn| {
        conn.pragma_update(None, "foreign_keys", "OFF")
            .map_err(StorageError::from)?;

        let rebuild: Result<()> = (|| {
            let tx = conn.unchecked_transaction().map_err(StorageError::from)?;
            if let Err(e) = apply_batch_and_check(&tx, file) {
                let _ = tx.rollback();
                return Err(e);
            }
            tx.commit().map_err(StorageError::from)
        })();

        // Enforcement comes back on however the rebuild ended; the pragma failure is only
        // surfaced when there is no more specific error to report.
        if let Err(pragma_err) = conn.pragma_update(None, "foreign_keys", "ON") {
            if rebuild.is_ok() {
                return Err(StorageError::from(pragma_err));
            }
        }
        rebuild
    })
}

/// Check that the rebuild violated no foreign key, then record the migration row.
///
/// `PRAGMA foreign_key_check` answers one row per violation (first column: the violated
/// table's name), so an empty mapping means the rebuild is consistent.
///
/// # Errors
///
/// [`StorageError::Sqlite`] when the batch fails, the check itself fails, or violations
/// exist (the caller rolls back).
fn apply_batch_and_check(conn: &rusqlite::Connection, file: &MigrationFile) -> Result<()> {
    conn.execute_batch(file.sql).map_err(StorageError::from)?;
    let violations: std::result::Result<Vec<String>, rusqlite::Error> = conn
        .prepare("PRAGMA foreign_key_check")
        .map_err(StorageError::from)?
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(StorageError::from)?
        .collect();
    if let Some(table) = violations.map_err(StorageError::from)?.first() {
        return Err(StorageError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some(format!("rebuild left foreign-key violations in `{table}`")),
        )));
    }
    record_migration(conn, file)
}

/// Write the `_migrations` row that claims this file as applied.
///
/// # Errors
///
/// [`StorageError::Sqlite`] when the insert fails.
fn record_migration(conn: &rusqlite::Connection, file: &MigrationFile) -> Result<()> {
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
                foreign_keys_off: false,
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

    /// The 0003 upgrade path: a real v2 library survives the `books` rebuild unharmed.
    #[test]
    fn cover_source_rebuild_preserves_rows_children_and_indexes() {
        let db = test_db();

        // Bring the database to version 2 exactly as a pre-0003 user has it.
        db.write(|c| {
            c.execute_batch(MIGRATIONS_TABLE)?;
            c.execute_batch(crate::migrations::M0001_INIT)?;
            c.execute(
                "INSERT INTO _migrations (version, name, checksum, applied_at) VALUES (1,'init',?1,7)",
                rusqlite::params![sha256_hex(crate::migrations::M0001_INIT)],
            )?;
            c.execute_batch(crate::migrations::M0002_BOOK_PUBLISHER)?;
            c.execute(
                "INSERT INTO _migrations (version, name, checksum, applied_at) VALUES (2,'book_publisher',?1,7)",
                rusqlite::params![sha256_hex(crate::migrations::M0002_BOOK_PUBLISHER)],
            )?;
            // Two books: one with an OpenLibrary cover, one with none; a publisher, an
            // author via the junction, and a quote referencing the first book.
            c.execute_batch("INSERT INTO publishers (id, name, name_norm, created_at, updated_at)
                             VALUES ('p1','دار النشر','دار النشر',1,1);
                             INSERT INTO authors (id, name, name_norm, created_at, updated_at)
                             VALUES ('a1','روالد داهل','روالد داهل',1,1);
                             INSERT INTO languages (code, name_ar, name_en, direction)
                             VALUES ('ar','العربية','Arabic','rtl');
                             INSERT INTO books (id, title, title_norm, language_code,
                                                cover_path, cover_source,
                                                source, created_at, updated_at, publisher_id)
                             VALUES ('b1','ت','ت','ar','covers/b1.jpg','openlibrary',
                                     'manual',1,1,'p1');
                             INSERT INTO books (id, title, title_norm, language_code,
                                                source, created_at, updated_at)
                             VALUES ('b2','ك','ك','ar','manual',1,1);
                             INSERT INTO book_authors (book_id, author_id)
                             VALUES ('b1','a1');
                             INSERT INTO quotes (id, book_id, text, text_norm, created_at, updated_at)
                             VALUES ('q1','b1','اقتباس','اقتباس',1,1);")?;
            Ok(())
        })
        .unwrap();

        let version = apply(&db, || Ok(())).unwrap();
        assert_eq!(version, 3);

        db.read(|c| {
            // Rows survive column for column.
            let (cover_path, cover_source, publisher_id): (String, String, String) = c
                .query_row(
                    "SELECT cover_path, cover_source, publisher_id FROM books WHERE id='b1'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            assert_eq!(
                (
                    cover_path.as_str(),
                    cover_source.as_str(),
                    publisher_id.as_str()
                ),
                ("covers/b1.jpg", "openlibrary", "p1")
            );
            // Children and their FKs survived: the quote still reaches the book row.
            let n: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM quotes q JOIN books b ON b.id = q.book_id
                     WHERE q.id='q1' AND b.title='ت'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "quotes must still join books");
            // The junction row survived.
            let n: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM book_authors WHERE book_id='b1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "book_authors must survive the rebuild");
            // The new CHECK vocabulary is live: both new sources are accepted...
            c.execute("UPDATE books SET cover_source='amazon' WHERE id='b1'", [])
                .unwrap();
            c.execute(
                "UPDATE books SET cover_source='internet_archive' WHERE id='b2'",
                [],
            )
            .unwrap();
            // ...and junk is not.
            assert!(
                c.execute(
                    "UPDATE books SET cover_source=' nonsense' WHERE id='b1'",
                    [],
                )
                .is_err(),
                "the widened CHECK must still reject unknown sources"
            );
            // The dropped-and-recreated indexes are back.
            for index in [
                "ux_books_isbn13",
                "ux_books_isbn10",
                "ix_books_title_norm",
                "ix_books_status",
                "ix_books_year",
                "ix_books_rating",
                "ix_books_progress",
                "ix_books_updated",
                "ix_books_created",
                "ix_books_series",
                "ix_books_loc",
            ] {
                let n: i64 = c
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                        [index],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(n, 1, "index `{index}` missing after the rebuild");
            }
            Ok(())
        })
        .unwrap();

        // Foreign-key enforcement is back on after the rebuild.
        let err = db.write(|c| {
            c.execute(
                "INSERT INTO books (id, title, title_norm, language_code, reading_status,
                                    progress_pct, source, created_at, updated_at)
                 VALUES ('b3','t','t','no_such_lang','to_read',0,'manual',1,1)",
                [],
            )?;
            Ok(())
        });
        assert!(
            err.is_err(),
            "foreign keys must be enforced again after the rebuild"
        );
    }
}
