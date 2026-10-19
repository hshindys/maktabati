//! SQLite connection management: PRAGMAs, a tiny pool, and typed query helpers.
//!
//! `rusqlite` is synchronous by design (ADR-0005), so **this crate never blocks an async
//! runtime** — `src-tauri` calls it from `spawn_blocking` (ADR-0010).

use std::path::Path;
use std::sync::{Condvar, Mutex};

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
                in_memory: true,
            });
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let size = opts.pool_size.max(1);
        let mut conns = Vec::with_capacity(size);
        for _ in 0..size {
            let conn = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX
                    | OpenFlags::SQLITE_OPEN_URI,
            )?;
            apply_pragmas(&conn, false)?;
            conns.push(conn);
        }

        Ok(Self {
            free: Mutex::new(conns),
            available: Condvar::new(),
            write_gate: Mutex::new(()),
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
