//! `tags` — flat user-defined labels (الوسم) attached to books.
//!
//! The table deliberately carries no `updated_at`: renaming a label is not a semantic change
//! to it, so [`Tag::apply`] touches no timestamp.

use maktabati_domain::tag::{Tag, TagInput};

use crate::conn::Db;
use crate::Result;

/// Every `tags` column, in the order [`from_row`] reads them.
const SELECT: &str = "SELECT id, name, name_norm, color, created_at FROM tags";

/// Map one `tags` row onto a [`Tag`].
fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        name: row.get(1)?,
        name_norm: row.get(2)?,
        color: row.get(3)?,
        created_at: row.get(4)?,
    })
}

/// Insert a new tag and return the stored row.
///
/// # Errors
///
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when another tag already holds this normalized name
///   (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn create(db: &Db, input: TagInput) -> Result<Tag> {
    db.write(|conn| {
        let tag = Tag::new(input, crate::conn::now_ms())?;
        conn.execute(
            "INSERT INTO tags (id, name, name_norm, color, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![tag.id, tag.name, tag.name_norm, tag.color, tag.created_at,],
        )
        .map_err(|e| crate::error::on_unique("tag", &tag.name, e))?;
        Ok(tag)
    })
}

/// Fetch one tag by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no tag has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get(db: &Db, id: &str) -> Result<Tag> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE id = ?1"))?;
        let mut rows = stmt.query([id])?;
        match rows.next()? {
            None => Err(crate::error::not_found("tag", id)),
            Some(row) => Ok(from_row(row)?),
        }
    })
}

/// List every tag, ordered by normalized name.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an empty table yields an empty `Vec`.
pub fn list(db: &Db) -> Result<Vec<Tag>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY name_norm ASC"))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Edit an existing tag in place and return the stored row.
///
/// `created_at` is never rewritten — an edit only changes the name and colour.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id does not exist (`storage.not_found`).
/// * [`crate::StorageError::Domain`] when the input fails domain validation (`validation.*`).
/// * [`crate::StorageError::Duplicate`] when the new normalized name collides with another
///   tag (`storage.duplicate`).
/// * [`crate::StorageError::Sqlite`] on any other database failure.
pub fn update(db: &Db, id: &str, input: TagInput) -> Result<Tag> {
    let mut tag = get(db, id)?;
    db.write(|conn| {
        tag.apply(input)?;
        let changed = conn
            .execute(
                "UPDATE tags SET name = ?2, name_norm = ?3, color = ?4 WHERE id = ?1",
                rusqlite::params![tag.id, tag.name, tag.name_norm, tag.color,],
            )
            .map_err(|e| crate::error::on_unique("tag", &tag.name, e))?;
        if changed == 0 {
            return Err(crate::error::not_found("tag", id));
        }
        Ok(tag)
    })
}

/// Delete one tag by id.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when no tag has this id (`storage.not_found`).
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        let removed = conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
        if removed == 0 {
            return Err(crate::error::not_found("tag", id));
        }
        Ok(())
    })
}

/// Find a tag by its normalized-name mirror.
///
/// `name_norm` is expected to already be normalized (`maktabati_domain::text::normalize`)
/// — this is an exact-match lookup, not a search.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; an absent row is `Ok(None)`.
pub fn find_by_name_norm(db: &Db, name_norm: &str) -> Result<Option<Tag>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE name_norm = ?1"))?;
        let mut rows = stmt.query([name_norm])?;
        match rows.next()? {
            None => Ok(None),
            Some(row) => Ok(Some(from_row(row)?)),
        }
    })
}

/// Search tags whose normalized name contains `query`.
///
/// The query is normalized first, so a typed «احمد» matches a stored «أحمد»; matching is a
/// substring test against `name_norm`.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure; a blank query or a non-positive `limit`
/// returns an empty `Vec` without touching the database.
pub fn search(db: &Db, query: &str, limit: i64) -> Result<Vec<Tag>> {
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

    fn input(name: &str) -> TagInput {
        TagInput {
            name: name.to_owned(),
            color: None,
        }
    }

    #[test]
    fn create_then_get_round_trips_every_field() {
        let db = test_db();
        let created = create(
            &db,
            TagInput {
                name: " #الحضارة_الإسلامية ".into(),
                color: Some("#3366cc".into()),
            },
        )
        .unwrap();

        assert!(created.created_at > 0);
        assert_eq!(created.name, "#الحضارة_الإسلامية");
        assert_eq!(created.color.as_deref(), Some("#3366cc"));

        let got = get(&db, &created.id).unwrap();
        assert_eq!(got, created, "every stored field must survive the read");
    }

    #[test]
    fn list_returns_tags_in_name_norm_order() {
        let db = test_db();
        for name in ["charlie", "alpha", "bravo"] {
            create(&db, input(name)).unwrap();
        }
        let names: Vec<String> = list(&db).unwrap().into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn update_edits_fields_but_keeps_id_and_created_at() {
        let db = test_db();
        let original = create(&db, input("old")).unwrap();
        let updated = update(
            &db,
            &original.id,
            TagInput {
                name: "  new ".into(),
                color: Some("#ff0000".into()),
            },
        )
        .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.created_at, original.created_at);
        assert_eq!(updated.name, "new");
        assert_eq!(updated.name_norm, "new");
        assert_eq!(updated.color.as_deref(), Some("#ff0000"));
        assert_eq!(get(&db, &original.id).unwrap(), updated);
    }

    #[test]
    fn delete_removes_the_row_and_a_second_delete_is_not_found() {
        let db = test_db();
        let t = create(&db, input("Gone")).unwrap();

        delete(&db, &t.id).unwrap();
        assert!(matches!(
            get(&db, &t.id),
            Err(StorageError::NotFound { .. })
        ));

        let err = delete(&db, &t.id).unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
    }

    #[test]
    fn a_duplicate_normalized_name_is_rejected() {
        let db = test_db();
        create(&db, input("وسم تجريبي")).unwrap();

        let typed = "وسم  تجريبي";
        let err = create(&db, input(typed)).unwrap_err();
        assert_eq!(err.code(), "storage.duplicate");
        assert!(matches!(&err, StorageError::Duplicate { kind, key }
                if kind == "tag" && key == typed));
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
        create(&db, input("وسم تجريبي")).unwrap();

        let hits = search(&db, "احمد", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "أحمد فكري");

        assert_eq!(search(&db, "احمد", 0).unwrap(), Vec::new());
        assert_eq!(search(&db, "\t", 10).unwrap(), Vec::new());
    }

    #[test]
    fn like_pattern_escapes_metacharacters_and_matches_substrings() {
        assert_eq!(like_pattern("احمد"), "%احمد%");
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        assert_eq!(like_pattern("back\\slash"), "%back\\\\slash%");
    }
}
