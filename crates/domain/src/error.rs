//! Typed domain errors.
//!
//! Every variant carries a **stable machine code** so `src-tauri` can turn it into an
//! `AppErrorDto { code, … }` and the webview can localize the message (ADR-0010).

use thiserror::Error;

/// Validation and rule failures raised by the domain layer.
///
/// The `code()` values are part of the IPC contract: adding a variant is fine, changing an
/// existing code is a breaking change for the frontend.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DomainError {
    /// A supplied ISBN failed its checksum or is malformed.
    #[error("invalid ISBN: {value}")]
    InvalidIsbn {
        /// The value as supplied by the user or a remote source.
        value: String,
    },

    /// A required textual field was empty or whitespace-only.
    #[error("field `{field}` must not be empty")]
    EmptyField {
        /// Machine name of the field (e.g. `title`).
        field: String,
    },

    /// A numeric field fell outside its permitted range.
    #[error("field `{field}` out of range: got {got}, expected {expected}")]
    OutOfRange {
        /// Machine name of the field.
        field: String,
        /// The rejected value, rendered for display.
        got: String,
        /// Human-readable description of the valid range.
        expected: String,
    },

    /// A string exceeded its maximum length.
    #[error("field `{field}` too long: {got} > {max}")]
    TooLong {
        /// Machine name of the field.
        field: String,
        /// Observed length in characters.
        got: usize,
        /// Permitted maximum in characters.
        max: usize,
    },

    /// A value was not one of the allowed enumeration members.
    #[error("field `{field}` has unsupported value `{got}`")]
    InvalidEnum {
        /// Machine name of the field.
        field: String,
        /// The rejected value.
        got: String,
    },

    /// A referenced entity does not exist.
    #[error("{kind} `{id}` not found")]
    NotFound {
        /// Entity kind, e.g. `book`, `author`.
        kind: String,
        /// Entity identifier.
        id: String,
    },

    /// A uniqueness constraint (ISBN, normalized name, category path) was violated.
    #[error("duplicate {kind}: `{key}`")]
    Duplicate {
        /// What was duplicated (`isbn`, `author`, `category_path`, …).
        kind: String,
        /// The conflicting normalized key.
        key: String,
    },

    /// An operation was attempted that the current state forbids.
    #[error("invalid state transition: {detail}")]
    InvalidTransition {
        /// Explanation of what was attempted.
        detail: String,
    },

    /// A note patch carried a stale revision (optimistic concurrency).
    #[error("revision conflict on {kind} `{id}`: expected {expected}, found {actual}")]
    RevisionConflict {
        /// Entity kind.
        kind: String,
        /// Entity identifier.
        id: String,
        /// Revision the caller believed current.
        expected: i64,
        /// Revision actually stored.
        actual: i64,
    },

    /// A path escaped its allowed root or was otherwise unsafe.
    #[error("unsafe path rejected: {detail}")]
    UnsafePath {
        /// Explanation of the rejection.
        detail: String,
    },
}

impl DomainError {
    /// Stable machine-readable code used as `AppErrorDto.code`.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidIsbn { .. } => "validation.isbn_invalid",
            Self::EmptyField { .. } => "validation.empty_field",
            Self::OutOfRange { .. } => "validation.out_of_range",
            Self::TooLong { .. } => "validation.too_long",
            Self::InvalidEnum { .. } => "validation.invalid_enum",
            Self::NotFound { .. } => "validation.not_found",
            Self::Duplicate { .. } => "validation.duplicate",
            Self::InvalidTransition { .. } => "validation.invalid_transition",
            Self::RevisionConflict { .. } => "validation.revision_conflict",
            Self::UnsafePath { .. } => "security.unsafe_path",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Every variant must map to a unique, namespaced code so the frontend can localize it.
    #[test]
    fn codes_are_unique_and_namespaced() {
        let all = [
            DomainError::InvalidIsbn { value: "x".into() },
            DomainError::EmptyField { field: "x".into() },
            DomainError::OutOfRange {
                field: "x".into(),
                got: "1".into(),
                expected: "0..=1".into(),
            },
            DomainError::TooLong {
                field: "x".into(),
                got: 2,
                max: 1,
            },
            DomainError::InvalidEnum {
                field: "x".into(),
                got: "y".into(),
            },
            DomainError::NotFound {
                kind: "x".into(),
                id: "y".into(),
            },
            DomainError::Duplicate {
                kind: "x".into(),
                key: "y".into(),
            },
            DomainError::InvalidTransition { detail: "x".into() },
            DomainError::RevisionConflict {
                kind: "x".into(),
                id: "y".into(),
                expected: 1,
                actual: 2,
            },
            DomainError::UnsafePath { detail: "x".into() },
        ];

        let mut seen = BTreeSet::new();
        for e in all {
            let code = e.code();
            assert!(code.contains('.'), "code `{code}` must be namespaced");
            assert!(seen.insert(code), "duplicate error code `{code}`");
        }
    }
}
