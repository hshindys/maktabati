//! `languages` — reference rows for the languages a book can be written in.
//!
//! The primary key is the `code` itself, and [`Language::apply`] deliberately never changes
//! it: every book's `language_code` points at this value.

use maktabati_domain::language::{Language, LanguageInput};
use maktabati_domain::Direction;

use crate::conn::Db;
use crate::Result;

/// Every `languages` column, in the order [`from_row`] reads them.
const SELECT: &str = "SELECT code, name_ar, name_en, direction, is_active, position
                      FROM languages";

/// Map one `languages` row onto a [`Language`].
fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Language> {
    let direction: String = row.get(3)?;
    Ok(Language {
        code: row.get(0)?,
        name_ar: row.get(1)?,
        name_en: row.get(2)?,
        // The SQL CHECK constraint only ever stores 'rtl'/'ltr', so `from_db` succeeds for
        // every row this schema can hold; the fallback covers a value written by a *newer*
        // build and keeps reads total — Ltr is the safe rendering default.
        direction: Direction::from_db(&direction).unwrap_or(Direction::Ltr),
        // `is_active` is an INTEGER 0/1 column: read it as i64 rather than relying on
        // rusqlite's bool FromSql.
        is_active: row.get::<_, i64>(4)? != 0,
        position: row.get(5)?,
    })
}

/// Insert a new language and return the stored row.
///
/// # Errors
///
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when the code is already taken (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn create(db: &Db, input: LanguageInput) -> Result<Language> {
    db.write(|conn| {
        let language = Language::new(input)?;
        conn.execute(
            "INSERT INTO languages (code, name_ar, name_en, direction, is_active, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                language.code,
                language.name_ar,
                language.name_en,
                language.direction.as_str(),
                i64::from(language.is_active),
                language.position,
            ],
        )
        .map_err(|e| crate::error::on_unique("language", &language.code, e))?;
        Ok(language)
    })
}

/// Fetch one language by its code.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no language has this code (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get(db: &Db, code: &str) -> Result<Language> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE code = ?1"))?;
        let mut rows = stmt.query([code])?;
        match rows.next()? {
            None => Err(crate::error::not_found("language", code)),
            Some(row) => Ok(from_row(row)?),
        }
    })
}

/// List every language in display order (`position`, then `code`).
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an empty table yields an empty `Vec`.
pub fn list(db: &Db) -> Result<Vec<Language>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY position ASC, code ASC"))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// List the languages shown in pickers (`is_active = 1`), in display order.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; no active rows yield an empty `Vec`.
pub fn list_active(db: &Db) -> Result<Vec<Language>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE is_active = 1 ORDER BY position ASC, code ASC"
        ))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Edit an existing language in place and return the stored row.
///
/// The `code` in `input` is validated but ignored — the primary key can never change.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the code does not exist (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn update(db: &Db, code: &str, input: LanguageInput) -> Result<Language> {
    let mut language = get(db, code)?;
    db.write(|conn| {
        language.apply(input)?;
        let changed = conn
            .execute(
                "UPDATE languages SET name_ar = ?2, name_en = ?3, direction = ?4,
                    is_active = ?5, position = ?6
             WHERE code = ?1",
                rusqlite::params![
                    language.code,
                    language.name_ar,
                    language.name_en,
                    language.direction.as_str(),
                    i64::from(language.is_active),
                    language.position,
                ],
            )
            .map_err(|e| crate::error::on_unique("language", &language.code, e))?;
        if changed == 0 {
            return Err(crate::error::not_found("language", code));
        }
        Ok(language)
    })
}

