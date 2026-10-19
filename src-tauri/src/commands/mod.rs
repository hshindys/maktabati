//! Tauri commands — the IPC surface.
//!
//! Every command is `async` and runs its blocking storage work on `spawn_blocking` (ADR-0010),
//! so the webview's main thread is never stalled by disk I/O.
//!
//! Commands are registered in [`crate::commands`] via `tauri_specta::collect_commands!`, which
//! also generates the TypeScript contract into `src/bindings.ts`. A CI drift gate fails the
//! build when the Rust surface changes without the generated file being refreshed (ADR-0002).

pub mod catalogue;

use std::sync::Arc;

use chrono::Datelike;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use maktabati_domain::Json;
use maktabati_storage::{conn, migrate, repo, Db};

use crate::error::{AppErrorDto, CmdResult, IntoCmdResult};
use crate::state::AppState;

/// Run a storage closure on the blocking pool.
async fn blocking<T, F>(db: Arc<Db>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Db) -> maktabati_storage::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || f(&db).into_cmd())
        .await
        .unwrap_or_else(|join_err| {
            Err(AppErrorDto::new(
                "internal",
                format!("background storage task failed: {join_err}"),
            ))
        })
}

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostics
// ─────────────────────────────────────────────────────────────────────────────

/// Static facts about this installation, shown in *Settings → About* and in diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppInfo {
    /// Product name as shipped.
    pub name: String,
    /// Bundle version, e.g. `0.1.0`.
    pub version: String,
    /// Absolute path of the data directory (`…/com.maktabati.desktop`).
    pub data_dir: String,
    /// Absolute path of the SQLite file inside the data directory.
    pub db_path: String,
    /// Highest applied migration version.
    pub schema_version: i64,
    /// Number of migrations embedded in this binary.
    pub embedded_migrations: i64,
    /// `true` when there are migrations this binary would still apply.
    pub pending_migrations: bool,
    /// Target family: `windows`, `linux` or `macos`.
    pub platform: String,
    /// Total size of the data directory in bytes (DB + WAL + covers + files + backups).
    pub data_dir_bytes: u64,
}

/// Return the facts about this installation.
///
/// # Errors
///
/// [`AppErrorDto`] when the data directory cannot be sized or the schema version cannot be read.
#[specta::specta]
#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    let version = state.app_version.clone();
    blocking(db, move |db| {
        let schema_version = repo::summary::schema_version(db)?;
        let embedded = i64::try_from(migrate::embedded().len()).unwrap_or(i64::MAX);
        Ok(AppInfo {
            name: "Maktabati".to_owned(),
            version,
            data_dir: data_dir.display().to_string(),
            db_path: data_dir.join("library.db").display().to_string(),
            schema_version,
            embedded_migrations: embedded,
            pending_migrations: migrate::pending(db)?,
            platform: std::env::consts::OS.to_owned(),
            data_dir_bytes: dir_size(&data_dir),
        })
    })
    .await
}

/// Recursive size of a directory, skipping unreadable entries.
fn dir_size(path: &std::path::Path) -> u64 {
    let mut total: u64 = 0;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            total = total.saturating_add(dir_size(&entry.path()));
        } else {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

/// Result of [`library_health`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct LibraryHealth {
    /// Result of `PRAGMA integrity_check` — `ok` or the first failure SQLite reports.
    pub integrity: String,
    /// `true` only when `integrity` is literally `ok`.
    pub integrity_ok: bool,
    /// `true` when the data directory exists and is writable right now.
    pub data_dir_writable: bool,
    /// Books with no credited author.
    pub books_without_author: i64,
    /// Books whose `search_document` row is missing (search would silently miss them).
    pub books_missing_from_search: i64,
    /// `reading_cycles` rows pointing at a book that no longer exists (should be 0 — FKs).
    pub orphan_cycles: i64,
    /// Rows in `activity_log` older than the oldest book they reference (should be 0).
    pub schema_version: i64,
    /// Size of the main database file in bytes.
    pub db_bytes: u64,
    /// Size of the write-ahead-log sidecar in bytes (non-zero after writes is normal).
    pub wal_bytes: u64,
    /// When this check ran, epoch milliseconds.
    pub checked_at_ms: i64,
}

