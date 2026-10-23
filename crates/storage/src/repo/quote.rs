//! `quotes` — passages lifted from books (الاقتباس).
//!
//! The table lives in `0001_init.sql`; this module gives it the same typed repository the
//! other aggregates have. Writes validate the owning book first so an unknown book id comes
//! back as `storage.not_found` rather than a raw foreign-key failure, and the quote's FTS
//! projection (`fts_quote`, search milestone M5) is deliberately untouched here.

use maktabati_domain::quote::{Quote, QuoteInput};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::conn::Db;
use crate::Result;

/// Every `quotes` column, in the order [`from_row`] reads them.
const SELECT: &str =
    "SELECT id, book_id, text, text_norm, note, page, chapter, color, created_at, updated_at FROM quotes";

/// Map one `quotes` row onto a [`Quote`].
#[allow(clippy::too_many_lines)]
fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Quote> {
    let color: String = row.get(7)?;
    Ok(Quote {
        id: row.get(0)?,
        book_id: row.get(1)?,
        text: row.get(2)?,
        text_norm: row.get(3)?,
        note: row.get(4)?,
        page: row.get(5)?,
        chapter: row.get(6)?,
        color: maktabati_domain::QuoteColor::from_db(&color)
            .unwrap_or(maktabati_domain::QuoteColor::Yellow),
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// A quote with the two book facts the global quotes screen renders.
///
/// The join is presentation-facing, like [`crate::repo::book::BookDetail`]: [`Quote`] stays
/// pure and the screen never joins per row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct QuoteWithBook {
    /// The quote itself.
    pub quote: Quote,
    /// The book's title.
    pub book_title: String,
    /// First author credited on the book, if any (display order).
    pub book_author: Option<String>,
}

/// Store one quote under an existing book.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the book does not exist (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn create(db: &Db, book_id: &str, input: QuoteInput) -> Result<Quote> {
    db.transact(|conn| {
        let known: Option<String> = conn
            .query_row("SELECT id FROM books WHERE id = ?1", [book_id], |r| {
                r.get(0)
            })
            .ok();
        if known.is_none() {
            return Err(crate::error::not_found("book", book_id));
        }
        let quote = Quote::new(book_id, input, crate::conn::now_ms())?;
        conn.execute(
            "INSERT INTO quotes (id, book_id, text, text_norm, note, page, chapter, color, \
                                 source, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'manual', ?9, ?9)",
            rusqlite::params![
                quote.id,
                quote.book_id,
                quote.text,
                quote.text_norm,
                quote.note,
                quote.page,
                quote.chapter,
                quote.color.as_str(),
                quote.created_at,
            ],
        )?;
        Ok(quote)
    })
}

/// List a book's quotes, newest first.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; a book with no quotes yields an empty
/// `Vec` (whether or not the book itself exists — the caller owns that distinction).
pub fn list(db: &Db, book_id: &str) -> Result<Vec<Quote>> {
    db.read(|conn| {
        let sql = format!("{SELECT} WHERE book_id = ?1 ORDER BY created_at DESC, id DESC");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([book_id], from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Every quote in the library, newest first, with the owning book's title and first author.
///
/// Powers the global quotations screen (`/quotes`), which shows quotes across books.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn list_all(db: &Db) -> Result<Vec<QuoteWithBook>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT q.id, q.book_id, q.text, q.text_norm, q.note, q.page, q.chapter, q.color, \
                    q.created_at, q.updated_at, b.title, a.name
             FROM quotes q
             JOIN books b ON b.id = q.book_id
             LEFT JOIN book_authors ba ON ba.book_id = q.book_id AND ba.position = 0
             LEFT JOIN authors a ON a.id = ba.author_id
             ORDER BY q.created_at DESC, q.id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let quote = Quote {
                id: row.get(0)?,
                book_id: row.get(1)?,
                text: row.get(2)?,
                text_norm: row.get(3)?,
                note: row.get(4)?,
                page: row.get(5)?,
                chapter: row.get(6)?,
                color: maktabati_domain::QuoteColor::from_db(&row.get::<_, String>(7)?)
                    .unwrap_or(maktabati_domain::QuoteColor::Yellow),
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            };
            Ok(QuoteWithBook {
                quote,
                book_title: row.get(10)?,
                book_author: row.get(11)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Edit one quote in place and return the stored row.
///
/// `updated_at` is stamped with `now_ms()`; id, book and creation time never move.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id does not exist (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn update(db: &Db, id: &str, input: QuoteInput) -> Result<Quote> {
    db.write(|conn| {
        let existing: Option<String> = conn
            .query_row("SELECT id FROM quotes WHERE id = ?1", [id], |r| r.get(0))
            .ok();
        if existing.is_none() {
            return Err(crate::error::not_found("quote", id));
        }
        // Seed the entity from the stored row so id/book/created_at stay put.
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE id = ?1"))?;
        let mut rows = stmt.query([id])?;
        let Some(row) = rows.next()? else {
            return Err(crate::error::not_found("quote", id));
        };
        let mut quote = from_row(row)?;
        drop(rows);
        drop(stmt);
        quote.apply(input, crate::conn::now_ms())?;
        conn.execute(
            "UPDATE quotes SET text = ?2, text_norm = ?3, note = ?4, page = ?5, chapter = ?6, \
                               color = ?7, updated_at = ?8
             WHERE id = ?1",
            rusqlite::params![
                quote.id,
                quote.text,
                quote.text_norm,
                quote.note,
                quote.page,
                quote.chapter,
                quote.color.as_str(),
                quote.updated_at,
            ],
        )?;
        Ok(quote)
    })
}

/// Delete one quote by id. Its FTS projection goes with the book-level index at M5.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id is unknown (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        let removed = conn.execute("DELETE FROM quotes WHERE id = ?1", [id])?;
        if removed == 0 {
            return Err(crate::error::not_found("quote", id));
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageError;
    use maktabati_domain::BookInput;
    use pretty_assertions::assert_eq;

    fn seeded_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        crate::migrate::apply(&db, || Ok(())).unwrap();
        db.write(crate::seed::apply).unwrap();
        db
    }

    fn a_book(db: &Db) -> String {
        crate::repo::book::create(
            db,
            BookInput {
                title: "الأيام".into(),
                language_code: Some("ar".into()),
                ..Default::default()
            },
        )
        .unwrap()
        .id
    }

    fn input(text: &str) -> QuoteInput {
        QuoteInput {
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn create_list_round_trips_every_field() {
        let db = seeded_db();
        let book = a_book(&db);
        let created = create(
            &db,
            &book,
            QuoteInput {
                text: "  من مروء الكرم  ".into(),
                note: Some("صفحة ذهبية".into()),
                page: Some(78),
                chapter: Some("الفصل الثاني".into()),
                color: Some(maktabati_domain::QuoteColor::Green),
            },
        )
        .unwrap();

        assert_eq!(created.book_id, book);
        assert_eq!(created.text, "من مروء الكرم");
        assert_eq!(created.note.as_deref(), Some("صفحة ذهبية"));
        assert_eq!(created.page, Some(78));
        assert_eq!(created.chapter.as_deref(), Some("الفصل الثاني"));
        assert_eq!(created.color, maktabati_domain::QuoteColor::Green);
        assert!(created.created_at > 0);

        let list = list(&db, &book).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], created);
    }

    #[test]
    fn quoting_an_unknown_book_is_not_found() {
        let db = seeded_db();
        let err = create(&db, "no-such-book", input("x")).unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
    }

    #[test]
    fn blank_text_is_a_domain_error() {
        let db = seeded_db();
        let book = a_book(&db);
        let err = create(&db, &book, input("   ")).unwrap_err();
        assert_eq!(err.code(), "validation.empty_field");
    }

    #[test]
    fn list_all_joins_book_title_and_first_author() {
        let db = seeded_db();
        let book = a_book(&db);
        // Credit one author at position 0.
        let author = crate::repo::author::create(
            &db,
            maktabati_domain::AuthorInput {
                name: "طه حسين".into(),
                ..Default::default()
            },
        )
        .unwrap();
        db.write(|c| {
            c.execute(
                "INSERT INTO book_authors (book_id, author_id, role) VALUES (?1, ?2, 'author')",
                rusqlite::params![book, author.id],
            )?;
            Ok(())
        })
        .unwrap();

        create(&db, &book, input("المتنبي أجمل من نفسي الأولى.")).unwrap();
        let all = list_all(&db).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].book_title, "الأيام");
        assert_eq!(all[0].book_author.as_deref(), Some("طه حسين"));
    }

    #[test]
    fn update_edits_fields_but_keeps_identity() {
        let db = seeded_db();
        let book = a_book(&db);
        let original = create(&db, &book, input("old")).unwrap();
        let updated = update(
            &db,
            &original.id,
            QuoteInput {
                text: "new".into(),
                page: Some(9),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.id, original.id);
        assert_eq!(updated.book_id, book);
        assert_eq!(updated.created_at, original.created_at);
        assert!(updated.updated_at >= original.updated_at);
        assert_eq!(updated.text, "new");
        assert_eq!(updated.page, Some(9));
    }

    #[test]
    fn deleting_then_updating_is_not_found() {
        let db = seeded_db();
        let book = a_book(&db);
        let q = create(&db, &book, input("gone")).unwrap();

        delete(&db, &q.id).unwrap();
        assert!(matches!(
            get_quote_err(&db, &q.id),
            Err(StorageError::NotFound { .. })
        ));

        let err = update(&db, &q.id, input("x")).unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
        assert_eq!(delete(&db, &q.id).unwrap_err().code(), "storage.not_found");
    }

    fn get_quote_err(db: &Db, id: &str) -> std::result::Result<(), StorageError> {
        db.read(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM quotes WHERE id = ?1", [id], |r| {
                    r.get(0)
                })?;
            if count == 0 {
                return Err(crate::error::not_found("quote", id));
            }
            Ok(())
        })
    }
}
