//! Note-page and note-block commands.
//!
//! One command per [`repo::note`] function. Thin async shells like every other command in
//! [`crate::commands`]: resolve the pool, hop to the blocking pool, map the error. The
//! surface keeps the repo's v1 simplifications visible — pages cannot re-anchor, blocks
//! order sequentially, and nothing writes `note_link` (no user links yet).

use std::sync::Arc;

use maktabati_domain::note::{NoteBlock, NoteBlockInput, NotePage, NotePageInput};
use tauri::State;

use crate::commands::{blocking, CmdResult};
use crate::state::AppState;
use maktabati_storage::repo;

/// List note pages, newest-edited first (pinned on top); a `bookId` narrows to a book.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn note_page_list(
    state: State<'_, AppState>,
    book_id: Option<String>,
) -> CmdResult<Vec<NotePage>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::note::list_pages(db, book_id.as_deref())).await
}

/// Fetch one note page by id.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when no page has that id.
#[specta::specta]
#[tauri::command]
pub async fn note_page_get(state: State<'_, AppState>, id: String) -> CmdResult<NotePage> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::note::get_page(db, &id)).await
}

/// Create a note page, optionally anchored to a book and/or a parent page.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when the input fails validation or the anchors are unknown.
#[specta::specta]
#[tauri::command]
pub async fn note_page_create(
    state: State<'_, AppState>,
    input: NotePageInput,
) -> CmdResult<NotePage> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::note::create_page(db, input)).await
}

/// Edit a note page's title, icon and pin; its anchors and template marks stay.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when the page is missing or the input fails validation.
#[specta::specta]
#[tauri::command]
pub async fn note_page_update(
    state: State<'_, AppState>,
    id: String,
    input: NotePageInput,
) -> CmdResult<NotePage> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::note::update_page(db, &id, input)).await
}

/// Delete a note page; nested pages and blocks cascade with it.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when no page has that id.
#[specta::specta]
#[tauri::command]
pub async fn note_page_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::note::delete_page(db, &id)).await
}

/// List a page's blocks in content order.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn note_block_list(
    state: State<'_, AppState>,
    page_id: String,
) -> CmdResult<Vec<NoteBlock>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::note::list_blocks(db, &page_id)).await
}

/// Insert a block after `afterBlockId` (`null` = first), shifting the rest down.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when the page or anchor is unknown, the anchor lives on
/// another page, or the payload fails validation.
#[specta::specta]
#[tauri::command]
pub async fn note_block_insert(
    state: State<'_, AppState>,
    page_id: String,
    after_block_id: Option<String>,
    input: NoteBlockInput,
) -> CmdResult<NoteBlock> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::note::insert_block(db, &page_id, after_block_id.as_deref(), input)
    })
    .await
}

/// Edit a block's kind or payload.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when the block is missing or the payload fails validation.
#[specta::specta]
#[tauri::command]
pub async fn note_block_update(
    state: State<'_, AppState>,
    block_id: String,
    input: NoteBlockInput,
) -> CmdResult<NoteBlock> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::note::update_block(db, &block_id, input)).await
}

/// Move a block to sit after `afterBlockId` (`null` = first).
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when the block or anchor is unknown, or the anchor lives
/// on another page.
#[specta::specta]
#[tauri::command]
pub async fn note_block_move(
    state: State<'_, AppState>,
    block_id: String,
    after_block_id: Option<String>,
) -> CmdResult<NoteBlock> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::note::move_block(db, &block_id, after_block_id.as_deref())
    })
    .await
}

/// Delete a block; the page keeps its remaining sequence.
///
/// # Errors
///
/// [`AppErrorDto`](crate::dto) when no block has that id.
#[specta::specta]
#[tauri::command]
pub async fn note_block_delete(state: State<'_, AppState>, block_id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::note::delete_block(db, &block_id)).await
}
