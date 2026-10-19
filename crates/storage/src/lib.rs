//! # maktabati-storage
//!
//! SQLite persistence for **Maktabati**: connection pool, checksummed migrations, seed data and
//! repositories.
//!
//! This crate is **fully synchronous** by design (ADR-0005): `rusqlite` beats async SQLite
//! crates on an embedded, single-user desktop workload, and there is no async runtime to carry
//! through the repository layer. `src-tauri` calls it from `spawn_blocking` (ADR-0010).
//!
//! ## Layout
//!
//! | Module | Contains |
//! |---|---|
//! | [`error`] | `StorageError` with stable IPC codes |
//! | [`conn`] | `Db` pool, PRAGMA set, transactions, `now_ms()` |
//! | [`migrate`] | embedded, checksummed, transactional migrations + pre-migration hook |
//! | [`migrations`] | `include_str!` of each migration file |
//! | [`seed`] | languages, bilingual taxonomy, default rubric (idempotent) |
//! | [`repo`] | typed queries against the schema |
//!
//! ## Usage
//!
//! ```no_run
//! use maktabati_storage::{conn::Db, migrate, seed};
//!
//! let db = Db::open(std::path::Path::new("library.db"))?;
//! migrate::apply(&db, || {
//!     // automatic pre-migration backup — abort if this fails
//!     Ok(())
//! })?;
//! db.write(seed::apply)?;
//! # Ok::<(), maktabati_storage::StorageError>(())
//! ```

#![forbid(unsafe_code)]

pub mod conn;
pub mod error;
pub mod migrate;
pub mod migrations;
pub mod repo;
pub mod seed;

pub use conn::{now_ms, Db, OpenOptions};
pub use error::{duplicate, not_found, Result, StorageError};
