//! Rating criteria, reviews and rubric evaluations.

use std::sync::Arc;

use tauri::State;

use maktabati_domain::{
    BookEvaluation, EvaluationAggregates, RatingCriterion, RatingCriterionInput, ReviewInput,
    RubricScoreInput,
};
use maktabati_storage::repo;

use super::blocking;
use crate::error::CmdResult;
use crate::state::AppState;

/// Read a book's review, active rubric axes and saved scores.
#[specta::specta]
#[tauri::command]
pub async fn evaluation_get(
    state: State<'_, AppState>,
    book_id: String,
) -> CmdResult<BookEvaluation> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::evaluation::get(db, &book_id)).await
}

/// Save an overall review and the rubric score changes as one transaction.
#[specta::specta]
#[tauri::command]
pub async fn evaluation_save(
    state: State<'_, AppState>,
    book_id: String,
    review: ReviewInput,
    scores: Vec<RubricScoreInput>,
) -> CmdResult<BookEvaluation> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::evaluation::save(db, &book_id, review, scores)
    })
    .await
}

/// List every rubric criterion, including inactive criteria used in Settings.
#[specta::specta]
#[tauri::command]
pub async fn rating_criteria_list(state: State<'_, AppState>) -> CmdResult<Vec<RatingCriterion>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::evaluation::list_criteria).await
}

/// Add a user-defined rubric criterion.
#[specta::specta]
#[tauri::command]
pub async fn rating_criterion_create(
    state: State<'_, AppState>,
    input: RatingCriterionInput,
) -> CmdResult<RatingCriterion> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::evaluation::create_criterion(db, input)).await
}

/// Edit a rubric criterion while preserving its identifier and saved scores.
#[specta::specta]
#[tauri::command]
pub async fn rating_criterion_update(
    state: State<'_, AppState>,
    id: String,
    input: RatingCriterionInput,
) -> CmdResult<RatingCriterion> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::evaluation::update_criterion(db, &id, input)
    })
    .await
}

/// Fetch threshold-qualified author and publisher rating aggregates.
#[specta::specta]
#[tauri::command]
pub async fn evaluation_aggregates(state: State<'_, AppState>) -> CmdResult<EvaluationAggregates> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::evaluation::aggregates).await
}
