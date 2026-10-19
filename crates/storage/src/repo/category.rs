//! `categories` — the bilingual taxonomy tree.
//!
//! Paths are materialized by the domain (`docs/03-data-model.md` §3.1), so this module's job
//! is to persist them, keep sibling `position`s dense enough to reorder, and let SQLite's
//! `ON DELETE CASCADE` take a subtree with its parent.

use maktabati_domain::category::new_category;
use maktabati_domain::{Category, CategoryInput};

use crate::conn::{now_ms, Db};
use crate::error::not_found;
use crate::Result;

/// Columns in the order [`from_row`] reads them.
const COLUMNS: &str = "id, parent_id, key, name_ar, name_en, name_norm, path, depth, position, \
                       icon, color, is_system, created_at, updated_at";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        key: row.get(2)?,
        name_ar: row.get(3)?,
        name_en: row.get(4)?,
        name_norm: row.get(5)?,
        path: row.get(6)?,
        // `depth` is stored as `INTEGER`; `usize` has no `FromSql` implementation.
        depth: usize::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
        position: row.get(8)?,
        icon: row.get(9)?,
        color: row.get(10)?,
        // `is_system` is `INTEGER CHECK (0,1)`; reading it as `i64` keeps a surprising
        // value from being coerced into a wrong `true`.
        is_system: row.get::<_, i64>(11)? != 0,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

/// Read one row on an already-open connection (used inside `create`/`update` transactions).
fn read_row(conn: &rusqlite::Connection, id: &str) -> Result<Category> {
    let sql = format!("SELECT {COLUMNS} FROM categories WHERE id = ?1");
    conn.query_row(&sql, [id], from_row).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => not_found("category", id),
        other => crate::StorageError::Sqlite(other),
    })
}

/// Next sibling slot (`0`-based, dense from the start).
fn next_position(conn: &rusqlite::Connection, parent_id: Option<&str>) -> Result<i64> {
    conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM categories WHERE parent_id IS ?1",
        [parent_id],
        |row| row.get(0),
    )
    .map_err(crate::StorageError::Sqlite)
}

/// Create a category under `parent_id` (`None` = a root), appended after its siblings.
///
/// `input.key` becomes the stable machine key; `position` is assigned here rather than by
/// the caller so two concurrent creates cannot claim the same slot.
///
/// # Errors
///
/// * [`maktabati_domain::DomainError`] when the key/names are empty, the key yields an
///   empty path, or nesting would exceed the depth cap.
/// * [`crate::StorageError::Duplicate`] when the same key already exists under that parent
///   (`ux_categories_parent_key` / `ux_categories_path`).
/// * [`crate::StorageError::NotFound`] when `parent_id` names a category that is not there.
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn create(db: &Db, input: CategoryInput, parent_id: Option<&str>) -> Result<Category> {
    db.write(|conn| {
        let parent = parent_id.map(|pid| read_row(conn, pid)).transpose()?;
        let position = next_position(conn, parent.as_ref().map(|p| p.id.as_str()))?;
        let category = new_category(input, parent.as_ref(), position, now_ms(), false)?;

        let sql = format!(
            "INSERT INTO categories ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, \
                           ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
        );
        conn.execute(
            &sql,
            rusqlite::params![
                category.id,
                category.parent_id,
                category.key,
                category.name_ar,
                category.name_en,
                category.name_norm,
                category.path,
                // `depth` is a `usize` in the domain and `INTEGER` in SQLite; neither
                // `ToSql` nor `FromSql` is implemented for `usize`.
                i64::try_from(category.depth).unwrap_or(0),
                category.position,
                category.icon,
                category.color,
                category.is_system,
                category.created_at,
                category.updated_at,
            ],
        )
        .map_err(|e| crate::error::on_unique("category", &category.key, e))?;
        Ok(category)
    })
}

/// Read one category.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id is unknown.
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get(db: &Db, id: &str) -> Result<Category> {
    db.read(|conn| read_row(conn, id))
}

