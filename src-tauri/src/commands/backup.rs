//! Backup restore and portable library export commands.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::State;

use maktabati_storage::{conn, migrate, StorageError};

use crate::error::{CmdResult, IntoCmdResult};
use crate::state::AppState;

/// List saved SQLite backups.
#[specta::specta]
#[tauri::command]
pub async fn backup_list(
    state: State<'_, AppState>,
) -> CmdResult<Vec<maktabati_backup::BackupInfo>> {
    let directory = state.data_dir.join("backups");
    super::blocking(Arc::clone(&state.db), move |_| {
        maktabati_backup::list_backups(&directory)
    })
    .await
}

/// Check SQLite integrity and schema compatibility of a saved backup.
#[specta::specta]
#[tauri::command]
pub async fn backup_verify(
    state: State<'_, AppState>,
    path: String,
) -> CmdResult<maktabati_backup::BackupVerification> {
    let directory = state.data_dir.join("backups");
    let path = PathBuf::from(path);
    super::blocking(Arc::clone(&state.db), move |_| {
        let version = migrate::embedded().last().map_or(0, |item| item.version);
        maktabati_backup::verify_backup(&directory, &path, version)
    })
    .await
}

/// Restore a verified backup and report the automatic pre-restore safety snapshot.
#[specta::specta]
#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    path: String,
) -> CmdResult<maktabati_backup::RestoreResult> {
    let directory = state.data_dir.join("backups");
    let path = PathBuf::from(path);
    super::blocking(Arc::clone(&state.db), move |db| {
        let result = maktabati_backup::restore_backup(db, &directory, &path)?;
        tracing::info!(
            restored = %path.display(),
            safety_snapshot = %result.safety_snapshot,
            "library backup restored"
        );
        Ok(result)
    })
    .await
}

/// Export the complete library as a portable JSON file.
#[specta::specta]
#[tauri::command]
pub async fn library_export_json(state: State<'_, AppState>, path: String) -> CmdResult<String> {
    let path = PathBuf::from(path);
    ensure_extension(&path, "json").into_cmd()?;
    super::blocking(Arc::clone(&state.db), move |db| {
        let contents = maktabati_backup::export_json(db)?;
        std::fs::write(&path, contents)?;
        Ok(path.display().to_string())
    })
    .await
}

/// Export every application table as an individual CSV in a new directory.
#[specta::specta]
#[tauri::command]
pub async fn library_export_csv(state: State<'_, AppState>, parent: String) -> CmdResult<String> {
    let parent = PathBuf::from(parent);
    super::blocking(Arc::clone(&state.db), move |db| {
        maktabati_backup::export_csv(db, &parent).map(|path| path.display().to_string())
    })
    .await
}

/// Replace the current library with a compatible portable JSON export.
#[specta::specta]
#[tauri::command]
pub async fn library_import_json(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    let path = PathBuf::from(path);
    ensure_extension(&path, "json").into_cmd()?;
    let safety_snapshot = state
        .data_dir
        .join("backups")
        .join(format!("pre-import-{}.db", conn::now_ms()));
    super::blocking(Arc::clone(&state.db), move |db| {
        let contents = std::fs::read_to_string(&path)?;
        let parent = safety_snapshot
            .parent()
            .ok_or_else(|| StorageError::Migration("bad import safety snapshot path".to_owned()))?;
        std::fs::create_dir_all(parent)?;
        match std::fs::symlink_metadata(&safety_snapshot) {
            Ok(_) => {
                return Err(StorageError::Migration(
                    "import safety snapshot already exists".to_owned(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        db.backup_to(&safety_snapshot)?;
        maktabati_backup::import_json(db, &contents)
    })
    .await
}

/// Export every note page and block as Markdown.
#[specta::specta]
#[tauri::command]
pub async fn notes_export_markdown(state: State<'_, AppState>, path: String) -> CmdResult<String> {
    let path = PathBuf::from(path);
    ensure_extension(&path, "md").into_cmd()?;
    super::blocking(Arc::clone(&state.db), move |db| {
        let markdown = maktabati_notes::export_markdown(db)?;
        std::fs::write(&path, markdown)?;
        Ok(path.display().to_string())
    })
    .await
}

fn ensure_extension(path: &Path, extension: &str) -> maktabati_storage::Result<()> {
    if path
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
    {
        Ok(())
    } else {
        Err(StorageError::Migration(format!(
            "export path must use the .{extension} extension"
        )))
    }
}
