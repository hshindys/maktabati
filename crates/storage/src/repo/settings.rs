//! `app_settings` — typed key/value preferences (non-secret).
//!
//! Values are stored as JSON so a preference can evolve from a string to an object without a
//! migration; the *shape* is owned by the caller, which deserializes into its own struct.

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::conn::Db;
use crate::Result;

/// Read one setting.
///
/// # Errors
///
/// * [`crate::StorageError::Sqlite`] on query failure.
/// * [`crate::StorageError::Serde`] when the stored JSON does not match `T`.
pub fn get<T: DeserializeOwned>(db: &Db, key: &str) -> Result<Option<T>> {
    db.read(|conn| {
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        match rows.next()? {
            None => Ok(None),
            Some(row) => {
                let raw: String = row.get(0)?;
                Ok(Some(serde_json::from_str(&raw)?))
            }
        }
    })
}

/// Write one setting (insert or replace).
///
/// # Errors
///
/// * [`crate::StorageError::Sqlite`] on query failure.
/// * [`crate::StorageError::Serde`] when the value cannot be serialized.
pub fn set<T: Serialize>(db: &Db, key: &str, value: &T) -> Result<()> {
    let raw = serde_json::to_string(value)?;
    db.write(|conn| {
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            rusqlite::params![key, raw, crate::conn::now_ms()],
        )?;
        Ok(())
    })
}

/// Read every setting as raw JSON — used by the diagnostics export.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn all(db: &Db) -> Result<Vec<(String, serde_json::Value)>> {
    db.read(|conn| {
        let mut stmt = conn.prepare("SELECT key, value FROM app_settings ORDER BY key")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (k, v) = row?;
            out.push((k, serde_json::from_str(&v)?));
        }
        Ok(out)
    })
}

/// Delete one setting.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn remove(db: &Db, key: &str) -> Result<()> {
    db.write(|conn| {
        conn.execute("DELETE FROM app_settings WHERE key = ?1", [key])?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageError;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;

    fn test_db() -> Db {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("s.db")).unwrap();
        crate::migrate::apply(&db, || Ok(())).unwrap();
        db
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Appearance {
        theme: String,
        accent: String,
        digits: String,
    }

    #[test]
    fn round_trips_a_struct() {
        let db = test_db();
        let v = Appearance {
            theme: "dark".into(),
            accent: "green".into(),
            digits: "arabic_indic".into(),
        };
        set(&db, "appearance", &v).unwrap();
        let back: Option<Appearance> = get(&db, "appearance").unwrap();
        assert_eq!(back, Some(v));
    }

    #[test]
    fn missing_key_is_none_not_an_error() {
        let db = test_db();
        assert_eq!(get::<serde_json::Value>(&db, "nope").unwrap(), None);
    }

    #[test]
    fn set_is_upsert_and_updates_timestamp() {
        let db = test_db();
        set(&db, "k", &1).unwrap();
        set(&db, "k", &2).unwrap();
        assert_eq!(get::<i32>(&db, "k").unwrap(), Some(2));
        let n: i64 = db
            .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM app_settings", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(n, 1, "upsert must not create a second row");
    }

    #[test]
    fn remove_deletes() {
        let db = test_db();
        set(&db, "k", &"v").unwrap();
        remove(&db, "k").unwrap();
        assert_eq!(get::<String>(&db, "k").unwrap(), None);
    }

    #[test]
    fn all_returns_sorted_pairs() {
        let db = test_db();
        set(&db, "b", &2).unwrap();
        set(&db, "a", &1).unwrap();
        let all = all(&db).unwrap();
        let keys: Vec<_> = all.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn wrong_shape_is_a_typed_error() {
        let db = test_db();
        set(&db, "k", &"a string").unwrap();
        let err = get::<i32>(&db, "k");
        assert!(matches!(err, Err(StorageError::Serde(_))));
    }
}
