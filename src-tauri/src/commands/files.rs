//! Attachment commands — attach, list, delete and open the files behind a book.
//!
//! `file_attach`, `file_list` and `file_delete` are the same thin async shell as
//! [`super::catalogue`]: clone the `Arc<Db>`, run the repo call on the blocking pool (ADR-0010),
//! map the error to [`AppErrorDto`] at the boundary. No SQL, no validation and no path handling
//! belong here — that is what `storage` and `domain` are for.
//!
//! `file_open` and `file_reveal` read the row, resolve the on-disk path and hand it to the
//! desktop. The OS call stays on the main thread: it is one `spawn` of the user's own file
//! handler, and parking the webview on it would be worse than doing it inline.

use std::sync::Arc;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use maktabati_domain::{BookFile, BookFileInput, FileKind, StorageMode};
use maktabati_storage::repo;

use super::blocking;
use crate::error::{os_failure, CmdResult};
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Attach / list / delete
// ─────────────────────────────────────────────────────────────────────────────

/// Attach a file to a book, copying it into the library or just remembering where it is.
///
/// `source_path` must be absolute: the dialog returns one, and a relative path would be
/// resolved against whatever the app's working directory happens to be — which is not where the
/// user thinks their file lives.
///
/// # Errors
///
/// * `storage.file_source_missing` when `source_path` is not a readable file.
/// * `storage.not_found` when `book_id` names no book.
/// * `storage.duplicate` when copy mode already holds these exact bytes.
#[specta::specta]
#[tauri::command]
pub async fn file_attach(
    state: State<'_, AppState>,
    book_id: String,
    kind: FileKind,
    storage_mode: StorageMode,
    source_path: String,
    mime: Option<String>,
    page_count: Option<i32>,
) -> CmdResult<BookFile> {
    let input = BookFileInput {
        book_id,
        kind,
        storage_mode,
        source_path,
        mime,
        page_count,
    };
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| repo::file::attach(db, &data_dir, input)).await
}

/// Every attachment on a book, oldest first.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn file_list(state: State<'_, AppState>, book_id: String) -> CmdResult<Vec<BookFile>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::file::list_for_book(db, &book_id)).await
}

/// Remove an attachment.
///
/// A copied blob is deleted only when no other row shares its hash; a reference deletes nothing
/// on disk, because the file belongs to the user and not to the library.
///
/// # Errors
///
/// [`AppErrorDto`] when the row does not exist.
#[specta::specta]
#[tauri::command]
pub async fn file_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| repo::file::delete(db, &data_dir, &id)).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Desktop hand-off
// ─────────────────────────────────────────────────────────────────────────────

/// Open an attachment with the system's default application.
///
/// # Errors
///
/// * `storage.not_found` when the attachment is gone.
/// * `files.missing` when the path is recorded but the bytes are not there — a copied blob
///   deleted from under the library, or a reference whose file has been moved.
/// * `files.open_failed` when the desktop declines to launch a handler.
#[specta::specta]
#[tauri::command]
pub async fn file_open(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    open_or_reveal(app, state, id, false).await
}

/// Reveal an attachment in the file manager.
///
/// # Errors
///
/// As [`file_open`], except the reported failure code is `files.reveal_failed`.
#[specta::specta]
#[tauri::command]
pub async fn file_reveal(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    open_or_reveal(app, state, id, true).await
}

/// Shared body of [`file_open`] and [`file_reveal`].
///
/// Both resolve the same path and differ only in which opener call they make and which failure
/// code they report, so a path-resolution bug cannot exist in one and not the other.
async fn open_or_reveal(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    reveal: bool,
) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    let file = blocking(db, move |db| repo::file::get(db, &id)).await?;

    let Some(path) = file.abs_path_in(&state.data_dir) else {
        return Err(os_failure(
            "files.missing",
            format!("`{}` records no path to open", file.original_name),
        ));
    };
    if !path.is_file() {
        return Err(os_failure(
            "files.missing",
            format!(
                "`{}` is no longer on disk at {}",
                file.original_name,
                path.display()
            ),
        ));
    }

    let verb = if reveal { "reveal" } else { "open" };
    let code = if reveal {
        "files.reveal_failed"
    } else {
        "files.open_failed"
    };
    let result = if reveal {
        app.opener().reveal_item_in_dir(&path)
    } else {
        // `open_path` takes `impl Into<String>`, not `impl AsRef<Path>` — it hands the value
        // straight to a `xdg-open`-style command, so a `PathBuf` is not accepted here. A
        // non-UTF-8 filename is rendered lossily rather than rejected: the alternative is
        // telling the user a file cannot be opened because of its own name.
        app.opener()
            .open_path(path.display().to_string(), None::<&str>)
    };
    result.map_err(|e| {
        os_failure(
            code,
            format!("could not {verb} `{}`: {e}", file.original_name),
        )
    })
}
