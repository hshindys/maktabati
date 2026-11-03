//! SQLite connection management: PRAGMAs, a tiny pool, and typed query helpers.
//!
//! `rusqlite` is synchronous by design (ADR-0005), so **this crate never blocks an async
//! runtime** — `src-tauri` calls it from `spawn_blocking` (ADR-0010).

use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, RwLock};

use rusqlite::{Connection, OpenFlags};

use crate::Result;

/// How many connections a file-backed database keeps. WAL allows many readers alongside one
/// writer; 4 is plenty for a single-user desktop app and keeps the dashboard aggregate from
/// blocking the next UI query.
pub const DEFAULT_POOL_SIZE: usize = 4;

/// Options for [`Db::open`].
#[derive(Debug, Clone)]
pub struct OpenOptions {
    /// `true` for an in-memory database (tests).
    pub in_memory: bool,
    /// Number of pooled connections (ignored for in-memory databases, which get one).
    pub pool_size: usize,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            in_memory: false,
            pool_size: DEFAULT_POOL_SIZE,
        }
    }
}

/// A small connection pool with a writer gate.
///
/// * [`Db::read`] — takes any free connection.
/// * [`Db::write`] — takes a free connection **and** holds the writer gate, so only one write
///   transaction is open at a time (SQLite's WAL model: many readers, one writer).
pub struct Db {
    free: Mutex<Vec<Connection>>,
    available: Condvar,
    write_gate: Mutex<()>,
    maintenance: RwLock<()>,
    path: Option<PathBuf>,
    pool_size: usize,
    in_memory: bool,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db")
            .field("in_memory", &self.in_memory)
            .finish_non_exhaustive()
    }
}

