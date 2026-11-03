//! Embedded migration SQL.
//!
//! Kept in a dedicated module so `migrate::embedded()` can name each file explicitly and a
//! missing file is a **compile error**, not a runtime surprise.

/// `0001_init.sql` — the initial schema (mirrors `docs/03-data-model.md` §3).
pub const M0001_INIT: &str = include_str!("../migrations/0001_init.sql");

/// `0002_book_publisher.sql` — links a book to its publisher (mirrors `docs/03-data-model.md` §3).
pub const M0002_BOOK_PUBLISHER: &str = include_str!("../migrations/0002_book_publisher.sql");

/// `0003_cover_sources.sql` — widens `books.cover_source`'s CHECK for the two new
/// internet providers (Amazon, Internet Archive). Apply with foreign keys off; see the
/// file header for why (it rebuilds `books`, and `DROP TABLE` runs an implicit
/// FK-checked DELETE).
pub const M0003_COVER_SOURCES: &str = include_str!("../migrations/0003_cover_sources.sql");

/// `0004_search_index.sql` — Arabic-normalized search projections and FTS5 synchronization.
pub const M0004_SEARCH_INDEX: &str = include_str!("../migrations/0004_search_index.sql");
