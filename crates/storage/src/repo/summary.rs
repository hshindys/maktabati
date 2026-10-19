//! Library-wide counts used by the dashboard, the shell badge and diagnostics.

use serde::{Deserialize, Serialize};
use specta::Type;

use maktabati_domain::ReadingStatus;

use crate::conn::Db;
use crate::Result;

/// Aggregate counts for the whole library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
pub struct LibrarySummary {
    /// Books in the library.
    pub books: i64,
    /// Books broken down by reading status (always contains every status, including zeros).
    pub by_status: Vec<(ReadingStatus, i64)>,
    /// Books broken down by language code.
    pub by_language: Vec<(String, i64)>,
    /// Distinct credited authors.
    pub authors: i64,
    /// Distinct publishers.
    pub publishers: i64,
    /// Categories in the taxonomy.
    pub categories: i64,
    /// Tags in use.
    pub tags: i64,
    /// Quotes stored.
    pub quotes: i64,
    /// Note pages (including templates and sub-pages).
    pub note_pages: i64,
    /// Attached files.
    pub files: i64,
    /// Shelves.
    pub shelves: i64,
    /// Sum of `page_count` across books (for page-based goals).
    pub pages_total: i64,
    /// Books finished within `year`.
    pub finished_in_year: i64,
    /// The year `finished_in_year` refers to.
    pub year: i32,
    /// Highest applied migration version — surfaced in diagnostics so support can see the schema.
    pub schema_version: i64,
}

/// The highest applied migration version, or 0 before any migration has run.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] when the metadata table cannot be read.
pub fn schema_version(db: &Db) -> Result<i64> {
    db.read(|conn| {
        let has_table: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_migrations'",
            [],
            |r| r.get(0),
        )?;
        if has_table == 0 {
            return Ok(0);
        }
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |r| r.get(0),
        )
        .map_err(Into::into)
    })
}

