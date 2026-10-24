//! `authors` — people credited on a book (author, translator, editor, illustrator, foreword).

use maktabati_domain::author::{Author, AuthorInput};

use crate::conn::Db;
use crate::Result;

/// Every `authors` column, in the order [`from_row`] reads them.
const SELECT: &str = "SELECT id, name, name_norm, name_original, name_original_norm, bio,
                             nationality, birth_year, death_year, photo_path,
                             created_at, updated_at
                      FROM authors";

/// Map one `authors` row onto an [`Author`].
fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Author> {
    Ok(Author {
        id: row.get(0)?,
        name: row.get(1)?,
        name_norm: row.get(2)?,
        name_original: row.get(3)?,
        name_original_norm: row.get(4)?,
        bio: row.get(5)?,
        nationality: row.get(6)?,
        birth_year: row.get(7)?,
        death_year: row.get(8)?,
        photo_path: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

/// Insert a new author and return the stored row.
///
/// # Errors
///
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when another author already holds this normalized
///   name (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn create(db: &Db, input: AuthorInput) -> Result<Author> {
    db.write(|conn| {
        let author = Author::new(input, crate::conn::now_ms())?;
        conn.execute(
            "INSERT INTO authors (id, name, name_norm, name_original, name_original_norm, bio,
                                  nationality, birth_year, death_year, photo_path,
                                  created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                author.id,
                author.name,
                author.name_norm,
                author.name_original,
                author.name_original_norm,
                author.bio,
                author.nationality,
                author.birth_year,
                author.death_year,
                author.photo_path,
                author.created_at,
                author.updated_at,
            ],
        )
        .map_err(|e| crate::error::on_unique("author", &author.name, e))?;
        Ok(author)
    })
}

/// Fetch one author by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no author has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get(db: &Db, id: &str) -> Result<Author> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE id = ?1"))?;
        let mut rows = stmt.query([id])?;
        match rows.next()? {
            None => Err(crate::error::not_found("author", id)),
            Some(row) => Ok(from_row(row)?),
        }
    })
}

/// List every author, ordered by normalized name.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an empty table yields an empty `Vec`.
pub fn list(db: &Db) -> Result<Vec<Author>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY name_norm ASC"))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Edit an existing author in place and return the stored row.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id does not exist (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when the new normalized name collides with another
///   author (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn update(db: &Db, id: &str, input: AuthorInput) -> Result<Author> {
    let mut author = get(db, id)?;
    db.write(|conn| {
        author.apply(input, crate::conn::now_ms())?;
        let changed = conn
            .execute(
                "UPDATE authors SET name = ?2, name_norm = ?3, name_original = ?4,
                    name_original_norm = ?5, bio = ?6, nationality = ?7, birth_year = ?8,
                    death_year = ?9, photo_path = ?10, updated_at = ?11
             WHERE id = ?1",
                rusqlite::params![
                    author.id,
                    author.name,
                    author.name_norm,
                    author.name_original,
                    author.name_original_norm,
                    author.bio,
                    author.nationality,
                    author.birth_year,
                    author.death_year,
                    author.photo_path,
                    author.updated_at,
                ],
            )
            .map_err(|e| crate::error::on_unique("author", &author.name, e))?;
        if changed == 0 {
            return Err(crate::error::not_found("author", id));
        }
        Ok(author)
    })
}

/// Delete one author by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no author has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        let removed = conn.execute("DELETE FROM authors WHERE id = ?1", [id])?;
        if removed == 0 {
            return Err(crate::error::not_found("author", id));
        }
        Ok(())
    })
}

/// Merge one author into another and delete the source.
///
/// Every credit on the source author moves to the target (`book_authors.author_id` is
/// rewritten); where the target already carries that exact `(book, role)` credit the
/// duplicate source row is dropped instead of crashing on the primary key. The source
/// author row itself goes with the merge — the books stay.
///
/// Self-merge is refused up front: `source == target` is a caller mistake, not a no-op,
/// and returning the target silently would make the click look like it did something.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when either author is unknown (`storage.not_found`).
/// * [`crate::StorageError::Domain`] with `validation.invalid_transition` when source equals target.
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn merge(db: &Db, source_id: &str, target_id: &str) -> Result<Author> {
    use maktabati_domain::DomainError;

    if source_id == target_id {
        return Err(DomainError::InvalidTransition {
            detail: format!("merging author {source_id} into itself"),
        }
        .into());
    }
    // Both rows must exist *before* anything moves, so a wrong id aborts instead of
    // leaving half the credits migrated (`NOT EXISTS` also covers the source's only row).
    let target = get(db, target_id)?;
    get(db, source_id)?;

    db.write(|conn| {
        // Credits the target already carries (same book, same role): the merge must not
        // crash on `(book_id, author_id, role)` primary keys — drop the stale duplicate.
        conn.execute(
            "DELETE FROM book_authors
              WHERE author_id = ?1
                AND EXISTS (SELECT 1 FROM book_authors keep
                             WHERE keep.book_id = book_authors.book_id
                               AND keep.author_id = ?2
                               AND keep.role = book_authors.role)",
            rusqlite::params![source_id, target_id],
        )?;
        conn.execute(
            "UPDATE book_authors SET author_id = ?2 WHERE author_id = ?1",
            rusqlite::params![source_id, target_id],
        )?;
        conn.execute("DELETE FROM authors WHERE id = ?1", [source_id])?;
        Ok(())
    })?;
    Ok(target)
}

