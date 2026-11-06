//! Reading goals, logged sessions and loans.

use chrono::NaiveDate;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::conn::Db;
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ReadingGoal {
    pub id: String,
    pub scope: String,
    pub year: i32,
    pub month: Option<i32>,
    pub target_books: Option<i32>,
    pub target_pages: Option<i32>,
    pub books_completed: i64,
    pub pages_read: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ReadingGoalInput {
    pub scope: String,
    pub year: i32,
    pub month: Option<i32>,
    pub target_books: Option<i32>,
    pub target_pages: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ReadingSession {
    pub id: String,
    pub book_id: String,
    pub book_title: String,
    pub session_date: String,
    pub pages_read: Option<i32>,
    pub minutes: Option<i32>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ReadingSessionInput {
    pub book_id: String,
    pub session_date: String,
    pub pages_read: Option<i32>,
    pub minutes: Option<i32>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Loan {
    pub id: String,
    pub book_id: String,
    pub book_title: String,
    pub borrower: String,
    pub phone: Option<String>,
    pub borrowed_at: String,
    pub due_date: Option<String>,
    pub returned_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct LoanInput {
    pub book_id: String,
    pub borrower: String,
    pub phone: Option<String>,
    pub borrowed_at: String,
    pub due_date: Option<String>,
    pub notes: Option<String>,
}

fn invalid(field: &str, value: impl ToString, expected: &str) -> crate::StorageError {
    maktabati_domain::DomainError::OutOfRange {
        field: field.to_owned(),
        got: value.to_string(),
        expected: expected.to_owned(),
    }
    .into()
}

fn check_date(field: &str, value: &str) -> Result<()> {
    if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
        return Err(invalid(field, value, "YYYY-MM-DD"));
    }
    Ok(())
}

/// Save or update a yearly/monthly reading target.
pub fn save_goal(db: &Db, input: ReadingGoalInput) -> Result<ReadingGoal> {
    if !matches!(input.scope.as_str(), "year" | "month") {
        return Err(maktabati_domain::DomainError::InvalidEnum {
            field: "scope".into(),
            got: input.scope,
        }
        .into());
    }
    if !(1..=9999).contains(&input.year) {
        return Err(invalid("year", input.year, "1..=9999"));
    }
    if input.scope == "month" {
        if !matches!(input.month, Some(1..=12)) {
            return Err(invalid("month", input.month.unwrap_or_default(), "1..=12"));
        }
    } else if input.month.is_some() {
        return Err(invalid(
            "month",
            input.month.unwrap_or_default(),
            "null for yearly goals",
        ));
    }
    if input.target_books.is_none() && input.target_pages.is_none() {
        return Err(maktabati_domain::DomainError::EmptyField {
            field: "target".into(),
        }
        .into());
    }
    for (field, value) in [
        ("target_books", input.target_books),
        ("target_pages", input.target_pages),
    ] {
        if let Some(value) = value {
            if value <= 0 {
                return Err(invalid(field, value, "> 0"));
            }
        }
    }

    db.transact(|conn| {
        let now = crate::conn::now_ms();
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM reading_goals
                 WHERE scope = ?1 AND year = ?2 AND month IS ?3
                 ORDER BY created_at LIMIT 1",
                rusqlite::params![input.scope, input.year, input.month],
                |row| row.get(0),
            )
            .optional()?;
        let id = existing.unwrap_or_else(maktabati_domain::ids::new_id);
        conn.execute(
            "INSERT INTO reading_goals
                 (id, scope, year, month, target_books, target_pages, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(id) DO UPDATE SET
                 target_books = excluded.target_books,
                 target_pages = excluded.target_pages,
                 updated_at = excluded.updated_at",
            rusqlite::params![
                id,
                input.scope,
                input.year,
                input.month,
                input.target_books,
                input.target_pages,
                now,
            ],
        )?;
        goal_for(conn, &id)
    })
}

fn goal_for(conn: &rusqlite::Connection, id: &str) -> Result<ReadingGoal> {
    Ok(conn.query_row(
        "SELECT id, scope, year, month, target_books, target_pages,
                (SELECT COUNT(*) FROM reading_cycles rc
                 WHERE rc.finished_at IS NOT NULL
                   AND CAST(strftime('%Y', rc.finished_at / 1000, 'unixepoch') AS INTEGER) = g.year
                   AND (g.scope = 'year' OR CAST(strftime('%m', rc.finished_at / 1000, 'unixepoch') AS INTEGER) = g.month)),
                (SELECT COALESCE(SUM(rs.pages_read), 0) FROM reading_sessions rs
                 WHERE substr(rs.session_date, 1, 4) = printf('%04d', g.year)
                   AND (g.scope = 'year' OR CAST(substr(rs.session_date, 6, 2) AS INTEGER) = g.month))
         FROM reading_goals g WHERE id = ?1",
        [id],
        |row| {
            Ok(ReadingGoal {
                id: row.get(0)?,
                scope: row.get(1)?,
                year: row.get(2)?,
                month: row.get(3)?,
                target_books: row.get(4)?,
                target_pages: row.get(5)?,
                books_completed: row.get(6)?,
                pages_read: row.get(7)?,
            })
        },
    )?)
}

/// List goals with the completed-book and logged-page totals for each period.
pub fn list_goals(db: &Db) -> Result<Vec<ReadingGoal>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id FROM reading_goals
             ORDER BY year DESC, CASE scope WHEN 'year' THEN 0 ELSE 1 END, month DESC",
        )?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter().map(|id| goal_for(conn, id)).collect()
    })
}

