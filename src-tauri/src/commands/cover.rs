//! Cover commands — pick/clear a book's cover image and resolve its display path.
//!
//! The library owns copied cover bytes (`covers/<book_id>.<ext>` under the data dir),
//! the same policy as attachments in "copy" mode: a cover cannot break because the
//! file it was picked from is deleted. `cover_url` resolves a row-relative path to an
//! absolute one so the webview can load it through Tauri's asset protocol (`convertFileSrc`),
//! which the CSP already allows.

use std::sync::Arc;

use tauri::State;

use maktabati_domain::Book;
use maktabati_storage::repo;

use super::blocking;
use crate::error::CmdResult;
use crate::state::AppState;

/// Copy a picked image into the library and set it as the book's cover.
///
/// `source_path` must be absolute (the file dialog returns one) and must end in one of
/// `jpg`, `jpeg`, `png`, `webp` — validated in the repo, not trusted from the webview.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the book is gone, the extension is not
/// an image one, or a copy/update failed.
#[specta::specta]
#[tauri::command]
pub async fn cover_set(
    state: State<'_, AppState>,
    book_id: String,
    source_path: String,
) -> CmdResult<Book> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| {
        repo::book::set_cover(db, &data_dir, &book_id, &source_path)
    })
    .await
}

/// Clear the book's cover and delete its copied bytes.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the book is gone or the update failed.
#[specta::specta]
#[tauri::command]
pub async fn cover_remove(state: State<'_, AppState>, book_id: String) -> CmdResult<Book> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| {
        repo::book::remove_cover(db, &data_dir, &book_id)
    })
    .await
}

/// Absolute filesystem path of a book's cover, for `convertFileSrc` in the webview.
///
/// `Ok(None)` when the book has no cover **or** the recorded file is gone from disk — the
/// honest miss renders the generated placeholder instead of a broken image.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn cover_url(state: State<'_, AppState>, book_id: String) -> CmdResult<Option<String>> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| -> maktabati_storage::Result<Option<String>> {
        let book = repo::book::get(db, &book_id)?;
        let Some(rel) = book.cover_path else {
            return Ok(None);
        };
        let path = data_dir.join(&rel);
        if !path.is_file() {
            // A vanished file is the same as no cover — and letting the caller know it was
            // suspect would need a log, so that lands here on purpose.
            tracing::warn!(book_id, rel, "cover recorded but missing on disk");
            return Ok(None);
        }
        Ok(Some(path.display().to_string()))
    })
    .await
}
