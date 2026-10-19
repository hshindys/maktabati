//! `books` — the catalogue's aggregate root, and the junction rows that carry its credits.
//!
//! `Book` deliberately holds no relations (`docs/03-data-model.md` §3.1), so this module owns
//! the three junction tables as well: a create or update writes the scalar row *and* replaces
//! its author credits, categories and tags in the same write.

use maktabati_domain::{
    text::normalize, AuthorCredit, AuthorRole, Book, BookFormat, BookInput, BookSource, BookStatus,
    Condition, CoverSource, Isbn,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::conn::{now_ms, Db};
use crate::error::not_found;
use crate::Result;

/// Columns in the order [`from_row`] reads them. `box` is quoted because it sits close enough
/// to SQL's type names to be worth not gambling on.
const COLUMNS: &str = "id, title, subtitle, original_title, title_norm, language_code, isbn10, \
                       isbn13, edition, publication_year, page_count, format, description, \
                       series_id, series_order, publisher_id, room, shelf, \"box\", \
                       acquired_date, acquired_from, price_minor, price_currency, condition, \
                       cover_path, cover_thumb, cover_source, reading_status, rating, \
                       current_page, progress_pct, source, external_ids, created_at, updated_at";

/// `INSERT` covering every column except none — kept literal (rather than built from
/// [`COLUMNS`]) so the bound-parameter order is visible next to it in review.
const INSERT: &str = "INSERT INTO books (
    id, title, subtitle, original_title, title_norm, language_code, isbn10, isbn13, edition,
    publication_year, page_count, format, description, series_id, series_order, publisher_id,
    room, shelf, \"box\", acquired_date, acquired_from, price_minor, price_currency, condition,
    cover_path, cover_thumb, cover_source, reading_status, rating, current_page, progress_pct,
    source, external_ids, created_at, updated_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
          ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35)";

/// `UPDATE` over the same columns minus `id` and `created_at`: an edit never moves a book's
/// identity or its place in "recently added".
const UPDATE: &str = "UPDATE books SET
    title = ?1, subtitle = ?2, original_title = ?3, title_norm = ?4, language_code = ?5,
    isbn10 = ?6, isbn13 = ?7, edition = ?8, publication_year = ?9, page_count = ?10,
    format = ?11, description = ?12, series_id = ?13, series_order = ?14, publisher_id = ?15,
    room = ?16, shelf = ?17, \"box\" = ?18, acquired_date = ?19, acquired_from = ?20,
    price_minor = ?21, price_currency = ?22, condition = ?23, cover_path = ?24,
    cover_thumb = ?25, cover_source = ?26, reading_status = ?27, rating = ?28,
    current_page = ?29, progress_pct = ?30, source = ?31, external_ids = ?32, updated_at = ?33
  WHERE id = ?34";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Book> {
    Ok(Book {
        id: row.get(0)?,
        title: row.get(1)?,
        subtitle: row.get(2)?,
        original_title: row.get(3)?,
        title_norm: row.get(4)?,
        language_code: row.get(5)?,
        isbn10: row.get(6)?,
        isbn13: row.get(7)?,
        edition: row.get(8)?,
        publication_year: row.get(9)?,
        page_count: row.get(10)?,
        // `CHECK`-constrained strings fall back to their first variant when a *newer* build
        // wrote a value this one has never heard of: a book must not vanish from the library
        // because its format name moved on.
        format: BookFormat::from_db(&row.get::<_, String>(11)?).unwrap_or(BookFormat::Print),
        description: row.get(12)?,
        series_id: row.get(13)?,
        series_order: row.get(14)?,
        publisher_id: row.get(15)?,
        room: row.get(16)?,
        shelf: row.get(17)?,
        box_name: row.get(18)?,
        acquired_date: row.get(19)?,
        acquired_from: row.get(20)?,
        price_minor: row.get(21)?,
        price_currency: row.get(22)?,
        condition: row
            .get::<_, Option<String>>(23)?
            .and_then(|raw| Condition::from_db(&raw)),
        cover_path: row.get(24)?,
        cover_thumb: row.get(25)?,
        cover_source: row
            .get::<_, Option<String>>(26)?
            .and_then(|raw| CoverSource::from_db(&raw)),
        reading_status: BookStatus::from_db(&row.get::<_, String>(27)?)
            .unwrap_or(BookStatus::ToRead),
        rating: row.get(28)?,
        current_page: row.get(29)?,
        progress_pct: row.get(30)?,
        source: row
            .get::<_, Option<String>>(31)?
            .and_then(|raw| BookSource::from_db(&raw))
            .unwrap_or(BookSource::Manual),
        // A blob that no longer parses degrades to "no external ids" rather than failing a
        // read: losing a Google Books key must never hide a book from its own page.
        external_ids: row
            .get::<_, Option<String>>(32)?
            .and_then(|raw| serde_json::from_str(&raw).ok()),
        created_at: row.get(33)?,
        updated_at: row.get(34)?,
    })
}

