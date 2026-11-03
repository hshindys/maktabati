//! Write-time search projections and the one-time backfill for the FTS5 schema.

use maktabati_domain::text::normalize;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::{conn::Db, Result};

struct BookProjection {
    title: String,
    title_norm: String,
    subtitle: Option<String>,
    original_title: Option<String>,
    authors: String,
    authors_norm: String,
    publisher: Option<String>,
    publisher_norm: String,
    series: Option<String>,
    series_norm: String,
    tags: String,
    tags_norm: String,
    categories: String,
    categories_norm: String,
    isbn: String,
    description: Option<String>,
    identifiers: Option<String>,
    year: Option<i64>,
    format: String,
    status: String,
    language: String,
    updated_at: i64,
}

fn read_book(conn: &Connection, book_id: &str) -> Result<BookProjection> {
    Ok(conn.query_row(
        "SELECT b.title, b.title_norm, b.subtitle, b.original_title,
                COALESCE((SELECT group_concat(name, ' ') FROM (
                    SELECT a.name AS name FROM book_authors ba
                    JOIN authors a ON a.id = ba.author_id
                    WHERE ba.book_id = b.id ORDER BY ba.position
                )), ''),
                COALESCE((SELECT group_concat(name_norm, ' ') FROM (
                    SELECT a.name_norm AS name_norm FROM book_authors ba
                    JOIN authors a ON a.id = ba.author_id
                    WHERE ba.book_id = b.id ORDER BY ba.position
                )), ''),
                p.name, COALESCE(p.name_norm, ''),
                s.name, COALESCE(s.name_norm, ''),
                COALESCE((SELECT group_concat(name, ' ') FROM (
                    SELECT t.name AS name FROM book_tags bt JOIN tags t ON t.id = bt.tag_id
                    WHERE bt.book_id = b.id ORDER BY t.name_norm
                )), ''),
                COALESCE((SELECT group_concat(name_norm, ' ') FROM (
                    SELECT t.name_norm AS name_norm FROM book_tags bt
                    JOIN tags t ON t.id = bt.tag_id
                    WHERE bt.book_id = b.id ORDER BY t.name_norm
                )), ''),
                COALESCE((SELECT group_concat(name, ' ') FROM (
                    SELECT c.name_ar || ' ' || c.name_en AS name
                    FROM book_categories bc JOIN categories c ON c.id = bc.category_id
                    WHERE bc.book_id = b.id ORDER BY bc.position
                )), ''),
                COALESCE((SELECT group_concat(name_norm, ' ') FROM (
                    SELECT c.name_norm AS name_norm
                    FROM book_categories bc JOIN categories c ON c.id = bc.category_id
                    WHERE bc.book_id = b.id ORDER BY bc.position
                )), ''),
                trim(COALESCE(b.isbn10, '') || ' ' || COALESCE(b.isbn13, '')),
                b.description, b.external_ids, b.publication_year, b.format,
                b.reading_status, b.language_code, b.updated_at
         FROM books b
         LEFT JOIN publishers p ON p.id = b.publisher_id
         LEFT JOIN series s ON s.id = b.series_id
         WHERE b.id = ?1",
        [book_id],
        |row| {
            Ok(BookProjection {
                title: row.get(0)?,
                title_norm: row.get(1)?,
                subtitle: row.get(2)?,
                original_title: row.get(3)?,
                authors: row.get(4)?,
                authors_norm: row.get(5)?,
                publisher: row.get(6)?,
                publisher_norm: row.get(7)?,
                series: row.get(8)?,
                series_norm: row.get(9)?,
                tags: row.get(10)?,
                tags_norm: row.get(11)?,
                categories: row.get(12)?,
                categories_norm: row.get(13)?,
                isbn: row.get(14)?,
                description: row.get(15)?,
                identifiers: row.get(16)?,
                year: row.get(17)?,
                format: row.get(18)?,
                status: row.get(19)?,
                language: row.get(20)?,
                updated_at: row.get(21)?,
            })
        },
    )?)
}

