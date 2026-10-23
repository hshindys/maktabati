//! Field validation shared by every entity input.
//!
//! `Book` has always validated its input before it became a value object; `Author`,
//! `Publisher`, `Series`, `Tag` and `Language` reach the same bar in the catalogue
//! milestone, so the checks live here once instead of being copied into each entity.
//! The wire-facing error codes are produced by `DomainError`, so validation failures
//! travel to the UI as the same localized keys whether they happen in a form or in a
//! repository.

use crate::{DomainError, Result};

/// Reject a required field that is blank once trimmed.
///
/// Every caller trims before storing, and this check makes the rule hold even if one
/// forgets: a name of `"   "` is a blank name, not a name.
pub(crate) fn check_required(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(DomainError::EmptyField {
            field: field.into(),
        });
    }
    Ok(())
}

/// Reject a string longer than `max` **characters**, not bytes.
///
/// Arabic sits at 2–3 bytes per character, so a byte cap would halve the effective
/// limit for the locale the product is built for.
pub(crate) fn check_len(field: &str, value: &str, max: usize) -> Result<()> {
    let len = value.chars().count();
    if len > max {
        return Err(DomainError::TooLong {
            field: field.into(),
            got: len,
            max,
        });
    }
    Ok(())
}

/// Reject an optional field that exceeds its cap; blank counts as absent.
pub(crate) fn check_opt_len(field: &str, value: Option<&str>, max: usize) -> Result<()> {
    match value {
        Some(v) if !v.trim().is_empty() => check_len(field, v, max),
        _ => Ok(()),
    }
}

/// Reject an out-of-range year. `None` means "not supplied" and is fine.
pub(crate) fn check_year(field: &str, year: Option<i32>) -> Result<()> {
    if let Some(y) = year {
        if !(1..=9999).contains(&y) {
            return Err(DomainError::OutOfRange {
                field: field.into(),
                got: y.to_string(),
                expected: "1..=9999".into(),
            });
        }
    }
    Ok(())
}

/// Reject an optional page number that is not positive (`None` = "not supplied").
pub(crate) fn check_page(field: &str, page: Option<i32>) -> Result<()> {
    if let Some(p) = page {
        if p <= 0 {
            return Err(DomainError::OutOfRange {
                field: field.into(),
                got: p.to_string(),
                expected: "> 0".into(),
            });
        }
    }
    Ok(())
}

/// Reject a death year earlier than the birth year (both optional).
pub(crate) fn check_lifespan(birth: Option<i32>, death: Option<i32>) -> Result<()> {
    if let (Some(b), Some(d)) = (birth, death) {
        if d < b {
            return Err(DomainError::OutOfRange {
                field: "death_year".into(),
                got: d.to_string(),
                expected: format!("≥ {b}"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn blank_required_field_is_rejected() {
        assert!(check_required("name", "").is_err());
        assert!(check_required("name", "  ").is_err());
        assert!(check_required("name", "أحمد").is_ok());
    }

    #[test]
    fn length_is_counted_in_characters_not_bytes() {
        // 60 Arabic letters ≈ 120 UTF-8 bytes — a byte cap would reject it.
        let arabic = "ح".repeat(60);
        assert_eq!(arabic.len(), 120);
        assert!(check_len("name", &arabic, 60).is_ok());
        assert!(check_len("name", &"ح".repeat(61), 60).is_err());
    }

    #[test]
    fn too_long_reports_the_real_counts() {
        let err = check_len("name", "abcd", 3).unwrap_err();
        assert_eq!(
            err,
            DomainError::TooLong {
                field: "name".into(),
                got: 4,
                max: 3,
            }
        );
        assert_eq!(err.code(), "validation.too_long");
    }

    #[test]
    fn blank_optional_is_treated_as_absent() {
        assert!(check_opt_len("bio", None, 10).is_ok());
        assert!(check_opt_len("bio", Some("   "), 10).is_ok());
        assert!(check_opt_len("bio", Some("12345"), 10).is_ok());
        assert!(check_opt_len("bio", Some("123456"), 5).is_err());
    }

    #[test]
    fn year_bounds_and_lifespan() {
        assert!(check_year("birth_year", None).is_ok());
        assert!(check_year("birth_year", Some(1978)).is_ok());
        assert!(check_year("birth_year", Some(0)).is_err());
        assert!(check_year("birth_year", Some(10_000)).is_err());

        assert!(check_lifespan(Some(1950), Some(2020)).is_ok());
        assert!(check_lifespan(None, Some(2020)).is_ok());
        let err = check_lifespan(Some(1990), Some(1980)).unwrap_err();
        assert_eq!(err.code(), "validation.out_of_range");
    }
}
