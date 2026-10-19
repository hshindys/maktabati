//! `Book` entity, its write input, and the validation rules that guard it.
//!
//! The `Book` is the aggregate root: relations (authors, publisher, categories, tags, series) are
//! owned by `storage` and referenced by id, so this entity stays a plain value that can be built,
//! tested and cloned cheaply.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    ids::new_id,
    text::{contains_arabic, normalize},
    AuthorRole, BookFormat, BookSource, Condition, CoverSource, Direction, Isbn, Rating, Result,
};

/// Maximum lengths, chosen to be generous for Arabic titles but bounded enough to keep the
/// list/table views and the search projection sane.
pub mod limits {
    /// Title length cap.
    pub const TITLE: usize = 500;
    /// Subtitle length cap.
    pub const SUBTITLE: usize = 500;
    /// Description length cap (Calibre-style blurbs are long).
    pub const DESCRIPTION: usize = 20_000;
    /// Free-text field cap (edition, location, source…).
    pub const SHORT_TEXT: usize = 200;
}

/// Re-export of the reading status enum under a book-specific name for readability.
pub use crate::ReadingStatus as BookStatus;

/// A book record — the library's aggregate root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Book {
    /// UUID v7.
    pub id: String,
    /// Primary title, exactly as entered.
    pub title: String,
    /// Subtitle.
    pub subtitle: Option<String>,
    /// Title in the original script (for translated works).
    pub original_title: Option<String>,
    /// Normalized mirror of `title` (search + duplicate detection).
    pub title_norm: String,
    /// Primary language code.
    pub language_code: String,
    /// Validated ISBN-10 (digits only).
    pub isbn10: Option<String>,
    /// Validated ISBN-13 (digits only).
    pub isbn13: Option<String>,
    /// Edition statement (`الطبعة الثالثة`, `3rd ed.`…).
    pub edition: Option<String>,
    /// Year of this edition's publication.
    pub publication_year: Option<i32>,
    /// Page count.
    pub page_count: Option<i32>,
    /// Format of the copy.
    pub format: BookFormat,
    /// Blurb / summary.
    pub description: Option<String>,
    /// Series membership.
    pub series_id: Option<String>,
    /// Order within the series (`f32` so `1.5` novellas are expressible).
    pub series_order: Option<f64>,
    /// Physical location: room.
    pub room: Option<String>,
    /// Physical location: shelf.
    pub shelf: Option<String>,
    /// Physical location: box.
    pub box_name: Option<String>,
    /// Acquisition date (`YYYY-MM-DD`).
    pub acquired_date: Option<String>,
    /// Where the copy came from.
    pub acquired_from: Option<String>,
    /// Acquisition price in minor units.
    pub price_minor: Option<i64>,
    /// Acquisition currency (ISO-4217).
    pub price_currency: Option<String>,
    /// Physical condition.
    pub condition: Option<Condition>,
    /// Relative cover path under `covers/`.
    pub cover_path: Option<String>,
    /// Relative thumbnail path.
    pub cover_thumb: Option<String>,
    /// Provenance of the cover.
    pub cover_source: Option<CoverSource>,
    /// Current reading status (denormalized for indexed filtering).
    pub reading_status: BookStatus,
    /// Overall rating in half-star units (`0..=10`).
    pub rating: Option<u8>,
    /// Current page while reading.
    pub current_page: Option<i32>,
    /// Derived progress percentage.
    pub progress_pct: i32,
    /// How the record entered the library.
    pub source: BookSource,
    /// External identifiers (Open Library / Google Books keys) as JSON.
    pub external_ids: Option<serde_json::Value>,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or updating a [`Book`].