impl Db {
    /// Open (or create) a database file, applying the PRAGMA set from ADR-0003.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError::Sqlite`] when the file cannot be opened or a PRAGMA fails.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, OpenOptions::default())
    }

    /// Open with explicit options.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError::Sqlite`] when the file cannot be opened or a PRAGMA fails.
    pub fn open_with(path: &Path, opts: OpenOptions) -> Result<Self> {
        if opts.in_memory {
            let conn = Connection::open_in_memory()?;
            apply_pragmas(&conn, true)?;
            return Ok(Self {
                free: Mutex::new(vec![conn]),
                available: Condvar::new(),
                write_gate: Mutex::new(()),
                maintenance: RwLock::new(()),
                path: None,
                pool_size: 1,
                in_memory: true,
            });
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let size = opts.pool_size.max(1);
        let conns = open_connections(path, size, true)?;
        let canonical_path = std::fs::canonicalize(path)?;

        Ok(Self {
            free: Mutex::new(conns),
            available: Condvar::new(),
            write_gate: Mutex::new(()),
            maintenance: RwLock::new(()),
            path: Some(canonical_path),
            pool_size: size,
            in_memory: false,
        })
    }

    /// In-memory databases share a single connection, so they must never be opened with a pool.
    #[must_use]
    pub const fn is_in_memory(&self) -> bool {
        self.in_memory
    }

    /// Run a closure with a connection for reading.
    ///
    /// # Errors
    ///
    /// Whatever the closure returns; [`crate::StorageError::Busy`] if every connection is in use for
    /// longer than the configured busy timeout (in practice this cannot happen, since the pool
    /// hands out connections without an internal timeout — it exists to keep call sites total).
    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let _maintenance = self.maintenance.read().unwrap_or_else(|e| e.into_inner());
        let conn = self.take()?;
        let out = f(&conn);
        self.give(conn);
        out
    }

    /// Run a closure with an exclusive writer connection.
    ///
    /// Only one writer runs at a time; readers continue on their own connections (WAL).
    ///
    /// # Errors
    ///
    /// Whatever the closure returns.
    pub fn write<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let _maintenance = self.maintenance.read().unwrap_or_else(|e| e.into_inner());
        let gate = self.write_gate.lock().unwrap_or_else(|e| e.into_inner());
        let conn = self.take()?;
        let out = f(&conn);
        self.give(conn);
        drop(gate);
        out
    }

    /// Run a closure inside an exclusive, all-or-nothing transaction.
    ///
    /// The transaction is committed when the closure returns `Ok`, and rolled back on `Err` or
    /// panic, so a partially applied write can never be observed.
    ///
    /// # Errors
    ///
    /// Whatever the closure returns, plus [`crate::StorageError::Sqlite`] on commit/rollback failure.
    pub fn transact<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let _maintenance = self.maintenance.read().unwrap_or_else(|e| e.into_inner());
        let gate = self.write_gate.lock().unwrap_or_else(|e| e.into_inner());
        let conn = self.take()?;

        let tx = conn.unchecked_transaction()?;
        let out = f(&tx);
        let finished = match &out {
            Ok(_) => tx.commit(),
            Err(_) => tx.rollback(),
        };
        self.give(conn);
        drop(gate);

        match finished {
            Ok(()) => out,
            Err(e) => Err(e.into()),
        }
    }

    /// Copy the whole database to `dest` using SQLite's **online backup API**.
    ///
    /// This is the mechanism behind the automatic pre-migration backup (ADR-0003). A plain
    /// `fs::copy` of a WAL database is not safe — committed pages may still live in the
    /// `-wal` sidecar — so the backup is done through SQLite itself and is therefore correct
    /// even while other connections are open.
    ///
    /// The writer gate is held for the duration, so the copy cannot observe a half-applied
    /// transaction.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError::Sqlite`] when the destination cannot be created or the copy fails;
    /// [`crate::StorageError::Io`] when the destination's parent directory cannot be created.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        let _maintenance = self.maintenance.read().unwrap_or_else(|e| e.into_inner());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let gate = self.write_gate.lock().unwrap_or_else(|e| e.into_inner());
        let source = self.take()?;
        let result = (|| -> Result<()> {
            let mut dst = Connection::open(dest)?;
            let backup = rusqlite::backup::Backup::new(&source, &mut dst)?;
            backup.run_to_completion(100, std::time::Duration::from_millis(5), None)?;
            Ok(())
        })();
        self.give(source);
        drop(gate);
        result
    }

    /// Atomically replace this file-backed database from `source`.
    ///
    /// A consistent safety snapshot is created before the pool is quiesced. The replacement is
    /// staged and integrity-checked first; then all pool connections are closed while the
    /// maintenance lock excludes readers and writers. If swapping or reopening the new pool
    /// fails, the original database file is restored and its connections are reopened.
    ///
    /// The caller is responsible for validating the source schema version before calling.
    ///
    /// # Errors
    ///
    /// Returns an error if snapshotting, staging, swapping, or recovery fails.
    pub fn replace_from(&self, source: &Path, safety_snapshot: &Path) -> Result<()> {
        let Some(db_path) = self.path.as_deref() else {
            return Err(crate::StorageError::Migration(
                "an in-memory database cannot be replaced".to_owned(),
            ));
        };
        let source_path = std::fs::canonicalize(source)?;
        let db_path = std::fs::canonicalize(db_path)?;
        if source_path == db_path {
            return Err(crate::StorageError::Migration(
                "restore source must be a separate file".to_owned(),
            ));
        }

        let safety_parent = safety_snapshot
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(safety_parent)?;
        let safety_name = safety_snapshot.file_name().ok_or_else(|| {
            crate::StorageError::Migration("safety snapshot path has no file name".to_owned())
        })?;
        let safety_path = std::fs::canonicalize(safety_parent)?.join(safety_name);
        if safety_path == db_path || safety_path == source_path {
            return Err(crate::StorageError::Migration(
                "safety snapshot must be separate from the current database and restore source"
                    .to_owned(),
            ));
        }

        let _maintenance = self.maintenance.write().unwrap_or_else(|e| e.into_inner());
        let _write_gate = self.write_gate.lock().unwrap_or_else(|e| e.into_inner());
        let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
        if free.len() != self.pool_size {
            return Err(crate::StorageError::Busy);
        }
        match std::fs::symlink_metadata(safety_snapshot) {
            Ok(_) => {
                return Err(crate::StorageError::Migration(format!(
                    "safety snapshot already exists: {}",
                    safety_snapshot.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let snapshot_result = (|| -> Result<()> {
            let source = free.first().ok_or(crate::StorageError::Busy)?;
            let mut destination = Connection::open(safety_snapshot)?;
            let backup = rusqlite::backup::Backup::new(source, &mut destination)?;
            backup.run_to_completion(100, std::time::Duration::from_millis(5), None)?;
            Ok(())
        })();
        if let Err(error) = snapshot_result {
            remove_if_exists(safety_snapshot)?;
            return Err(error);
        }

        let name = db_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "library.db".to_owned());
        let suffix = format!("{}-{}", std::process::id(), now_ms());
        let candidate = db_path.with_file_name(format!("{name}.restore-{suffix}.tmp"));
        let rollback = db_path.with_file_name(format!("{name}.rollback-{suffix}.db"));
        if candidate.exists() || rollback.exists() {
            return Err(crate::StorageError::Migration(
                "temporary restore path already exists".to_owned(),
            ));
        }

        let prepare_result = (|| -> Result<()> {
            let source = Connection::open_with_flags(
                source,
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX
                    | OpenFlags::SQLITE_OPEN_URI,
            )?;
            let mut destination = Connection::open(&candidate)?;
            let backup = rusqlite::backup::Backup::new(&source, &mut destination)?;
            backup.run_to_completion(100, std::time::Duration::from_millis(5), None)?;
            drop(backup);
            drop(destination);

            let staged = Connection::open_with_flags(
                &candidate,
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX
                    | OpenFlags::SQLITE_OPEN_URI,
            )?;
            let integrity: String =
                staged.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
            if integrity != "ok" {
                return Err(crate::StorageError::Migration(format!(
                    "restore source failed integrity_check: {integrity}"
                )));
            }
            Ok(())
        })();
        if let Err(error) = prepare_result {
            remove_if_exists(&candidate)?;
            return Err(error);
        }

        let Some(connection) = free.first() else {
            remove_if_exists(&candidate)?;
            return Err(crate::StorageError::Busy);
        };
        let checkpoint_result =
            connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            });
        let checkpoint: (i64, i64, i64) = match checkpoint_result {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                remove_if_exists(&candidate)?;
                return Err(error.into());
            }
        };
        if checkpoint.0 != 0 {
            remove_if_exists(&candidate)?;
            return Err(crate::StorageError::Busy);
        }

        drop(std::mem::take(&mut *free));
        let swap_result = (|| -> Result<Vec<Connection>> {
            remove_if_exists(&sidecar_path(&db_path, "-wal"))?;
            remove_if_exists(&sidecar_path(&db_path, "-shm"))?;
            std::fs::rename(&db_path, &rollback)?;
            if let Err(error) = std::fs::rename(&candidate, &db_path) {
                std::fs::rename(&rollback, &db_path)?;
                return Err(error.into());
            }
            open_connections(&db_path, self.pool_size, false)
        })();

        match swap_result {
            Ok(connections) => {
                *free = connections;
                if let Err(error) = std::fs::remove_file(&rollback) {
                    tracing::warn!(
                        path = %rollback.display(),
                        %error,
                        "could not remove post-restore rollback file"
                    );
                }
                Ok(())
            }
            Err(error) => {
                free.clear();
                let rollback_result = if rollback.exists() {
                    remove_if_exists(&db_path)
                        .and_then(|()| remove_if_exists(&sidecar_path(&db_path, "-wal")))
                        .and_then(|()| remove_if_exists(&sidecar_path(&db_path, "-shm")))
                        .and_then(|()| std::fs::rename(&rollback, &db_path).map_err(Into::into))
                } else {
                    Ok(())
                };
                let recovery_error = rollback_result.err();
                match open_connections(&db_path, self.pool_size, false) {
                    Ok(connections) => {
                        *free = connections;
                        if let Some(recovery_error) = recovery_error {
                            Err(crate::StorageError::Migration(format!(
                                "restore failed: {error}; rollback failed: {recovery_error}"
                            )))
                        } else {
                            Err(error)
                        }
                    }
                    Err(reopen_error) => Err(crate::StorageError::Migration(format!(
                        "restore failed: {error}; database recovery failed: {}",
                        recovery_error.map_or_else(
                            || reopen_error.to_string(),
                            |recovery| format!("{recovery}; {reopen_error}")
                        )
                    ))),
                }
            }
        }
    }

    fn take(&self) -> Result<Connection> {
        let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(conn) = free.pop() {
                return Ok(conn);
            }
            // Should not happen with sane usage; fall through to wait rather than deadlock.
            free = self.available.wait(free).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn give(&self, conn: Connection) {
        let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
        free.push(conn);
        drop(free);
        self.available.notify_one();
    }
}

/// Apply the PRAGMA set documented in ADR-0003.
fn apply_pragmas(conn: &Connection, in_memory: bool) -> Result<()> {
    if !in_memory {
        // `journal_mode` is persistent; the rest are per-connection.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "wal_autocheckpoint", 1000)?;
    }
    // SQLite defaults foreign_keys to OFF — this is the single most important line here.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    if !in_memory {
        conn.pragma_update(None, "mmap_size", 268_435_456i64)?;
    }
    Ok(())
}

