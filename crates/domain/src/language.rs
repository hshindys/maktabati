//! `Language` — the reference rows a book can be written in.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::validate::{check_len, check_required};
use crate::{Direction, Result};

/// Length caps for language fields.
pub mod limits {
    /// Display-name cap, per script.
    pub const NAME: usize = 80;
    /// Primary-key cap (`ar`, `pt-BR`…).
    pub const CODE: usize = 16;
}

/// A catalogue language (`ar`, `en`, `fr`…).
///
/// Rows are seeded rather than invented casually: a language decides script, direction
/// and date formatting, so a typo'd code would change how every book already stored
/// under it renders. Creation exists for the languages the seed does not ship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Language {
    /// ISO-ish code, also the primary key.
    pub code: String,
    /// Arabic display name.
    pub name_ar: String,
    /// English display name.
    pub name_en: String,
    /// Reading/writing direction.
    pub direction: Direction,
    /// Whether the language shows in pickers.
    pub is_active: bool,
    /// Sort position.
    pub position: i32,
}

/// Input for creating or updating a [`Language`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct LanguageInput {
    /// Code (required; primary key).
    pub code: String,
    /// Arabic display name (required).
    pub name_ar: String,
    /// English display name (required).
    pub name_en: String,
    /// Reading/writing direction.
    pub direction: Direction,
    /// Whether the language shows in pickers.
    pub is_active: bool,
    /// Sort position.
    pub position: i32,
}

impl Default for LanguageInput {
    fn default() -> Self {
        Self {
            code: String::new(),
            name_ar: String::new(),
            name_en: String::new(),
            direction: Direction::Ltr,
            is_active: true,
            position: 0,
        }
    }
}

impl Language {
    /// Validate an input and build the row (creation; the code *is* the id).
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::EmptyField`] for a blank code or display name.
    /// * [`crate::DomainError::TooLong`] when a field exceeds its cap.
    pub fn new(input: LanguageInput) -> Result<Self> {
        Self::validate(&input)?;
        Ok(Self {
            code: input.code.trim().to_owned(),
            name_ar: input.name_ar.trim().to_owned(),
            name_en: input.name_en.trim().to_owned(),
            direction: input.direction,
            is_active: input.is_active,
            position: input.position,
        })
    }

    /// Apply an edit in place (the code is immutable — it is the primary key and the
    /// value every book's `language_code` points at).
    ///
    /// # Errors
    ///
    /// Same as [`Language::new`].
    pub fn apply(&mut self, input: LanguageInput) -> Result<()> {
        Self::validate(&input)?;
        self.name_ar = input.name_ar.trim().to_owned();
        self.name_en = input.name_en.trim().to_owned();
        self.direction = input.direction;
        self.is_active = input.is_active;
        self.position = input.position;
        Ok(())
    }

    fn validate(input: &LanguageInput) -> Result<()> {
        let code = input.code.trim();
        check_required("code", code)?;
        check_len("code", code, limits::CODE)?;
        let name_ar = input.name_ar.trim();
        check_required("name_ar", name_ar)?;
        check_len("name_ar", name_ar, limits::NAME)?;
        let name_en = input.name_en.trim();
        check_required("name_en", name_en)?;
        check_len("name_en", name_en, limits::NAME)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DomainError;
    use pretty_assertions::assert_eq;

    fn arabic() -> LanguageInput {
        LanguageInput {
            code: " ar ".into(),
            name_ar: " العربية ".into(),
            name_en: "Arabic".into(),
            direction: Direction::Rtl,
            ..Default::default()
        }
    }

    #[test]
    fn new_language_trims_fields() {
        let l = Language::new(arabic()).unwrap();
        assert_eq!(l.code, "ar");
        assert_eq!(l.name_ar, "العربية");
        assert_eq!(l.direction, Direction::Rtl);
        assert!(l.is_active);
    }

    #[test]
    fn blank_code_is_rejected() {
        let err = Language::new(LanguageInput {
            code: "  ".into(),
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(
            err,
            DomainError::EmptyField {
                field: "code".into()
            }
        );
    }

    #[test]
    fn direction_round_trips_through_the_db_literal() {
        let l = Language::new(arabic()).unwrap();
        assert_eq!(l.direction.as_str(), "rtl");
        assert_eq!(
            Direction::from_db(l.direction.as_str()),
            Some(Direction::Rtl)
        );
    }

    #[test]
    fn apply_cannot_change_the_code() {
        let mut l = Language::new(arabic()).unwrap();
        l.apply(LanguageInput {
            code: "fr".into(),
            name_en: "French".into(),
            ..arabic()
        })
        .unwrap();
        assert_eq!(l.code, "ar", "the primary key must stay put");
        assert_eq!(l.name_en, "French");
    }
}