///
/// Relations are supplied separately by `storage` so this type stays free of foreign keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct BookInput {
    /// Title (required).
    pub title: String,
    /// Subtitle.
    pub subtitle: Option<String>,
    /// Original-script title.
    pub original_title: Option<String>,
    /// Language code (required; must exist in `languages`).
    pub language_code: Option<String>,
    /// ISBN-10 as typed (validated).
    pub isbn10: Option<String>,
    /// ISBN-13 as typed (validated).
    pub isbn13: Option<String>,
    /// Edition.
    pub edition: Option<String>,
    /// Publication year.
    pub publication_year: Option<i32>,
    /// Page count.
    pub page_count: Option<i32>,
    /// Format.
    pub format: Option<BookFormat>,
    /// Description.
    pub description: Option<String>,
    /// Series id.
    pub series_id: Option<String>,
    /// Series order.
    pub series_order: Option<f64>,
    /// Room.
    pub room: Option<String>,
    /// Shelf.
    pub shelf: Option<String>,
    /// Box.
    pub box_name: Option<String>,
    /// Acquisition date.
    pub acquired_date: Option<String>,
    /// Acquisition source.
    pub acquired_from: Option<String>,
    /// Price in minor units.
    pub price_minor: Option<i64>,
    /// Price currency.
    pub price_currency: Option<String>,
    /// Condition.
    pub condition: Option<Condition>,
    /// Overall rating (half-star units).
    pub rating: Option<u8>,
    /// Current page.
    pub current_page: Option<i32>,
    /// Reading status.
    pub reading_status: Option<BookStatus>,
    /// Cover path (relative).
    pub cover_path: Option<String>,
    /// Cover provenance.
    pub cover_source: Option<CoverSource>,
    /// Provenance of the record.
    pub source: Option<BookSource>,
    /// External identifiers.
    pub external_ids: Option<serde_json::Value>,
    /// Author credits (id + role). Validated by `storage`, checked for at least one author
    /// is **not** required — many Arabic books are catalogued before their author is known.
    pub authors: Vec<AuthorCredit>,
    /// Publisher id.
    pub publisher_id: Option<String>,
    /// Category ids.
    pub categories: Vec<String>,
    /// Tag ids.
    pub tags: Vec<String>,
}

/// A single author credit on a book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AuthorCredit {
    /// Author id.
    pub author_id: String,
    /// Role played.
    pub role: AuthorRole,
}

impl Book {
    /// Build a new book, validating ISBNs and deriving the normalized mirror.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::EmptyField`] when the title is blank.
    /// * [`crate::DomainError::InvalidIsbn`] when either ISBN fails its checksum.
    /// * [`crate::DomainError::TooLong`] when a field exceeds its cap.
    /// * [`crate::DomainError::OutOfRange`] for year / pages / rating / progress.
    pub fn new(input: BookInput, now_ms: i64) -> Result<Self> {
        let title = input.title.trim().to_owned();
        if title.is_empty() {
            return Err(crate::DomainError::EmptyField {
                field: "title".into(),
            });
        }
        check_len("title", &title, limits::TITLE)?;
        check_opt_len("subtitle", input.subtitle.as_deref(), limits::SUBTITLE)?;
        check_opt_len(
            "original_title",
            input.original_title.as_deref(),
            limits::TITLE,
        )?;
        check_opt_len("edition", input.edition.as_deref(), limits::SHORT_TEXT)?;
        check_opt_len(
            "description",
            input.description.as_deref(),
            limits::DESCRIPTION,
        )?;
        check_opt_len("room", input.room.as_deref(), limits::SHORT_TEXT)?;
        check_opt_len("shelf", input.shelf.as_deref(), limits::SHORT_TEXT)?;
        check_opt_len("box_name", input.box_name.as_deref(), limits::SHORT_TEXT)?;
        check_opt_len(
            "acquired_from",
            input.acquired_from.as_deref(),
            limits::SHORT_TEXT,
        )?;
        check_year(input.publication_year)?;
        check_pages(input.page_count)?;
        check_rating(input.rating)?;

        let (isbn10, isbn13) = resolve_isbns(input.isbn10.as_deref(), input.isbn13.as_deref())?;
        let title_norm = normalize(&title);

        let reading_status = input.reading_status.unwrap_or(BookStatus::ToRead);
        let current_page = input.current_page;
        let page_count = input.page_count;
        let progress_pct = compute_progress(current_page, page_count);

        Ok(Self {
            id: new_id(),
            title,
            subtitle: input.subtitle.filter_non_empty(),
            original_title: input.original_title.filter_non_empty(),
            title_norm,
            language_code: input.language_code.unwrap_or_else(|| "ar".to_owned()),
            isbn10,
            isbn13,
            edition: input.edition.filter_non_empty(),
            publication_year: input.publication_year,
            page_count,
            format: input.format.unwrap_or(BookFormat::Print),
            description: input.description.filter_non_empty(),
            series_id: input.series_id,
            series_order: input.series_order,
            room: input.room.filter_non_empty(),
            shelf: input.shelf.filter_non_empty(),
            box_name: input.box_name.filter_non_empty(),
            acquired_date: input.acquired_date.filter_non_empty(),
            acquired_from: input.acquired_from.filter_non_empty(),
            price_minor: input.price_minor,
            price_currency: input.price_currency.filter_non_empty(),
            condition: input.condition,
            cover_path: input.cover_path,
            cover_thumb: None,
            cover_source: input.cover_source,
            reading_status,
            rating: input.rating,
            current_page,
            progress_pct,
            source: input.source.unwrap_or(BookSource::Manual),
            external_ids: input.external_ids,
            created_at: now_ms,
            updated_at: now_ms,
        })
    }

