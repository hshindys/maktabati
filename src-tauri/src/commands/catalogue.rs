//! Catalogue CRUD — books plus the six entities the add-book wizard creates inline.
//!
//! One command per [`repo`] function, named `<entity>_<verb>`, with exactly one omission:
//! `repo::*::find_by_name_norm` stays Rust-side. A form reaches the same outcome through
//! `*_search` for autocomplete and through the `validation.duplicate` error that `create` and
//! `update` already return, so a second IPC path would only duplicate the normalisation rules.
//!
//! Every command is the same thin async shell: clone the `Arc<Db>`, run the repo call on the
//! blocking pool (ADR-0010), map the error to [`AppErrorDto`] at the boundary (ADR-0010).
//! No validation, normalisation or SQL belongs in this layer — that is what the `domain` and
//! `storage` crates are for.

use std::sync::Arc;

use tauri::State;

use maktabati_domain::{
    Author, AuthorInput, Book, BookInput, Category, CategoryInput, Language, LanguageInput,
    Publisher, PublisherInput, Series, SeriesInput, Tag, TagInput,
};
use maktabati_storage::repo;

use super::blocking;
use crate::error::CmdResult;
use crate::state::AppState;

/// How many rows `*_search` returns when the caller does not say.
const SEARCH_LIMIT: i64 = 20;

// ─────────────────────────────────────────────────────────────────────────────
// Authors
// ─────────────────────────────────────────────────────────────────────────────

/// Create an author from the wizard's inline form.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails or that display name already exists.
#[specta::specta]
#[tauri::command]
pub async fn author_create(state: State<'_, AppState>, input: AuthorInput) -> CmdResult<Author> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::author::create(db, input)).await
}

/// Fetch one author by id.
///
/// # Errors
///
/// [`AppErrorDto`] when no author with that id exists.
#[specta::specta]
#[tauri::command]
pub async fn author_get(state: State<'_, AppState>, id: String) -> CmdResult<Author> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::author::get(db, &id)).await
}

/// List every author.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn author_list(state: State<'_, AppState>) -> CmdResult<Vec<Author>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::author::list).await
}

/// Update an author's editable fields.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails, the author does not exist, or the new name collides.
#[specta::specta]
#[tauri::command]
pub async fn author_update(
    state: State<'_, AppState>,
    id: String,
    input: AuthorInput,
) -> CmdResult<Author> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::author::update(db, &id, input)).await
}

/// Delete an author and the book credits that name them; the books themselves remain.
///
/// # Errors
///
/// [`AppErrorDto`] when the author does not exist or the delete fails.
#[specta::specta]
#[tauri::command]
pub async fn author_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::author::delete(db, &id)).await
}

/// Merge one author into another and delete the source.
///
/// Every credit the source carries moves to the target; a `(book, role)` credit the
/// target already has is dropped as the duplicate. The books stay, so a click that
/// reached the wrong pair can be recovered by re-editing the books — never by data loss.
///
/// # Errors
///
/// [`AppErrorDto`] when either author is missing, source equals target, or the write failed.
#[specta::specta]
#[tauri::command]
pub async fn author_merge(
    state: State<'_, AppState>,
    source_id: String,
    target_id: String,
) -> CmdResult<Author> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::author::merge(db, &source_id, &target_id)
    })
    .await
}

/// Substring search over the normalised name, for autocomplete.
///
/// An empty query or a non-positive limit returns nothing.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn author_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> CmdResult<Vec<Author>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::author::search(db, &query, limit.unwrap_or(SEARCH_LIMIT))
    })
    .await
}

// ─────────────────────────────────────────────────────────────────────────────
// Publishers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a publisher from the wizard's inline form.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails or that display name already exists.
#[specta::specta]
#[tauri::command]
pub async fn publisher_create(
    state: State<'_, AppState>,
    input: PublisherInput,
) -> CmdResult<Publisher> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::publisher::create(db, input)).await
}

/// Fetch one publisher by id.
///
/// # Errors
///
/// [`AppErrorDto`] when no publisher with that id exists.
#[specta::specta]
#[tauri::command]
pub async fn publisher_get(state: State<'_, AppState>, id: String) -> CmdResult<Publisher> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::publisher::get(db, &id)).await
}

/// List every publisher.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn publisher_list(state: State<'_, AppState>) -> CmdResult<Vec<Publisher>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::publisher::list).await
}

/// Update a publisher's editable fields.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails, the publisher does not exist, or the name collides.
#[specta::specta]
#[tauri::command]
pub async fn publisher_update(
    state: State<'_, AppState>,
    id: String,
    input: PublisherInput,
) -> CmdResult<Publisher> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::publisher::update(db, &id, input)).await
}

/// Delete a publisher; books that credited it keep their other fields (`ON DELETE SET NULL`).
///
/// # Errors
///
/// [`AppErrorDto`] when the publisher does not exist or the delete fails.
#[specta::specta]
#[tauri::command]
pub async fn publisher_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::publisher::delete(db, &id)).await
}

