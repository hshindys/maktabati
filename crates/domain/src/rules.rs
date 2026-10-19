//! Cross-entity business rules.
//!
//! These are the rules that do not belong to a single entity: status transitions, rubric
//! aggregation, local "similar books" scoring and the metadata merge policy (ADR-0008).
//! All of them are pure functions so they can be unit-tested exhaustively.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{BookStatus, Rating};

/// Allowed reading-status transitions and their meaning for reading cycles.
///
/// The full state machine is deliberately permissive (a reader can jump from `abandoned` back to
/// `reading` — people do redecide) but it always records *what happened*, so `reading_cycles`
/// keeps the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct StatusTransition {
    /// Status being left.
    pub from: BookStatus,
    /// Status being entered.
    pub to: BookStatus,
    /// Whether entering `to` should open a new reading cycle.
    pub opens_cycle: bool,
    /// Whether entering `to` should close the currently open cycle.
    pub closes_cycle: bool,
}

/// Compute the effect of a status change.
#[must_use]
pub fn transition(from: BookStatus, to: BookStatus) -> StatusTransition {
    StatusTransition {
        from,
        to,
        opens_cycle: to == BookStatus::Reading && from != BookStatus::Reading,
        closes_cycle: matches!(from, BookStatus::Reading) && to != BookStatus::Reading,
    }
}

/// A single rubric score row (`rubric_scores`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct RubricScore {
    /// Criterion id.
    pub criterion_id: String,
    /// Score in half-star units (`0..=10`).
    pub score: u8,
    /// Criterion weight (from `rating_criteria.weight`).
    pub weight: f64,
}

/// Minimum number of rated books before an aggregate is shown (spec §6.29): prevents a single
/// perfect score from making an author look "the best".
pub const MIN_BOOKS_FOR_AGGREGATE: usize = 3;

/// Aggregate computed over a set of books for one author or publisher.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct Aggregate {
    /// Number of books that contributed at least one score.
    pub book_count: usize,
    /// Weighted mean over all scored criteria, in half-star units (`0..=10`), `None` when there
    /// is not enough data.
    pub overall: Option<f64>,
    /// Per-criterion weighted means, keyed by criterion id.
    pub per_criterion: Vec<(String, f64)>,
    /// How many books contributed to each criterion, keyed by criterion id.
    pub per_criterion_counts: Vec<(String, usize)>,
}

/// Weighted mean of scores in half-star units.
///
/// Returns `None` when `scores` is empty or every weight is zero, so callers never display a
/// misleading `0.0`.
#[must_use]
pub fn weighted_mean(scores: &[RubricScore]) -> Option<f64> {
    let mut sum = 0.0;
    let mut weight = 0.0;
    for s in scores {
        if s.weight <= 0.0 {
            continue;
        }
        sum += f64::from(s.score) * s.weight;
        weight += s.weight;
    }
    if weight <= 0.0 {
        return None;
    }
    Some(sum / weight)
}

/// Compute the aggregate for one author/publisher.
///
/// `books` maps a book id to its rubric scores. `overall` is the weighted mean across every
/// scored criterion of every book (each criterion's mean first, then averaged across criteria,
/// so a criterion the user scores on every book does not dominate one they score rarely).
#[must_use]
pub fn aggregate(books: &[(String, Vec<RubricScore>)]) -> Aggregate {
    let book_count = books.iter().filter(|(_, s)| !s.is_empty()).count();

    let mut per_criterion: Vec<(String, f64)> = Vec::new();
    let mut per_criterion_counts: Vec<(String, usize)> = Vec::new();

    // Group scores by criterion across books.
    let mut criterion_ids: Vec<String> = Vec::new();
    for (_, scores) in books {
        for s in scores {
            if !criterion_ids.contains(&s.criterion_id) {
                criterion_ids.push(s.criterion_id.clone());
            }
        }
    }
    criterion_ids.sort();

    for cid in &criterion_ids {
        let mut values: Vec<RubricScore> = Vec::new();
        for (_, scores) in books {
            if let Some(s) = scores.iter().find(|s| &s.criterion_id == cid) {
                values.push(s.clone());
            }
        }
        let n = values.len();
        per_criterion_counts.push((cid.clone(), n));
        if let Some(mean) = weighted_mean(&values) {
            per_criterion.push((cid.clone(), round_half_unit(mean)));
        }
    }

    // Overall = mean of the criterion means (equal weight per criterion), rounded to the
    // nearest half-unit so it stays on the same scale as a user-entered rating.
    let overall = if per_criterion.is_empty() {
        None
    } else {
        let sum: f64 = per_criterion.iter().map(|(_, v)| v).sum();
        Some(round_half_unit(sum / per_criterion.len() as f64))
    };

    Aggregate {
        book_count,
        overall,
        per_criterion,
        per_criterion_counts,
    }
}