    /// Apply an edit, recomputing derived fields and re-validating.
    ///
    /// # Errors
    ///
    /// Same validation failures as [`Book::new`].
    pub fn apply(&mut self, input: BookInput, now_ms: i64) -> Result<()> {
        let updated = Self {
            id: self.id.clone(),
            created_at: self.created_at,
            cover_thumb: self.cover_thumb.clone(),
            ..Self::new(input, now_ms)?
        };
        updated.copy_derived_into(self);
        self.updated_at = now_ms;
        Ok(())
    }

    fn copy_derived_into(&self, to: &mut Book) {
        to.title.clone_from(&self.title);
        to.subtitle.clone_from(&self.subtitle);
        to.original_title.clone_from(&self.original_title);
        to.title_norm.clone_from(&self.title_norm);
        to.language_code.clone_from(&self.language_code);
        to.isbn10.clone_from(&self.isbn10);
        to.isbn13.clone_from(&self.isbn13);
        to.edition.clone_from(&self.edition);
        to.publication_year = self.publication_year;
        to.page_count = self.page_count;
        to.format = self.format;
        to.description.clone_from(&self.description);
        to.series_id.clone_from(&self.series_id);
        to.series_order = self.series_order;
        to.room.clone_from(&self.room);
        to.shelf.clone_from(&self.shelf);
        to.box_name.clone_from(&self.box_name);
        to.acquired_date.clone_from(&self.acquired_date);
        to.acquired_from.clone_from(&self.acquired_from);
        to.price_minor = self.price_minor;
        to.price_currency.clone_from(&self.price_currency);
        to.condition = self.condition;
        to.cover_path.clone_from(&self.cover_path);
        to.cover_source = self.cover_source;
        to.reading_status = self.reading_status;
        to.rating = self.rating;
        to.current_page = self.current_page;
        to.progress_pct = self.progress_pct;
        to.source = self.source;
        to.external_ids.clone_from(&self.external_ids);
    }

    /// Set the reading status (no-op when unchanged).
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::InvalidTransition`] when finishing a book with no page information
    ///   and no page count — we refuse to invent progress.
    pub fn set_status(&mut self, status: BookStatus, now_ms: i64) -> Result<()> {
        if status == self.reading_status {
            return Ok(());
        }
        if status == BookStatus::Finished && self.progress_pct < 100 {
            // Moving to "finished" implies the last page; record it rather than leaving a
            // contradictory 60%-finished "finished" book.
            if let Some(pages) = self.page_count {
                self.current_page = Some(pages);
                self.progress_pct = 100;
            } else if self.progress_pct > 0 && self.current_page.is_none() {
                return Err(crate::DomainError::InvalidTransition {
                    detail: "cannot finish a book without a page count or current page".into(),
                });
            } else {
                self.progress_pct = 100;
            }
        }
        self.reading_status = status;
        self.updated_at = now_ms;
        Ok(())
    }

    /// Update `current_page` and recompute `progress_pct`.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::OutOfRange`] when the page is negative or beyond the page count.
    pub fn set_current_page(&mut self, page: Option<i32>, now_ms: i64) -> Result<()> {
        if let (Some(p), Some(total)) = (page, self.page_count) {
            if p > total {
                return Err(crate::DomainError::OutOfRange {
                    field: "current_page".into(),
                    got: p.to_string(),
                    expected: format!("0..={total}"),
                });
            }
        }
        self.current_page = page;
        self.progress_pct = compute_progress(page, self.page_count);
        self.updated_at = now_ms;
        Ok(())
    }

    /// Writing direction of the title (per-field bidi isolation in the UI).
    #[must_use]
    pub fn direction(&self) -> Direction {
        if contains_arabic(&self.title) {
            Direction::Rtl
        } else {
            Direction::Ltr
        }
    }

    /// Parse the stored rating into the typed value object.
    #[must_use]
    pub fn rating_vo(&self) -> Option<Rating> {
        self.rating.and_then(|r| Rating::from_units(r).ok())
    }
}