/// Substring search over the normalised name, for autocomplete.
///
/// An empty query or a non-positive limit returns nothing.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn publisher_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> CmdResult<Vec<Publisher>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::publisher::search(db, &query, limit.unwrap_or(SEARCH_LIMIT))
    })
    .await
}

// ─────────────────────────────────────────────────────────────────────────────
// Series
// ─────────────────────────────────────────────────────────────────────────────

/// Create a series from the wizard's inline form.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails or that display name already exists.
#[specta::specta]
#[tauri::command]
pub async fn series_create(state: State<'_, AppState>, input: SeriesInput) -> CmdResult<Series> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::series::create(db, input)).await
}

/// Fetch one series by id.
///
/// # Errors
///
/// [`AppErrorDto`] when no series with that id exists.
#[specta::specta]
#[tauri::command]
pub async fn series_get(state: State<'_, AppState>, id: String) -> CmdResult<Series> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::series::get(db, &id)).await
}

/// List every series.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn series_list(state: State<'_, AppState>) -> CmdResult<Vec<Series>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::series::list).await
}

/// Update a series' editable fields.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails, the series does not exist, or the name collides.
#[specta::specta]
#[tauri::command]
pub async fn series_update(
    state: State<'_, AppState>,
    id: String,
    input: SeriesInput,
) -> CmdResult<Series> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::series::update(db, &id, input)).await
}

/// Delete a series; books that belonged to it keep their other fields (`ON DELETE SET NULL`).
///
/// # Errors
///
/// [`AppErrorDto`] when the series does not exist or the delete fails.
#[specta::specta]
#[tauri::command]
pub async fn series_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::series::delete(db, &id)).await
}

/// Substring search over the normalised name, for autocomplete.
///
/// An empty query or a non-positive limit returns nothing.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn series_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> CmdResult<Vec<Series>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::series::search(db, &query, limit.unwrap_or(SEARCH_LIMIT))
    })
    .await
}

// ─────────────────────────────────────────────────────────────────────────────
// Tags
// ─────────────────────────────────────────────────────────────────────────────

/// Create a tag from the wizard's inline form.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails or that display name already exists.
#[specta::specta]
#[tauri::command]
pub async fn tag_create(state: State<'_, AppState>, input: TagInput) -> CmdResult<Tag> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tag::create(db, input)).await
}

/// Fetch one tag by id.
///
/// # Errors
///
/// [`AppErrorDto`] when no tag with that id exists.
#[specta::specta]
#[tauri::command]
pub async fn tag_get(state: State<'_, AppState>, id: String) -> CmdResult<Tag> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tag::get(db, &id)).await
}

/// List every tag.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn tag_list(state: State<'_, AppState>) -> CmdResult<Vec<Tag>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::tag::list).await
}

/// Update a tag's editable fields.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails, the tag does not exist, or the name collides.
#[specta::specta]
#[tauri::command]
pub async fn tag_update(state: State<'_, AppState>, id: String, input: TagInput) -> CmdResult<Tag> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tag::update(db, &id, input)).await
}

/// Delete a tag; the book-tag links are removed with it.
///
/// # Errors
///
/// [`AppErrorDto`] when the tag does not exist or the delete fails.
#[specta::specta]
#[tauri::command]
pub async fn tag_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::tag::delete(db, &id)).await
}

/// Substring search over the normalised name, for autocomplete.
///
/// An empty query or a non-positive limit returns nothing.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn tag_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> CmdResult<Vec<Tag>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::tag::search(db, &query, limit.unwrap_or(SEARCH_LIMIT))
    })
    .await
}

// ─────────────────────────────────────────────────────────────────────────────
// Languages
// ─────────────────────────────────────────────────────────────────────────────

/// Create a language. Its `code` is the primary key and never changes afterwards.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails or that code already exists.
#[specta::specta]
#[tauri::command]
pub async fn language_create(
    state: State<'_, AppState>,
    input: LanguageInput,
) -> CmdResult<Language> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::language::create(db, input)).await
}

/// Fetch one language by its code.
///
/// # Errors
///
/// [`AppErrorDto`] when no language with that code exists.
#[specta::specta]
#[tauri::command]
pub async fn language_get(state: State<'_, AppState>, code: String) -> CmdResult<Language> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::language::get(db, &code)).await
}

/// List every language, active or not.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn language_list(state: State<'_, AppState>) -> CmdResult<Vec<Language>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::language::list).await
}

/// List only the active languages — the wizard's language picker.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn language_list_active(state: State<'_, AppState>) -> CmdResult<Vec<Language>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::language::list_active).await
}

/// Update a language's editable fields (the `code` itself never changes).
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails or the language does not exist.
#[specta::specta]
#[tauri::command]
pub async fn language_update(
    state: State<'_, AppState>,
    code: String,
    input: LanguageInput,
) -> CmdResult<Language> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::language::update(db, &code, input)).await
}

