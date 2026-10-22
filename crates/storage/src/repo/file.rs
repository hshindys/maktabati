//! `book_files` — attachments on a book.
//!
//! The table predates this module (see `docs/03-data-model.md`): it was written into
//! `0001_init.sql` so the feature would not need a migration. This module is therefore only the
//! code around the contract, not a schema change.
//!
//! ## Two storage modes, one table
//!
//! * **Copy** — the bytes are laid down at `<data_dir>/files/<sha256>` and the library owns them.
//!   The blob is named by its content, so the same file attached to two books is stored once and
//!   `ux_files_sha_copy` rejects a second copy-mode row with the same hash.
//! * **Reference** — the file stays where the user keeps it; only its absolute path and hash are
//!   recorded. Nothing is copied, and nothing is deleted here.
//!
//! ## Why the copy happens before the row
//!
//! A row pointing at a missing blob is worse than no row at all: the book page would offer a file
//! that cannot be opened. Copying first means a failed insert can undo the copy, leaving the
//! library exactly as it was. The reverse order would leave an orphan blob behind, which is
//! harmless but never reclaimed.

use std::path::Path;

use maktabati_domain::{BookFile, BookFileInput, DomainError, StorageMode};
use rusqlite::OptionalExtension;

use crate::conn::{now_ms, Db};
use crate::error::{file_source_missing, not_found, on_unique, StorageError};
use crate::Result;

/// Columns in the order [`from_row`] reads them.
const COLUMNS: &str = "id, book_id, kind, original_name, storage_mode, rel_path, abs_path, \
                       sha256, size_bytes, mime, page_count, reader_position, added_at, updated_at";

