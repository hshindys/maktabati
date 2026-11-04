//! Reviews, configurable rubric criteria, per-book scores and entity aggregates.

use std::collections::BTreeMap;

use maktabati_domain::{
    ids::new_id,
    rules::{self, Aggregate, RubricScore},
    BookEvaluation, EntityRatingAggregate, EvaluationAggregates, Rating, RatingCriterion,
    RatingCriterionInput, Review, ReviewInput, RubricScoreInput, RubricScoreRecord,
};
use rusqlite::OptionalExtension;

use crate::{conn::Db, error::not_found, Result};

const CRITERION_COLUMNS: &str = "id, key, name_ar, name_en, description_ar, description_en, \
                                weight, position, is_active, is_system, created_at, updated_at";

fn criterion_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RatingCriterion> {
    Ok(RatingCriterion {
        id: row.get(0)?,
        key: row.get(1)?,
        name_ar: row.get(2)?,
        name_en: row.get(3)?,
        description_ar: row.get(4)?,
        description_en: row.get(5)?,
        weight: row.get(6)?,
        position: row.get(7)?,
        is_active: row.get::<_, i64>(8)? != 0,
        is_system: row.get::<_, i64>(9)? != 0,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn review_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Review> {
    Ok(Review {
        id: row.get(0)?,
        book_id: row.get(1)?,
        rating: row.get(2)?,
        body: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn score_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RubricScoreRecord> {
    Ok(RubricScoreRecord {
        book_id: row.get(0)?,
        criterion_id: row.get(1)?,
        score: row.get(2)?,
        note: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

/// List rubric criteria, including inactive criteria for the Settings editor.
pub fn list_criteria(db: &Db) -> Result<Vec<RatingCriterion>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {CRITERION_COLUMNS} FROM rating_criteria ORDER BY position, id"
        ))?;
        let rows = stmt.query_map([], criterion_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

/// Add a user-defined rubric criterion.
pub fn create_criterion(db: &Db, input: RatingCriterionInput) -> Result<RatingCriterion> {
    let input = input.validate()?;
    let id = new_id();
    let key = format!("custom_{id}");
    let now = crate::conn::now_ms();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO rating_criteria
             (id, key, name_ar, name_en, description_ar, description_en, weight, position,
              is_active, is_system, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?10)",
            rusqlite::params![
                id,
                key,
                input.name_ar,
                input.name_en,
                input.description_ar,
                input.description_en,
                input.weight,
                input.position,
                input.is_active,
                now,
            ],
        )?;
        conn.query_row(
            &format!("SELECT {CRITERION_COLUMNS} FROM rating_criteria WHERE id = ?1"),
            [&id],
            criterion_from_row,
        )
        .map_err(Into::into)
    })
}

/// Edit a system or custom criterion without changing its stable identity.
pub fn update_criterion(db: &Db, id: &str, input: RatingCriterionInput) -> Result<RatingCriterion> {
    let input = input.validate()?;
    let now = crate::conn::now_ms();
    db.write(|conn| {
        let changed = conn.execute(
            "UPDATE rating_criteria
             SET name_ar = ?2, name_en = ?3, description_ar = ?4, description_en = ?5,
                 weight = ?6, position = ?7, is_active = ?8, updated_at = ?9
             WHERE id = ?1",
            rusqlite::params![
                id,
                input.name_ar,
                input.name_en,
                input.description_ar,
                input.description_en,
                input.weight,
                input.position,
                input.is_active,
                now,
            ],
        )?;
        if changed == 0 {
            return Err(not_found("rating_criterion", id));
        }
        conn.query_row(
            &format!("SELECT {CRITERION_COLUMNS} FROM rating_criteria WHERE id = ?1"),
            [id],
            criterion_from_row,
        )
        .map_err(Into::into)
    })
}

/// Fetch a book's current review and active rubric scores.
pub fn get(db: &Db, book_id: &str) -> Result<BookEvaluation> {
    db.read(|conn| get_conn(conn, book_id))
}

fn get_conn(conn: &rusqlite::Connection, book_id: &str) -> Result<BookEvaluation> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM books WHERE id = ?1)",
        [book_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(not_found("book", book_id));
    }

    let review = conn
        .query_row(
            "SELECT id, book_id, rating, body, created_at, updated_at
             FROM reviews WHERE book_id = ?1 ORDER BY created_at DESC, id DESC LIMIT 1",
            [book_id],
            review_from_row,
        )
        .optional()?;
    let mut criteria_stmt = conn.prepare(&format!(
        "SELECT {CRITERION_COLUMNS} FROM rating_criteria
         WHERE is_active = 1 ORDER BY position, id"
    ))?;
    let criteria = criteria_stmt
        .query_map([], criterion_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut scores_stmt = conn.prepare(
        "SELECT rs.book_id, rs.criterion_id, rs.score, rs.note, rs.updated_at
         FROM rubric_scores rs
         JOIN rating_criteria rc ON rc.id = rs.criterion_id
         WHERE rs.book_id = ?1 AND rc.is_active = 1
         ORDER BY rc.position, rc.id",
    )?;
    let scores = scores_stmt
        .query_map([book_id], score_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let weights: BTreeMap<&str, f64> = criteria
        .iter()
        .map(|criterion| (criterion.id.as_str(), criterion.weight))
        .collect();
    let rubric_scores = scores
        .iter()
        .map(|score| RubricScore {
            criterion_id: score.criterion_id.clone(),
            score: score.score,
            weight: weights
                .get(score.criterion_id.as_str())
                .copied()
                .unwrap_or_default(),
        })
        .collect();
    let aggregate = rules::aggregate(&[(book_id.to_owned(), rubric_scores)]);

    Ok(BookEvaluation {
        review,
        criteria,
        scores,
        aggregate,
    })
}

/// Save a review and the submitted rubric scores atomically.
pub fn save(
    db: &Db,
    book_id: &str,
    review: ReviewInput,
    scores: Vec<RubricScoreInput>,
) -> Result<BookEvaluation> {
    let review = review.validate()?;
    for score in &scores {
        if let Some(value) = score.score {
            Rating::from_units(value)?;
        }
        if let Some(note) = &score.note {
            if note.chars().count() > 5_000 {
                return Err(maktabati_domain::DomainError::TooLong {
                    field: "rubric_note".to_owned(),
                    got: note.chars().count(),
                    max: 5_000,
                }
                .into());
            }
        }
    }

    db.transact(|conn| {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM books WHERE id = ?1)",
            [book_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(not_found("book", book_id));
        }
        let now = crate::conn::now_ms();
        let existing_review: Option<String> = conn
            .query_row(
                "SELECT id FROM reviews WHERE book_id = ?1
                 ORDER BY created_at DESC, id DESC LIMIT 1",
                [book_id],
                |row| row.get(0),
            )
            .optional()?;
        match (&existing_review, review.rating, &review.body) {
            (Some(id), None, None) => {
                conn.execute("DELETE FROM reviews WHERE id = ?1", [id])?;
            }
            (Some(id), rating, body) => {
                conn.execute(
                    "UPDATE reviews SET rating = ?2, body = ?3, updated_at = ?4 WHERE id = ?1",
                    rusqlite::params![id, rating, body, now],
                )?;
            }
            (None, Some(rating), body) => {
                conn.execute(
                    "INSERT INTO reviews (id, book_id, rating, body, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    rusqlite::params![new_id(), book_id, rating, body, now],
                )?;
            }
            (None, None, Some(body)) => {
                conn.execute(
                    "INSERT INTO reviews (id, book_id, rating, body, created_at, updated_at)
                     VALUES (?1, ?2, NULL, ?3, ?4, ?4)",
                    rusqlite::params![new_id(), book_id, body, now],
                )?;
            }
            (None, None, None) => {}
        }
        conn.execute(
            "UPDATE books SET rating = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![book_id, review.rating, now],
        )?;

        for score in &scores {
            if score.criterion_id.trim().is_empty() {
                return Err(maktabati_domain::DomainError::EmptyField {
                    field: "criterion_id".to_owned(),
                }
                .into());
            }
            if let Some(value) = score.score {
                let active: bool = conn
                    .query_row(
                        "SELECT is_active FROM rating_criteria WHERE id = ?1",
                        [&score.criterion_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or(false);
                if !active {
                    return Err(not_found("active_rating_criterion", &score.criterion_id));
                }
                let note = score.note.as_deref().map(str::trim).filter(|s| !s.is_empty());
                conn.execute(
                    "INSERT INTO rubric_scores (book_id, criterion_id, score, note, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(book_id, criterion_id) DO UPDATE SET
                       score = excluded.score, note = excluded.note, updated_at = excluded.updated_at",
                    rusqlite::params![book_id, score.criterion_id, value, note, now],
                )?;
            } else {
                conn.execute(
                    "DELETE FROM rubric_scores WHERE book_id = ?1 AND criterion_id = ?2",
                    rusqlite::params![book_id, score.criterion_id],
                )?;
            }
        }
        get_conn(conn, book_id)
    })
}

/// List weighted rubric aggregates for every author and publisher with scored books.
pub fn aggregates(db: &Db) -> Result<EvaluationAggregates> {
    db.read(|conn| {
        Ok(EvaluationAggregates {
            authors: entity_aggregates(conn, true)?,
            publishers: entity_aggregates(conn, false)?,
        })
    })
}

fn entity_aggregates(
    conn: &rusqlite::Connection,
    authors: bool,
) -> Result<Vec<EntityRatingAggregate>> {
    let sql = if authors {
        "SELECT DISTINCT ba.author_id, rs.book_id, rs.criterion_id, rs.score, rc.weight
         FROM rubric_scores rs
         JOIN rating_criteria rc ON rc.id = rs.criterion_id AND rc.is_active = 1
         JOIN book_authors ba ON ba.book_id = rs.book_id
         ORDER BY ba.author_id, rs.book_id, rs.criterion_id"
    } else {
        "SELECT b.publisher_id, rs.book_id, rs.criterion_id, rs.score, rc.weight
         FROM rubric_scores rs
         JOIN rating_criteria rc ON rc.id = rs.criterion_id AND rc.is_active = 1
         JOIN books b ON b.id = rs.book_id
         WHERE b.publisher_id IS NOT NULL
         ORDER BY b.publisher_id, rs.book_id, rs.criterion_id"
    };
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    let mut grouped: BTreeMap<String, BTreeMap<String, Vec<RubricScore>>> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let entity_id: String = row.get(0)?;
        let book_id: String = row.get(1)?;
        let score = RubricScore {
            criterion_id: row.get(2)?,
            score: row.get(3)?,
            weight: row.get(4)?,
        };
        grouped
            .entry(entity_id)
            .or_default()
            .entry(book_id)
            .or_default()
            .push(score);
    }
    Ok(grouped
        .into_iter()
        .map(|(entity_id, books)| {
            let books = books.into_iter().collect::<Vec<_>>();
            let aggregate: Aggregate = rules::aggregate(&books);
            EntityRatingAggregate {
                entity_id,
                rating: rules::aggregate_rating(&aggregate),
                aggregate,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use maktabati_domain::{AuthorCredit, AuthorInput, AuthorRole, BookInput, PublisherInput};
    use pretty_assertions::assert_eq;

    fn seeded_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("evaluation.db")).unwrap();
        crate::migrate::apply(&db, || Ok(())).unwrap();
        db.write(crate::seed::apply).unwrap();
        db
    }

    fn criterion_input(name: &str) -> RatingCriterionInput {
        RatingCriterionInput {
            name_ar: format!("معيار {name}"),
            name_en: name.to_owned(),
            description_ar: None,
            description_en: None,
            weight: 1.0,
            position: 99,
            is_active: true,
        }
    }

    #[test]
    fn criteria_can_be_added_and_edited_without_changing_identity() {
        let db = seeded_db();
        let created = create_criterion(&db, criterion_input("Story")).unwrap();
        let updated = update_criterion(
            &db,
            &created.id,
            RatingCriterionInput {
                name_en: "Narrative".to_owned(),
                is_active: false,
                ..criterion_input("Story")
            },
        )
        .unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.key, created.key);
        assert_eq!(updated.name_en, "Narrative");
        assert!(!updated.is_active);
    }

    #[test]
    fn review_and_rubric_scores_round_trip_and_can_be_cleared() {
        let db = seeded_db();
        let book = crate::repo::book::create(
            &db,
            BookInput {
                title: "Test book".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let criterion = list_criteria(&db).unwrap().remove(0);
        let saved = save(
            &db,
            &book.id,
            ReviewInput {
                rating: Some(9),
                body: Some("A careful review".to_owned()),
            },
            vec![RubricScoreInput {
                criterion_id: criterion.id.clone(),
                score: Some(8),
                note: Some("Thoughtful".to_owned()),
            }],
        )
        .unwrap();
        assert_eq!(
            saved.review.as_ref().unwrap().body.as_deref(),
            Some("A careful review")
        );
        assert_eq!(saved.scores[0].score, 8);
        assert_eq!(saved.aggregate.overall, Some(8.0));

        let cleared = save(
            &db,
            &book.id,
            ReviewInput::default(),
            vec![RubricScoreInput {
                criterion_id: criterion.id,
                score: None,
                note: None,
            }],
        )
        .unwrap();
        assert!(cleared.review.is_none());
        assert!(cleared.scores.is_empty());
    }

    #[test]
    fn aggregates_apply_the_three_scored_books_threshold_for_authors_and_publishers() {
        let db = seeded_db();
        let author = crate::repo::author::create(
            &db,
            AuthorInput {
                name: "Shared author".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let publisher = crate::repo::publisher::create(
            &db,
            PublisherInput {
                name: "Shared press".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let criterion = list_criteria(&db).unwrap().remove(0);
        for index in 0..3 {
            let book = crate::repo::book::create(
                &db,
                BookInput {
                    title: format!("Evaluation book {index}"),
                    authors: vec![AuthorCredit {
                        author_id: author.id.clone(),
                        role: AuthorRole::Author,
                    }],
                    publisher_id: Some(publisher.id.clone()),
                    ..Default::default()
                },
            )
            .unwrap();
            save(
                &db,
                &book.id,
                ReviewInput::default(),
                vec![RubricScoreInput {
                    criterion_id: criterion.id.clone(),
                    score: Some(8),
                    note: None,
                }],
            )
            .unwrap();
        }
        let values = aggregates(&db).unwrap();
        assert_eq!(values.authors[0].aggregate.book_count, 3);
        assert_eq!(values.authors[0].rating.unwrap().units(), 8);
        assert_eq!(values.publishers[0].aggregate.book_count, 3);
        assert_eq!(values.publishers[0].rating.unwrap().units(), 8);
    }

    #[test]
    fn aggregate_rating_is_hidden_below_three_books() {
        let db = seeded_db();
        let book = crate::repo::book::create(
            &db,
            BookInput {
                title: "Single score".to_owned(),
                ..Default::default()
            },
        )
        .unwrap();
        let criterion = list_criteria(&db).unwrap().remove(0);
        save(
            &db,
            &book.id,
            ReviewInput::default(),
            vec![RubricScoreInput {
                criterion_id: criterion.id,
                score: Some(8),
                note: None,
            }],
        )
        .unwrap();
        assert!(aggregates(&db).unwrap().authors.is_empty());
    }
}
