//! Search command for the global palette.

use std::sync::Arc;

use maktabati_search::SearchResult;
use tauri::State;

use super::blocking;
use crate::error::CmdResult;
use crate::state::AppState;

/// Search books, notes, quotes and authors through the SQLite FTS5 index.
#[specta::specta]
#[tauri::command]
pub async fn search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> CmdResult<Vec<SearchResult>> {
    let db = Arc::clone(&state.db);
    let limit = limit.unwrap_or(8) as usize;
    blocking(db, move |db| maktabati_search::search(db, &query, limit)).await
}
