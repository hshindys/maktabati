//! `BookFile` — an attachment on a book: ePub, PDF, audio, and so on.
//!
//! An attachment is one row of `book_files`, which already exists in
//! `crates/storage/migrations/0001_init.sql`. The table was written ahead of this module so the
//! schema would not have to change when the feature landed; only the code around it is new.
//!
//! The entity is a plain value, like every other: `sha256` and `size_bytes` are filled in by
//! `storage` *after* the bytes are read, because this crate performs no I/O.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    ids::new_id,
    validate::{check_len, check_opt_len, check_required},
    DomainError, FileKind, Result, StorageMode,
};

/// Field caps for an attachment.
pub mod limits {
    /// Directory, under the app's data directory, that owns copied attachments.
    ///
    /// Declared here rather than in `maktabati-files`: this crate owns the row's `rel_path`
    /// invariant and cannot depend on the files crate without a cycle. `maktabati-files`
    /// re-exports it, so the name has exactly one source.
    pub const FILES_SUBDIR: &str = "files";
    /// File name cap (file systems cap a component at 255 bytes; this is generous).
    pub const NAME: usize = 255;
    /// MIME type cap.
    pub const MIME: usize = 100;
    /// Reader position cap (an ePub CFI, a page number or a percentage).
    pub const POSITION: usize = 200;
    /// Maximum accepted attachment size, 2 GiB.
    ///
    /// Well past any real book, but a bound nonetheless: the value is stored in an SQLite
    /// `INTEGER` and shown in the UI, and an unbounded number invites a copy that never finishes.
    pub const SIZE: i64 = 2 * 1024 * 1024 * 1024;
}

/// An attachment belonging to a book.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct BookFile {
    /// UUID v7.
    pub id: String,
    /// Owning book.
    pub book_id: String,
    /// What kind of file it is.
    pub kind: FileKind,
    /// Name the file had when the user picked it.
    pub original_name: String,
    /// Whether the library owns the bytes.
    pub storage_mode: StorageMode,
    /// Path under `<data_dir>/files/`, set in copy mode.
    pub rel_path: Option<String>,
    /// Absolute path, set in reference mode.
    pub abs_path: Option<String>,
    /// Lower-case hex SHA-256 of the bytes.
    pub sha256: String,
    /// Size in bytes.
    pub size_bytes: i64,
    /// MIME type as reported by the picker.
    pub mime: Option<String>,
    /// Page count, when the picker could read it.
    pub page_count: Option<i32>,
    /// Last reading position marker.
    pub reader_position: Option<String>,
    /// When the attachment was added, epoch ms UTC.
    pub added_at: i64,
    /// When the attachment row last changed, epoch ms UTC.
    pub updated_at: i64,
}

/// What the UI knows about an attachment before its bytes are read.
///
/// The identity of an attachment is its content, so `sha256` and `size_bytes` cannot be part of
/// this: they are measured by `storage` from `source_path`. `kind` and `storage_mode` are chosen
/// in the wizard — `kind` drives the filter on the picker, `storage_mode` decides whether the
/// library copies the bytes or just remembers where they are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct BookFileInput {
    /// Book the file belongs to (required; must already exist).
    pub book_id: String,
    /// Kind of attachment.
    pub kind: FileKind,
    /// How it will be stored.
    pub storage_mode: StorageMode,
    /// Absolute path the bytes are read from.
    pub source_path: String,
    /// MIME type, when known.
    pub mime: Option<String>,
    /// Page count, when known.
    pub page_count: Option<i32>,
}

impl BookFile {
    /// Build a stored attachment row from the file's measured facts.
    ///
    /// # Errors
    ///
    /// * [`DomainError::EmptyField`] when the book id or the file name is blank.
    /// * [`DomainError::TooLong`] when a field exceeds its cap.
    /// * [`DomainError::OutOfRange`] when the size is negative or above [`limits::SIZE`].
    pub fn new(
        input: &BookFileInput,
        original_name: &str,
        sha256: String,
        size_bytes: i64,
        now_ms: i64,
    ) -> Result<Self> {
        check_required("book_id", &input.book_id)?;
        check_required("source_path", &input.source_path)?;

        let name = original_name.trim().to_owned();
        check_required("original_name", &name)?;
        check_len("original_name", &name, limits::NAME)?;
        check_opt_len("mime", input.mime.as_deref(), limits::MIME)?;

        if !(0..=limits::SIZE).contains(&size_bytes) {
            return Err(DomainError::OutOfRange {
                field: "size_bytes".into(),
                got: size_bytes.to_string(),
                expected: format!("0..={}", limits::SIZE),
            });
        }

        // Copy mode stores one blob per content hash, so the row's own path is derived from it;
        // reference mode keeps the user's path. Exactly one of the two is set, and the database
        // does not enforce that — the invariant belongs to the writer, and this is the only one.
        let (rel_path, abs_path) = match input.storage_mode {
            StorageMode::Copy => (
                Some(format!("{}.{}", input.kind.as_str(), &sha256[..12])),
                None,
            ),
            StorageMode::Reference => (None, Some(input.source_path.trim().to_owned())),
        };

        Ok(Self {
            id: new_id(),
            book_id: input.book_id.trim().to_owned(),
            kind: input.kind,
            original_name: name,
            storage_mode: input.storage_mode,
            rel_path,
            abs_path,
            sha256,
            size_bytes,
            mime: input
                .mime
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            page_count: input.page_count,
            reader_position: None,
            added_at: now_ms,
            updated_at: now_ms,
        })
    }