fn read_row(conn: &rusqlite::Connection, id: &str) -> Result<Book> {
    let sql = format!("SELECT {COLUMNS} FROM books WHERE id = ?1");
    conn.query_row(&sql, [id], from_row).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => not_found("book", id),
        other => crate::StorageError::Sqlite(other),
    })
}

fn ext_json(book: &Book) -> Result<Option<String>> {
    Ok(book
        .external_ids
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?)
}

/// The value a book's uniqueness is judged on — the ISBN it was refused for, shown back to
/// the user as-is (`ux_books_isbn13` is the only unique index other than the UUID primary key).
fn isbn_key(book: &Book) -> &str {
    book.isbn13
        .as_deref()
        .or(book.isbn10.as_deref())
        .unwrap_or(book.title.as_str())
}

fn insert_row(conn: &rusqlite::Connection, book: &Book) -> Result<()> {
    let ext = ext_json(book)?;
    conn.execute(
        INSERT,
        rusqlite::params![
            book.id,
            book.title,
            book.subtitle,
            book.original_title,
            book.title_norm,
            book.language_code,
            book.isbn10,
            book.isbn13,
            book.edition,
            book.publication_year,
            book.page_count,
            book.format.as_str(),
            book.description,
            book.series_id,
            book.series_order,
            book.publisher_id,
            book.room,
            book.shelf,
            book.box_name,
            book.acquired_date,
            book.acquired_from,
            book.price_minor,
            book.price_currency,
            book.condition.as_ref().map(|c| c.as_str()),
            book.cover_path,
            book.cover_thumb,
            book.cover_source.as_ref().map(|c| c.as_str()),
            book.reading_status.as_str(),
            book.rating,
            book.current_page,
            book.progress_pct,
            book.source.as_str(),
            ext,
            book.created_at,
            book.updated_at,
        ],
    )
    .map_err(|e| crate::error::on_unique("book", isbn_key(book), e))?;
    Ok(())
}

fn update_row(conn: &rusqlite::Connection, book: &Book) -> Result<()> {
    let ext = ext_json(book)?;
    conn.execute(
        UPDATE,
        rusqlite::params![
            book.title,
            book.subtitle,
            book.original_title,
            book.title_norm,
            book.language_code,
            book.isbn10,
            book.isbn13,
            book.edition,
            book.publication_year,
            book.page_count,
            book.format.as_str(),
            book.description,
            book.series_id,
            book.series_order,
            book.publisher_id,
            book.room,
            book.shelf,
            book.box_name,
            book.acquired_date,
            book.acquired_from,
            book.price_minor,
            book.price_currency,
            book.condition.as_ref().map(|c| c.as_str()),
            book.cover_path,
            book.cover_thumb,
            book.cover_source.as_ref().map(|c| c.as_str()),
            book.reading_status.as_str(),
            book.rating,
            book.current_page,
            book.progress_pct,
            book.source.as_str(),
            ext,
            book.updated_at,
            book.id,
        ],
    )
    .map_err(|e| crate::error::on_unique("book", isbn_key(book), e))?;
    Ok(())
}

