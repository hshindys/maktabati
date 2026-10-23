//! `Quote` entity and its write input.
//!
//! A quote (اقتباس) is text the user lifted out of a book together with where it came from
//! (page and/or chapter). The `quotes` table already exists in `0001_init.sql` — this module
//! only gives it the typed domain rulebook the other entities have: a trimmed, non-empty,
//! length-capped body, an optional positive page, an optional short chapter label and an
//! optional highlight colour.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::enums::QuoteColor;
use crate::validate::{check_len, check_opt_len, check_page, check_required};
use crate::{ids::new_id, text::normalize, Result};

/// Length caps for quote fields.
pub mod limits {
    /// Quote body cap — generous for long critical passages, still bounded.
    pub const TEXT: usize = 8_000;
    /// Quote note cap.
    pub const NOTE: usize = 2_000;
}

/// A passage lifted from a book.
///
/// `source`, `file_id` and `position_ref` stay storage-level for now: the reader (M6) is what
/// will quote from attachments; a hand-entered quote needs none of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Quote {
    /// UUID v7.
    pub id: String,
    /// The book the passage was lifted from.
    pub book_id: String,
    /// Passage as written.
    pub text: String,
    /// Normalized mirror (search + duplicate detection), same contract as `books.title_norm`.
    pub text_norm: String,
    /// The user's own annotation.
    pub note: Option<String>,
    /// Page in the copy the user read.
    pub page: Option<i32>,
    /// Chapter label, as printed («الفصل الرابع», `Ch. 3`…).
    pub chapter: Option<String>,
    /// Highlight colour (reader palette).
    pub color: QuoteColor,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last edit timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or editing a [`Quote`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct QuoteInput {
    /// Passage (required).
    pub text: String,
    /// The user's annotation.
    pub note: Option<String>,
    /// Page in the copy.
    pub page: Option<i32>,
    /// Chapter label.
    pub chapter: Option<String>,
    /// Highlight colour; defaults to yellow.
    pub color: Option<QuoteColor>,
}

impl Quote {
    /// Build a new quote, deriving id, the normalized mirror and timestamps.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::EmptyField`] when the body is blank.
    /// * [`crate::DomainError::TooLong`] when the body, note or chapter exceed their caps.
    /// * [`crate::DomainError::OutOfRange`] when the page is not positive.
    pub fn new(book_id: impl Into<String>, input: QuoteInput, now_ms: i64) -> Result<Self> {
        let (text, text_norm, note, page, chapter, color) = Self::prepare(input)?;
        Ok(Self {
            id: new_id(),
            book_id: book_id.into(),
            text,
            text_norm,
            note,
            page,
            chapter,
            color,
            created_at: now_ms,
            updated_at: now_ms,
        })
    }

    /// Apply an edit in place, re-validating and stamping `updated_at`.
    ///
    /// # Errors
    ///
    /// Same as [`Quote::new`].
    pub fn apply(&mut self, input: QuoteInput, now_ms: i64) -> Result<()> {
        let (text, text_norm, note, page, chapter, color) = Self::prepare(input)?;
        self.text = text;
        self.text_norm = text_norm;
        self.note = note;
        self.page = page;
        self.chapter = chapter;
        self.color = color;
        self.updated_at = now_ms;
        Ok(())
    }

    /// Validate and normalize a raw input — shared by create and edit.
    #[allow(clippy::type_complexity)]
    fn prepare(
        input: QuoteInput,
    ) -> Result<(
        String,
        String,
        Option<String>,
        Option<i32>,
        Option<String>,
        QuoteColor,
    )> {
        let text = input.text.trim().to_owned();
        check_required("text", &text)?;
        check_len("text", &text, limits::TEXT)?;
        // `check_opt_len` validates; the stored value is the trimmed, absent-when-blank one.
        let note = input
            .note
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_owned());
        let chapter = input
            .chapter
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_owned());
        check_opt_len("note", note.as_deref(), limits::NOTE)?;
        check_opt_len(
            "chapter",
            chapter.as_deref(),
            crate::book::limits::SHORT_TEXT,
        )?;
        check_page("page", input.page)?;
        let text_norm = normalize(&text);
        let color = input.color.unwrap_or(QuoteColor::Yellow);
        Ok((text, text_norm, note, input.page, chapter, color))
    }
}

#[cfg(test)]
use crate::DomainError;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn input(text: &str) -> QuoteInput {
        QuoteInput {
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn creates_with_defaults() {
        let q = Quote::new("b1", input(" « قال الشابي …» "), 10).unwrap();
        assert_eq!(q.book_id, "b1");
        assert_eq!(q.text, "« قال الشابي …»");
        assert_eq!(q.color, QuoteColor::Yellow);
        assert_eq!(q.created_at, 10);
        assert_eq!(q.updated_at, 10);
        assert_eq!(q.page, None);
        assert_eq!(q.note, None);
    }

    #[test]
    fn blank_text_is_rejected() {
        let err = Quote::new("b1", input("   "), 1).unwrap_err();
        assert_eq!(
            err,
            DomainError::EmptyField {
                field: "text".into()
            }
        );
    }

    #[test]
    fn page_must_be_positive() {
        let err = Quote::new(
            "b1",
            QuoteInput {
                text: "x".into(),
                page: Some(0),
                ..Default::default()
            },
            1,
        )
        .unwrap_err();
        assert!(matches!(err, DomainError::OutOfRange { field, .. } if field == "page"));
    }

    #[test]
    fn apply_edits_and_stamps_updated_at() {
        let mut q = Quote::new("b1", input("old"), 10).unwrap();
        q.apply(
            QuoteInput {
                text: "new".into(),
                page: Some(42),
                chapter: Some("الفصل الأول".into()),
                note: Some("ذهبي".into()),
                color: Some(QuoteColor::Purple),
            },
            20,
        )
        .unwrap();
        assert_eq!(q.text, "new");
        assert_eq!(q.text_norm, normalize("new"));
        assert_eq!(q.page, Some(42));
        assert_eq!(q.chapter.as_deref(), Some("الفصل الأول"));
        assert_eq!(q.note.as_deref(), Some("ذهبي"));
        assert_eq!(q.color, QuoteColor::Purple);
        assert_eq!(q.created_at, 10);
        assert_eq!(q.updated_at, 20);
    }

    #[test]
    fn overlong_text_is_rejected() {
        let err = Quote::new("b1", input(&"أ".repeat(limits::TEXT + 1)), 1).unwrap_err();
        assert!(matches!(
            err,
            DomainError::TooLong {
                max: limits::TEXT,
                ..
            }
        ));
    }
}