/// Round to the nearest 0.5 so aggregates land on the same grid as user ratings.
#[must_use]
pub fn round_half_unit(v: f64) -> f64 {
    (v * 2.0).round() / 2.0
}

/// Convert an aggregate to the typed [`Rating`] for display, if it qualifies.
#[must_use]
pub fn aggregate_rating(agg: &Aggregate) -> Option<Rating> {
    if agg.book_count < MIN_BOOKS_FOR_AGGREGATE {
        return None;
    }
    agg.overall
        .and_then(|v| Rating::from_units(round_half_unit(v) as u8).ok())
}

/// A local similarity signal between two books.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Similarity {
    /// The other book's id.
    pub book_id: String,
    /// Score in `0.0..=1.0`.
    pub score: f64,
    /// Human-readable reasons, for the UI tooltip ("shared author", "shared series"…).
    pub reasons: Vec<String>,
}

/// Weights for the local similarity model (documented in `03-data-model.md` §related).
const W_SERIES: f64 = 0.5;
const W_AUTHOR: f64 = 0.3;
const W_CATEGORY: f64 = 0.15;
const W_TAG: f64 = 0.05;

/// Score one candidate book against a reference book using **local data only** (no network —
/// a stated product value).
///
/// Returns a normalized `0.0..=1.0` score plus the reasons, sorted by score descending.
#[must_use]
pub fn similarity(reference: &SimilarityInput, candidates: &[SimilarityInput]) -> Vec<Similarity> {
    let mut out: Vec<Similarity> = Vec::new();

    for c in candidates {
        if c.book_id == reference.book_id {
            continue;
        }
        let mut score = 0.0;
        let mut reasons: Vec<String> = Vec::new();

        if let (Some(a), Some(b)) = (&reference.series_id, &c.series_id) {
            if a == b {
                score += W_SERIES;
                reasons.push("series".to_owned());
            }
        }
        let shared_authors = intersection_count(&reference.author_ids, &c.author_ids);
        if shared_authors > 0 {
            score += W_AUTHOR;
            reasons.push("author".to_owned());
        }
        let shared_categories = intersection_count(&reference.category_ids, &c.category_ids);
        if shared_categories > 0 {
            score += W_CATEGORY * f64::from(shared_categories.min(3));
            reasons.push("category".to_owned());
        }
        let shared_tags = intersection_count(&reference.tag_ids, &c.tag_ids);
        if shared_tags > 0 {
            score += W_TAG * f64::from(shared_tags.min(3));
            reasons.push("tag".to_owned());
        }

        if score > 0.0 {
            out.push(Similarity {
                book_id: c.book_id.clone(),
                score: score.clamp(0.0, 1.0),
                reasons,
            });
        }
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.book_id.cmp(&b.book_id))
    });
    out
}

/// The subset of a book needed for similarity scoring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
pub struct SimilarityInput {
    /// Book id.
    pub book_id: String,
    /// Series id, if any.
    pub series_id: Option<String>,
    /// Credited author ids.
    pub author_ids: Vec<String>,
    /// Category ids.
    pub category_ids: Vec<String>,
    /// Tag ids.
    pub tag_ids: Vec<String>,
}

