//! Goodreads parsing and transactional row import.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use maktabati_domain::{
    text::normalize, AuthorInput, AuthorRole, BookFormat, BookInput, BookSource, BookStatus,
    DomainError, Isbn, PublisherInput, Rating, ReviewInput, TagInput,
};
use maktabati_metadata::goodreads::{binding_to_format, parse_goodreads_csv, GoodreadsRow};
use maktabati_storage::{repo, Db, StorageError};

use super::blocking;
use crate::{
    error::{AppError, AppErrorDto, CmdResult},
    state::AppState,
};

/// One row that could not be imported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct GoodreadsImportFailure {
    /// Book title from the source row.
    pub title: String,
    /// Localizable error details.
    pub error: AppErrorDto,
}

/// Summary of a Goodreads import run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct GoodreadsImportReport {
    /// Rows imported successfully.
    pub imported: usize,
    /// Rows skipped because their ISBN already exists.
    pub skipped: usize,
    /// Rows that failed validation or persistence.
    pub failures: Vec<GoodreadsImportFailure>,
}

#[specta::specta]
#[tauri::command]
pub async fn goodreads_parse(text: String) -> CmdResult<Vec<GoodreadsRow>> {
    parse_goodreads_csv(&text).map_err(|e| AppErrorDto::new("import", e.to_string()))
}

/// Import Goodreads rows in Rust, preserving review text and reporting every failed row.
#[specta::specta]
#[tauri::command]
pub async fn goodreads_import_commit(
    state: State<'_, AppState>,
    rows: Vec<GoodreadsRow>,
) -> CmdResult<GoodreadsImportReport> {
    let db = Arc::clone(&state.db);
    blocking(db, move |db| Ok(commit_rows(db, rows))).await
}

fn commit_rows(db: &Db, rows: Vec<GoodreadsRow>) -> GoodreadsImportReport {
    let mut report = GoodreadsImportReport::default();
    for row in rows {
        let title = if row.title.trim().is_empty() {
            format!("#{}", row.goodreads_id)
        } else {
            row.title.clone()
        };
        match import_row(db, &row) {
            Ok(true) => report.imported += 1,
            Ok(false) => report.skipped += 1,
            Err(error) => report
                .failures
                .push(GoodreadsImportFailure { title, error }),
        }
    }
    report
}

/// Returns `false` only when an ISBN match already exists.
fn import_row(db: &Db, row: &GoodreadsRow) -> Result<bool, AppErrorDto> {
    let isbn10 = canonical_isbn(row.isbn10.as_deref(), false)?;
    let isbn13 = canonical_isbn(row.isbn13.as_deref(), true)?;
    for isbn in [isbn13.as_deref(), isbn10.as_deref()].into_iter().flatten() {
        if !repo::book::find_by_isbn(db, isbn)
            .map_err(storage_dto)?
            .is_empty()
        {
            return Ok(false);
        }
    }

    let author_id = if row.author.trim().is_empty() {
        None
    } else {
        Some(author_id_for(db, &row.author).map_err(storage_dto)?)
    };
    let publisher_id = row
        .publisher
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| publisher_id_for(db, name))
        .transpose()
        .map_err(storage_dto)?;
    let mut tag_ids = Vec::new();
    for name in &row.tags {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let id = tag_id_for(db, name).map_err(storage_dto)?;
        if !tag_ids.contains(&id) {
            tag_ids.push(id);
        }
    }

    let rating = match row.rating {
        Some(stars) if stars > 5 => {
            return Err(AppError::from(DomainError::OutOfRange {
                field: "goodreads_rating".to_owned(),
                got: stars.to_string(),
                expected: "0..=5 stars".to_owned(),
            })
            .into_dto());
        }
        Some(0) | None => None,
        Some(stars) => Some(
            Rating::from_units(stars * 2)
                .map_err(|error| AppError::from(error).into_dto())?
                .units(),
        ),
    };
    let page_count = row.page_count.map(i32::try_from).transpose().map_err(|_| {
        AppError::from(DomainError::OutOfRange {
            field: "page_count".to_owned(),
            got: row.page_count.unwrap_or_default().to_string(),
            expected: "0..=2147483647".to_owned(),
        })
        .into_dto()
    })?;
    let book = repo::book::create(
        db,
        BookInput {
            title: row.title.clone(),
            isbn10,
            isbn13,
            publication_year: row.publication_year,
            page_count,
            format: binding_to_format(row.binding.as_deref()).and_then(BookFormat::from_db),
            source: Some(BookSource::Goodreads),
            reading_status: Some(match row.status.as_str() {
                "finished" | "read" => BookStatus::Finished,
                "reading" | "currently-reading" => BookStatus::Reading,
                _ => BookStatus::ToRead,
            }),
            rating,
            authors: author_id
                .map(|author_id| {
                    vec![maktabati_domain::AuthorCredit {
                        author_id,
                        role: AuthorRole::Author,
                    }]
                })
                .unwrap_or_default(),
            publisher_id,
            tags: tag_ids,
            ..Default::default()
        },
    )
    .map_err(storage_dto)?;

    let review = row
        .review
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty());
    if rating.is_some() || review.is_some() {
        if let Err(error) = repo::evaluation::save(
            db,
            &book.id,
            ReviewInput {
                rating,
                body: review.map(str::to_owned),
            },
            Vec::new(),
        ) {
            let mut dto = storage_dto(error);
            if let Err(rollback_error) = repo::book::delete(db, &book.id) {
                dto.message = format!(
                    "{}; failed to roll back the imported book: {rollback_error}",
                    dto.message
                );
            }
            return Err(dto);
        }
    }
    Ok(true)
}