/// Compute progress percentage, capped at 100.
///
/// Returns 0 when there is nothing to compute against.
#[must_use]
pub fn compute_progress(current_page: Option<i32>, page_count: Option<i32>) -> i32 {
    match (current_page, page_count) {
        (Some(p), Some(total)) if total > 0 => {
            ((i64::from(p) * 100) / i64::from(total)).clamp(0, 100) as i32
        }
        _ => 0,
    }
}

fn check_len(field: &str, value: &str, max: usize) -> Result<()> {
    let len = value.chars().count();
    if len > max {
        return Err(crate::DomainError::TooLong {
            field: field.into(),
            got: len,
            max,
        });
    }
    Ok(())
}

fn check_opt_len(field: &str, value: Option<&str>, max: usize) -> Result<()> {
    match value {
        Some(v) if !v.trim().is_empty() => check_len(field, v, max),
        _ => Ok(()),
    }
}

fn check_year(year: Option<i32>) -> Result<()> {
    if let Some(y) = year {
        if !(1..=9999).contains(&y) {
            return Err(crate::DomainError::OutOfRange {
                field: "publication_year".into(),
                got: y.to_string(),
                expected: "1..=9999".into(),
            });
        }
    }
    Ok(())
}

fn check_pages(pages: Option<i32>) -> Result<()> {
    if let Some(p) = pages {
        if p <= 0 {
            return Err(crate::DomainError::OutOfRange {
                field: "page_count".into(),
                got: p.to_string(),
                expected: "> 0".into(),
            });
        }
    }
    Ok(())
}

fn check_rating(rating: Option<u8>) -> Result<()> {
    if let Some(r) = rating {
        Rating::from_units(r)?;
    }
    Ok(())
}

/// Validate both ISBNs, deriving the missing one from the other when possible.
fn resolve_isbns(
    isbn10: Option<&str>,
    isbn13: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    let ten = match isbn10.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(Isbn::parse(s)?),
        None => None,
    };
    let thirteen = match isbn13.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(Isbn::parse(s)?),
        None => None,
    };

    // Derive the missing form so the user only ever types one of them.
    let (ten, thirteen) = match (ten, thirteen) {
        (Some(t), None) => {
            let derived = Isbn::parse(&t.to_isbn13())?;
            (Some(t.to_string()), Some(derived.to_string()))
        }
        (None, Some(t)) => (t.to_isbn10(), Some(t.to_string())),
        (Some(t), Some(th)) => {
            // Both supplied: they must describe the same book.
            if t.to_isbn13() != th.to_string() {
                return Err(crate::DomainError::InvalidIsbn {
                    value: format!("{t} / {th}"),
                });
            }
            (Some(t.to_string()), Some(th.to_string()))
        }
        (None, None) => (None, None),
    };
    Ok((ten, thirteen))
}

/// Internal helper: `Option<String>::filter` on non-empty trimmed content.
trait FilterNonEmpty {
    fn filter_non_empty(self) -> Option<String>;
}

