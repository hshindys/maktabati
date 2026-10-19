//! Half-star ratings.
//!
//! Stored everywhere as an `INTEGER 0..=10` so that "4.5 stars" is `9` — exact, sortable,
//! indexable and immune to floating-point drift (which matters because rubric aggregates are
//! averages *of* these values).
//!
//! The product spec (StoryGraph/Hardcover precedent) requires half stars; the brief's rubric
//! ("معايير التقييم") uses the same scale.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{DomainError, Result};

/// A rating on a 0–5 scale expressible in half-star steps, stored as `0..=10`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Type,
)]
#[serde(transparent)]
pub struct Rating(u8);

impl Rating {
    /// Highest representable value (5★).
    pub const MAX: u8 = 10;

    /// Build from the raw half-star units (`0..=10`).
    ///
    /// # Errors
    ///
    /// * [`DomainError::OutOfRange`] when `units > 10`.
    pub fn from_units(units: u8) -> Result<Self> {
        if units > Self::MAX {
            return Err(DomainError::OutOfRange {
                field: "rating".into(),
                got: units.to_string(),
                expected: "0..=10 (half-star units)".into(),
            });
        }
        Ok(Self(units))
    }

    /// Build from a human star value (`0.0..=5.0` in 0.5 steps).
    ///
    /// # Errors
    ///
    /// * [`DomainError::OutOfRange`] when out of `0..=5`.
    /// * [`DomainError::InvalidEnum`] when not a multiple of 0.5.
    pub fn from_stars(stars: f32) -> Result<Self> {
        if !(0.0..=5.0).contains(&stars) {
            return Err(DomainError::OutOfRange {
                field: "rating".into(),
                got: stars.to_string(),
                expected: "0.0..=5.0".into(),
            });
        }
        let units = stars * 2.0;
        let rounded = units.round();
        if (units - rounded).abs() > f32::EPSILON {
            return Err(DomainError::InvalidEnum {
                field: "rating".into(),
                got: stars.to_string(),
            });
        }
        Self::from_units(rounded as u8)
    }

    /// The raw stored units (`0..=10`).
    #[must_use]
    pub const fn units(self) -> u8 {
        self.0
    }

    /// The star value for display (`0.0..=5.0`).
    #[must_use]
    pub fn stars(self) -> f32 {
        f32::from(self.0) / 2.0
    }

    /// Whole stars — used by star widgets for the filled portion.
    #[must_use]
    pub const fn whole_stars(self) -> u8 {
        self.0 / 2
    }

    /// Whether a half star must be drawn after the whole stars.
    #[must_use]
    pub const fn has_half(self) -> bool {
        self.0 % 2 == 1
    }
}

impl std::fmt::Display for Rating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.stars();
        if (s - s.round()).abs() < f32::EPSILON {
            write!(f, "{}", s.round())
        } else {
            write!(f, "{s}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    #[test]
    fn half_stars_round_trip() {
        let r = Rating::from_stars(4.5).unwrap();
        assert_eq!(r.units(), 9);
        assert_eq!(r.stars(), 4.5);
        assert!(r.has_half());
        assert_eq!(r.whole_stars(), 4);
        assert_eq!(r.to_string(), "4.5");
    }

    #[test]
    fn whole_stars_have_no_half() {
        let r = Rating::from_stars(4.0).unwrap();
        assert_eq!(r.units(), 8);
        assert!(!r.has_half());
        assert_eq!(r.to_string(), "4");
    }

    #[test]
    fn rejects_out_of_range_and_quarter_stars() {
        assert!(Rating::from_stars(-0.5).is_err());
        assert!(Rating::from_stars(5.5).is_err());
        assert!(Rating::from_stars(4.25).is_err());
        assert!(Rating::from_units(11).is_err());
        assert!(Rating::from_units(10).is_ok());
    }

    proptest! {
        #[test]
        fn prop_units_round_trip(units in 0u8..=10) {
            let r = Rating::from_units(units).unwrap();
            prop_assert_eq!(r.units(), units);
            prop_assert_eq!(Rating::from_stars(r.stars()).unwrap().units(), units);
        }

        #[test]
        fn prop_from_stars_rejects_invalid(stars in 0.0f32..10.0) {
            let expected_ok = (0.0..=5.0).contains(&stars) && ((stars * 2.0) % 1.0).abs() < 1e-6;
            prop_assert_eq!(Rating::from_stars(stars).is_ok(), expected_ok);
        }
    }
}