fn canonical_isbn(raw: Option<&str>, isbn13: bool) -> Result<Option<String>, AppErrorDto> {
    let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let isbn = Isbn::parse(raw).map_err(|error| AppError::from(error).into_dto())?;
    if isbn.is_isbn13() != isbn13 {
        return Err(AppError::from(DomainError::InvalidIsbn {
            value: raw.to_owned(),
        })
        .into_dto());
    }
    Ok(Some(isbn.as_str().to_owned()))
}

fn author_id_for(db: &Db, name: &str) -> maktabati_storage::Result<String> {
    let normalized = normalize(name);
    if let Some(author) = repo::author::find_by_name_norm(db, &normalized)? {
        return Ok(author.id);
    }
    Ok(repo::author::create(
        db,
        AuthorInput {
            name: name.to_owned(),
            ..Default::default()
        },
    )?
    .id)
}

fn publisher_id_for(db: &Db, name: &str) -> maktabati_storage::Result<String> {
    let normalized = normalize(name);
    if let Some(publisher) = repo::publisher::find_by_name_norm(db, &normalized)? {
        return Ok(publisher.id);
    }
    Ok(repo::publisher::create(
        db,
        PublisherInput {
            name: name.to_owned(),
            ..Default::default()
        },
    )?
    .id)
}

fn tag_id_for(db: &Db, name: &str) -> maktabati_storage::Result<String> {
    let normalized = normalize(name);
    if let Some(tag) = repo::tag::find_by_name_norm(db, &normalized)? {
        return Ok(tag.id);
    }
    Ok(repo::tag::create(
        db,
        TagInput {
            name: name.to_owned(),
            ..Default::default()
        },
    )?
    .id)
}

fn storage_dto(error: StorageError) -> AppErrorDto {
    AppError::from(error).into_dto()
}

#[cfg(test)]
mod tests {
    use super::*;
    use maktabati_domain::ReadingStatus;

    fn seeded_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("goodreads.db")).unwrap();
        maktabati_storage::migrate::apply(&db, || Ok(())).unwrap();
        db.write(maktabati_storage::seed::apply).unwrap();
        db
    }

    fn source_row() -> GoodreadsRow {
        GoodreadsRow {
            goodreads_id: "42".to_owned(),
            title: "Imported title".to_owned(),
            author: "A. Writer".to_owned(),
            isbn10: None,
            isbn13: Some("9780306406157".to_owned()),
            rating: Some(4),
            status: "reading".to_owned(),
            page_count: Some(250),
            publication_year: Some(2020),
            date_read: None,
            publisher: Some("Example Press".to_owned()),
            binding: Some("Paperback".to_owned()),
            tags: vec!["history".to_owned(), "history".to_owned()],
            review: Some("  A useful book.  ".to_owned()),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn import_saves_reviews_relations_status_and_skips_duplicate_isbns() {
        let db = seeded_db();
        let report = commit_rows(&db, vec![source_row(), source_row()]);
        assert_eq!(report.imported, 1);
        assert_eq!(report.skipped, 1);
        assert!(report.failures.is_empty());

        let books = repo::book::find_by_isbn(&db, "9780306406157").unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].reading_status, ReadingStatus::Reading);
        assert_eq!(books[0].rating, Some(8));
        let evaluation = repo::evaluation::get(&db, &books[0].id).unwrap();
        assert_eq!(
            evaluation.review.unwrap().body.as_deref(),
            Some("A useful book.")
        );
        assert_eq!(repo::author::list(&db).unwrap().len(), 1);
        assert_eq!(repo::publisher::list(&db).unwrap().len(), 1);
        assert_eq!(repo::tag::list(&db).unwrap().len(), 1);
    }

    #[test]
    fn import_maps_parser_finished_status_to_finished() {
        let db = seeded_db();
        let mut row = source_row();
        row.status = "finished".to_owned();

        let report = commit_rows(&db, vec![row]);
        assert_eq!(report.imported, 1);
        let books = repo::book::find_by_isbn(&db, "9780306406157").unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].reading_status, ReadingStatus::Finished);
    }

    #[test]
    fn row_validation_failures_are_reported_without_aborting_other_rows() {
        let db = seeded_db();
        let mut invalid = source_row();
        invalid.goodreads_id = "bad".to_owned();
        invalid.title.clear();
        invalid.isbn13 = None;
        let report = commit_rows(&db, vec![invalid, source_row()]);
        assert_eq!(report.imported, 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].error.code, "validation.empty_field");
    }
}
