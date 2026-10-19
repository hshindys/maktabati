//! `series` — book series (السلسلة); a book belongs to at most one series.

use maktabati_domain::series::{Series, SeriesInput};

use crate::conn::Db;
use crate::Result;

/// Every `series` column, in the order [`from_row`] reads them.
const SELECT: &str = "SELECT id, name, name_norm, description, created_at, updated_at
                      FROM series";

/// Map one `series` row onto a [`Series`].
fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Series> {
    Ok(Series {
        id: row.get(0)?,
        name: row.get(1)?,
        name_norm: row.get(2)?,
        description: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

/// Insert a new series and return the stored row.
///
/// # Errors
///
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when another series already holds this normalized
///   name (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn create(db: &Db, input: SeriesInput) -> Result<Series> {
    db.write(|conn| {
        let series = Series::new(input, crate::conn::now_ms())?;
        conn.execute(
            "INSERT INTO series (id, name, name_norm, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                series.id,
                series.name,
                series.name_norm,
                series.description,
                series.created_at,
                series.updated_at,
            ],
        )
        .map_err(|e| crate::error::on_unique("series", &series.name, e))?;
        Ok(series)
    })
}

/// Fetch one series by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no series has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get(db: &Db, id: &str) -> Result<Series> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE id = ?1"))?;
        let mut rows = stmt.query([id])?;
        match rows.next()? {
            None => Err(crate::error::not_found("series", id)),
            Some(row) => Ok(from_row(row)?),
        }
    })
}

/// List every series, ordered by normalized name.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an empty table yields an empty `Vec`.
pub fn list(db: &Db) -> Result<Vec<Series>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY name_norm ASC"))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Edit an existing series in place and return the stored row.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id does not exist (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when the new normalized name collides with another
///   series (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn update(db: &Db, id: &str, input: SeriesInput) -> Result<Series> {
    let mut series = get(db, id)?;
    db.write(|conn| {
        series.apply(input, crate::conn::now_ms())?;
        let changed = conn
            .execute(
                "UPDATE series SET name = ?2, name_norm = ?3, description = ?4, updated_at = ?5
             WHERE id = ?1",
                rusqlite::params![
                    series.id,
                    series.name,
                    series.name_norm,
                    series.description,
                    series.updated_at,
                ],
            )
            .map_err(|e| crate::error::on_unique("series", &series.name, e))?;
        if changed == 0 {
            return Err(crate::error::not_found("series", id));
        }
        Ok(series)
    })
}

/// Delete one series by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no series has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        let removed = conn.execute("DELETE FROM series WHERE id = ?1", [id])?;
        if removed == 0 {
            return Err(crate::error::not_found("series", id));
        }
        Ok(())
    })
}

/// Find a series by its normalized-name mirror.
///
/// `name_norm` is expected to already be normalized (`maktabati_domain::text::normalize`)
/// — this is an exact-match lookup, not a search.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an absent row is `Ok(None)`.
pub fn find_by_name_norm(db: &Db, name_norm: &str) -> Result<Option<Series>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE name_norm = ?1"))?;
        let mut rows = stmt.query([name_norm])?;
        match rows.next()? {
            None => Ok(None),
            Some(row) => Ok(Some(from_row(row)?)),
        }
    })
}

/// Search series whose normalized name contains `query`.
///
/// The query is normalized first, so a typed «سلسلة» matches a stored «سلسلة» written with
/// different variants; matching is a substring test against `name_norm`.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; a blank query or a non-positive `limit`
/// returns an empty `Vec` without touching the database.
pub fn search(db: &Db, query: &str, limit: i64) -> Result<Vec<Series>> {
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

    fn input(name: &str) -> SeriesInput {
        SeriesInput {
            name: name.to_owned(),
            description: None,
        }
    }

    #[test]
    fn create_then_get_round_trips_every_field() {
        let db = test_db();
        let created = create(
            &db,
            SeriesInput {
                name: " مقدمة في الحضارة ".into(),
                description: Some("سلسلة علمية مصورة".into()),
            },
        )
        .unwrap();

        assert!(created.created_at > 0);
        assert!(created.updated_at > 0);
        assert_eq!(created.name, "مقدمة في الحضارة");
        assert_eq!(created.description.as_deref(), Some("سلسلة علمية مصورة"));

        let got = get(&db, &created.id).unwrap();
        assert_eq!(got, created, "every stored field must survive the read");
    }

    #[test]
    fn list_returns_series_in_name_norm_order() {
        let db = test_db();
        for name in ["charlie", "alpha", "bravo"] {
            create(&db, input(name)).unwrap();
        }
        let names: Vec<String> = list(&db).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn update_edits_fields_but_keeps_id_and_created_at() {
        let db = test_db();
        let original = create(&db, input("Old series")).unwrap();
        let updated = update(
            &db,
            &original.id,
            SeriesInput {
                name: "New series".into(),
                description: Some("وصف جديد".into()),
            },
        )
        .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.created_at, original.created_at);
        assert!(updated.updated_at >= original.updated_at);
        assert_eq!(updated.name, "New series");
        assert_eq!(updated.name_norm, "new series");
        assert_eq!(updated.description.as_deref(), Some("وصف جديد"));
        assert_eq!(get(&db, &original.id).unwrap(), updated);
    }

    #[test]
    fn delete_removes_the_row_and_a_second_delete_is_not_found() {
        let db = test_db();
        let s = create(&db, input("Gone")).unwrap();

        delete(&db, &s.id).unwrap();
        assert!(matches!(
            get(&db, &s.id),
            Err(StorageError::NotFound { .. })
        ));

        let err = delete(&db, &s.id).unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
    }

    #[test]
    fn a_duplicate_normalized_name_is_rejected() {
        let db = test_db();
        create(&db, input("سلسلة الروائع")).unwrap();

        let typed = "سلسلة  الروائع";
        let err = create(&db, input(typed)).unwrap_err();
        assert_eq!(err.code(), "storage.duplicate");
        assert!(matches!(&err, StorageError::Duplicate { kind, key }
                if kind == "series" && key == typed));
    }

    #[test]
    fn a_blank_name_surfaces_as_a_validation_error() {
        let db = test_db();
        let err = create(&db, input("\t")).unwrap_err();
        assert_eq!(err.code(), "validation.empty_field");
    }

    #[test]
    fn search_matches_by_normalized_query() {
        let db = test_db();
        create(&db, input("أحمد فكري")).unwrap();
        create(&db, input("سلسلة عربية")).unwrap();

        let hits = search(&db, "احمد", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "أحمد فكري");

        assert_eq!(search(&db, "احمد", -3).unwrap(), Vec::new());
        assert_eq!(search(&db, "", 10).unwrap(), Vec::new());
    }

    #[test]
    fn like_pattern_escapes_metacharacters_and_matches_substrings() {
        assert_eq!(like_pattern("احمد"), "%احمد%");
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        assert_eq!(like_pattern("back\\slash"), "%back\\\\slash%");
    }
}
