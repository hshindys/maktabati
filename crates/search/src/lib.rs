//! Unified search over the library's SQLite FTS5 projections.

use maktabati_domain::text::normalize;
use maktabati_storage::{repo, Db, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Search result kinds returned to the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SearchResultKind {
    Book,
    Note,
    Quote,
    Author,
}

/// A compact, presentation-neutral result from one of the searchable library entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SearchResult {
    /// Entity type.
    pub kind: SearchResultKind,
    /// Stable entity id (note results use the page id).
    pub id: String,
    /// Main display text.
    pub title: String,
    /// Secondary display text such as author, book title, or note excerpt.
    pub detail: Option<String>,
    /// Owning book id for notes and quotes.
    pub book_id: Option<String>,
    /// Reading status for book results.
    pub status: Option<String>,
}

/// Escape a normalized query into literal FTS5 terms joined with AND.
fn match_expression(query: &str) -> String {
    normalize(query)
        .split_whitespace()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Search all library entities. Empty input returns the most recently added books.
///
/// The returned count is capped at 50 so a caller cannot accidentally materialize the
/// whole library for every keystroke.
pub fn search(db: &Db, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let limit = limit.min(50);
    if limit == 0 {
        return Ok(Vec::new());
    }
    let expression = match_expression(query);
    let books = repo::book::search(db, if expression.is_empty() { "" } else { query }, limit)?;
    let book_results: Vec<SearchResult> = books
        .into_iter()
        .map(|book| SearchResult {
            kind: SearchResultKind::Book,
            id: book.id,
            title: book.title,
            detail: None,
            book_id: None,
            status: Some(book.reading_status.as_str().to_owned()),
        })
        .collect();

    if expression.is_empty() {
        return Ok(book_results);
    }

    let (note_results, quote_results, author_results) = db.read(|conn| {
        let notes = {
            let mut stmt = conn.prepare(
                "SELECT d.page_id, p.title, d.text, d.book_id
                 FROM fts_note f
                 JOIN search_note_doc d ON d.rowid = f.rowid
                 JOIN notes_page p ON p.id = d.page_id
                 WHERE fts_note MATCH ?1
                 ORDER BY bm25(fts_note), p.updated_at DESC, d.page_id
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![expression, limit as i64], |row| {
                Ok(SearchResult {
                    kind: SearchResultKind::Note,
                    id: row.get(0)?,
                    title: row.get(1)?,
                    detail: Some(row.get(2)?),
                    book_id: row.get(3)?,
                    status: None,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        let quotes = {
            let mut stmt = conn.prepare(
                "SELECT q.id, q.text, b.title, q.book_id
                 FROM fts_quote f
                 JOIN quotes q ON q.rowid = f.rowid
                 JOIN books b ON b.id = q.book_id
                 WHERE fts_quote MATCH ?1
                 ORDER BY bm25(fts_quote), q.updated_at DESC, q.id
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![expression, limit as i64], |row| {
                Ok(SearchResult {
                    kind: SearchResultKind::Quote,
                    id: row.get(0)?,
                    title: row.get(1)?,
                    detail: Some(row.get(2)?),
                    book_id: Some(row.get(3)?),
                    status: None,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        let authors = {
            let mut stmt = conn.prepare(
                "SELECT a.id, a.name, a.name_original
                 FROM fts_author f
                 JOIN authors a ON a.rowid = f.rowid
                 WHERE fts_author MATCH ?1
                 ORDER BY bm25(fts_author), a.name_norm, a.id
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![expression, limit as i64], |row| {
                Ok(SearchResult {
                    kind: SearchResultKind::Author,
                    id: row.get(0)?,
                    title: row.get(1)?,
                    detail: row.get(2)?,
                    book_id: None,
                    status: None,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        Ok((notes, quotes, authors))
    })?;

    let groups = [book_results, note_results, quote_results, author_results];
    let mut results = Vec::with_capacity(limit);
    for rank in 0..limit {
        let before = results.len();
        for group in &groups {
            if let Some(result) = group.get(rank) {
                results.push(result.clone());
                if results.len() == limit {
                    break;
                }
            }
        }
        if results.len() == before {
            break;
        }
    }
    Ok(results)
}

/// Search only books, preserving the Hermes `/books?query=` response shape.
pub fn search_books(db: &Db, query: &str, limit: usize) -> Result<Vec<maktabati_domain::Book>> {
    repo::book::search(db, query, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maktabati_domain::{
        AuthorCredit, AuthorInput, AuthorRole, BookInput, Json, NoteBlockInput, NoteBlockType,
        NotePageInput,
    };
    use std::time::Instant;

    fn test_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("search.db")).unwrap();
        maktabati_storage::migrate::apply(&db, || Ok(())).unwrap();
        db.write(maktabati_storage::seed::apply).unwrap();
        maktabati_storage::repo::search_index::reindex_if_needed(&db).unwrap();
        db
    }

    fn add_book(db: &Db, title: &str, author_id: Option<String>) -> String {
        let input = BookInput {
            title: title.to_owned(),
            authors: author_id
                .map(|author_id| {
                    vec![AuthorCredit {
                        author_id,
                        role: AuthorRole::Author,
                    }]
                })
                .unwrap_or_default(),
            ..BookInput::default()
        };
        maktabati_storage::repo::book::create(db, input).unwrap().id
    }

    #[test]
    fn searches_arabic_normalization_and_mixed_text() {
        let db = test_db();
        let id = add_book(&db, "مُقَدِّمَة في Maktabati", None);

        let results = search_books(&db, "مقدمة في maktabati", 10).unwrap();
        assert_eq!(
            results
                .iter()
                .map(|book| book.id.as_str())
                .collect::<Vec<_>>(),
            [id.as_str()]
        );

        let folded = add_book(&db, "مدرسة عربية", None);
        assert_eq!(search_books(&db, "مُدَرِّسَة", 10).unwrap()[0].id, folded);
        let folded = add_book(&db, "إِلَى هُدَى", None);
        assert_eq!(search_books(&db, "الي هدي", 10).unwrap()[0].id, folded);
    }

    #[test]
    fn searches_notes_quotes_and_authors() {
        let db = test_db();
        let author = maktabati_storage::repo::author::create(
            &db,
            AuthorInput {
                name: "ليلى الكاتبة".to_owned(),
                ..AuthorInput::default()
            },
        )
        .unwrap();
        let book_id = add_book(&db, "كتاب الاختبار", Some(author.id.clone()));
        let page = maktabati_storage::repo::note::create_page(
            &db,
            NotePageInput {
                book_id: Some(book_id.clone()),
                title: "ملاحظات".to_owned(),
                ..NotePageInput::default()
            },
        )
        .unwrap();
        let block = maktabati_storage::repo::note::insert_block(
            &db,
            &page.id,
            None,
            NoteBlockInput {
                block_type: NoteBlockType::Paragraph,
                payload: Json::from(serde_json::json!({"text": "النص المخفي"})),
            },
        )
        .unwrap();
        let quote = maktabati_storage::repo::quote::create(
            &db,
            &book_id,
            maktabati_domain::QuoteInput {
                text: "حكمة نافعة".to_owned(),
                note: Some("على الهامش".to_owned()),
                ..maktabati_domain::QuoteInput::default()
            },
        )
        .unwrap();

        assert!(search(&db, "ليلى", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Author));
        assert!(search(&db, "النص المخفي", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Note));
        assert!(search(&db, "ملاحظات", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Note));
        assert!(search(&db, "حكمه نافعه", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Quote));
        assert!(search(&db, "الهامش", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Quote));

        maktabati_storage::repo::note::update_block(
            &db,
            &block.id,
            NoteBlockInput {
                block_type: NoteBlockType::Paragraph,
                payload: Json::from(serde_json::json!({"text": "نص مختلف"})),
            },
        )
        .unwrap();
        assert!(search(&db, "نص مختلف", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Note));
        assert!(!search(&db, "النص المخفي", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Note));

        maktabati_storage::repo::quote::update(
            &db,
            &quote.id,
            maktabati_domain::QuoteInput {
                text: "حكمة مميزة".to_owned(),
                ..maktabati_domain::QuoteInput::default()
            },
        )
        .unwrap();
        assert!(!search(&db, "حكمه نافعه", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Quote));

        maktabati_storage::repo::author::update(
            &db,
            &author.id,
            AuthorInput {
                name: "سلمى الكاتبة".to_owned(),
                ..AuthorInput::default()
            },
        )
        .unwrap();
        assert_eq!(search_books(&db, "سلمى", 10).unwrap().len(), 1);
        assert!(!search(&db, "ليلى", 10)
            .unwrap()
            .iter()
            .any(|result| result.kind == SearchResultKind::Author));

        maktabati_storage::repo::book::update(
            &db,
            &book_id,
            BookInput {
                title: "كتاب محدث".to_owned(),
                authors: vec![AuthorCredit {
                    author_id: author.id.clone(),
                    role: AuthorRole::Author,
                }],
                ..BookInput::default()
            },
        )
        .unwrap();
        assert_eq!(search_books(&db, "كتاب محدث", 10).unwrap()[0].id, book_id);
        assert!(!search_books(&db, "كتاب الاختبار", 10)
            .unwrap()
            .iter()
            .any(|book| book.id == book_id));
        maktabati_storage::repo::author::delete(&db, &author.id).unwrap();
        assert!(search_books(&db, "سلمى", 10).unwrap().is_empty());
    }

    #[test]
    fn one_query_returns_all_matching_entity_kinds() {
        let db = test_db();
        let author = maktabati_storage::repo::author::create(
            &db,
            AuthorInput {
                name: "مكتبة المؤلف".to_owned(),
                ..AuthorInput::default()
            },
        )
        .unwrap();
        let book_id = add_book(&db, "مكتبة مفيدة", Some(author.id.clone()));
        let page = maktabati_storage::repo::note::create_page(
            &db,
            NotePageInput {
                book_id: Some(book_id.clone()),
                title: "مكتبة الملاحظات".to_owned(),
                ..NotePageInput::default()
            },
        )
        .unwrap();
        maktabati_storage::repo::note::insert_block(
            &db,
            &page.id,
            None,
            NoteBlockInput {
                block_type: NoteBlockType::Paragraph,
                payload: Json::from(serde_json::json!({"text": "مكتبة شخصية"})),
            },
        )
        .unwrap();
        maktabati_storage::repo::quote::create(
            &db,
            &book_id,
            maktabati_domain::QuoteInput {
                text: "مكتبة وقراءة".to_owned(),
                ..maktabati_domain::QuoteInput::default()
            },
        )
        .unwrap();

        let kinds: Vec<_> = search(&db, "مكتبة", 8)
            .unwrap()
            .into_iter()
            .map(|result| result.kind)
            .collect();
        for kind in [
            SearchResultKind::Book,
            SearchResultKind::Note,
            SearchResultKind::Quote,
            SearchResultKind::Author,
        ] {
            assert!(kinds.contains(&kind), "missing {kind:?} from {kinds:?}");
        }
    }

    #[test]
    fn empty_query_returns_recent_books_and_limit_is_bounded() {
        let db = test_db();
        for i in 0..55 {
            add_book(&db, &format!("كتاب {i}"), None);
        }
        let results = search(&db, "   ", 1000).unwrap();
        assert_eq!(results.len(), 50);
        assert!(results
            .iter()
            .all(|result| result.kind == SearchResultKind::Book));
        assert!(search(&db, "كتاب", 0).unwrap().is_empty());
    }

    #[test]
    fn five_thousand_books_search_within_one_hundred_milliseconds() {
        let db = test_db();
        db.transact(|conn| {
            for i in 0..5_000 {
                conn.execute(
                    "INSERT INTO books (id, title, title_norm, language_code, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'ar', ?4, ?4)",
                    rusqlite::params![
                        format!("book-{i}"),
                        format!("مكتبة رقم {i}"),
                        normalize(&format!("مكتبة رقم {i}")),
                        i
                    ],
                )?;
            }
            Ok(())
        })
        .unwrap();
        db.write(|conn| {
            conn.execute("UPDATE search_index_state SET version = 0 WHERE id = 1", [])?;
            Ok(())
        })
        .unwrap();
        maktabati_storage::repo::search_index::reindex_if_needed(&db).unwrap();

        let started = Instant::now();
        let found = search_books(&db, "مكتبة", 50).unwrap();
        assert_eq!(found.len(), 50);
        assert!(
            started.elapsed().as_millis() < 100,
            "5k-book FTS query took {:?}",
            started.elapsed()
        );
    }
}
