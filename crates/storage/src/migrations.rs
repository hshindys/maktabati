//! Embedded migration SQL.
//!
//! Kept in a dedicated module so `migrate::embedded()` can name each file explicitly and a
//! missing file is a **compile error**, not a runtime surprise.

/// `0001_init.sql` — the initial schema (mirrors `docs/03-data-model.md` §3).
pub const M0001_INIT: &str = include_str!("../migrations/0001_init.sql");

/// `0002_book_publisher.sql` — links a book to its publisher (mirrors `docs/03-data-model.md` §3).
pub const M0002_BOOK_PUBLISHER: &str = include_str!("../migrations/0002_book_publisher.sql");