/// Compute [`LibrarySummary`].
///
/// `year` selects the window for [`LibrarySummary::finished_in_year`] (typically the current
/// calendar year in the user's locale).
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on any query failure.
pub fn library_summary(db: &Db, year: i32) -> Result<LibrarySummary> {
    db.read(|conn| {
        let scalar =
            |sql: &str| -> Result<i64> { Ok(conn.query_row(sql, [], |r| r.get::<_, i64>(0))?) };

        let books = scalar("SELECT COUNT(*) FROM books")?;
        let authors = scalar("SELECT COUNT(*) FROM authors")?;
        let publishers = scalar("SELECT COUNT(*) FROM publishers")?;
        let categories = scalar("SELECT COUNT(*) FROM categories")?;
        let tags = scalar("SELECT COUNT(*) FROM tags")?;
        let quotes = scalar("SELECT COUNT(*) FROM quotes")?;
        let note_pages = scalar("SELECT COUNT(*) FROM notes_page")?;
        let files = scalar("SELECT COUNT(*) FROM book_files")?;
        let shelves = scalar("SELECT COUNT(*) FROM shelves")?;
        let pages_total =
            scalar("SELECT COALESCE(SUM(page_count), 0) FROM books WHERE page_count IS NOT NULL")?;
        // Finished cycles are stored as epoch ms; SQLite's strftime works in seconds.
        let finished_in_year: i64 = conn.query_row(
            "SELECT COUNT(*) FROM reading_cycles
             WHERE finished_at IS NOT NULL
               AND CAST(strftime('%Y', finished_at / 1000, 'unixepoch') AS INTEGER) = ?1",
            [i64::from(year)],
            |r| r.get(0),
        )?;

        let mut by_status = Vec::new();
        for status in ReadingStatus::ALL {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM books WHERE reading_status = ?1",
                [status.as_str()],
                |r| r.get(0),
            )?;
            by_status.push((*status, n));
        }

        let mut stmt = conn.prepare(
            "SELECT language_code, COUNT(*) FROM books GROUP BY language_code ORDER BY 2 DESC",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let by_language = rows.collect::<std::result::Result<Vec<_>, _>>()?;

        let schema_version = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |r| r.get(0),
        )?;

        Ok(LibrarySummary {
            books,
            by_status,
            by_language,
            authors,
            publishers,
            categories,
            tags,
            quotes,
            note_pages,
            files,
            shelves,
            pages_total,
            finished_in_year,
            year,
            schema_version,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conn::Db;
    use crate::{migrate, seed};
    use maktabati_domain::book::{Book, BookInput};
    use pretty_assertions::assert_eq;

    fn seeded_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.write(seed::apply).unwrap();
        db
    }

    #[test]
    fn empty_library_reports_zeroes_for_every_status() {
        let db = seeded_db();
        let s = library_summary(&db, 2026).unwrap();
        assert_eq!(s.books, 0);
        assert_eq!(s.by_status.len(), ReadingStatus::ALL.len());
        assert!(s.by_status.iter().all(|(_, n)| *n == 0));
        assert_eq!(s.categories, 51, "seeded taxonomy size");
        assert_eq!(
            s.schema_version,
            migrate::embedded().last().unwrap().version
        );
    }

    #[test]
    fn counts_books_by_status_and_language() {
        let db = seeded_db();
        let now = crate::now_ms();
        let mut finished = Book::new(
            BookInput {
                title: "كتاب منتهي".into(),
                page_count: Some(100),
                ..Default::default()
            },
            now,
        )
        .unwrap();
        finished.set_status(ReadingStatus::Finished, now).unwrap();
        finished.progress_pct = 100;

        let reading = Book::new(
            BookInput {
                title: "Reading book".into(),
                language_code: Some("en".into()),
                ..Default::default()
            },
            now,
        )
        .unwrap();

        db.write(|c| {
            for b in [&finished, &reading] {
                c.execute(
                    "INSERT INTO books (id, title, title_norm, language_code, reading_status,
                                        progress_pct, rating, current_page, page_count,
                                        created_at, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    rusqlite::params![
                        b.id,
                        b.title,
                        b.title_norm,
                        b.language_code,
                        b.reading_status.as_str(),
                        b.progress_pct,
                        b.rating,
                        b.current_page,
                        b.page_count,
                        b.created_at,
                        b.updated_at
                    ],
                )?;
            }
            Ok(())
        })
        .unwrap();

        let s = library_summary(&db, 2026).unwrap();
        assert_eq!(s.books, 2);
        assert_eq!(
            s.by_status
                .iter()
                .find(|(st, _)| *st == ReadingStatus::Finished)
                .map(|(_, n)| *n),
            Some(1)
        );
        assert_eq!(
            s.by_status
                .iter()
                .find(|(st, _)| *st == ReadingStatus::ToRead)
                .map(|(_, n)| *n),
            Some(1)
        );
        assert_eq!(s.pages_total, 100);
        let langs: std::collections::HashMap<_, _> = s.by_language.iter().cloned().collect();
        assert_eq!(langs.get("ar"), Some(&1));
        assert_eq!(langs.get("en"), Some(&1));
    }

    #[test]
    fn finished_in_year_counts_only_that_year() {
        let db = seeded_db();
        let mut b = Book::new(
            BookInput {
                title: "t".into(),
                page_count: Some(10),
                ..Default::default()
            },
            1,
        )
        .unwrap();
        b.set_status(ReadingStatus::Finished, 1).unwrap();

        let finished_at = chrono::Utc::now().timestamp_millis();
        db.write(|c| {
            c.execute(
                "INSERT INTO books (id, title, title_norm, language_code, reading_status,
                                    progress_pct, created_at, updated_at)
                 VALUES (?1,'t','t','ar','finished',100,1,1)",
                [&b.id],
            )?;
            c.execute(
                "INSERT INTO reading_cycles (id, book_id, started_at, finished_at, source, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'manual', ?3)",
                rusqlite::params![
                    maktabati_domain::ids::new_id(),
                    b.id,
                    1_i64,
                    finished_at
                ],
            )?;
            Ok(())
        })
        .unwrap();

        let this_year = library_summary(&db, 2026).unwrap().finished_in_year;
        assert_eq!(this_year, 1);
        let other = library_summary(&db, 1999).unwrap().finished_in_year;
        assert_eq!(other, 0);
    }
}
