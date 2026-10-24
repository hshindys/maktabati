//! Typed repositories.
//!
//! Each module owns one aggregate. Every function here takes `&Db` (or `&Connection` inside a
//! transaction) and returns domain types — no SQL or `rusqlite` types cross this boundary, so
//! `src-tauri` never sees a row handle.

pub mod author;
pub mod book;
pub mod category;
pub mod file;
pub mod language;
pub mod note;
pub mod publisher;
pub mod quote;
pub mod series;
pub mod settings;
pub mod summary;
pub mod tag;

/// Tests that the SQL `CHECK` constraints and the Rust enums describe the same vocabulary.
///
/// This is the guard named in `docs/03-data-model.md` §5: adding a variant to `domain::enums`
/// without a matching column value (or vice versa) fails here instead of at runtime.
#[cfg(test)]
mod schema_conformance {
    use maktabati_domain::{
        AuthorRole, BookFormat, BookSource, Condition, CoverSource, CustomColumnType, FileKind,
        NoteBlockType, QuoteColor, ReadingStatus, ShelfKind, StorageMode,
    };

    use crate::conn::Db;
    use crate::migrate;

    /// Every value the Rust enums can produce must appear in `0001_init.sql`.
    #[test]
    fn every_rust_enum_variant_exists_in_the_ddl() {
        let sql = crate::migrations::M0001_INIT;

        let check = |name: &str, values: &[&str]| {
            for v in values {
                assert!(
                    sql.contains(v),
                    "enum {name} variant `{v}` has no CHECK constraint in 0001_init.sql"
                );
            }
        };

        check(
            "ReadingStatus",
            &ReadingStatus::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "BookFormat",
            &BookFormat::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "Condition",
            &Condition::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "AuthorRole",
            &AuthorRole::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "BookSource",
            &BookSource::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "CoverSource",
            &CoverSource::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "StorageMode",
            &StorageMode::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "FileKind",
            &FileKind::ALL.iter().map(|e| e.as_str()).collect::<Vec<_>>(),
        );
        check(
            "ShelfKind",
            &ShelfKind::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "CustomColumnType",
            &CustomColumnType::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "QuoteColor",
            &QuoteColor::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
        check(
            "NoteBlockType",
            &NoteBlockType::ALL
                .iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>(),
        );
    }

    /// Inserting a value the enum does not know must be rejected by SQLite itself.
    #[test]
    fn check_constraints_reject_unknown_enum_values() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("c.db")).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.write(crate::seed::apply).unwrap();

        let err = db.write(|c| {
            c.execute(
                "INSERT INTO books (id, title, title_norm, language_code, reading_status,
                                    progress_pct, source, created_at, updated_at)
                 VALUES ('x','t','t','ar','reading_but_wrong',0,'manual',1,1)",
                [],
            )?;
            Ok(())
        });
        assert!(err.is_err(), "unknown reading_status must be rejected");

        let err = db.write(|c| {
            c.execute(
                "INSERT INTO books (id, title, title_norm, language_code, reading_status,
                                    progress_pct, source, created_at, updated_at)
                 VALUES ('y','t','t','ar','to_read',0,'from_the_future',1,1)",
                [],
            )?;
            Ok(())
        });
        assert!(err.is_err(), "unknown source must be rejected");
    }

    /// Rating and progress bounds are enforced by the schema, not only by the domain.
    #[test]
    fn numeric_bounds_are_enforced_by_the_schema() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("b.db")).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.write(crate::seed::apply).unwrap();

        for (id, rating, progress) in [("r1", Some(11), 0), ("r2", None, 101)] {
            let err = db.write(move |c| {
                c.execute(
                    "INSERT INTO books (id, title, title_norm, language_code, reading_status,
                                        progress_pct, rating, created_at, updated_at)
                     VALUES (?1,'t','t','ar','to_read',?2,?3,1,1)",
                    rusqlite::params![id, progress, rating],
                )?;
                Ok(())
            });
            assert!(
                err.is_err(),
                "rating={rating:?} progress={progress} must be rejected"
            );
        }
    }
}
