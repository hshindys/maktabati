//! The IPC error contract (ADR-0010).
//!
//! Rust-side failures are a real enum ([`AppError`]) with `From` conversions for every domain
//! and storage error. Across the IPC boundary they are **one shape**: [`AppErrorDto`] with a
//! stable machine `code`, a human message, an optional `i18n_key` and optional structured
//! `details`. The frontend switches on `code`, never on the English/Arabic text, so translating
//! a message is never a breaking change.
//!
//! The domain and storage crates deliberately know nothing about the wire format: every DTO
//! mapping lives here, at the boundary.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;

use maktabati_domain::Json;

/// The single error type every Tauri command returns.
///
/// `code` is a stable, dotted identifier that the webview maps to a localized string and to a
/// recovery affordance. `message` is the developer/English text — useful in logs and in the
/// "copy diagnostics" button, not shown raw to end users when a translation exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct AppErrorDto {
    /// Stable machine-readable identifier, e.g. `validation.isbn_invalid`.
    pub code: String,
    /// Human-readable message (English). Not a user-facing string — see `i18n_key`.
    pub message: String,
    /// Key the frontend looks up in the active locale for the user-facing text.
    pub i18n_key: Option<String>,
    /// Structured context (field name, conflicting id, expected bounds…).
    ///
    /// [`Json`] rather than a bare `serde_json::Value`: specta's built-in impl for `Value`
    /// expands structurally at every use site and never terminates, so a `Value` anywhere in the
    /// exported surface hangs the bindings generator. See `maktabati_domain::json`.
    pub details: Option<Json>,
}

impl AppErrorDto {
    /// Build a DTO from a stable code and a message, with no extra context.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            i18n_key: None,
            details: None,
        }
    }

    /// Attach an i18n key for the user-facing message.
    #[must_use]
    pub fn with_i18n_key(mut self, key: impl Into<String>) -> Self {
        self.i18n_key = Some(key.into());
        self
    }

    /// Attach structured details.
    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(Json::from(details));
        self
    }
}

impl fmt::Display for AppErrorDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppErrorDto {}

