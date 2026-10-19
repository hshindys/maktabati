//! `publishers` — the publishing houses credited on books.

use maktabati_domain::publisher::{Publisher, PublisherInput};

use crate::conn::Db;
use crate::Result;

/// Every `publishers` column, in the order [`from_row`] reads them.
const SELECT: &str = "SELECT id, name, name_norm, country, city, website, notes, cover_path,
                             created_at, updated_at
                      FROM publishers";

/// Map one `publishers` row onto a [`Publisher`].
fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Publisher> {
    Ok(Publisher {
        id: row.get(0)?,
        name: row.get(1)?,
        name_norm: row.get(2)?,
        country: row.get(3)?,
        city: row.get(4)?,
        website: row.get(5)?,
        notes: row.get(6)?,
        cover_path: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// Insert a new publisher and return the stored row.
///
/// # Errors
///
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when another publisher already holds this normalized
///   name (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn create(db: &Db, input: PublisherInput) -> Result<Publisher> {
    db.write(|conn| {
        let publisher = Publisher::new(input, crate::conn::now_ms())?;
        conn.execute(
            "INSERT INTO publishers (id, name, name_norm, country, city, website, notes,
                                     cover_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                publisher.id,
                publisher.name,
                publisher.name_norm,
                publisher.country,
                publisher.city,
                publisher.website,
                publisher.notes,
                publisher.cover_path,
                publisher.created_at,
                publisher.updated_at,
            ],
        )
        .map_err(|e| crate::error::on_unique("publisher", &publisher.name, e))?;
        Ok(publisher)
    })
}

/// Fetch one publisher by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no publisher has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get(db: &Db, id: &str) -> Result<Publisher> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE id = ?1"))?;
        let mut rows = stmt.query([id])?;
        match rows.next()? {
            None => Err(crate::error::not_found("publisher", id)),
            Some(row) => Ok(from_row(row)?),
        }
    })
}

/// List every publisher, ordered by normalized name.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an empty table yields an empty `Vec`.
pub fn list(db: &Db) -> Result<Vec<Publisher>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY name_norm ASC"))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Edit an existing publisher in place and return the stored row.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id does not exist (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when the new normalized name collides with another
///   publisher (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn update(db: &Db, id: &str, input: PublisherInput) -> Result<Publisher> {
    let mut publisher = get(db, id)?;
    db.write(|conn| {
        publisher.apply(input, crate::conn::now_ms())?;
        let changed = conn
            .execute(
                "UPDATE publishers SET name = ?2, name_norm = ?3, country = ?4, city = ?5,
                    website = ?6, notes = ?7, cover_path = ?8, updated_at = ?9
             WHERE id = ?1",
                rusqlite::params![
                    publisher.id,
                    publisher.name,
                    publisher.name_norm,
                    publisher.country,
                    publisher.city,
                    publisher.website,
                    publisher.notes,
                    publisher.cover_path,
                    publisher.updated_at,
                ],
            )
            .map_err(|e| crate::error::on_unique("publisher", &publisher.name, e))?;
        if changed == 0 {
            return Err(crate::error::not_found("publisher", id));
        }
        Ok(publisher)
    })
}

/// Delete one publisher by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no publisher has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        let removed = conn.execute("DELETE FROM publishers WHERE id = ?1", [id])?;
        if removed == 0 {
            return Err(crate::error::not_found("publisher", id));
        }
        Ok(())
    })
}

/// Find a publisher by its normalized-name mirror.
///
/// `name_norm` is expected to already be normalized (`maktabati_domain::text::normalize`)
/// — this is an exact-match lookup, not a search.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an absent row is `Ok(None)`.
pub fn find_by_name_norm(db: &Db, name_norm: &str) -> Result<Option<Publisher>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE name_norm = ?1"))?;
        let mut rows = stmt.query([name_norm])?;
        match rows.next()? {
            None => Ok(None),
            Some(row) => Ok(Some(from_row(row)?)),
        }
    })
}