/// `INSERT` covering every column.
const INSERT: &str = "INSERT INTO book_files (
    id, book_id, kind, original_name, storage_mode, rel_path, abs_path, sha256, size_bytes,
    mime, page_count, reader_position, added_at, updated_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BookFile> {
    Ok(BookFile {
        id: row.get(0)?,
        book_id: row.get(1)?,
        // A `CHECK`-constrained string falls back to the first variant when a *newer* build
        // wrote a value this one has never heard of: an attachment must not vanish from the book
        // page because its kind name moved on.
        kind: maktabati_domain::FileKind::from_db(&row.get::<_, String>(2)?)
            .unwrap_or(maktabati_domain::FileKind::Other),
        original_name: row.get(3)?,
        storage_mode: maktabati_domain::StorageMode::from_db(&row.get::<_, String>(4)?)
            .unwrap_or(StorageMode::Reference),
        rel_path: row.get(5)?,
        abs_path: row.get(6)?,
        sha256: row.get(7)?,
        size_bytes: row.get(8)?,
        mime: row.get(9)?,
        page_count: row.get(10)?,
        reader_position: row.get(11)?,
        added_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn read_row(conn: &rusqlite::Connection, id: &str) -> Result<BookFile> {
    let sql = format!("SELECT {COLUMNS} FROM book_files WHERE id = ?1");
    conn.query_row(&sql, [id], from_row).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => not_found("book_file", id),
        other => crate::StorageError::Sqlite(other),
    })
}

fn insert_row(conn: &rusqlite::Connection, file: &BookFile) -> Result<()> {
    conn.execute(
        INSERT,
        rusqlite::params![
            file.id,
            file.book_id,
            file.kind.as_str(),
            file.original_name,
            file.storage_mode.as_str(),
            file.rel_path,
            file.abs_path,
            file.sha256,
            file.size_bytes,
            file.mime,
            file.page_count,
            file.reader_position,
            file.added_at,
            file.updated_at,
        ],
    )
    // The collision that can actually happen is a second copy-mode row with the same content.
    // Reporting the file's own name (not its hash) keeps the message readable.
    .map_err(|e| on_unique("book_file", &file.original_name, e))?;
    Ok(())
}

/// `true` when `book_id` names a book. The FK would enforce this too, but SQLite's bare error
/// says only that a constraint failed — the caller would otherwise report "database error"
/// instead of "no such book".
fn book_exists(conn: &rusqlite::Connection, book_id: &str) -> Result<bool> {
    let found: Option<String> = conn
        .query_row("SELECT id FROM books WHERE id = ?1", [book_id], |r| r.get(0))
        .optional()?;
    Ok(found.is_some())
}

/// Attach a file to a book, copying it or merely referencing it.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the source file or the book does not exist.
/// * [`crate::StorageError::Duplicate`] when the same bytes are already in the library in copy
///   mode.
/// * [`crate::StorageError::Io`] when the file cannot be read, hashed or copied.
pub fn attach(db: &Db, data_dir: &Path, input: BookFileInput) -> Result<BookFile> {
    let source = Path::new(input.source_path.trim());
    let source_display = input.source_path.trim().to_owned();

    if !source.is_file() {
        return Err(file_source_missing(&source_display));
    }
    let original_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .ok_or_else(|| file_source_missing(&source_display))?;

    db.read(|conn| book_exists(conn, &input.book_id))?
        .then_some(())
        .ok_or_else(|| not_found("book", &input.book_id))?;

    let (size, sha256) = maktabati_files::hash_file(source)?;
    let size_bytes = i64::try_from(size).map_err(|_| {
        StorageError::Domain(DomainError::OutOfRange {
            field: "size_bytes".into(),
            got: size.to_string(),
            expected: format!("0..={}", maktabati_domain::file::limits::SIZE),
        })
    })?;

    let file = BookFile::new(&input, &original_name, sha256.clone(), size_bytes, now_ms())?;

    if input.storage_mode == StorageMode::Copy {
        let blob_was_present = maktabati_files::blob_exists(data_dir, &sha256);
        maktabati_files::store_blob(data_dir, &sha256, source)?;
        let inserted = db.write(|conn| insert_row(conn, &file));
        if inserted.is_err() && !blob_was_present {
            // Undo the copy so a rejected attachment leaves no orphan behind.
            if let Err(e) = maktabati_files::remove_blob(data_dir, &sha256) {
                tracing::warn!("could not remove orphaned blob {sha256}: {e}");
            }
        }
        inserted?;
    } else {
        db.write(|conn| insert_row(conn, &file))?;
    }

    Ok(file)
}

/// Every attachment on a book, oldest first.
///
/// # Errors
///
/// [`crate::StorageError::Sqlite`] on query failure.
pub fn list_for_book(db: &Db, book_id: &str) -> Result<Vec<BookFile>> {
    db.read(|conn| {
        let sql = format!(
            "SELECT {COLUMNS} FROM book_files WHERE book_id = ?1 ORDER BY added_at ASC, id ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([book_id], from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
}

/// Read one attachment.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id is unknown.
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn get(db: &Db, id: &str) -> Result<BookFile> {
    db.read(|conn| read_row(conn, id))
}

/// Detach a file, removing the copied blob when nothing else owns it.
///
/// The row goes first: a blob with no row is inert, while a row with no blob is a broken
/// attachment the UI would offer. Removing the blob is best-effort and never fails the call —
/// an orphaned blob costs disk space, a failed delete costs the user their record.
///
/// # Errors
///
/// * [`crate::StorageError::NotFound`] when the id is unknown.
/// * [`crate::StorageError::Sqlite`] on query failure.
pub fn delete(db: &Db, data_dir: &Path, id: &str) -> Result<()> {
    let file = get(db, id)?;
    db.write(|conn| {
        conn.execute("DELETE FROM book_files WHERE id = ?1", [id])?;
        Ok(())
    })?;

    if file.storage_mode == StorageMode::Copy {
        let still_owned = db.read(|conn| -> Result<bool> {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM book_files WHERE sha256 = ?1",
                [&file.sha256],
                |r| r.get(0),
            )?;
            Ok(count > 0)
        })?;
        if !still_owned {
            if let Err(e) = maktabati_files::remove_blob(data_dir, &file.sha256) {
                tracing::warn!("could not remove blob {}: {e}", file.sha256);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use maktabati_domain::{BookInput, FileKind};
    use maktabati_files as files;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    /// A library with one book and one attachment to attach to, in a temp directory.
    fn library() -> (tempfile::TempDir, Db, PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("l.db")).unwrap();
        crate::migrate::apply(&db, || Ok(())).unwrap();
        db.write(crate::seed::apply).unwrap();
        let book = crate::repo::book::create(&db, BookInput {
            title: "مقدمة".into(),
            ..Default::default()
        })
        .unwrap();
        let data = dir.path().join("data");
        (dir, db, data, book.id)
    }

    fn file_input(book_id: &str, path: &str, mode: StorageMode) -> BookFileInput {
        BookFileInput {
            book_id: book_id.into(),
            kind: FileKind::Epub,
            storage_mode: mode,
            source_path: path.into(),
            mime: None,
            page_count: None,
        }
    }

    #[test]
    fn attaching_in_copy_mode_stores_the_blob_and_the_row() {
        let (dir, db, data, book_id) = library();
        let source = dir.path().join("a.epub");
        std::fs::write(&source, b"book-bytes").unwrap();

        let attached = attach(&db, &data, file_input(&book_id, source.to_str().unwrap(), StorageMode::Copy)).unwrap();
        assert!(attached.rel_path.is_some());
        assert!(attached.abs_path.is_none());
        assert_eq!(attached.size_bytes, 10);
        assert!(files::blob_exists(&data, &attached.sha256));

        let listed = list_for_book(&db, &book_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, attached.id);
    }

    #[test]
    fn attaching_in_reference_mode_copies_nothing() {
        let (dir, db, data, book_id) = library();
        let source = dir.path().join("b.pdf");
        std::fs::write(&source, b"pdf-bytes").unwrap();

        let attached = attach(&db, &data, file_input(&book_id, source.to_str().unwrap(), StorageMode::Reference)).unwrap();
        assert!(attached.rel_path.is_none());
        assert_eq!(attached.abs_path.as_deref(), Some(source.to_str().unwrap()));
        assert!(
            !files::blob_path(&data, &attached.sha256).exists(),
            "reference mode must not copy the bytes"
        );
    }

    #[test]
    fn the_same_bytes_twice_in_copy_mode_are_rejected() {
        let (dir, db, data, book_id) = library();
        let a = dir.path().join("a.epub");
        let b = dir.path().join("b.epub");
        std::fs::write(&a, b"same-bytes").unwrap();
        std::fs::write(&b, b"same-bytes").unwrap();

        attach(&db, &data, file_input(&book_id, a.to_str().unwrap(), StorageMode::Copy)).unwrap();
        let second = attach(&db, &data, file_input(&book_id, b.to_str().unwrap(), StorageMode::Copy));
        assert!(second.is_err(), "ux_files_sha_copy must reject the second copy");
        assert_eq!(second.unwrap_err().code(), "storage.duplicate");
    }

    #[test]
    fn attaching_a_missing_file_names_the_source() {
        let (_dir, db, data, book_id) = library();
        let missing = "/definitely/not/here.epub";
        let err = attach(&db, &data, file_input(&book_id, missing, StorageMode::Copy))
            .unwrap_err();
        assert_eq!(err.code(), "storage.file_source_missing");
        match err {
            StorageError::FileSourceMissing { path } => assert_eq!(path, missing),
            other => panic!("expected FileSourceMissing, got {other:?}"),
        }
    }

    #[test]
    fn attaching_to_a_missing_book_is_not_found() {
        let (dir, db, data, _book_id) = library();
        let source = dir.path().join("a.epub");
        std::fs::write(&source, b"x").unwrap();
        let err = attach(&db, &data, file_input("no-such-book", source.to_str().unwrap(), StorageMode::Copy))
            .unwrap_err();
        assert_eq!(err.code(), "storage.not_found");
    }

    #[test]
    fn deleting_a_reference_keeps_the_original_file() {
        let (dir, db, data, book_id) = library();
        let source = dir.path().join("keep.pdf");
        std::fs::write(&source, b"keep").unwrap();
        let attached = attach(&db, &data, file_input(&book_id, source.to_str().unwrap(), StorageMode::Reference)).unwrap();

        delete(&db, &data, &attached.id).unwrap();
        assert!(list_for_book(&db, &book_id).unwrap().is_empty());
        assert!(source.is_file(), "reference mode must never delete the user's file");
        assert!(get(&db, &attached.id).is_err());
    }

    #[test]
    fn deleting_a_copy_removes_the_blob_only_when_unowned() {
        let (dir, db, data, book_id) = library();
        let a = dir.path().join("a.epub");
        let b = dir.path().join("b.epub");
        std::fs::write(&a, b"shared").unwrap();
        std::fs::write(&b, b"shared").unwrap();

        let first = attach(&db, &data, file_input(&book_id, a.to_str().unwrap(), StorageMode::Reference)).unwrap();
        // Reference rows share the hash too, so a copy-mode attach of the same bytes must not be
        // rejected — the unique index is partial (`WHERE storage_mode = 'copy'`).
        let second = attach(&db, &data, file_input(&book_id, b.to_str().unwrap(), StorageMode::Copy)).unwrap();

        delete(&db, &data, &first.id).unwrap();
        assert!(files::blob_exists(&data, &second.sha256), "still owned by the copy row");
        delete(&db, &data, &second.id).unwrap();
        assert!(!files::blob_exists(&data, &second.sha256));
    }

    #[test]
    fn an_empty_library_lists_nothing() {
        let (_dir, db, _data, book_id) = library();
        assert!(list_for_book(&db, &book_id).unwrap().is_empty());
    }
}
