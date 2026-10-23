//! Quotes — the IPC surface for passages lifted from books (الاقتباسات).
//!
//! One command per [`repo::quote`] function. Thin async shells like every other command in
//! [`crate::commands`]: clone the `Arc<Db>`, run the repo call on the blocking pool (ADR-0010),
//! map the error at the boundary.

use std::sync::Arc;

use tauri::State;

use maktabati_domain::{Quote, QuoteInput};
use maktabati_storage::repo;

use super::blocking;
use crate::error::CmdResult;
use crate::state::AppState;

/// Create a quote on a book.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the book does not exist, the input fails
/// validation, or the write fails.
#[specta::specta]
#[tauri::command]
pub async fn quote_create(
    state: State<'_, AppState>,
    book_id: String,
    input: QuoteInput,
) -> CmdResult<Quote> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::quote::create(db, &book_id, input)).await
}

/// List a book's quotes, newest first.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn quote_list(state: State<'_, AppState>, book_id: String) -> CmdResult<Vec<Quote>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::quote::list(db, &book_id)).await
}

/// List every quote in the library with the owning book's title and first author.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn quote_list_all(
    state: State<'_, AppState>,
) -> CmdResult<Vec<repo::quote::QuoteWithBook>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::quote::list_all).await
}

/// Edit a quote.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the id is unknown, validation fails, or the
/// write fails.
#[specta::specta]
#[tauri::command]
pub async fn quote_update(
    state: State<'_, AppState>,
    id: String,
    input: QuoteInput,
) -> CmdResult<Quote> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::quote::update(db, &id, input)).await
}

/// Delete a quote.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the id is unknown or the write fails.
#[specta::specta]
#[tauri::command]
pub async fn quote_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::quote::delete(db, &id)).await
}