impl FilterNonEmpty for Option<String> {
    fn filter_non_empty(self) -> Option<String> {
        self.and_then(|s| {
            let t = s.trim().to_owned();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn minimal(title: &str) -> BookInput {
        BookInput {
            title: title.into(),
            ..Default::default()
        }
    }

    #[test]
    fn creates_book_with_defaults() {
        let b = Book::new(minimal("مقدمة في الحضارة الإسلامية"), 100).unwrap();
        assert_eq!(b.language_code, "ar");
        assert_eq!(b.format, BookFormat::Print);
        assert_eq!(b.reading_status, BookStatus::ToRead);
        assert_eq!(b.progress_pct, 0);
        assert_eq!(b.title_norm, "مقدمه في الحضاره الاسلاميه");
        assert_eq!(b.direction(), Direction::Rtl);
        assert_eq!(b.created_at, 100);
    }

    #[test]
    fn rejects_empty_title() {
        assert!(Book::new(minimal("   "), 1).is_err());
    }

    #[test]
    fn derives_isbn13_from_isbn10_and_vice_versa() {
        let b = Book::new(
            BookInput {
                title: "x".into(),
                isbn10: Some("0306406152".into()),
                ..Default::default()
            },
            1,
        )
        .unwrap();
        assert_eq!(b.isbn10.as_deref(), Some("0306406152"));
        assert_eq!(b.isbn13.as_deref(), Some("9780306406157"));

        let b2 = Book::new(
            BookInput {
                title: "x".into(),
                isbn13: Some("9780306406157".into()),
                ..Default::default()
            },
            1,
        )
        .unwrap();
        assert_eq!(b2.isbn13.as_deref(), Some("9780306406157"));
        assert_eq!(b2.isbn10.as_deref(), Some("0306406152"));
    }

    #[test]
    fn rejects_conflicting_isbns() {
        // Both are valid on their own — the conflict is the point. `9780140449143` is the
        // ISBN-13 of ISBN-10 `0140449140`, not of `0306406152`, so the pair describes two
        // different books.
        assert!(Book::new(
            BookInput {
                title: "x".into(),
                isbn10: Some("0306406152".into()),
                isbn13: Some("9780140449143".into()),
                ..Default::default()
            },
            1,
        )
        .is_err());
    }

    #[test]
    fn rejects_invalid_isbn() {
        assert!(Book::new(
            BookInput {
                title: "x".into(),
                isbn13: Some("9780306406158".into()),
                ..Default::default()
            },
            1,
        )
        .is_err());
    }

    #[test]
    fn progress_is_computed_and_capped() {
        assert_eq!(compute_progress(Some(50), Some(200)), 25);
        assert_eq!(compute_progress(Some(400), Some(200)), 100);
        assert_eq!(compute_progress(Some(0), Some(200)), 0);
        assert_eq!(compute_progress(None, Some(200)), 0);
        assert_eq!(compute_progress(Some(50), None), 0);
        assert_eq!(compute_progress(Some(50), Some(0)), 0);
    }

    #[test]
    fn set_current_page_updates_progress() {
        let mut b = Book::new(
            BookInput {
                title: "x".into(),
                page_count: Some(400),
                ..Default::default()
            },
            1,
        )
        .unwrap();
        b.set_current_page(Some(200), 2).unwrap();
        assert_eq!(b.progress_pct, 50);
        assert!(b.set_current_page(Some(500), 3).is_err());
    }

    #[test]
    fn finishing_records_the_last_page() {
        let mut b = Book::new(
            BookInput {
                title: "x".into(),
                page_count: Some(300),
                current_page: Some(180),
                ..Default::default()
            },
            1,
        )
        .unwrap();
        assert_eq!(b.progress_pct, 60);
        b.set_status(BookStatus::Finished, 2).unwrap();
        assert_eq!(b.current_page, Some(300));
        assert_eq!(b.progress_pct, 100);
    }

    #[test]
    fn rejects_out_of_range_fields() {
        assert!(Book::new(
            BookInput {
                title: "x".into(),
                page_count: Some(-1),
                ..Default::default()
            },
            1,
        )
        .is_err());
        assert!(Book::new(
            BookInput {
                title: "x".into(),
                publication_year: Some(0),
                ..Default::default()
            },
            1,
        )
        .is_err());
        assert!(Book::new(
            BookInput {
                title: "x".into(),
                rating: Some(11),
                ..Default::default()
            },
            1,
        )
        .is_err());
    }

    #[test]
    fn apply_preserves_id_and_created_at() {
        let mut b = Book::new(minimal("الأيام"), 10).unwrap();
        let id = b.id.clone();
        b.apply(minimal("الأيام - السيرة الذاتية"), 20).unwrap();
        assert_eq!(b.id, id);
        assert_eq!(b.created_at, 10);
        assert_eq!(b.updated_at, 20);
        assert_eq!(b.title, "الأيام - السيرة الذاتية");
    }

    #[test]
    fn rejects_overlong_title() {
        let long = "أ".repeat(limits::TITLE + 1);
        assert!(Book::new(minimal(&long), 1).is_err());
    }
}