/// Remove a goal.
pub fn delete_goal(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        if conn.execute("DELETE FROM reading_goals WHERE id = ?1", [id])? == 0 {
            return Err(crate::error::not_found("reading goal", id));
        }
        Ok(())
    })
}

/// Record a reading session; its pages contribute to period page goals.
pub fn create_session(db: &Db, input: ReadingSessionInput) -> Result<ReadingSession> {
    check_date("session_date", &input.session_date)?;
    if input.pages_read.is_none() && input.minutes.is_none() {
        return Err(maktabati_domain::DomainError::EmptyField {
            field: "pages_read_or_minutes".into(),
        }
        .into());
    }
    for (field, value) in [("pages_read", input.pages_read), ("minutes", input.minutes)] {
        if let Some(value) = value {
            if value < 0 {
                return Err(invalid(field, value, ">= 0"));
            }
        }
    }
    let id = maktabati_domain::ids::new_id();
    db.transact(|conn| {
        let book_title = conn
            .query_row(
                "SELECT title FROM books WHERE id = ?1",
                [&input.book_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| crate::error::not_found("book", &input.book_id))?;
        conn.execute(
            "INSERT INTO reading_sessions
                 (id, book_id, session_date, pages_read, minutes, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                input.book_id,
                input.session_date,
                input.pages_read,
                input.minutes,
                input.note,
                crate::conn::now_ms(),
            ],
        )?;
        Ok(ReadingSession {
            id,
            book_id: input.book_id,
            book_title,
            session_date: input.session_date,
            pages_read: input.pages_read,
            minutes: input.minutes,
            note: input.note,
        })
    })
}

/// List the most recent reading sessions.
pub fn list_sessions(db: &Db) -> Result<Vec<ReadingSession>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT rs.id, rs.book_id, b.title, rs.session_date, rs.pages_read, rs.minutes, rs.note
             FROM reading_sessions rs JOIN books b ON b.id = rs.book_id
             ORDER BY rs.session_date DESC, rs.created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ReadingSession {
                id: row.get(0)?,
                book_id: row.get(1)?,
                book_title: row.get(2)?,
                session_date: row.get(3)?,
                pages_read: row.get(4)?,
                minutes: row.get(5)?,
                note: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Record a loan.
pub fn create_loan(db: &Db, input: LoanInput) -> Result<Loan> {
    let borrower = input.borrower.trim();
    if borrower.is_empty() {
        return Err(maktabati_domain::DomainError::EmptyField {
            field: "borrower".into(),
        }
        .into());
    }
    check_date("borrowed_at", &input.borrowed_at)?;
    if let Some(due_date) = input.due_date.as_deref() {
        check_date("due_date", due_date)?;
        if due_date < input.borrowed_at.as_str() {
            return Err(invalid("due_date", due_date, "on or after borrowed_at"));
        }
    }
    let id = maktabati_domain::ids::new_id();
    db.transact(|conn| {
        let book_title = conn
            .query_row(
                "SELECT title FROM books WHERE id = ?1",
                [&input.book_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| crate::error::not_found("book", &input.book_id))?;
        let already_lent: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM loans WHERE book_id = ?1 AND returned_at IS NULL)",
            [&input.book_id],
            |row| row.get(0),
        )?;
        if already_lent {
            return Err(maktabati_domain::DomainError::InvalidTransition {
                detail: "this book already has an active loan".into(),
            }
            .into());
        }
        let now = crate::conn::now_ms();
        conn.execute(
            "INSERT INTO loans
                 (id, book_id, borrower, phone, borrowed_at, due_date, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            rusqlite::params![
                id,
                input.book_id,
                borrower,
                input.phone,
                input.borrowed_at,
                input.due_date,
                input.notes,
                now
            ],
        )?;
        Ok(Loan {
            id,
            book_id: input.book_id,
            book_title,
            borrower: borrower.to_owned(),
            phone: input.phone,
            borrowed_at: input.borrowed_at,
            due_date: input.due_date,
            returned_at: None,
            notes: input.notes,
        })
    })
}

/// List active and returned loans, active loans first.
pub fn list_loans(db: &Db) -> Result<Vec<Loan>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT l.id, l.book_id, b.title, l.borrower, l.phone, l.borrowed_at,
                    l.due_date, l.returned_at, l.notes
             FROM loans l JOIN books b ON b.id = l.book_id
             ORDER BY (l.returned_at IS NOT NULL), l.due_date IS NULL, l.due_date, l.borrowed_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Loan {
                id: row.get(0)?,
                book_id: row.get(1)?,
                book_title: row.get(2)?,
                borrower: row.get(3)?,
                phone: row.get(4)?,
                borrowed_at: row.get(5)?,
                due_date: row.get(6)?,
                returned_at: row.get(7)?,
                notes: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Mark a loan returned on the supplied calendar date.
pub fn return_loan(db: &Db, id: &str, returned_at: &str) -> Result<()> {
    check_date("returned_at", returned_at)?;
    db.write(|conn| {
        let borrowed_at: Option<String> = conn
            .query_row("SELECT borrowed_at FROM loans WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        let Some(borrowed_at) = borrowed_at else {
            return Err(crate::error::not_found("loan", id));
        };
        if returned_at < borrowed_at.as_str() {
            return Err(invalid(
                "returned_at",
                returned_at,
                "on or after borrowed_at",
            ));
        }
        conn.execute(
            "UPDATE loans SET returned_at = ?2, updated_at = ?3
             WHERE id = ?1 AND returned_at IS NULL",
            rusqlite::params![id, returned_at, crate::conn::now_ms()],
        )?;
        Ok(())
    })
}

/// Delete a loan record.
pub fn delete_loan(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        if conn.execute("DELETE FROM loans WHERE id = ?1", [id])? == 0 {
            return Err(crate::error::not_found("loan", id));
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, seed};
    use maktabati_domain::book::BookInput;

    fn db_with_book() -> (Db, String) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("tracking.db")).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.write(seed::apply).unwrap();
        crate::repo::book::create(
            &db,
            BookInput {
                title: "Test book".into(),
                page_count: Some(200),
                ..Default::default()
            },
        )
        .unwrap();
        let actual_id = db
            .read(|conn| {
                Ok(conn.query_row(
                    "SELECT id FROM books WHERE title = 'Test book'",
                    [],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        (db, actual_id)
    }

    #[test]
    fn goals_update_and_aggregate_session_pages() {
        let (db, book_id) = db_with_book();
        let goal = save_goal(
            &db,
            ReadingGoalInput {
                scope: "year".into(),
                year: 2026,
                month: None,
                target_books: Some(12),
                target_pages: Some(1000),
            },
        )
        .unwrap();
        crate::repo::book::set_status(&db, &book_id, maktabati_domain::BookStatus::Reading)
            .unwrap();
        crate::repo::book::set_status(&db, &book_id, maktabati_domain::BookStatus::Finished)
            .unwrap();
        create_session(
            &db,
            ReadingSessionInput {
                book_id,
                session_date: "2026-04-03".into(),
                pages_read: Some(25),
                minutes: Some(45),
                note: None,
            },
        )
        .unwrap();
        assert_eq!(list_goals(&db).unwrap()[0].pages_read, 25);
        assert_eq!(list_goals(&db).unwrap()[0].books_completed, 1);
        let updated = save_goal(
            &db,
            ReadingGoalInput {
                scope: "year".into(),
                year: 2026,
                month: None,
                target_books: Some(20),
                target_pages: None,
            },
        )
        .unwrap();
        assert_eq!(updated.id, goal.id);
        assert_eq!(updated.pages_read, 25);
    }

    #[test]
    fn loans_can_be_returned_and_deleted() {
        let (db, book_id) = db_with_book();
        let loan = create_loan(
            &db,
            LoanInput {
                book_id,
                borrower: "Reader".into(),
                phone: None,
                borrowed_at: "2026-01-02".into(),
                due_date: Some("2026-01-10".into()),
                notes: None,
            },
        )
        .unwrap();
        assert_eq!(list_loans(&db).unwrap().len(), 1);
        assert!(create_loan(
            &db,
            LoanInput {
                book_id: loan.book_id.clone(),
                borrower: "Another Reader".into(),
                phone: None,
                borrowed_at: "2026-01-03".into(),
                due_date: None,
                notes: None,
            },
        )
        .is_err());
        return_loan(&db, &loan.id, "2026-01-08").unwrap();
        assert_eq!(
            list_loans(&db).unwrap()[0].returned_at.as_deref(),
            Some("2026-01-08")
        );
        delete_loan(&db, &loan.id).unwrap();
        assert!(list_loans(&db).unwrap().is_empty());
    }

    #[test]
    fn invalid_dates_and_empty_targets_are_rejected() {
        assert!(check_date("borrowed_at", "2026-02-31").is_err());
        assert!(save_goal(
            &db_with_book().0,
            ReadingGoalInput {
                scope: "year".into(),
                year: 2026,
                month: None,
                target_books: None,
                target_pages: None,
            }
        )
        .is_err());
    }
}