/// Every category, ordered by materialized `path` — a parent always precedes its children,
/// so the list can be replayed top-down.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn list(db: &Db) -> Result<Vec<Category>> {
    db.read(|conn| {
        let sql = format!("SELECT {COLUMNS} FROM categories ORDER BY path");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
}

/// Direct children of `parent_id`, in the sibling order the UI edits (`None` = roots).
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn children(db: &Db, parent_id: Option<&str>) -> Result<Vec<Category>> {
    db.read(|conn| {
        let sql = format!(
            "SELECT {COLUMNS} FROM categories WHERE parent_id IS ?1 ORDER BY position, name_norm"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([parent_id], from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
}

/// Rename a category (and change its icon/colour).
///
/// Only display fields move: `input.key` is **ignored in favour of the stored key**, and
/// `parent_id`/`position`/`path`/`depth` stay put. The key is the stable machine identity —
/// it is baked into this node's materialized path and every descendant's, and into saved
/// shelves and filters — and reparenting would have to rewrite a whole subtree's paths in
/// the same transaction. Both are therefore separate operations, not a side effect of a
/// rename (`docs/03-data-model.md` §3.1).
///
/// # Errors
///
/// * [`maktabati_domain::DomainError`] when the supplied names are empty.
/// * [`crate::StorageError::NotFound`] when the id (or the parent, when one is stored) is gone.
/// * [`crate::StorageError::Sqlite`] on any other statement failure.
pub fn update(db: &Db, id: &str, input: CategoryInput) -> Result<Category> {
    db.write(|conn| {
        let existing = read_row(conn, id)?;
        // Validate against a parent built from the *stored* tree, so the depth/path rules
        // are checked exactly as creation would check them.
        let parent = existing
            .parent_id
            .as_deref()
            .map(|pid| read_row(conn, pid))
            .transpose()?;
        let input = CategoryInput {
            key: existing.key.clone(),
            ..input
        };
        let validated = new_category(
            input,
            parent.as_ref(),
            existing.position,
            now_ms(),
            existing.is_system,
        )?;
        debug_assert_eq!(
            validated.path, existing.path,
            "the key and parent are unchanged, so the materialized path must be identical"
        );

        conn.execute(
            "UPDATE categories SET name_ar = ?1, name_en = ?2, name_norm = ?3, icon = ?4, \
             color = ?5, updated_at = ?6 WHERE id = ?7",
            rusqlite::params![
                validated.name_ar,
                validated.name_en,
                validated.name_norm,
                validated.icon,
                validated.color,
                validated.updated_at,
                id,
            ],
        )?;

        let mut updated = existing;
        updated.name_ar = validated.name_ar;
        updated.name_en = validated.name_en;
        updated.name_norm = validated.name_norm;
        updated.icon = validated.icon;
        updated.color = validated.color;
        updated.updated_at = validated.updated_at;
        Ok(updated)
    })
}

/// Delete a category. **The subtree goes with it** (`ON DELETE CASCADE`), because a child
/// without its parent has no place in the taxonomy; the books themselves are untouched —
/// only their `book_categories` links are removed.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id is unknown.
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete(db: &Db, id: &str) -> Result<()> {
    db.write(|conn| {
        conn.execute("DELETE FROM categories WHERE id = ?1", [id])?;
        if conn.changes() == 0 {
            return Err(not_found("category", id));
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

    fn literature() -> CategoryInput {
        CategoryInput {
            key: "literature".into(),
            name_ar: "الأدب".into(),
            name_en: "Literature".into(),
            ..Default::default()
        }
    }

    #[test]
    fn creates_a_root_and_reads_it_back() {
        let db = test_db();
        let c = create(&db, literature(), None).unwrap();
        assert_eq!(c.path, "/literature");
        assert_eq!(c.depth, 0);
        assert_eq!(c.position, 0);
        assert_eq!(c.parent_id, None);
        assert!(
            !c.is_system,
            "user-created categories are never system ones"
        );
        assert_eq!(get(&db, &c.id).unwrap(), c);
    }

    #[test]
    fn children_follow_their_parent_and_siblings_stack_in_order() {
        let db = test_db();
        let root = create(&db, literature(), None).unwrap();
        let other_root = create(
            &db,
            CategoryInput {
                key: "history".into(),
                ..literature()
            },
            None,
        )
        .unwrap();
        let novels = create(
            &db,
            CategoryInput {
                key: "novels".into(),
                name_ar: "روايات".into(),
                name_en: "Novels".into(),
                ..Default::default()
            },
            Some(&root.id),
        )
        .unwrap();
        let essays = create(
            &db,
            CategoryInput {
                key: "essays".into(),
                name_ar: "مقالات".into(),
                name_en: "Essays".into(),
                ..Default::default()
            },
            Some(&root.id),
        )
        .unwrap();

        assert_eq!(novels.path, "/literature/novels");
        assert_eq!(novels.depth, 1);
        assert_eq!(novels.position, 0, "first child claims slot 0");
        assert_eq!(essays.position, 1, "second child stacks after it");

        let roots = children(&db, None).unwrap();
        assert_eq!(
            roots.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec![root.id.as_str(), other_root.id.as_str()],
            "roots come back in the order they were created"
        );
        let kids = children(&db, Some(&root.id)).unwrap();
        assert_eq!(kids, vec![novels, essays]);

        // `list` is path-ordered, so a parent always precedes its children, and siblings
        // come back by key (`essays` < `novels`), not by insertion.
        let all = list(&db).unwrap();
        let paths = all.iter().map(|c| c.path.as_str()).collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "/history",
                "/literature",
                "/literature/essays",
                "/literature/novels"
            ]
        );
    }

    #[test]
    fn rename_keeps_the_key_path_and_position() {
        let db = test_db();
        let root = create(&db, literature(), None).unwrap();
        let child = create(
            &db,
            CategoryInput {
                key: "novels".into(),
                name_ar: "روايات".into(),
                name_en: "Novels".into(),
                ..Default::default()
            },
            Some(&root.id),
        )
        .unwrap();

        let renamed = update(
            &db,
            &child.id,
            CategoryInput {
                key: "should-be-ignored".into(),
                name_ar: "أدب روائي".into(),
                name_en: "Fiction".into(),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(renamed.id, child.id);
        assert_eq!(renamed.key, "novels", "the machine key is immutable");
        assert_eq!(renamed.path, "/literature/novels", "and so is the path");
        assert_eq!(renamed.name_ar, "أدب روائي");
        assert_eq!(renamed.name_en, "Fiction");
        assert_eq!(renamed.created_at, child.created_at);
        assert_eq!(renamed.position, child.position);
        assert!(renamed.updated_at >= child.updated_at);
    }

    #[test]
    fn rename_validates_the_names() {
        let db = test_db();
        let root = create(&db, literature(), None).unwrap();
        let err = update(
            &db,
            &root.id,
            CategoryInput {
                name_ar: "   ".into(),
                ..literature()
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "validation.empty_field");
    }

    #[test]
    fn the_same_key_twice_under_one_parent_is_a_duplicate() {
        let db = test_db();
        let root = create(&db, literature(), None).unwrap();
        create(
            &db,
            CategoryInput {
                key: "novels".into(),
                name_ar: "روايات".into(),
                name_en: "Novels".into(),
                ..Default::default()
            },
            Some(&root.id),
        )
        .unwrap();
        let err = create(
            &db,
            CategoryInput {
                key: "novels".into(),
                name_ar: "روايات أخرى".into(),
                name_en: "Novels".into(),
                ..Default::default()
            },
            Some(&root.id),
        )
        .unwrap_err();
        assert_eq!(err.code(), "storage.duplicate");
    }

    #[test]
    fn unknown_parent_is_not_found() {
        let db = test_db();
        let err = create(&db, literature(), Some("nope")).unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
        assert!(matches!(err, StorageError::NotFound { .. }));
    }

    #[test]
    fn deleting_a_parent_takes_the_subtree_with_it() {
        let db = test_db();
        let root = create(&db, literature(), None).unwrap();
        let child = create(
            &db,
            CategoryInput {
                key: "novels".into(),
                name_ar: "روايات".into(),
                name_en: "Novels".into(),
                ..Default::default()
            },
            Some(&root.id),
        )
        .unwrap();

        delete(&db, &root.id).unwrap();
        assert!(list(&db).unwrap().is_empty(), "the whole subtree is gone");
        assert_eq!(get(&db, &child.id).unwrap_err().code(), "storage.not_found");
        assert_eq!(
            delete(&db, &root.id).unwrap_err().code(),
            "storage.not_found"
        );
    }
}
