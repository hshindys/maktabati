//! Reviews and rubric evaluations.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{rules::Aggregate, Rating, Result};

/// A user-editable rating axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct RatingCriterion {
    /// Stable identifier.
    pub id: String,
    /// Stable key used for built-in criteria.
    pub key: String,
    /// Arabic display name.
    pub name_ar: String,
    /// English display name.
    pub name_en: String,
    /// Arabic explanatory text.
    pub description_ar: Option<String>,
    /// English explanatory text.
    pub description_en: Option<String>,
    /// Weight used when combining scores.
    pub weight: f64,
    /// Display order.
    pub position: i32,
    /// Whether this axis is available for new scores.
    pub is_active: bool,
    /// Whether this axis was included with the application.
    pub is_system: bool,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Writable fields for a rubric criterion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct RatingCriterionInput {
    /// Arabic display name.
    pub name_ar: String,
    /// English display name.
    pub name_en: String,
    /// Arabic explanatory text.
    pub description_ar: Option<String>,
    /// English explanatory text.
    pub description_en: Option<String>,
    /// Weight used when combining scores (`0..=100`).
    pub weight: f64,
    /// Display order.
    pub position: i32,
    /// Whether this axis is available for new scores.
    pub is_active: bool,
}

impl RatingCriterionInput {
    /// Validate and trim user-editable criterion fields.
    pub fn validate(mut self) -> Result<Self> {
        self.name_ar = self.name_ar.trim().to_owned();
        self.name_en = self.name_en.trim().to_owned();
        if self.name_ar.is_empty() {
            return Err(crate::DomainError::EmptyField {
                field: "name_ar".to_owned(),
            });
        }
        if self.name_en.is_empty() {
            return Err(crate::DomainError::EmptyField {
                field: "name_en".to_owned(),
            });
        }
        for (field, value) in [("name_ar", &self.name_ar), ("name_en", &self.name_en)] {
            if value.chars().count() > 120 {
                return Err(crate::DomainError::TooLong {
                    field: field.to_owned(),
                    got: value.chars().count(),
                    max: 120,
                });
            }
        }
        for (field, value) in [
            ("description_ar", &mut self.description_ar),
            ("description_en", &mut self.description_en),
        ] {
            *value = value.take().and_then(|text| {
                let trimmed = text.trim().to_owned();
                (!trimmed.is_empty()).then_some(trimmed)
            });
            if let Some(text) = value {
                if text.chars().count() > 1_000 {
                    return Err(crate::DomainError::TooLong {
                        field: field.to_owned(),
                        got: text.chars().count(),
                        max: 1_000,
                    });
                }
            }
        }
        if !self.weight.is_finite() || !(0.0..=100.0).contains(&self.weight) {
            return Err(crate::DomainError::OutOfRange {
                field: "weight".to_owned(),
                got: self.weight.to_string(),
                expected: "0..=100".to_owned(),
            });
        }
        Ok(self)
    }
}

/// One written review for a book.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Review {
    /// UUID v7.
    pub id: String,
    /// Owning book id.
    pub book_id: String,
    /// Overall rating in half-star units (`0..=10`).
    pub rating: Option<u8>,
    /// Review body.
    pub body: Option<String>,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Editable overall rating and review body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct ReviewInput {
    /// Overall rating in half-star units (`0..=10`).
    pub rating: Option<u8>,
    /// Review body.
    pub body: Option<String>,
}

impl ReviewInput {
    /// Validate and normalize the review fields. Both absent means clear the current review.
    pub fn validate(mut self) -> Result<Self> {
        if let Some(rating) = self.rating {
            Rating::from_units(rating)?;
        }
        self.body = self.body.take().and_then(|text| {
            let trimmed = text.trim().to_owned();
            (!trimmed.is_empty()).then_some(trimmed)
        });
        if let Some(body) = &self.body {
            if body.chars().count() > 20_000 {
                return Err(crate::DomainError::TooLong {
                    field: "body".to_owned(),
                    got: body.chars().count(),
                    max: 20_000,
                });
            }
        }
        Ok(self)
    }
}

/// A per-book score update; `None` clears that criterion's score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct RubricScoreInput {
    /// Criterion identifier.
    pub criterion_id: String,
    /// Score in half-star units (`0..=10`), or `None` to clear.
    pub score: Option<u8>,
    /// Optional note for this axis.
    pub note: Option<String>,
}

/// A persisted per-book rubric score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct RubricScoreRecord {
    /// Owning book id.
    pub book_id: String,
    /// Criterion identifier.
    pub criterion_id: String,
    /// Score in half-star units (`0..=10`).
    pub score: u8,
    /// Optional note for this axis.
    pub note: Option<String>,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// All evaluation data needed to display and edit a book's review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct BookEvaluation {
    /// Current review, if one has been written.
    pub review: Option<Review>,
    /// Active rubric criteria.
    pub criteria: Vec<RatingCriterion>,
    /// Saved scores belonging to active criteria.
    pub scores: Vec<RubricScoreRecord>,
    /// This book's weighted rubric aggregate.
    pub aggregate: Aggregate,
}

/// One author or publisher's aggregate and the threshold-qualified star rating.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct EntityRatingAggregate {
    /// Author or publisher identifier.
    pub entity_id: String,
    /// Full aggregate, including its contributing-book count.
    pub aggregate: Aggregate,
    /// Aggregate rating, present only when the domain threshold is met.
    pub rating: Option<Rating>,
}

/// Aggregates grouped by entity type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct EvaluationAggregates {
    /// Author aggregates.
    pub authors: Vec<EntityRatingAggregate>,
    /// Publisher aggregates.
    pub publishers: Vec<EntityRatingAggregate>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn criterion() -> RatingCriterionInput {
        RatingCriterionInput {
            name_ar: "  تحرير ".to_owned(),
            name_en: " Editing ".to_owned(),
            description_ar: Some("  وصف  ".to_owned()),
            description_en: None,
            weight: 1.0,
            position: 0,
            is_active: true,
        }
    }

    #[test]
    fn criterion_input_trims_and_validates_bilingual_names_and_weight() {
        let valid = criterion().validate().unwrap();
        assert_eq!(valid.name_ar, "تحرير");
        assert_eq!(valid.name_en, "Editing");
        assert_eq!(valid.description_ar.as_deref(), Some("وصف"));

        assert!(RatingCriterionInput {
            name_ar: String::new(),
            ..criterion()
        }
        .validate()
        .is_err());
        assert!(RatingCriterionInput {
            weight: f64::NAN,
            ..criterion()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn review_input_trims_body_and_rejects_out_of_range_ratings() {
        assert_eq!(
            ReviewInput {
                rating: Some(9),
                body: Some("  جيد  ".to_owned()),
            }
            .validate()
            .unwrap()
            .body
            .as_deref(),
            Some("جيد")
        );
        assert!(ReviewInput {
            rating: Some(11),
            body: None,
        }
        .validate()
        .is_err());
    }
}