/// Delete one language by code.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no language has this code (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure — a language still referenced by a
///   book fails with `storage.foreign_key_violation` (the column has no `ON DELETE` action).
pub fn delete(db: &Db, code: &str) -> Result<()> {
    db.write(|conn| {
        let removed = conn.execute("DELETE FROM languages WHERE code = ?1", [code])?;
        if removed == 0 {
            return Err(crate::error::not_found("language", code));
        }
        Ok(())
    })
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

    /// A valid input for `code`, with every field the validator requires filled in.
    fn base(code: &str) -> LanguageInput {
        LanguageInput {
            code: code.to_owned(),
            name_ar: "اللغة".to_owned(),
            name_en: "Language".to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn create_then_get_round_trips_every_field_including_direction() {
        let db = test_db();
        let created = create(
            &db,
            LanguageInput {
                code: " ar ".into(),
                name_ar: " العربية ".into(),
                name_en: "Arabic".into(),
                direction: Direction::Rtl,
                is_active: true,
                position: 7,
            },
        )
        .unwrap();

        assert_eq!(created.code, "ar", "the code is trimmed");
        assert_eq!(created.name_ar, "العربية");
        assert_eq!(created.direction, Direction::Rtl);

        let got = get(&db, "ar").unwrap();
        assert_eq!(got, created, "every stored field must survive the read");
        assert_eq!(got.direction, Direction::Rtl, "rtl must round-trip");
    }

    #[test]
    fn list_orders_by_position_then_code() {
        let db = test_db();
        for input in [
            LanguageInput {
                position: 5,
                ..base("fr")
            },
            LanguageInput {
                position: 1,
                ..base("ar")
            },
            LanguageInput {
                position: 1,
                ..base("en")
            },
        ] {
            create(&db, input).unwrap();
        }

        let codes: Vec<String> = list(&db).unwrap().into_iter().map(|l| l.code).collect();
        assert_eq!(codes, vec!["ar", "en", "fr"]);
    }

    #[test]
    fn list_active_filters_out_inactive_rows() {
        let db = test_db();
        create(&db, base("ar")).unwrap();
        create(
            &db,
            LanguageInput {
                is_active: false,
                ..base("fr")
            },
        )
        .unwrap();

        let active: Vec<String> = list_active(&db)
            .unwrap()
            .into_iter()
            .map(|l| l.code)
            .collect();
        assert_eq!(active, vec!["ar"]);
        assert_eq!(list(&db).unwrap().len(), 2, "list() must include every row");
    }

    #[test]
    fn update_edits_fields_but_cannot_change_the_code() {
        let db = test_db();
        create(
            &db,
            LanguageInput {
                name_en: "Arabic".into(),
                ..base("ar")
            },
        )
        .unwrap();

        let updated = update(
            &db,
            "ar",
            LanguageInput {
                code: "fr".into(),
                name_en: "French".into(),
                position: 9,
                ..base("ar")
            },
        )
        .unwrap();

        assert_eq!(updated.code, "ar", "the primary key must stay put");
        assert_eq!(updated.name_en, "French");
        assert_eq!(updated.position, 9);
        assert_eq!(get(&db, "ar").unwrap().name_en, "French");
        assert!(matches!(get(&db, "fr"), Err(StorageError::NotFound { .. })));
    }

    #[test]
    fn delete_removes_the_row_and_a_second_delete_is_not_found() {
        let db = test_db();
        create(&db, base("ar")).unwrap();

        delete(&db, "ar").unwrap();
        assert!(matches!(get(&db, "ar"), Err(StorageError::NotFound { .. })));

        let err = delete(&db, "ar").unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
    }

    #[test]
    fn a_duplicate_code_is_rejected() {
        let db = test_db();
        create(&db, base("ar")).unwrap();

        let err = create(
            &db,
            LanguageInput {
                name_en: "Arabic again".into(),
                ..base("ar")
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "storage.duplicate");
        assert!(matches!(&err, StorageError::Duplicate { kind, key }
                if kind == "language" && key == "ar"));
    }

    #[test]
    fn a_blank_code_surfaces_as_a_validation_error() {
        let db = test_db();
        let err = create(
            &db,
            LanguageInput {
                code: "  ".into(),
                ..base("ar")
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "validation.empty_field");
    }
}