fn open_connections(path: &Path, pool_size: usize, create: bool) -> Result<Vec<Connection>> {
    let mut connections = Vec::with_capacity(pool_size);
    for _ in 0..pool_size {
        let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX
            | OpenFlags::SQLITE_OPEN_URI;
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        let connection = Connection::open_with_flags(path, flags)?;
        apply_pragmas(&connection, false)?;
        connections.push(connection);
    }
    Ok(connections)
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| std::ffi::OsString::from("library.db"), ToOwned::to_owned);
    name.push(suffix);
    path.with_file_name(name)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Current time as epoch milliseconds UTC — the single clock used by every table.
#[must_use]
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Db {
    /// Test helper: an in-memory database.
    #[cfg(test)]
    pub(crate) fn open_in_memory_for_test() -> Self {
        Self::open_with(
            Path::new(":memory:"),
            OpenOptions {
                in_memory: true,
                pool_size: 1,
            },
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageError;
    use pretty_assertions::assert_eq;

    #[test]
    fn pragmas_are_applied() {
        let db = Db::open_in_memory_for_test();
        db.read(|c| {
            let fk: i64 = c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
            assert_eq!(fk, 1, "foreign_keys must be ON");
            let busy: i64 = c.query_row("PRAGMA busy_timeout", [], |r| r.get(0))?;
            assert_eq!(busy, 5000);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn wal_mode_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let db = Db::open(&path).unwrap();
        db.read(|c| {
            let mode: String = c.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
            assert_eq!(mode.to_lowercase(), "wal");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn replace_from_swaps_database_and_keeps_a_safety_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let target_path = dir.path().join("library.db");
        let source_path = dir.path().join("backup.db");
        let safety_path = dir.path().join("safety.db");
        let db = Db::open(&target_path).unwrap();
        db.write(|connection| {
            connection.execute_batch("CREATE TABLE marker (value TEXT NOT NULL)")?;
            connection.execute("INSERT INTO marker VALUES ('before')", [])?;
            Ok(())
        })
        .unwrap();
        {
            let source = Connection::open(&source_path).unwrap();
            source
                .execute_batch("CREATE TABLE marker (value TEXT NOT NULL)")
                .unwrap();
            source
                .execute("INSERT INTO marker VALUES ('restored')", [])
                .unwrap();
        }

        db.replace_from(&source_path, &safety_path).unwrap();

        db.read(|connection| {
            let value: String =
                connection.query_row("SELECT value FROM marker", [], |row| row.get(0))?;
            assert_eq!(value, "restored");
            Ok(())
        })
        .unwrap();
        let safety = Connection::open(safety_path).unwrap();
        let value: String = safety
            .query_row("SELECT value FROM marker", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "before");
    }

    #[test]
    fn replace_from_invalid_database_keeps_current_database_usable() {
        let dir = tempfile::tempdir().unwrap();
        let target_path = dir.path().join("library.db");
        let source_path = dir.path().join("invalid.db");
        let safety_path = dir.path().join("safety.db");
        let db = Db::open(&target_path).unwrap();
        db.write(|connection| {
            connection.execute_batch("CREATE TABLE marker (value TEXT NOT NULL)")?;
            connection.execute("INSERT INTO marker VALUES ('before')", [])?;
            Ok(())
        })
        .unwrap();
        std::fs::write(&source_path, b"not a sqlite database").unwrap();

        assert!(db.replace_from(&source_path, &safety_path).is_err());

        db.read(|connection| {
            let value: String =
                connection.query_row("SELECT value FROM marker", [], |row| row.get(0))?;
            assert_eq!(value, "before");
            Ok(())
        })
        .unwrap();
        assert!(safety_path.exists());
    }

    #[test]
    fn transaction_rolls_back_on_error() {
        let db = Db::open_in_memory_for_test();
        db.write(|c| {
            c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT NOT NULL)")?;
            Ok(())
        })
        .unwrap();

        let err = db.transact(|c| {
            c.execute("INSERT INTO t (v) VALUES ('a')", [])?;
            Err::<(), StorageError>(StorageError::Busy)
        });
        assert!(err.is_err());

        db.read(|c| {
            let n: i64 = c.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))?;
            assert_eq!(n, 0, "rolled-back insert must not be visible");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn transaction_commits_on_success() {
        let db = Db::open_in_memory_for_test();
        db.transact(|c| {
            c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)")?;
            c.execute("INSERT INTO t DEFAULT VALUES", [])?;
            Ok(())
        })
        .unwrap();
        db.read(|c| {
            let n: i64 = c.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))?;
            assert_eq!(n, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn backup_to_produces_a_readable_copy() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("src.db");
        let db = Db::open(&db_path).unwrap();
        db.write(|c| {
            c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT NOT NULL)")?;
            c.execute("INSERT INTO t (v) VALUES ('hello')", [])?;
            Ok(())
        })
        .unwrap();

        let dest = dir.path().join("backups").join("snap.db");
        db.backup_to(&dest).unwrap();
        assert!(dest.exists());

        // The copy must be independently readable and contain the data.
        let copied = Connection::open(&dest).unwrap();
        let n: i64 = copied
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let v: String = copied
            .query_row("SELECT v FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "hello");
    }

    #[test]
    fn backup_to_overwrites_a_previous_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        db.write(|c| {
            c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)")?;
            Ok(())
        })
        .unwrap();
        let dest = dir.path().join("snap.db");
        db.backup_to(&dest).unwrap();
        db.backup_to(&dest).unwrap();
        assert!(dest.exists());
    }

    #[test]
    fn backup_to_fails_cleanly_when_the_destination_is_unwritable() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        // A path whose parent is an existing *file* cannot be created.
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        let err = db.backup_to(&blocker.join("nested").join("snap.db"));
        assert!(err.is_err(), "unwritable destination must surface an error");
    }

    #[test]
    fn concurrent_reads_and_writes_are_serialized_safely() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        let db = std::sync::Arc::new(Db::open(&path).unwrap());
        db.write(|c| {
            c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)")?;
            Ok(())
        })
        .unwrap();

        let mut handles = Vec::new();
        for i in 0..8 {
            let db = db.clone();
            handles.push(std::thread::spawn(move || {
                db.write(move |c| {
                    c.execute("INSERT INTO t DEFAULT VALUES", [])?;
                    Ok(())
                })
                .map(|_| i)
            }));
        }
        for h in handles {
            h.join().unwrap().unwrap();
        }
        db.read(|c| {
            let n: i64 = c.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))?;
            assert_eq!(n, 8);
            Ok(())
        })
        .unwrap();
    }
}