/// Rebuild one book's normalized projection in the caller's transaction.
pub(crate) fn sync_book(conn: &Connection, book_id: &str) -> Result<()> {
    let doc = read_book(conn, book_id)?;
    let isbn_norm = normalize(&doc.isbn);
    let description_norm = normalize(doc.description.as_deref().unwrap_or_default());
    let identifiers_norm = normalize(doc.identifiers.as_deref().unwrap_or_default());
    let subtitle_norm = normalize(doc.subtitle.as_deref().unwrap_or_default());
    let original_title_norm = normalize(doc.original_title.as_deref().unwrap_or_default());

    conn.execute(
        "INSERT INTO search_document (
            book_id, title, title_norm, subtitle, original_title, authors, authors_norm,
            publisher, series, tags, categories, isbn, description, identifiers, year, format,
            status, language, tags_norm, publisher_norm, categories_norm, description_norm,
            identifiers_norm, updated_at, subtitle_norm, original_title_norm, series_norm,
            isbn_norm
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
            ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28
         )
         ON CONFLICT(book_id) DO UPDATE SET
            title = excluded.title, title_norm = excluded.title_norm,
            subtitle = excluded.subtitle, original_title = excluded.original_title,
            authors = excluded.authors, authors_norm = excluded.authors_norm,
            publisher = excluded.publisher, series = excluded.series, tags = excluded.tags,
            categories = excluded.categories, isbn = excluded.isbn,
            description = excluded.description, identifiers = excluded.identifiers,
            year = excluded.year, format = excluded.format, status = excluded.status,
            language = excluded.language, tags_norm = excluded.tags_norm,
            publisher_norm = excluded.publisher_norm, categories_norm = excluded.categories_norm,
            description_norm = excluded.description_norm,
            identifiers_norm = excluded.identifiers_norm, updated_at = excluded.updated_at,
            subtitle_norm = excluded.subtitle_norm,
            original_title_norm = excluded.original_title_norm, series_norm = excluded.series_norm,
            isbn_norm = excluded.isbn_norm",
        params![
            book_id,
            doc.title,
            doc.title_norm,
            doc.subtitle,
            doc.original_title,
            doc.authors,
            doc.authors_norm,
            doc.publisher,
            doc.series,
            doc.tags,
            doc.categories,
            doc.isbn,
            doc.description,
            doc.identifiers,
            doc.year,
            doc.format,
            doc.status,
            doc.language,
            doc.tags_norm,
            doc.publisher_norm,
            doc.categories_norm,
            description_norm,
            identifiers_norm,
            doc.updated_at,
            subtitle_norm,
            original_title_norm,
            doc.series_norm,
            isbn_norm,
        ],
    )?;
    Ok(())
}

fn collect_text(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) if !text.trim().is_empty() => out.push(text.clone()),
        Value::Array(values) => values.iter().for_each(|value| collect_text(value, out)),
        Value::Object(values) => values.values().for_each(|value| collect_text(value, out)),
        _ => {}
    }
}

/// Rebuild the full-page note projection in the caller's transaction.
pub(crate) fn sync_note_page(conn: &Connection, page_id: &str) -> Result<()> {
    let page: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT title, book_id FROM notes_page WHERE id = ?1",
            [page_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((title, book_id)) = page else {
        return Ok(());
    };

    let blocks = {
        let mut stmt = conn.prepare(
            "SELECT payload FROM note_block WHERE page_id = ?1
             ORDER BY CAST(position AS INTEGER), id",
        )?;
        let rows = stmt.query_map([page_id], |row| row.get::<_, String>(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    conn.execute("DELETE FROM search_note_doc WHERE page_id = ?1", [page_id])?;
    let mut parts = vec![title];
    for payload in blocks {
        let value: Value = serde_json::from_str(&payload)?;
        collect_text(&value, &mut parts);
    }
    let text = parts.join(" ");
    conn.execute(
        "INSERT INTO search_note_doc (page_id, book_id, text, text_norm)
         VALUES (?1, ?2, ?3, ?4)",
        params![page_id, book_id, text, normalize(&text)],
    )?;
    Ok(())
}

fn note_page_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM notes_page ORDER BY id")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn book_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM books ORDER BY id")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// Reindex projections once after migration 0004, and rebuild the FTS tables transactionally.
///
/// The migration creates the schema and FTS triggers. This Rust backfill applies the domain's
/// Arabic normalization to records that predate those triggers, then records completion.
pub fn reindex_if_needed(db: &Db) -> Result<bool> {
    db.transact(|conn| {
        let indexed_version: Option<i64> = conn
            .query_row(
                "SELECT version FROM search_index_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if indexed_version.is_some_and(|version| version >= 1) {
            return Ok(false);
        }

        for table in [
            "fts_book",
            "fts_book_norm",
            "fts_note",
            "fts_quote",
            "fts_author",
        ] {
            conn.execute(
                &format!("INSERT INTO {table}({table}) VALUES ('rebuild')"),
                [],
            )?;
        }
        conn.execute("DELETE FROM search_document", [])?;
        conn.execute("DELETE FROM search_note_doc", [])?;

        for id in book_ids(conn)? {
            sync_book(conn, &id)?;
        }
        for id in note_page_ids(conn)? {
            sync_note_page(conn, &id)?;
        }

        let quotes = {
            let mut stmt = conn.prepare("SELECT id, note FROM quotes ORDER BY id")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        for (id, note) in quotes {
            conn.execute(
                "UPDATE quotes SET note_norm = ?2 WHERE id = ?1",
                params![id, normalize(note.as_deref().unwrap_or_default())],
            )?;
        }

        for table in [
            "fts_book",
            "fts_book_norm",
            "fts_note",
            "fts_quote",
            "fts_author",
        ] {
            conn.execute(
                &format!("INSERT INTO {table}({table}) VALUES ('rebuild')"),
                [],
            )?;
        }
        conn.execute(
            "INSERT INTO search_index_state (id, version) VALUES (1, 1)
             ON CONFLICT(id) DO UPDATE SET version = excluded.version",
            [],
        )?;
        Ok(true)
    })
}
