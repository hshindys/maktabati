//! Reading goals, sessions and lending commands.

use std::sync::Arc;

use tauri::State;

use maktabati_storage::repo;
use maktabati_storage::repo::tracking::{
    Loan, LoanInput, ReadingGoal, ReadingGoalInput, ReadingSession, ReadingSessionInput,
};

use super::blocking;
use crate::error::CmdResult;
use crate::state::AppState;

#[specta::specta]
#[tauri::command]
pub async fn reading_goal_list(state: State<'_, AppState>) -> CmdResult<Vec<ReadingGoal>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::tracking::list_goals).await
}

#[specta::specta]
#[tauri::command]
pub async fn reading_goal_save(
    state: State<'_, AppState>,
    input: ReadingGoalInput,
) -> CmdResult<ReadingGoal> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tracking::save_goal(db, input)).await
}

#[specta::specta]
#[tauri::command]
pub async fn reading_goal_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tracking::delete_goal(db, &id)).await
}

#[specta::specta]
#[tauri::command]
pub async fn reading_session_list(state: State<'_, AppState>) -> CmdResult<Vec<ReadingSession>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::tracking::list_sessions).await
}

#[specta::specta]
#[tauri::command]
pub async fn reading_session_create(
    state: State<'_, AppState>,
    input: ReadingSessionInput,
) -> CmdResult<ReadingSession> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tracking::create_session(db, input)).await
}

#[specta::specta]
#[tauri::command]
pub async fn loan_list(state: State<'_, AppState>) -> CmdResult<Vec<Loan>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::tracking::list_loans).await
}

#[specta::specta]
#[tauri::command]
pub async fn loan_create(state: State<'_, AppState>, input: LoanInput) -> CmdResult<Loan> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tracking::create_loan(db, input)).await
}

#[specta::specta]
#[tauri::command]
pub async fn loan_return(
    state: State<'_, AppState>,
    id: String,
    returned_at: String,
) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::tracking::return_loan(db, &id, &returned_at)
    })
    .await
}

#[specta::specta]
#[tauri::command]
pub async fn loan_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tracking::delete_loan(db, &id)).await
}