/// Does a row with this key exist? `sql` is a literal from this module, never user input.
fn present(conn: &rusqlite::Connection, sql: &str, value: &str) -> Result<bool> {
    Ok(conn
        .query_row(sql, [value], |row| row.get::<_, i64>(0))
        .optional()?
        .is_some())
}

/// Check every foreign key by hand rather than leaving it to SQLite: a bare constraint
/// failure tells the user nothing about *which* lookup missed.
fn check_references(conn: &rusqlite::Connection, book: &Book) -> Result<()> {
    if !present(
        conn,
        "SELECT 1 FROM languages WHERE code = ?1",
        &book.language_code,
    )? {
        return Err(not_found("language", &book.language_code));
    }
    if let Some(id) = book.series_id.as_deref() {
        if !present(conn, "SELECT 1 FROM series WHERE id = ?1", id)? {
            return Err(not_found("series", id));
        }
    }
    if let Some(id) = book.publisher_id.as_deref() {
        if !present(conn, "SELECT 1 FROM publishers WHERE id = ?1", id)? {
            return Err(not_found("publisher", id));
        }
    }
    Ok(())
}

/// Replace a book's author credits, keeping the order the caller supplied.
fn write_credits(
    conn: &rusqlite::Connection,
    book_id: &str,
    credits: &[AuthorCredit],
) -> Result<()> {
    conn.execute("DELETE FROM book_authors WHERE book_id = ?1", [book_id])?;
    for (position, credit) in credits.iter().enumerate() {
        if !present(
            conn,
            "SELECT 1 FROM authors WHERE id = ?1",
            &credit.author_id,
        )? {
            return Err(not_found("author", &credit.author_id));
        }
        conn.execute(
            "INSERT INTO book_authors (book_id, author_id, role, position) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                book_id,
                credit.author_id,
                credit.role.as_str(),
                position as i64
            ],
        )
        .map_err(|e| crate::error::on_unique("author", &credit.author_id, e))?;
    }
    Ok(())
}

/// Replace a book's category links, keeping the order the caller supplied.
fn write_categories(conn: &rusqlite::Connection, book_id: &str, ids: &[String]) -> Result<()> {
    conn.execute("DELETE FROM book_categories WHERE book_id = ?1", [book_id])?;
    for (position, id) in ids.iter().enumerate() {
        if !present(conn, "SELECT 1 FROM categories WHERE id = ?1", id)? {
            return Err(not_found("category", id));
        }
        conn.execute(
            "INSERT INTO book_categories (book_id, category_id, position) VALUES (?1, ?2, ?3)",
            rusqlite::params![book_id, id, position as i64],
        )
        .map_err(|e| crate::error::on_unique("category", id, e))?;
    }
    Ok(())
}

/// Replace a book's tag links (tags are a set — the table has no `position`).
fn write_tags(conn: &rusqlite::Connection, book_id: &str, ids: &[String]) -> Result<()> {
    conn.execute("DELETE FROM book_tags WHERE book_id = ?1", [book_id])?;
    for id in ids {
        if !present(conn, "SELECT 1 FROM tags WHERE id = ?1", id)? {
            return Err(not_found("tag", id));
        }
        conn.execute(
            "INSERT INTO book_tags (book_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![book_id, id],
        )
        .map_err(|e| crate::error::on_unique("tag", id, e))?;
    }
    Ok(())
}