/// Delete a language. Books have no `ON DELETE` rule for it, so one still in use blocks this.
///
/// # Errors
///
/// [`AppErrorDto`] when the language does not exist or any book still uses it
/// (`storage.foreign_key_violation`).
#[specta::specta]
#[tauri::command]
pub async fn language_delete(state: State<'_, AppState>, code: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::language::delete(db, &code)).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Categories (tree)
// ─────────────────────────────────────────────────────────────────────────────

/// Create a category under `parent_id`, or at the root when it is `null`.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails, the parent does not exist, or the key already exists.
#[specta::specta]
#[tauri::command]
pub async fn category_create(
    state: State<'_, AppState>,
    input: CategoryInput,
    parent_id: Option<String>,
) -> CmdResult<Category> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::category::create(db, input, parent_id.as_deref())
    })
    .await
}

/// Fetch one category by id.
///
/// # Errors
///
/// [`AppErrorDto`] when no category with that id exists.
#[specta::specta]
#[tauri::command]
pub async fn category_get(state: State<'_, AppState>, id: String) -> CmdResult<Category> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::category::get(db, &id)).await
}

/// List the whole tree in path order (siblings ordered by their stable key).
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn category_list(state: State<'_, AppState>) -> CmdResult<Vec<Category>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::category::list).await
}

/// List the direct children of a category (`null` parent = the roots).
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn category_children(
    state: State<'_, AppState>,
    parent_id: Option<String>,
) -> CmdResult<Vec<Category>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::category::children(db, parent_id.as_deref())
    })
    .await
}

/// Update a category's display name and colour.
///
/// The key, parent, position and path never change: descendant paths and saved filters are
/// built on them.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails or the category does not exist.
#[specta::specta]
#[tauri::command]
pub async fn category_update(
    state: State<'_, AppState>,
    id: String,
    input: CategoryInput,
) -> CmdResult<Category> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::category::update(db, &id, input)).await
}

/// Delete a category and its whole subtree, plus their book links.
///
/// # Errors
///
/// [`AppErrorDto`] when the category does not exist or the delete fails.
#[specta::specta]
#[tauri::command]
pub async fn category_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::category::delete(db, &id)).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Books
// ─────────────────────────────────────────────────────────────────────────────

/// Create a book with its author credits, categories and tags.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails or a referenced entity does not exist.
#[specta::specta]
#[tauri::command]
pub async fn book_create(state: State<'_, AppState>, input: BookInput) -> CmdResult<Book> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::book::create(db, input)).await
}

/// Fetch one book's own row.
///
/// # Errors
///
/// [`AppErrorDto`] when no book with that id exists.
#[specta::specta]
#[tauri::command]
pub async fn book_get(state: State<'_, AppState>, id: String) -> CmdResult<Book> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::book::get(db, &id)).await
}

/// Fetch a book with the relations the per-book page renders, in one round trip.
///
/// # Errors
///
/// [`AppErrorDto`] when no book with that id exists.
#[specta::specta]
#[tauri::command]
pub async fn book_detail(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<repo::book::BookDetail> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::book::detail(db, &id)).await
}

/// List every book (ordering and filtering beyond this arrive with search, M4).
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn book_list(state: State<'_, AppState>) -> CmdResult<Vec<Book>> {
    let db = Arc::clone(&state.db);
    blocking(db, repo::book::list).await
}

/// Update a book and its relations from an edited form.
///
/// # Errors
///
/// [`AppErrorDto`] when validation fails, the book does not exist, or a referenced entity does not.
#[specta::specta]
#[tauri::command]
pub async fn book_update(
    state: State<'_, AppState>,
    id: String,
    input: BookInput,
) -> CmdResult<Book> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::book::update(db, &id, input)).await
}

/// Quick status change ("mark as read"), touching only `reading_status`.
///
/// The edit modal covers a status change too, but a one-tap control on the book page must
/// not have to round-trip a full `BookInput` — the same call an edit would make anyway with
/// everything else unchanged, minus the risk of clobbering what another modal has open.
///
/// # Errors
///
/// [`AppErrorDto`] when the book does not exist.
#[specta::specta]
#[tauri::command]
pub async fn book_status_set(
    state: State<'_, AppState>,
    id: String,
    status: maktabati_domain::ReadingStatus,
) -> CmdResult<Book> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::book::set_status(db, &id, status)).await
}

/// Delete a book with its cycles, links, notes, files and search row.
///
/// # Errors
///
/// [`AppErrorDto`] when the book does not exist or the delete fails.
#[specta::specta]
#[tauri::command]
pub async fn book_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::book::delete(db, &id)).await
}

/// Books whose canonical ISBN matches the given one (either ISBN form is accepted).
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails — an unparseable ISBN simply matches nothing.
#[specta::specta]
#[tauri::command]
pub async fn book_find_by_isbn(state: State<'_, AppState>, isbn: String) -> CmdResult<Vec<Book>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| repo::book::find_by_isbn(db, &isbn)).await
}

/// Books that look like the one being saved: same canonical ISBN, or same title and author.
///
/// This powers the save-time duplicate warning. It never blocks the save; pass `exclude_id`
/// while editing so the book does not match itself.
///
/// # Errors
///
/// [`AppErrorDto`] when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn book_find_duplicates(
    state: State<'_, AppState>,
    input: BookInput,
    exclude_id: Option<String>,
) -> CmdResult<Vec<Book>> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| {
        repo::book::find_duplicates(db, &input, exclude_id.as_deref())
    })
    .await
}