    /// The absolute path this attachment is stored at, once the library's data directory is known.
    ///
    /// `None` in copy mode would mean the blob is missing — callers treat that as an absent file,
    /// never as a crash, so a half-finished copy never takes the app down with it.
    #[must_use]
    pub fn abs_path_in(&self, data_dir: &std::path::Path) -> Option<std::path::PathBuf> {
        match self.storage_mode {
            StorageMode::Copy => Some(data_dir.join(limits::FILES_SUBDIR).join(&self.sha256)),
            StorageMode::Reference => self.abs_path.as_deref().map(std::path::PathBuf::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn input() -> BookFileInput {
        BookFileInput {
            book_id: "b1".into(),
            kind: FileKind::Epub,
            storage_mode: StorageMode::Copy,
            source_path: "/home/hatem/Books/a.epub".into(),
            mime: Some("application/epub+zip".into()),
            page_count: None,
        }
    }

    #[test]
    fn builds_a_copy_row_with_a_derived_relative_path() {
        let f = BookFile::new(&input(), "a.epub", "deadbeef".repeat(8), 1234, 100).unwrap();
        assert_eq!(f.book_id, "b1");
        assert_eq!(f.original_name, "a.epub");
        assert_eq!(f.sha256, "deadbeef".repeat(8));
        assert_eq!(f.size_bytes, 1234);
        assert!(f.rel_path.as_deref().unwrap().starts_with("epub."));
        assert!(
            f.abs_path.is_none(),
            "copy mode must not record the user's path"
        );
        assert_eq!(f.reader_position, None);
        assert_eq!(f.added_at, 100);
        assert_eq!(f.updated_at, 100);
    }

    #[test]
    fn reference_mode_keeps_the_user_path() {
        let mut i = input();
        i.storage_mode = StorageMode::Reference;
        let f = BookFile::new(&i, "a.epub", "ab".repeat(32), 5, 1).unwrap();
        assert!(f.rel_path.is_none());
        assert_eq!(f.abs_path.as_deref(), Some("/home/hatem/Books/a.epub"));
    }

    #[test]
    fn blank_book_id_or_name_is_rejected() {
        let mut i = input();
        i.book_id = "  ".into();
        assert!(BookFile::new(&i, "a.epub", "ab".repeat(32), 1, 1).is_err());
        assert!(BookFile::new(&input(), "   ", "ab".repeat(32), 1, 1).is_err());
    }

    #[test]
    fn rejects_impossible_sizes() {
        for size in [-1, limits::SIZE + 1] {
            assert!(BookFile::new(&input(), "a.epub", "ab".repeat(32), size, 1).is_err());
        }
        assert!(BookFile::new(&input(), "a.epub", "ab".repeat(32), 0, 1).is_ok());
    }

    #[test]
    fn rejects_overlong_name() {
        let long = "ا".repeat(limits::NAME + 1);
        assert!(BookFile::new(&input(), &long, "ab".repeat(32), 1, 1).is_err());
    }

    #[test]
    fn blank_mime_is_absent_not_empty() {
        let mut i = input();
        i.mime = Some("   ".into());
        let f = BookFile::new(&i, "a.epub", "ab".repeat(32), 1, 1).unwrap();
        assert_eq!(f.mime, None);
    }

    #[test]
    fn abs_path_in_follows_the_storage_mode() {
        let data = std::path::Path::new("/tmp/library");
        let copy = BookFile::new(&input(), "a.epub", "ab".repeat(32), 1, 1).unwrap();
        assert_eq!(
            copy.abs_path_in(data),
            Some(data.join("files").join("ab".repeat(32)))
        );

        let mut i = input();
        i.storage_mode = StorageMode::Reference;
        let reference = BookFile::new(&i, "a.epub", "ab".repeat(32), 1, 1).unwrap();
        assert_eq!(
            reference.abs_path_in(data),
            Some(std::path::PathBuf::from("/home/hatem/Books/a.epub"))
        );
    }
}
