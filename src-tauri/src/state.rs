//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::Arc;

use maktabati_storage::Db;

/// State handed to every command via `tauri::State`.
///
/// The database handle is wrapped in an [`Arc`] so a command can clone it into
/// `spawn_blocking` (ADR-0010): `rusqlite` is synchronous, and blocking a Tauri webview thread
/// with disk I/O is the single easiest way to make a desktop app feel slow.
pub struct AppState {
    /// The pooled SQLite handle.
    pub db: Arc<Db>,
    /// The OS application-data directory that owns the DB, covers, files and backups.
    pub data_dir: PathBuf,
    /// Resolved `CARGO_PKG_VERSION` of the app crate.
    pub app_version: String,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("db", &self.db)
            .field("data_dir", &self.data_dir)
            .field("app_version", &self.app_version)
            .finish()
    }
}
