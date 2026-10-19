//! `Publisher` entity and its write input.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::validate::{check_len, check_opt_len, check_required};
use crate::{ids::new_id, text::normalize, Result};

/// Length caps for publisher fields.
pub mod limits {
    /// Display-name cap.
    pub const NAME: usize = 300;
    /// Free-text cap (country, city, website…).
    pub const SHORT_TEXT: usize = 200;
    /// Notes cap.
    pub const NOTES: usize = 5_000;
}

/// A publishing house.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Publisher {
    /// UUID v7.
    pub id: String,
    /// Name as written.
    pub name: String,
    /// Normalized mirror for search and duplicate detection.
    pub name_norm: String,
    /// Country.
    pub country: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Website.
    pub website: Option<String>,
    /// Free-text notes.
    pub notes: Option<String>,
    /// Logo/cover path.
    pub cover_path: Option<String>,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or updating a [`Publisher`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct PublisherInput {
    /// Name (required).
    pub name: String,
    /// Country.
    pub country: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Website.
    pub website: Option<String>,
    /// Notes.
    pub notes: Option<String>,
    /// Cover path.
    pub cover_path: Option<String>,
}

impl Publisher {
    /// Build a new publisher, deriving `id`, the normalized mirror and timestamps.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::EmptyField`] when the name is blank.
    /// * [`crate::DomainError::TooLong`] when a field exceeds its cap.
    pub fn new(input: PublisherInput, now_ms: i64) -> Result<Self> {
        Self::check(&input)?;
        let name = input.name.trim().to_owned();
        let name_norm = normalize(&name);
        Ok(Self {
            id: new_id(),
            name,
            name_norm,
            country: input.country.filter(|s| !s.trim().is_empty()),
            city: input.city.filter(|s| !s.trim().is_empty()),
            website: input.website.filter(|s| !s.trim().is_empty()),
            notes: input.notes.filter(|s| !s.trim().is_empty()),
            cover_path: input.cover_path,
            created_at: now_ms,
            updated_at: now_ms,
        })
    }

    /// Recompute derived fields after an edit.
    ///
    /// # Errors
    ///
    /// Same as [`Publisher::new`] — an edit can never loosen what creation enforces.
    pub fn apply(&mut self, input: PublisherInput, now_ms: i64) -> Result<()> {
        Self::check(&input)?;
        self.name = input.name.trim().to_owned();
        self.name_norm = normalize(&self.name);
        self.country = input.country.filter(|s| !s.trim().is_empty());
        self.city = input.city.filter(|s| !s.trim().is_empty());
        self.website = input.website.filter(|s| !s.trim().is_empty());
        self.notes = input.notes.filter(|s| !s.trim().is_empty());
        self.cover_path = input.cover_path;
        self.updated_at = now_ms;
        Ok(())
    }

    /// Validate a raw input — shared by `new` and `apply`.
    fn check(input: &PublisherInput) -> Result<()> {
        let name = input.name.trim();
        check_required("name", name)?;
        check_len("name", name, limits::NAME)?;
        check_opt_len("country", input.country.as_deref(), limits::SHORT_TEXT)?;
        check_opt_len("city", input.city.as_deref(), limits::SHORT_TEXT)?;
        check_opt_len("website", input.website.as_deref(), limits::SHORT_TEXT)?;
        check_opt_len("notes", input.notes.as_deref(), limits::NOTES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_publisher_normalizes() {
        let p = Publisher::new(
            PublisherInput {
                name: " دار المعارف ".into(),
                ..Default::default()
            },
            5,
        )
        .unwrap();
        assert_eq!(p.name, "دار المعارف");
        assert_eq!(p.name_norm, "دار المعارف");
        assert_eq!(p.created_at, 5);
    }

    #[test]
    fn blank_name_is_rejected() {
        let err = Publisher::new(
            PublisherInput {
                name: "  ".into(),
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
    fn overlong_notes_are_rejected() {
        let err = Publisher::new(
            PublisherInput {
                name: "دار".into(),
                notes: Some("ن".repeat(limits::NOTES + 1)),
                ..Default::default()
            },
            1,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            crate::DomainError::TooLong {
                max: limits::NOTES,
                ..
            }
        ));
    }
}