/// Find an author by its normalized-name mirror.
///
/// `name_norm` is expected to already be normalized (`maktabati_domain::text::normalize`)
/// — this is an exact-match lookup, not a search.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an absent row is `Ok(None)`.
pub fn find_by_name_norm(db: &Db, name_norm: &str) -> Result<Option<Author>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE name_norm = ?1"))?;
        let mut rows = stmt.query([name_norm])?;
        match rows.next()? {
            None => Ok(None),
            Some(row) => Ok(Some(from_row(row)?)),
        }
    })
}

/// Search authors whose normalized name contains `query`.
///
/// The query is normalized first, so a typed «أحمد» matches a stored «احمد»; matching is a
/// case-insensitive-by-construction substring test against `name_norm`.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; a blank query or a non-positive `limit`
/// returns an empty `Vec` without touching the database.
pub fn search(db: &Db, query: &str, limit: i64) -> Result<Vec<Author>> {
    if limit <= 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let pattern = like_pattern(&maktabati_domain::text::normalize(query));
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE name_norm LIKE ?1 ESCAPE '\\' ORDER BY name_norm ASC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(rusqlite::params![pattern, limit], from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Build a `LIKE` pattern that matches `q` anywhere in a name.
///
/// `%`, `_` and `\` are LIKE metacharacters, so all three are escaped (the SQL uses
/// `ESCAPE '\'`) before the substring wildcards are wrapped around the query.
fn like_pattern(q: &str) -> String {
    let mut pattern = String::with_capacity(q.len() + 2);
    pattern.push('%');
    for c in q.chars() {
        if matches!(c, '\\' | '%' | '_') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageError;
    use pretty_assertions::assert_eq;

    fn test_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        crate::migrate::apply(&db, || Ok(())).unwrap();
        db
    }

    fn input(name: &str) -> AuthorInput {
        AuthorInput {
            name: name.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn create_then_get_round_trips_every_field() {
        let db = test_db();
        let created = create(
            &db,
            AuthorInput {
                name: "  أحمد فكري ".into(),
                name_original: Some("Ahmad Fakri".into()),
                bio: Some("مؤرخ مصري".into()),
                nationality: Some("مصر".into()),
                birth_year: Some(1930),
                death_year: Some(2010),
                photo_path: Some("covers/ahmad.webp".into()),
            },
        )
        .unwrap();

        assert!(created.created_at > 0);
        assert!(created.updated_at > 0);
        assert_eq!(created.name, "أحمد فكري");
        assert_eq!(created.name_norm, "احمد فكري");
        assert_eq!(created.name_original_norm.as_deref(), Some("ahmad fakri"));

        let got = get(&db, &created.id).unwrap();
        assert_eq!(got, created, "every stored field must survive the read");
    }

    #[test]
    fn list_returns_authors_in_name_norm_order() {
        let db = test_db();
        for name in ["charlie", "alpha", "bravo"] {
            create(&db, input(name)).unwrap();
        }
        let names: Vec<String> = list(&db).unwrap().into_iter().map(|a| a.name).collect();
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn update_edits_fields_but_keeps_id_and_created_at() {
        let db = test_db();
        let original = create(&db, input("Tolstoy")).unwrap();
        let updated = update(
            &db,
            &original.id,
            AuthorInput {
                name: "Leo Tolstoy".into(),
                bio: Some("روايات وقصص".into()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.created_at, original.created_at);
        assert!(updated.updated_at >= original.updated_at);
        assert_eq!(updated.name, "Leo Tolstoy");
        assert_eq!(updated.name_norm, "leo tolstoy");
        assert_eq!(updated.bio.as_deref(), Some("روايات وقصص"));
        assert_eq!(get(&db, &original.id).unwrap(), updated);
    }

    #[test]
    fn delete_removes_the_row_and_a_second_delete_is_not_found() {
        let db = test_db();
        let a = create(&db, input("Gone")).unwrap();

        delete(&db, &a.id).unwrap();
        assert!(matches!(
            get(&db, &a.id),
            Err(StorageError::NotFound { .. })
        ));

        let err = delete(&db, &a.id).unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
    }

    #[test]
    fn a_duplicate_normalized_name_is_rejected() {
        let db = test_db();
        create(&db, input("أحمد فكري")).unwrap();

        // A whitespace variant normalizes to the same mirror, so this collides — and the
        // error carries the name the user typed, not the folded mirror they never saw.
        let typed = "احمد  فكري";
        let err = create(&db, input(typed)).unwrap_err();
        assert_eq!(err.code(), "storage.duplicate");
        assert!(matches!(&err, StorageError::Duplicate { kind, key }
                if kind == "author" && key == typed));
    }

    #[test]
    fn a_blank_name_surfaces_as_a_validation_error() {
        let db = test_db();
        let err = create(&db, input("   ")).unwrap_err();
        assert_eq!(err.code(), "validation.empty_field");
    }

    #[test]
    fn search_matches_by_normalized_query() {
        let db = test_db();
        create(&db, input("أحمد فكري")).unwrap();
        create(&db, input("نجيب محفوظ")).unwrap();

        let hits = search(&db, "احمد", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "أحمد فكري");

        assert_eq!(search(&db, "احمد", 0).unwrap(), Vec::new());
        assert_eq!(search(&db, "   ", 10).unwrap(), Vec::new());
    }

    #[test]
    fn like_pattern_escapes_metacharacters_and_matches_substrings() {
        assert_eq!(like_pattern("احمد"), "%احمد%");
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        assert_eq!(like_pattern("back\\slash"), "%back\\\\slash%");
    }

    // ── merge ─────────────────────────────────────────────────────────────────

    /// Two authors plus three books credited through `book_authors`, so the merge has real
    /// junction rows to move: one book credited to the source alone, one credited to both
    /// (the primary-key clash case), one to the target alone.
    fn seeded_authors_and_books(db: &Db) -> (String, String, String, String, String) {
        use maktabati_domain::book::{AuthorCredit, BookInput};
        use maktabati_domain::enums::{AuthorRole, Direction};
        use maktabati_domain::language::LanguageInput;

        let source = create(&db, input("أحمد فكري")).unwrap();
        let target = create(&db, input("عبد الله")).unwrap();

        // `book::create` validates the language code must exist; the fixture needs one.
        crate::repo::language::create(
            &db,
            LanguageInput {
                code: "ar".to_owned(),
                name_ar: "العربية".to_owned(),
                name_en: "Arabic".to_owned(),
                direction: Direction::Rtl,
                position: 0,
                ..Default::default()
            },
        )
        .unwrap();

        let make_book = |title: &str, credit: AuthorCredit| {
            crate::repo::book::create(
                &db,
                BookInput {
                    title: title.to_owned(),
                    authors: vec![credit],
                    ..Default::default()
                },
            )
            .unwrap()
            .id
        };

        let only_source = make_book(
            "الكتاب الأول",
            AuthorCredit {
                author_id: source.id.clone(),
                role: AuthorRole::Author,
            },
        );
        let shared = make_book(
            "الكتاب الثاني",
            AuthorCredit {
                author_id: source.id.clone(),
                role: AuthorRole::Author,
            },
        );
        let only_target = make_book(
            "الكتاب الثالث",
            AuthorCredit {
                author_id: target.id.clone(),
                role: AuthorRole::Author,
            },
        );
        (source.id, target.id, only_source, shared, only_target)
    }

    #[test]
    fn merge_moves_credits_deduplicates_and_removes_the_source() {
        let db = test_db();
        let (source, target, only_source, shared, only_target) = seeded_authors_and_books(&db);

        let merged = merge(&db, &source, &target).unwrap();
        assert_eq!(merged.id, target);

        // The source author is gone; the target keeps its row intact.
        assert!(matches!(
            get(&db, &source).unwrap_err(),
            StorageError::NotFound { .. }
        ));
        assert_eq!(get(&db, &target).unwrap().name, "عبد الله");

        // Every book keeps exactly one credit row, whose author is the target — including
        // the book the two authors shared, which would have crashed on the primary key.
        let book_credits = |book_id: &str| -> Vec<String> {
            db.write(|conn| {
                let mut stmt =
                    conn.prepare("SELECT author_id FROM book_authors WHERE book_id = ?1")?;
                let rows = stmt
                    .query_map([book_id], |r| r.get(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .unwrap()
        };
        for book in [&only_source, &shared, &only_target] {
            assert_eq!(book_credits(book), vec![target.clone()], "book {book}");
        }
    }

    #[test]
    fn merge_refuses_self_merge_and_unknown_authors() {
        let db = test_db();
        let (source, target, only_source, shared, _) = seeded_authors_and_books(&db);

        let err = merge(&db, &source, &source).unwrap_err();
        assert_eq!(err.code(), "validation.invalid_transition");

        let err = merge(&db, "no-such-author", &target).unwrap_err();
        assert!(matches!(err, StorageError::NotFound { .. }));

        // Refused merges moved nothing: both authors still exist with their credits.
        assert!(get(&db, &source).is_ok());
        assert!(get(&db, &target).is_ok());
        let credits = |book_id: &str| -> Vec<String> {
            db.write(|conn| {
                let mut stmt =
                    conn.prepare("SELECT author_id FROM book_authors WHERE book_id = ?1")?;
                let rows = stmt
                    .query_map([book_id], |r| r.get(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .unwrap()
        };
        assert_eq!(credits(&only_source), vec![source.clone()]);
        assert_eq!(credits(&shared), vec![source.clone()]);
    }
}
