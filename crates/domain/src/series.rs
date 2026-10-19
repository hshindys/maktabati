//! `Series` entity and its write input.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::validate::{check_len, check_opt_len, check_required};
use crate::{ids::new_id, text::normalize, Result};

/// Length caps for series fields.
pub mod limits {
    /// Series-name cap.
    pub const NAME: usize = 300;
    /// Description cap.
    pub const DESCRIPTION: usize = 5_000;
}

/// A book series (السلسلة).
///
/// A book belongs to at most one series in v1 — a deliberate simplification documented in
/// `docs/03-data-model.md`; multi-series membership would complicate the add form for a case
/// that is rare in practice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Series {
    /// UUID v7.
    pub id: String,
    /// Name as written.
    pub name: String,
    /// Normalized mirror.
    pub name_norm: String,
    /// Description.
    pub description: Option<String>,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or updating a [`Series`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct SeriesInput {
    /// Name (required).
    pub name: String,
    /// Description.
    pub description: Option<String>,
}

impl Series {
    /// Build a new series.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::EmptyField`] when the name is blank.
    /// * [`crate::DomainError::TooLong`] when a field exceeds its cap.
    pub fn new(input: SeriesInput, now_ms: i64) -> Result<Self> {
        Self::check(&input)?;
        let name = input.name.trim().to_owned();
        let name_norm = normalize(&name);
        Ok(Self {
            id: new_id(),
            name,
            name_norm,
            description: input.description.filter(|s| !s.trim().is_empty()),
            created_at: now_ms,
            updated_at: now_ms,
        })
    }

    /// Recompute derived fields after an edit.
    ///
    /// # Errors
    ///
    /// Same as [`Series::new`] — an edit can never loosen what creation enforces.
    pub fn apply(&mut self, input: SeriesInput, now_ms: i64) -> Result<()> {
        Self::check(&input)?;
        self.name = input.name.trim().to_owned();
        self.name_norm = normalize(&self.name);
        self.description = input.description.filter(|s| !s.trim().is_empty());
        self.updated_at = now_ms;
        Ok(())
    }

    /// Validate a raw input — shared by `new` and `apply`.
    fn check(input: &SeriesInput) -> Result<()> {
        let name = input.name.trim();
        check_required("name", name)?;
        check_len("name", name, limits::NAME)?;
        check_opt_len(
            "description",
            input.description.as_deref(),
            limits::DESCRIPTION,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DomainError;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_series_trims_and_normalizes() {
        let s = Series::new(
            SeriesInput {
                name: "  مقدمة في الحضارة ".into(),
                description: None,
            },
            9,
        )
        .unwrap();
        assert_eq!(s.name, "مقدمة في الحضارة");
        assert_eq!(s.name_norm, normalize("مقدمة في الحضارة"));
        assert_eq!(s.created_at, 9);
    }

    #[test]
    fn blank_name_is_rejected() {
        let err = Series::new(
            SeriesInput {
                name: "   ".into(),
                description: None,
            },
            1,
        )
        .unwrap_err();
        assert_eq!(
            err,
            DomainError::EmptyField {
                field: "name".into()
            }
        );
    }

    #[test]
    fn apply_keeps_id_and_recomputes_the_mirror() {
        let mut s = Series::new(
            SeriesInput {
                name: "a".into(),
                description: Some("old".into()),
            },
            1,
        )
        .unwrap();
        let id = s.id.clone();
        s.apply(
            SeriesInput {
                name: "  ب  ".into(),
                description: None,
            },
            2,
        )
        .unwrap();
        assert_eq!(s.id, id);
        assert_eq!(s.name, "ب");
        assert_eq!(s.description, None);
        assert_eq!(s.updated_at, 2);
    }
}