fn intersection_count(a: &[String], b: &[String]) -> u32 {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    a.iter().filter(|x| b.contains(x)).count() as u32
}

/// Field-level merge policy when several sources describe the same book (ADR-0008).
///
/// Precedence: **existing local value > Google Books > Open Library > Wikidata**.
/// An empty/`None` remote value never clobbers a non-empty local one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum SourceRank {
    /// A value the user typed or confirmed.
    #[default]
    Local = 3,
    /// Google Books.
    GoogleBooks = 2,
    /// Open Library.
    OpenLibrary = 1,
    /// Wikidata enrichment.
    Wikidata = 0,
}

/// Merge one optional field, returning the winning value and its rank.
#[must_use]
pub fn merge_field(
    current: Option<(String, SourceRank)>,
    incoming: Option<(String, SourceRank)>,
) -> Option<(String, SourceRank)> {
    match (current, incoming) {
        (Some(cur), Some(inc)) => {
            // An empty incoming value never wins, regardless of rank.
            if inc.0.trim().is_empty() {
                return Some(cur);
            }
            if cur.0.trim().is_empty() || inc.1 > cur.1 {
                Some(inc)
            } else {
                Some(cur)
            }
        }
        (Some(cur), None) => Some(cur),
        (None, Some(inc)) if !inc.0.trim().is_empty() => Some(inc),
        (None, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn transition_opens_and_closes_cycles() {
        let t = transition(BookStatus::ToRead, BookStatus::Reading);
        assert!(t.opens_cycle);
        assert!(!t.closes_cycle);

        let t = transition(BookStatus::Reading, BookStatus::Finished);
        assert!(!t.opens_cycle);
        assert!(t.closes_cycle);

        // Paused → Reading reopens a cycle.
        let t = transition(BookStatus::Paused, BookStatus::Reading);
        assert!(t.opens_cycle);

        // ToRead → Finished is allowed and closes nothing.
        let t = transition(BookStatus::ToRead, BookStatus::Finished);
        assert!(!t.opens_cycle);
        assert!(!t.closes_cycle);
    }

    #[test]
    fn weighted_mean_ignores_zero_weight_and_empty() {
        assert_eq!(weighted_mean(&[]), None);
        assert_eq!(
            weighted_mean(&[RubricScore {
                criterion_id: "a".into(),
                score: 8,
                weight: 0.0
            }]),
            None
        );
        let m = weighted_mean(&[
            RubricScore {
                criterion_id: "a".into(),
                score: 8,
                weight: 1.0,
            },
            RubricScore {
                criterion_id: "b".into(),
                score: 4,
                weight: 3.0,
            },
        ])
        .unwrap();
        assert!((m - 5.0).abs() < 1e-9);
    }

    #[test]
    fn aggregate_gates_on_min_books() {
        // One perfect book must NOT produce a published aggregate.
        let one = vec![(
            "b1".to_owned(),
            vec![RubricScore {
                criterion_id: "print".into(),
                score: 10,
                weight: 1.0,
            }],
        )];
        let agg = aggregate(&one);
        assert_eq!(agg.book_count, 1);
        assert_eq!(agg.overall, Some(10.0));
        assert_eq!(aggregate_rating(&agg), None, "gate must hold at < 3 books");

        // Three books at 8.0 do qualify.
        let three: Vec<_> = (1..=3)
            .map(|i| {
                (
                    format!("b{i}"),
                    vec![RubricScore {
                        criterion_id: "print".into(),
                        score: 8,
                        weight: 1.0,
                    }],
                )
            })
            .collect();
        let agg = aggregate(&three);
        assert_eq!(agg.overall, Some(8.0));
        assert_eq!(aggregate_rating(&agg), Some(Rating::from_units(8).unwrap()));
    }

    #[test]
    fn aggregate_averages_criteria_not_books() {
        // Book 1 scores two criteria, book 2 scores one. The rare criterion must not be
        // drowned out.
        let books = vec![
            (
                "b1".to_owned(),
                vec![
                    RubricScore {
                        criterion_id: "style".into(),
                        score: 10,
                        weight: 1.0,
                    },
                    RubricScore {
                        criterion_id: "print".into(),
                        score: 10,
                        weight: 1.0,
                    },
                ],
            ),
            ("b2".to_owned(), vec![]),
        ];
        let agg = aggregate(&books);
        assert_eq!(agg.book_count, 1);
        assert_eq!(agg.per_criterion.len(), 2);
        assert_eq!(agg.overall, Some(10.0));
    }

    #[test]
    fn similarity_ranks_series_then_author_then_category() {
        let reference = SimilarityInput {
            book_id: "r".into(),
            series_id: Some("s1".into()),
            author_ids: vec!["a1".into()],
            category_ids: vec!["c1".into()],
            tag_ids: vec![],
        };
        let candidates = vec![
            // same series + author + category → highest
            SimilarityInput {
                book_id: "same_series".into(),
                series_id: Some("s1".into()),
                author_ids: vec!["a1".into()],
                category_ids: vec!["c1".into()],
                tag_ids: vec![],
            },
            // same author + category only
            SimilarityInput {
                book_id: "same_author".into(),
                series_id: None,
                author_ids: vec!["a1".into()],
                category_ids: vec!["c1".into()],
                tag_ids: vec![],
            },
            // same category only
            SimilarityInput {
                book_id: "same_cat".into(),
                series_id: None,
                author_ids: vec![],
                category_ids: vec!["c1".into()],
                tag_ids: vec![],
            },
            // nothing in common — excluded
            SimilarityInput {
                book_id: "unrelated".into(),
                series_id: None,
                author_ids: vec!["zzz".into()],
                category_ids: vec!["zzz".into()],
                tag_ids: vec![],
            },
        ];
        let ranked = similarity(&reference, &candidates);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].book_id, "same_series");
        assert_eq!(ranked[1].book_id, "same_author");
        assert_eq!(ranked[2].book_id, "same_cat");
        assert!(ranked[0].score > ranked[1].score);
        assert!(ranked[1].score > ranked[2].score);
        assert!(ranked[0].score <= 1.0);
        assert_eq!(ranked[0].reasons, vec!["series", "author", "category"]);
    }

    #[test]
    fn similarity_excludes_self() {
        let r = SimilarityInput {
            book_id: "r".into(),
            author_ids: vec!["a1".into()],
            ..Default::default()
        };
        let c = vec![SimilarityInput {
            book_id: "r".into(),
            author_ids: vec!["a1".into()],
            ..Default::default()
        }];
        assert!(similarity(&r, &c).is_empty());
    }

    #[test]
    fn merge_policy_prefers_local_and_ignores_empty_remote() {
        use SourceRank::*;

        // Local wins over any remote.
        assert_eq!(
            merge_field(Some(("لي".into(), Local)), Some(("me".into(), GoogleBooks))),
            Some(("لي".into(), Local))
        );
        // Google Books beats Open Library.
        assert_eq!(
            merge_field(
                Some(("ol".into(), OpenLibrary)),
                Some(("gb".into(), GoogleBooks))
            ),
            Some(("gb".into(), GoogleBooks))
        );
        // An empty remote never clobbers a local value, even with a higher rank.
        assert_eq!(
            merge_field(
                Some(("لي".into(), Local)),
                Some(("   ".into(), GoogleBooks))
            ),
            Some(("لي".into(), Local))
        );
        // Missing local → remote fills in.
        assert_eq!(
            merge_field(None, Some(("gb".into(), GoogleBooks))),
            Some(("gb".into(), GoogleBooks))
        );
        // Nothing at all.
        assert_eq!(merge_field(None, None), None);
        assert_eq!(merge_field(None, Some(("  ".into(), Wikidata))), None);
    }

    #[test]
    fn round_half_unit() {
        assert_eq!(super::round_half_unit(4.24), 4.0);
        assert_eq!(super::round_half_unit(4.26), 4.5);
        assert_eq!(super::round_half_unit(4.75), 5.0);
    }
}