/// Internal error enum. Never crosses IPC directly — [`AppError::into_dto`] maps it.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// A persistence failure from `maktabati-storage`.
    #[error(transparent)]
    Storage(#[from] maktabati_storage::StorageError),
    /// A domain rule violation (invalid ISBN, illegal status transition…).
    #[error(transparent)]
    Domain(#[from] maktabati_domain::DomainError),
    /// Something unexpected — logged, reported generically.
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Map to the wire shape. Codes are passed through unchanged so the frontend can match on
    /// them; `details` are derived from the originating error's fields.
    #[must_use]
    pub fn into_dto(self) -> AppErrorDto {
        match self {
            AppError::Storage(e) => AppErrorDto {
                code: e.code().to_owned(),
                message: e.to_string(),
                i18n_key: Some(format!("error.{}", e.code())),
                details: storage_details(&e).map(Json::from),
            },
            AppError::Domain(e) => AppErrorDto {
                code: e.code().to_owned(),
                message: e.to_string(),
                i18n_key: Some(format!("error.{}", e.code())),
                details: Some(Json::from(domain_details(&e))),
            },
            AppError::Internal(msg) => AppErrorDto {
                code: "internal".to_owned(),
                message: msg,
                i18n_key: Some("error.internal".to_owned()),
                details: None,
            },
        }
    }
}

/// Structured context for a domain error, used as `AppErrorDto.details`.
fn domain_details(e: &maktabati_domain::DomainError) -> serde_json::Value {
    use maktabati_domain::DomainError as D;
    match e {
        D::InvalidIsbn { value } => json!({ "field": "isbn", "value": value }),
        D::EmptyField { field } => json!({ "field": field }),
        D::OutOfRange {
            field,
            got,
            expected,
        } => {
            json!({ "field": field, "got": got, "expected": expected })
        }
        D::TooLong { field, got, max } => json!({ "field": field, "got": got, "max": max }),
        D::InvalidEnum { field, got } => json!({ "field": field, "got": got }),
        D::NotFound { kind, id } => json!({ "kind": kind, "id": id }),
        D::Duplicate { kind, key } => json!({ "kind": kind, "key": key }),
        D::InvalidTransition { detail } => json!({ "detail": detail }),
        D::RevisionConflict {
            kind,
            id,
            expected,
            actual,
        } => json!({ "kind": kind, "id": id, "expected": expected, "actual": actual }),
        D::UnsafePath { detail } => json!({ "detail": detail }),
        // `DomainError` is `#[non_exhaustive]`: a newer `domain` may add a variant this build
        // does not know. `message` already carries `Display`, so there is nothing to add here —
        // but the arm must exist or forward compatibility would be a panic.
        _ => json!({}),
    }
}

/// Structured context for a storage error, used as `AppErrorDto.details`.
fn storage_details(e: &maktabati_storage::StorageError) -> Option<serde_json::Value> {
    use maktabati_storage::StorageError as S;
    match e {
        S::MigrationChecksumMismatch {
            version,
            name,
            db,
            file,
        } => Some(json!({
            "version": version, "name": name, "db_checksum": db, "file_checksum": file
        })),
        S::NotFound { kind, id } => Some(json!({ "kind": kind, "id": id })),
        S::Duplicate { kind, key } => Some(json!({ "kind": kind, "key": key })),
        // `Sqlite`/`Io`/`Serde`/`Migration`/`Busy` carry their detail in the message already.
        _ => None,
    }
}

/// Convenience alias so every command has one obvious error type.
pub type CmdResult<T> = Result<T, AppErrorDto>;

/// Turn a `Result<_, E>` into a [`CmdResult`], where `AppError` knows how to build from `E`.
pub trait IntoCmdResult<T> {
    /// Convert, mapping the error to the wire DTO.
    fn into_cmd(self) -> CmdResult<T>;
}

impl<T, E> IntoCmdResult<T> for Result<T, E>
where
    AppError: From<E>,
{
    fn into_cmd(self) -> CmdResult<T> {
        self.map_err(|e| AppError::from(e).into_dto())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn dto_serializes_to_the_documented_shape() {
        let dto = AppErrorDto::new("validation.duplicate", "duplicate book")
            .with_i18n_key("error.validation.duplicate")
            .with_details(json!({ "kind": "book", "key": "9780306406157" }));
        let v = serde_json::to_value(&dto).unwrap();
        assert_eq!(v["code"], "validation.duplicate");
        assert_eq!(v["message"], "duplicate book");
        assert_eq!(v["i18n_key"], "error.validation.duplicate");
        assert_eq!(v["details"]["kind"], "book");
        assert_eq!(v["details"]["key"], "9780306406157");
        assert!(
            v.get("kind").is_none(),
            "no redundant tag field on a struct"
        );
    }

    #[test]
    fn storage_errors_keep_their_stable_code() {
        let e = AppError::Storage(maktabati_storage::StorageError::NotFound {
            kind: "book".into(),
            id: "b1".into(),
        });
        let dto = e.into_dto();
        assert_eq!(dto.code, "storage.not_found");
        assert_eq!(dto.i18n_key.as_deref(), Some("error.storage.not_found"));
        assert_eq!(
            dto.details,
            Some(json!({ "kind": "book", "id": "b1" }).into()),
            "details carry the whole failing record, not just one of its fields"
        );
    }

    #[test]
    fn domain_errors_are_namespaced_and_carry_details() {
        let dto = AppError::Domain(maktabati_domain::DomainError::InvalidIsbn {
            value: "9780306406158".into(),
        })
        .into_dto();
        assert_eq!(dto.code, "validation.isbn_invalid");
        assert_eq!(
            dto.i18n_key.as_deref(),
            Some("error.validation.isbn_invalid")
        );
        assert_eq!(
            dto.details,
            Some(json!({ "field": "isbn", "value": "9780306406158" }).into()),
            "details name the rejected field alongside the value that failed it"
        );
    }

    #[test]
    fn internal_errors_are_generic_and_context_free() {
        let dto = AppError::Internal("kaboom".into()).into_dto();
        assert_eq!(dto.code, "internal");
        assert_eq!(dto.i18n_key.as_deref(), Some("error.internal"));
        assert!(dto.details.is_none());
    }

    #[test]
    fn sql_and_io_failures_carry_no_details() {
        let e = AppError::Storage(maktabati_storage::StorageError::Busy).into_dto();
        assert_eq!(e.code, "storage.busy");
        assert!(e.details.is_none());
    }

    #[test]
    fn into_cmd_maps_domain_errors() {
        let r: Result<(), maktabati_domain::DomainError> =
            Err(maktabati_domain::DomainError::InvalidIsbn {
                value: "bad".into(),
            });
        let err = r.into_cmd().unwrap_err();
        assert_eq!(err.code, "validation.isbn_invalid");
    }

    #[test]
    fn into_cmd_passes_through_success() {
        let r: Result<i32, maktabati_storage::StorageError> = Ok(7);
        assert_eq!(r.into_cmd().unwrap(), 7);
    }

    #[test]
    fn dto_round_trips_through_json() {
        let dto = AppErrorDto::new("x", "y");
        let back: AppErrorDto =
            serde_json::from_str(&serde_json::to_string(&dto).unwrap()).unwrap();
        assert_eq!(back, dto);
    }
}