fn read_credits(conn: &rusqlite::Connection, book_id: &str) -> Result<Vec<AuthorCredit>> {
    let mut stmt = conn
        .prepare("SELECT author_id, role FROM book_authors WHERE book_id = ?1 ORDER BY position")?;
    let rows = stmt.query_map([book_id], |row| {
        let role: String = row.get(1)?;
        Ok(AuthorCredit {
            author_id: row.get(0)?,
            role: AuthorRole::from_db(&role).unwrap_or(AuthorRole::Author),
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn read_category_ids(conn: &rusqlite::Connection, book_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT category_id FROM book_categories WHERE book_id = ?1 ORDER BY position")?;
    let rows = stmt.query_map([book_id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn read_tag_ids(conn: &rusqlite::Connection, book_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT tag_id FROM book_tags WHERE book_id = ?1")?;
    let rows = stmt.query_map([book_id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// A book with the relations the per-book page renders. They live outside [`Book`] by design,
/// so the page asks for both in one call instead of four round trips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct BookDetail {
    /// The book's scalar fields.
    pub book: Book,
    /// Author credits, in order, with roles.
    pub authors: Vec<AuthorCredit>,
    /// Category ids the book is filed under.
    pub categories: Vec<String>,
    /// Tag ids.
    pub tags: Vec<String>,
}

/// Create a book from a validated input, replacing nothing (there is nothing to replace yet).
///
/// # Errors
///
/// * [`maktabati_domain::DomainError`] for a blank title, a malformed ISBN, or an
///   out-of-range year / page count / rating.
/// * [`crate::StorageError::NotFound`] when the language, publisher, series, an author
///   credit, a category or a tag does not exist.
/// * [`crate::StorageError::Duplicate`] when the ISBN is already taken (`ux_books_isbn13`).
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn create(db: &Db, input: BookInput) -> Result<Book> {
    // `transact`, not `write`: the scalar row and three junction rewrites either all land or
    // none do — a half-written book would be invisible to its own page.
    db.transact(|conn| {
        let credits = input.authors.clone();
        let categories = input.categories.clone();
        let tags = input.tags.clone();
        let book = Book::new(input, now_ms())?;
        check_references(conn, &book)?;
        insert_row(conn, &book)?;
        write_credits(conn, &book.id, &credits)?;
        write_categories(conn, &book.id, &categories)?;
        write_tags(conn, &book.id, &tags)?;
        Ok(book)
    })
}

/// Read one book (scalar fields only — ask [`detail`] for its credits).
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id is unknown.
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get(db: &Db, id: &str) -> Result<Book> {
    db.read(|conn| read_row(conn, id))
}

/// Read one book together with its credits, categories and tags.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id is unknown.
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn detail(db: &Db, id: &str) -> Result<BookDetail> {
    db.read(|conn| {
        let book = read_row(conn, id)?;
        Ok(BookDetail {
            authors: read_credits(conn, id)?,
            categories: read_category_ids(conn, id)?,
            tags: read_tag_ids(conn, id)?,
            book,
        })
    })
}

/// Every book, newest-added first (the library's default order).
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn list(db: &Db) -> Result<Vec<Book>> {
    db.read(|conn| {
        let sql = format!("SELECT {COLUMNS} FROM books ORDER BY created_at DESC, id DESC");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
}

/// Edit a book in place and replace its credits, categories and tags.
///
/// `id` and `created_at` are preserved — an edit never moves a book's identity or its place
/// in "recently added".
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id (or any relation it now names) is gone.
/// * [`maktabati_domain::DomainError`] under the same rules as [`create`].
/// * [`crate::StorageError::Duplicate`] when the edit gives the book an ISBN already taken.
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn update(db: &Db, id: &str, input: BookInput) -> Result<Book> {
    // An edit that replaces the credits must not be able to lose them while keeping the old
    // book row (or vice versa), so this is one transaction too.
    db.transact(|conn| {
        let mut book = read_row(conn, id)?;
        let credits = input.authors.clone();
        let categories = input.categories.clone();
        let tags = input.tags.clone();
        book.apply(input, now_ms())?;
        check_references(conn, &book)?;
        update_row(conn, &book)?;
        write_credits(conn, &book.id, &credits)?;
        write_categories(conn, &book.id, &categories)?;
        write_tags(conn, &book.id, &tags)?;
        Ok(book)
    })
}

/// Delete a book. Its credits, categories and tags go with it (`ON DELETE CASCADE`); the
/// authors, categories and tags themselves are untouched.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id is unknown.
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        conn.execute("DELETE FROM books WHERE id = ?1", [id])?;
        if conn.changes() == 0 {
            return Err(not_found("book", id));
        }
        Ok(())
    })
}

/// Books carrying this ISBN in either column (ISBN-10 and ISBN-13 of the same book are both
/// accepted as "the" identifier).
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn find_by_isbn(db: &Db, isbn: &str) -> Result<Vec<Book>> {
    let isbn = isbn.trim();
    if isbn.is_empty() {
        return Ok(vec![]);
    }
    db.read(|conn| {
        let sql = format!("SELECT {COLUMNS} FROM books WHERE isbn13 = ?1 OR isbn10 = ?1");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([isbn], from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
}

/// Normalized name mirrors for the authors a form is crediting. An author the form names but
/// the database has lost simply contributes nothing here — the caller validates that
/// separately, and this is only a similarity probe.
fn input_author_norms(
    conn: &rusqlite::Connection,
    credits: &[AuthorCredit],
) -> Result<Vec<String>> {
    let mut norms = Vec::new();
    for credit in credits {
        if let Ok(norm) = conn.query_row(
            "SELECT name_norm FROM authors WHERE id = ?1",
            [credit.author_id.as_str()],
            |row| row.get(0),
        ) {
            norms.push(norm);
        }
    }
    Ok(norms)
}

/// Normalized name mirrors of the authors already credited on a book.
fn stored_author_norms(conn: &rusqlite::Connection, book_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT a.name_norm FROM book_authors ba JOIN authors a ON a.id = ba.author_id \
         WHERE ba.book_id = ?1",
    )?;
    let rows = stmt.query_map([book_id], |row| row.get(0))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Candidates the save-time duplicate check should warn about: any book with the same ISBN,
/// plus books whose normalized title *and* at least one normalized author name match.
///
/// When the input names no authors, the title alone decides — an unattributed book re-entered
/// is usually the same copy. The warning never blocks a save (a second edition of the same
/// title is a legitimate acquisition); `exclude_id` keeps an edit from matching itself.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn find_duplicates(db: &Db, input: &BookInput, exclude_id: Option<&str>) -> Result<Vec<Book>> {
    db.read(|conn| {
        let mut hits: Vec<Book> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let keep = |book: Book, hits: &mut Vec<Book>, seen: &mut Vec<String>| {
            if exclude_id == Some(book.id.as_str()) || seen.contains(&book.id) {
                return;
            }
            seen.push(book.id.clone());
            hits.push(book);
        };

        // Mirror what `Book::new` stores: `resolve_isbns` always writes *both* forms of a
        // valid ISBN as digits, so one canonical probe finds a book however it was typed in.
        for raw in [input.isbn13.as_deref(), input.isbn10.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let Ok(isbn) = Isbn::parse(raw) else { continue };
            let sql = format!("SELECT {COLUMNS} FROM books WHERE isbn13 = ?1 OR isbn10 = ?1");
            let mut stmt = conn.prepare(&sql)?;
            for row in stmt.query_map([isbn.to_isbn13()], from_row)? {
                keep(row?, &mut hits, &mut seen);
            }
        }

        let title_norm = normalize(&input.title);
        let sql = format!("SELECT {COLUMNS} FROM books WHERE title_norm = ?1");
        let mut stmt = conn.prepare(&sql)?;
        let candidates: Vec<Book> = stmt
            .query_map([title_norm.as_str()], from_row)?
            .collect::<rusqlite::Result<_>>()?;

        let input_norms = input_author_norms(conn, &input.authors)?;
        for book in candidates {
            if input_norms.is_empty() {
                keep(book, &mut hits, &mut seen);
                continue;
            }
            let stored = stored_author_norms(conn, &book.id)?;
            if stored.iter().any(|norm| input_norms.contains(norm)) {
                keep(book, &mut hits, &mut seen);
            }
        }
        Ok(hits)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageError;
    use maktabati_domain::{AuthorInput, CategoryInput, PublisherInput, TagInput};
    use pretty_assertions::assert_eq;

    /// Migrated **and** seeded: `books.language_code` is a foreign key into `languages`, and
    /// the seed is what puts `ar` there.
    fn seeded_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        crate::migrate::apply(&db, || Ok(())).unwrap();
        db.write(crate::seed::apply).unwrap();
        db
    }

    fn full_input() -> BookInput {
        BookInput {
            title: "مقدمة في الحضارة الإسلامية".into(),
            subtitle: Some("جزء أول".into()),
            original_title: Some("Introduction to Islamic Civilisation".into()),
            language_code: Some("ar".into()),
            isbn13: Some("9789953210551".into()),
            edition: Some("الطبعة الثالثة".into()),
            publication_year: Some(2005),
            page_count: Some(412),
            format: Some(BookFormat::Print),
            description: Some("كتاب مرجعي في تاريخ الحضارة.".into()),
            series_id: None,
            series_order: None,
            room: Some("المكتبة الرئيسية".into()),
            shelf: Some("رف 3".into()),
            box_name: Some("صندوق أ".into()),
            acquired_date: Some("2026-01-05".into()),
            acquired_from: Some("معرض الكتاب".into()),
            price_minor: Some(1_500),
            price_currency: Some("SAR".into()),
            condition: Some(Condition::VeryGood),
            rating: Some(9),
            current_page: Some(100),
            reading_status: Some(BookStatus::Reading),
            cover_path: Some("covers/muqaddima.jpg".into()),
            cover_source: Some(CoverSource::Manual),
            source: Some(BookSource::Manual),
            external_ids: None,
            ..Default::default()
        }
    }

    fn author_named(db: &Db, name: &str) -> maktabati_domain::Author {
        crate::repo::author::create(
            db,
            AuthorInput {
                name: name.into(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn credit(author: &maktabati_domain::Author) -> AuthorCredit {
        AuthorCredit {
            author_id: author.id.clone(),
            role: AuthorRole::Author,
        }
    }

    fn tag_named(db: &Db, name: &str) -> maktabati_domain::Tag {
        crate::repo::tag::create(
            db,
            TagInput {
                name: name.into(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn publisher_named(db: &Db, name: &str) -> maktabati_domain::Publisher {
        crate::repo::publisher::create(
            db,
            PublisherInput {
                name: name.into(),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn seeded_category_id(db: &Db) -> String {
        crate::repo::category::list(db).unwrap()[0].id.clone()
    }

    #[test]
    fn creates_a_book_and_reads_every_scalar_back() {
        let db = seeded_db();
        let publisher = publisher_named(&db, "دار ابن حزم");
        let input = BookInput {
            publisher_id: Some(publisher.id.clone()),
            ..full_input()
        };
        let created = create(&db, input).unwrap();
        assert!(created.id.len() > 30, "a UUID v7");
        assert!(created.created_at > 0);
        assert_eq!(created.title_norm, normalize("مقدمة في الحضارة الإسلامية"));
        assert_eq!(
            created.progress_pct, 24,
            "derived from current_page / page_count"
        );

        let read = get(&db, &created.id).unwrap();
        assert_eq!(read, created);
        assert_eq!(read.publisher_id.as_deref(), Some(publisher.id.as_str()));
        assert_eq!(read.box_name.as_deref(), Some("صندوق أ"));
        assert_eq!(read.condition, Some(Condition::VeryGood));
        assert_eq!(read.rating, Some(9));
        assert_eq!(read.reading_status, BookStatus::Reading);
        assert_eq!(read.cover_source, Some(CoverSource::Manual));
        assert_eq!(read.source, BookSource::Manual);
    }

    #[test]
    fn relations_round_trip_in_order() {
        let db = seeded_db();
        let ahmed = author_named(&db, "أحمد فكري");
        let mustafa = author_named(&db, "مصطفى لطفي");
        let category = seeded_category_id(&db);
        let tag = tag_named(&db, "مراجعة");
        let other_tag = tag_named(&db, "مرجع");

        let input = BookInput {
            authors: vec![
                AuthorCredit {
                    author_id: ahmed.id.clone(),
                    role: AuthorRole::Author,
                },
                AuthorCredit {
                    author_id: mustafa.id.clone(),
                    role: AuthorRole::Translator,
                },
            ],
            categories: vec![category.clone()],
            tags: vec![tag.id.clone(), other_tag.id.clone()],
            ..full_input()
        };
        let created = create(&db, input).unwrap();
        let detail = detail(&db, &created.id).unwrap();

        assert_eq!(
            detail.authors,
            vec![
                AuthorCredit {
                    author_id: ahmed.id.clone(),
                    role: AuthorRole::Author,
                },
                AuthorCredit {
                    author_id: mustafa.id.clone(),
                    role: AuthorRole::Translator,
                },
            ]
        );
        assert_eq!(detail.categories, vec![category]);
        assert_eq!(detail.tags, vec![tag.id, other_tag.id]);
    }

    #[test]
    fn a_missing_foreign_key_names_what_was_missing() {
        let db = seeded_db();

        let no_language = create(
            &db,
            BookInput {
                language_code: Some("xx".into()),
                ..full_input()
            },
        )
        .unwrap_err();
        assert_eq!(no_language.code(), "storage.not_found");
        assert!(matches!(
            no_language,
            StorageError::NotFound { ref kind, .. } if kind == "language"
        ));

        let no_author = create(
            &db,
            BookInput {
                authors: vec![AuthorCredit {
                    author_id: "ghost".into(),
                    role: AuthorRole::Author,
                }],
                ..full_input()
            },
        )
        .unwrap_err();
        assert!(matches!(
            no_author,
            StorageError::NotFound { ref kind, ref id } if kind == "author" && id == "ghost"
        ));

        let no_publisher = create(
            &db,
            BookInput {
                publisher_id: Some("ghost".into()),
                ..full_input()
            },
        )
        .unwrap_err();
        assert!(matches!(
            no_publisher,
            StorageError::NotFound { ref kind, .. } if kind == "publisher"
        ));

        let no_category = create(
            &db,
            BookInput {
                categories: vec!["ghost".into()],
                ..full_input()
            },
        )
        .unwrap_err();
        assert!(matches!(
            no_category,
            StorageError::NotFound { ref kind, .. } if kind == "category"
        ));

        let no_tag = create(
            &db,
            BookInput {
                tags: vec!["ghost".into()],
                ..full_input()
            },
        )
        .unwrap_err();
        assert!(matches!(
            no_tag,
            StorageError::NotFound { ref kind, .. } if kind == "tag"
        ));
    }

    #[test]
    fn the_domain_still_guards_the_input() {
        let db = seeded_db();
        let err = create(
            &db,
            BookInput {
                title: "   ".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "validation.empty_field");

        let err = create(
            &db,
            BookInput {
                title: "كتاب".into(),
                isbn13: Some("9789953210559".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "validation.isbn_invalid");
    }

    #[test]
    fn the_same_isbn_twice_is_a_duplicate() {
        let db = seeded_db();
        create(&db, full_input()).unwrap();
        let err = create(
            &db,
            BookInput {
                title: "نسخة أخرى".into(),
                ..full_input()
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "storage.duplicate");
    }

    #[test]
    fn editing_keeps_the_id_and_creation_time_and_replaces_the_credits() {
        let db = seeded_db();
        let ahmed = author_named(&db, "أحمد فكري");
        let created = create(
            &db,
            BookInput {
                authors: vec![AuthorCredit {
                    author_id: ahmed.id.clone(),
                    role: AuthorRole::Author,
                }],
                ..full_input()
            },
        )
        .unwrap();

        let edited = update(
            &db,
            &created.id,
            BookInput {
                title: "مقدمة في الحضارة الإسلامية — طبعة محققة".into(),
                authors: vec![],
                rating: None,
                ..full_input()
            },
        )
        .unwrap();

        assert_eq!(edited.id, created.id);
        assert_eq!(edited.created_at, created.created_at);
        assert_eq!(edited.title, "مقدمة في الحضارة الإسلامية — طبعة محققة");
        assert_eq!(edited.rating, None);
        assert_eq!(edited.updated_at, created.updated_at.max(edited.updated_at));

        let detail = detail(&db, &created.id).unwrap();
        assert!(
            detail.authors.is_empty(),
            "the credits are replaced, not merged"
        );
        assert_eq!(get(&db, &created.id).unwrap().title, edited.title);
    }

    #[test]
    fn deleting_a_book_takes_its_junction_rows_with_it() {
        let db = seeded_db();
        let ahmed = author_named(&db, "أحمد فكري");
        let created = create(
            &db,
            BookInput {
                authors: vec![AuthorCredit {
                    author_id: ahmed.id.clone(),
                    role: AuthorRole::Author,
                }],
                ..full_input()
            },
        )
        .unwrap();

        delete(&db, &created.id).unwrap();
        assert!(list(&db).unwrap().is_empty());
        assert_eq!(
            get(&db, &created.id).unwrap_err().code(),
            "storage.not_found"
        );
        assert_eq!(
            delete(&db, &created.id).unwrap_err().code(),
            "storage.not_found"
        );

        let links: i64 = db
            .read(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM book_authors WHERE book_id = ?1",
                    [created.id.as_str()],
                    |row| row.get(0),
                )
                .map_err(crate::StorageError::Sqlite)
            })
            .unwrap();
        assert_eq!(links, 0, "the junction rows follow the book");
    }

    #[test]
    fn listing_is_newest_first() {
        let db = seeded_db();
        let first = create(
            &db,
            BookInput {
                title: "الأول".into(),
                ..Default::default()
            },
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = create(
            &db,
            BookInput {
                title: "الثاني".into(),
                ..Default::default()
            },
        )
        .unwrap();

        let listed: Vec<String> = list(&db).unwrap().into_iter().map(|b| b.id).collect();
        assert_eq!(listed, vec![second.id, first.id]);
    }

    #[test]
    fn finds_an_isbn_in_either_column() {
        let db = seeded_db();
        let created = create(&db, full_input()).unwrap();

        let by_13 = find_by_isbn(&db, "9789953210551").unwrap();
        assert_eq!(by_13.len(), 1);
        assert_eq!(by_13[0].id, created.id);
        assert_eq!(
            find_by_isbn(&db, "   ").unwrap(),
            vec![],
            "a blank probe matches nothing"
        );
    }

    #[test]
    fn the_duplicate_check_sees_isbn_and_title_plus_author() {
        let db = seeded_db();
        let ahmed = author_named(&db, "أحمد فكري");
        let other = author_named(&db, "نجيب سرور");
        let stored = create(
            &db,
            BookInput {
                authors: vec![credit(&ahmed)],
                ..full_input()
            },
        )
        .unwrap();

        // The title+author probes carry **no ISBN**, so what each case proves is what it says.
        let probe = |author: &maktabati_domain::Author, title: &str| BookInput {
            authors: vec![credit(author)],
            title: title.into(),
            ..Default::default()
        };

        // Same title, same author → the one stored book.
        let hits = find_duplicates(&db, &probe(&ahmed, "مقدمة في الحضارة الإسلامية"), None).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, stored.id);

        // Same title, different author → nothing.
        let hits = find_duplicates(&db, &probe(&other, "مقدمة في الحضارة الإسلامية"), None).unwrap();
        assert_eq!(hits, vec![]);

        // Same ISBN, unrelated title → still a hit: a mistyped title is not a new book.
        let hits = find_duplicates(
            &db,
            &BookInput {
                title: "عنوان آخر تماماً".into(),
                isbn13: Some("9789953210551".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);

        // Editing the stored book must not warn about the stored book.
        let hits = find_duplicates(
            &db,
            &probe(&ahmed, "مقدمة في الحضارة الإسلامية"),
            Some(&stored.id),
        )
        .unwrap();
        assert_eq!(hits, vec![]);
    }

    #[test]
    fn editing_can_move_a_book_to_another_category() {
        let db = seeded_db();
        let root = crate::repo::category::create(
            &db,
            CategoryInput {
                key: "poetry".into(),
                name_ar: "شعر".into(),
                name_en: "Poetry".into(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        let seeded = seeded_category_id(&db);
        let created = create(
            &db,
            BookInput {
                categories: vec![seeded.clone()],
                ..full_input()
            },
        )
        .unwrap();

        update(
            &db,
            &created.id,
            BookInput {
                categories: vec![root.id.clone()],
                ..full_input()
            },
        )
        .unwrap();

        let detail = detail(&db, &created.id).unwrap();
        assert_eq!(detail.categories, vec![root.id]);
    }
}