/// Search publishers whose normalized name contains `query`.
///
/// The query is normalized first, so a typed «دار» matches a stored «دار» written with
/// different variants; matching is a substring test against `name_norm`.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; a blank query or a non-positive `limit`
/// returns an empty `Vec` without touching the database.
pub fn search(db: &Db, query: &str, limit: i64) -> Result<Vec<Publisher>> {
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

    fn input(name: &str) -> PublisherInput {
        PublisherInput {
            name: name.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn create_then_get_round_trips_every_field() {
        let db = test_db();
        let created = create(
            &db,
            PublisherInput {
                name: " دار المعارف ".into(),
                country: Some("مصر".into()),
                city: Some("القاهرة".into()),
                website: Some("https://dar-al-maaref.example".into()),
                notes: Some("ناشئة قديمة".into()),
                cover_path: Some("covers/dar.webp".into()),
            },
        )
        .unwrap();

        assert!(created.created_at > 0);
        assert!(created.updated_at > 0);
        assert_eq!(created.name, "دار المعارف");
        assert_eq!(created.name_norm, "دار المعارف");

        let got = get(&db, &created.id).unwrap();
        assert_eq!(got, created, "every stored field must survive the read");
    }

    #[test]
    fn list_returns_publishers_in_name_norm_order() {
        let db = test_db();
        for name in ["charlie", "alpha", "bravo"] {
            create(&db, input(name)).unwrap();
        }
        let names: Vec<String> = list(&db).unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn update_edits_fields_but_keeps_id_and_created_at() {
        let db = test_db();
        let original = create(&db, input("Dar")).unwrap();
        let updated = update(
            &db,
            &original.id,
            PublisherInput {
                name: "Dar al-Manar".into(),
                country: Some("تونس".into()),
                notes: Some("من دار إلى أخرى".into()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.created_at, original.created_at);
        assert!(updated.updated_at >= original.updated_at);
        assert_eq!(updated.name, "Dar al-Manar");
        // `normalize` folds case, spaces and hyphens away, so "Dar al-Manar" mirrors to
        // "dar almanar" — the hyphen is punctuation, not part of the name.
        assert_eq!(updated.name_norm, "dar almanar");
        assert_eq!(updated.country.as_deref(), Some("تونس"));
        assert_eq!(updated.notes.as_deref(), Some("من دار إلى أخرى"));
        assert_eq!(get(&db, &original.id).unwrap(), updated);
    }

    #[test]
    fn delete_removes_the_row_and_a_second_delete_is_not_found() {
        let db = test_db();
        let p = create(&db, input("Gone")).unwrap();

        delete(&db, &p.id).unwrap();
        assert!(matches!(
            get(&db, &p.id),
            Err(StorageError::NotFound { .. })
        ));

        let err = delete(&db, &p.id).unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
    }

    #[test]
    fn a_duplicate_normalized_name_is_rejected() {
        let db = test_db();
        create(&db, input("دار المعارف")).unwrap();

        let typed = "دار  المعارف";
        let err = create(&db, input(typed)).unwrap_err();
        assert_eq!(err.code(), "storage.duplicate");
        assert!(matches!(&err, StorageError::Duplicate { kind, key }
                if kind == "publisher" && key == typed));
    }

    #[test]
    fn a_blank_name_surfaces_as_a_validation_error() {
        let db = test_db();
        let err = create(&db, input("  ")).unwrap_err();
        assert_eq!(err.code(), "validation.empty_field");
    }

    #[test]
    fn search_matches_by_normalized_query() {
        let db = test_db();
        create(&db, input("أحمد فكري")).unwrap();
        create(&db, input("دار المعارف")).unwrap();

        let hits = search(&db, "احمد", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "أحمد فكري");

        assert_eq!(search(&db, "احمد", 0).unwrap(), Vec::new());
        assert_eq!(search(&db, " ", 10).unwrap(), Vec::new());
    }

    #[test]
    fn like_pattern_escapes_metacharacters_and_matches_substrings() {
        assert_eq!(like_pattern("احمد"), "%احمد%");
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        assert_eq!(like_pattern("back\\slash"), "%back\\\\slash%");
    }
}
