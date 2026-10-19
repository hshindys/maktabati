//! `Author` entity and its write input.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::validate::{check_len, check_lifespan, check_opt_len, check_required, check_year};
use crate::{ids::new_id, text::normalize, Direction, Result};

/// Length caps for author fields.
pub mod limits {
    /// Display-name cap.
    pub const NAME: usize = 300;
    /// Biography cap.
    pub const BIO: usize = 20_000;
    /// Free-text cap (nationality…).
    pub const SHORT_TEXT: usize = 200;
}

/// A person credited on a book (author, translator, editor, illustrator, foreword).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Author {
    /// UUID v7.
    pub id: String,
    /// Display name, exactly as the user wrote it (Arabic or Latin script).
    pub name: String,
    /// Normalized mirror used for search and duplicate detection.
    pub name_norm: String,
    /// Name in the author's own script, when different from `name`.
    pub name_original: Option<String>,
    /// Normalized mirror of `name_original`.
    pub name_original_norm: Option<String>,
    /// Biography.
    pub bio: Option<String>,
    /// Country or region of origin (free text, user-owned).
    pub nationality: Option<String>,
    /// Birth year.
    pub birth_year: Option<i32>,
    /// Death year (`None` for the living).
    pub death_year: Option<i32>,
    /// Relative path under `covers/`.
    pub photo_path: Option<String>,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or updating an [`Author`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct AuthorInput {
    /// Display name (required).
    pub name: String,
    /// Own-script name.
    pub name_original: Option<String>,
    /// Biography.
    pub bio: Option<String>,
    /// Nationality.
    pub nationality: Option<String>,
    /// Birth year.
    pub birth_year: Option<i32>,
    /// Death year.
    pub death_year: Option<i32>,
    /// Photo path.
    pub photo_path: Option<String>,
}

impl Author {
    /// Build a new author, deriving `id`, the normalized mirror and timestamps.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::EmptyField`] when the name is blank.
    /// * [`crate::DomainError::TooLong`] when a field exceeds its cap.
    /// * [`crate::DomainError::OutOfRange`] for an implausible year, or a death before birth.
    pub fn new(input: AuthorInput, now_ms: i64) -> Result<Self> {
        Self::check(&input)?;
        let name = input.name.trim().to_owned();
        let name_original = input
            .name_original
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_owned());
        let name_norm = normalize(&name);
        let name_original_norm = name_original.as_deref().map(normalize);
        Ok(Self {
            id: new_id(),
            name,
            name_norm,
            name_original,
            name_original_norm,
            bio: input.bio.filter(|s| !s.trim().is_empty()),
            nationality: input.nationality.filter(|s| !s.trim().is_empty()),
            birth_year: input.birth_year,
            death_year: input.death_year,
            photo_path: input.photo_path,
            created_at: now_ms,
            updated_at: now_ms,
        })
    }

    /// Recompute derived fields after an edit.
    ///
    /// # Errors
    ///
    /// Same as [`Author::new`] — an edit can never loosen what creation enforces.
    pub fn apply(&mut self, input: AuthorInput, now_ms: i64) -> Result<()> {
        Self::check(&input)?;
        self.name = input.name.trim().to_owned();
        self.name_norm = normalize(&self.name);
        self.name_original = input
            .name_original
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_owned());
        self.name_original_norm = self.name_original.as_deref().map(normalize);
        self.bio = input.bio.filter(|s| !s.trim().is_empty());
        self.nationality = input.nationality.filter(|s| !s.trim().is_empty());
        self.birth_year = input.birth_year;
        self.death_year = input.death_year;
        self.photo_path = input.photo_path;
        self.updated_at = now_ms;
        Ok(())
    }

    /// Validate a raw input — shared by `new` and `apply`.
    fn check(input: &AuthorInput) -> Result<()> {
        let name = input.name.trim();
        check_required("name", name)?;
        check_len("name", name, limits::NAME)?;
        check_opt_len(
            "name_original",
            input.name_original.as_deref(),
            limits::NAME,
        )?;
        check_opt_len("bio", input.bio.as_deref(), limits::BIO)?;
        check_opt_len(
            "nationality",
            input.nationality.as_deref(),
            limits::SHORT_TEXT,
        )?;
        check_year("birth_year", input.birth_year)?;
        check_year("death_year", input.death_year)?;
        check_lifespan(input.birth_year, input.death_year)
    }

    /// Writing direction of the author's name, used for per-field bidi isolation.
    #[must_use]
    pub fn direction(&self) -> Direction {
        if crate::text::contains_arabic(&self.name) {
            Direction::Rtl
        } else {
            Direction::Ltr
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_author_normalizes_name() {
        let a = Author::new(
            AuthorInput {
                name: "  أحمد فكري  ".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
        assert_eq!(a.name, "أحمد فكري");
        assert_eq!(a.name_norm, "احمد فكري");
        assert_eq!(a.direction(), Direction::Rtl);
    }

    #[test]
    fn apply_recomputes_derived_fields() {
        let mut a = Author::new(
            AuthorInput {
                name: "Tolstoy".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
        assert_eq!(a.name_norm, "tolstoy");
        a.apply(
            AuthorInput {
                name: "Leo Tolstoy".into(),
                ..Default::default()
            },
            2,
        )
        .unwrap();
        assert_eq!(a.name_norm, "leo tolstoy");
        assert_eq!(a.updated_at, 2);
        assert_eq!(a.created_at, 1);
    }

    #[test]
    fn blank_name_is_rejected() {
        let err = Author::new(
            AuthorInput {
                name: "   ".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap_err();
        assert_eq!(
            err,
            crate::DomainError::EmptyField {
                field: "name".into()
            }
        );
    }

    #[test]
    fn death_before_birth_is_rejected() {
        let err = Author::new(
            AuthorInput {
                name: "أحمد فكري".into(),
                birth_year: Some(1990),
                death_year: Some(1980),
                ..Default::default()
            },
            1,
        )
        .unwrap_err();
        assert_eq!(err.code(), "validation.out_of_range");
    }
}
