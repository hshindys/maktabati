//! Embedded migration SQL.
//!
//! Kept in a dedicated module so `migrate::embedded()` can name each file explicitly and a
//! missing file is a **compile error**, not a runtime surprise.

/// `0001_init.sql` — the complete schema (mirrors `docs/03-data-model.md` §3).
pub const M0001_INIT: &str = include_str!("../migrations/0001_init.sql");