/// Run integrity and referential sanity checks. Powers the Settings → Storage health screen.
///
/// # Errors
///
/// [`AppErrorDto`] when the checks cannot be executed.
#[specta::specta]
#[tauri::command]
pub async fn library_health(state: State<'_, AppState>) -> CmdResult<LibraryHealth> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| {
        let db_path = data_dir.join("library.db");
        let (integrity, books_without_author, books_missing_from_search, orphan_cycles) =
            db.read(|c| {
                let integrity: String =
                    c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?;
                let books_without_author: i64 = c.query_row(
                    "SELECT COUNT(*) FROM books b
                     WHERE NOT EXISTS (SELECT 1 FROM book_authors ba WHERE ba.book_id = b.id)",
                    [],
                    |r| r.get(0),
                )?;
                let books_missing_from_search: i64 = c.query_row(
                    "SELECT COUNT(*) FROM books b
                     WHERE NOT EXISTS (SELECT 1 FROM search_document sd WHERE sd.book_id = b.id)",
                    [],
                    |r| r.get(0),
                )?;
                let orphan_cycles: i64 = c.query_row(
                    "SELECT COUNT(*) FROM reading_cycles rc
                     WHERE NOT EXISTS (SELECT 1 FROM books b WHERE b.id = rc.book_id)",
                    [],
                    |r| r.get(0),
                )?;
                Ok((
                    integrity,
                    books_without_author,
                    books_missing_from_search,
                    orphan_cycles,
                ))
            })?;

        Ok(LibraryHealth {
            integrity_ok: integrity == "ok",
            integrity,
            data_dir_writable: dir_is_writable(&data_dir),
            books_without_author,
            books_missing_from_search,
            orphan_cycles,
            schema_version: repo::summary::schema_version(db)?,
            db_bytes: file_size(&db_path),
            wal_bytes: file_size(&std::path::PathBuf::from(format!(
                "{}-wal",
                db_path.display()
            ))),
            checked_at_ms: conn::now_ms(),
        })
    })
    .await
}

fn file_size(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn dir_is_writable(path: &std::path::Path) -> bool {
    let probe = path.join(format!(".write-probe-{}", std::process::id()));
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Library
// ─────────────────────────────────────────────────────────────────────────────

/// Aggregate library counts for the dashboard and the sidebar badges.
///
/// `year` selects the window for the "finished this year" counter; when omitted the caller's
/// current Gregorian year is used.
///
/// # Errors
///
/// [`AppErrorDto`] when any aggregate query fails.
#[specta::specta]
#[tauri::command]
pub async fn library_summary(
    state: State<'_, AppState>,
    year: Option<i32>,
) -> CmdResult<maktabati_storage::repo::summary::LibrarySummary> {
    let db = Arc::clone(&state.db);
    let year = year.unwrap_or_else(|| chrono::Utc::now().year());
    blocking(db, move |db| repo::summary::library_summary(db, year)).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings
// ─────────────────────────────────────────────────────────────────────────────

/// Read one non-secret setting as raw JSON, or `null` when unset.
///
/// The payload is [`Json`] rather than `serde_json::Value`: the built-in specta impl for `Value`
/// expands structurally at every use site and `Value` contains `Value`, so exporting it does not
/// terminate. See `maktabati_domain::json` — the wire format is identical either way.
///
/// # Errors
///
/// [`AppErrorDto`] when the stored JSON cannot be decoded.
#[specta::specta]
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>, key: String) -> CmdResult<Option<Json>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::settings::all(db).map(|pairs| {
            pairs
                .into_iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| Json::from(v))
        })
    })
    .await
}

/// Write one non-secret setting. Upserts.
///
/// # Errors
///
/// [`AppErrorDto`] when the value cannot be serialized or persisted.
#[specta::specta]
#[tauri::command]
pub async fn settings_set(state: State<'_, AppState>, key: String, value: Json) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    // The storage layer owns `serde_json::Value`; the conversion is lossless (see `domain::json`).
    let value = value.into_value();
    blocking(db, move |db| repo::settings::set(db, &key, &value)).await
}

/// Read every setting, as shown on the Settings → Diagnostics page.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn settings_all(state: State<'_, AppState>) -> CmdResult<Vec<(String, Json)>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::settings::all(db)
            .map(|pairs| pairs.into_iter().map(|(k, v)| (k, Json::from(v))).collect())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn dir_size_counts_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), vec![0u8; 100]).unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub").join("b"), vec![0u8; 50]).unwrap();
        assert_eq!(dir_size(dir.path()), 150);
    }

    #[test]
    fn dir_size_of_missing_path_is_zero() {
        assert_eq!(dir_size(std::path::Path::new("/definitely/not/here")), 0);
    }

    #[test]
    fn file_size_of_missing_file_is_zero() {
        assert_eq!(file_size(std::path::Path::new("/nope.bin")), 0);
    }

    #[test]
    fn dir_is_writable_probes_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        assert!(dir_is_writable(dir.path()));
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(
            leftovers.is_empty(),
            "probe file must be removed: {leftovers:?}"
        );
        assert!(!dir_is_writable(std::path::Path::new(
            "/definitely/not/here"
        )));
    }

    #[test]
    fn app_info_and_health_are_specta_exportable() {
        // Compile-time guarantee: these are the types the generated `bindings.ts` depends on.
        fn assert_type<T: specta::Type>() {}
        assert_type::<AppInfo>();
        assert_type::<LibraryHealth>();
    }
}
