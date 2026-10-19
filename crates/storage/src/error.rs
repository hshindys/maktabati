//! Typed storage errors.
//!
//! `code()` values are stable and cross the IPC boundary as `AppErrorDto.code` (ADR-0010).

use thiserror::Error;

/// Failures produced by the persistence layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StorageError {
    /// Underlying SQLite failure.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// A migration file is missing, malformed or out of order.
    #[error("migration error: {0}")]
    Migration(String),

    /// A migration already applied to this database no longer matches the file on disk.
    ///
    /// This is a hard stop: someone edited an applied migration, which means the database and
    /// the code no longer describe the same schema.
    #[error("applied migration {version} ({name}) checksum mismatch: db={db} file={file}")]
    MigrationChecksumMismatch {
        /// Migration version.
        version: i64,
        /// Migration name.
        name: String,
        /// Checksum recorded in the database.
        db: String,
        /// Checksum of the embedded file.
        file: String,
    },

    /// A domain validation rule rejected the input.
    #[error(transparent)]
    Domain(#[from] maktabati_domain::DomainError),

    /// Serialization failure (JSON columns).
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// A referenced row does not exist.
    #[error("{kind} `{id}` not found")]
    NotFound {
        /// Entity kind.
        kind: String,
        /// Entity id.
        id: String,
    },

    /// A uniqueness constraint was violated by user data.
    #[error("duplicate {kind}: `{key}`")]
    Duplicate {
        /// What was duplicated.
        kind: String,
        /// The conflicting key.
        key: String,
    },

    /// A file-system operation failed while opening or backing up the database.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The database is busy beyond `busy_timeout`.
    #[error("database is busy")]
    Busy,
}

impl StorageError {
    /// Stable machine-readable code used as `AppErrorDto.code`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Sqlite(e) => sqlite_code(e),
            Self::Migration(_) => "storage.migration",
            Self::MigrationChecksumMismatch { .. } => "storage.migration_checksum",
            Self::Domain(d) => d.code(),
            Self::Serde(_) => "storage.serialization",
            Self::NotFound { .. } => "storage.not_found",
            Self::Duplicate { .. } => "storage.duplicate",
            Self::Io(_) => "storage.io",
            Self::Busy => "storage.busy",
        }
    }
}

/// Map a [`rusqlite::Error`] onto the app's stable error codes.
///
/// Constraint *detail* lives in SQLite's extended result code: the primary code is always the
/// generic `SQLITE_CONSTRAINT`, so testing the primary code alone would collapse unique,
/// foreign-key and check violations into one bucket. Extended codes are therefore tested first.
///
/// `busy` is the reverse case — SQLite reports it as a primary code (`ErrorCode::DatabaseBusy`)
/// with extended variants such as `SQLITE_BUSY_SNAPSHOT`, so the `ErrorCode` enum is matched
/// rather than the raw integer, which would miss those variants.
fn sqlite_code(err: &rusqlite::Error) -> &'static str {
    use rusqlite::ffi::{self, ErrorCode};

    match err {
        rusqlite::Error::QueryReturnedNoRows => "storage.not_found",
        rusqlite::Error::SqliteFailure(failure, _) => match failure.extended_code {
            ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                "storage.unique_violation"
            }
            ffi::SQLITE_CONSTRAINT_FOREIGNKEY => "storage.foreign_key_violation",
            _ => match failure.code {
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => "storage.busy",
                ErrorCode::ConstraintViolation => "storage.constraint_violation",
                ErrorCode::ReadOnly => "storage.read_only",
                ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => "storage.corrupt",
                ErrorCode::CannotOpen => "storage.cannot_open",
                _ => "storage.sqlite",
            },
        },
        _ => "storage.sqlite",
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Translate a unique-constraint SQLite error into a friendlier [`StorageError::Duplicate`].
///
/// Used by repositories when a `CREATE` collides with an existing normalized name / ISBN.
#[must_use]
pub fn duplicate(kind: &str, key: &str) -> StorageError {
    StorageError::Duplicate {
        kind: kind.to_owned(),
        key: key.to_owned(),
    }
}

/// Translate a missing-row result into [`StorageError::NotFound`].
#[must_use]
pub fn not_found(kind: &str, id: &str) -> StorageError {
    StorageError::NotFound {
        kind: kind.to_owned(),
        id: id.to_owned(),
    }
}
