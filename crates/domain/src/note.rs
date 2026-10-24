//! `NotePage` and `NoteBlock` — the entities behind the notes screen (الملاحظات).
//!
//! The tables live in `0001_init.sql`: `notes_page` is a document (optionally anchored to a
//! book and/or a parent page), `note_block` is its flat sequence of typed content blocks, and
//! `note_link` records cross-references (out of v1's scope — nothing writes links yet).
//!
//! The schema allows nesting (`parent_page_id`, `parent_block_id`) and fractional-order
//! text positions to keep an offline-first sync honest later. v1 is deliberately simpler:
//! pages order by the `position` integer the schema gives them, blocks keep a strictly
//! sequential integer ordering stored in the `position` text column, and neither a page
//! nor a block moves between parents. Those are real capabilities the editor has no user
//! for yet — and a UI that pretends otherwise would orphan real sequences.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::enums::NoteBlockType;
use crate::json::Json;
use crate::validate::{check_len, check_opt_len};
use crate::{ids::new_id, text::normalize, Result};

/// Length caps for note fields.
pub mod limits {
    /// Page title cap.
    pub const TITLE: usize = 300;
    /// Icon identifier cap (a short name into the icon set, not raw markup).
    pub const ICON: usize = 60;
    /// Block text cap — same generous bound as quotes, for long critical passages.
    pub const TEXT: usize = 8_000;
}

/// A note document: a page, optionally anchored to a book and/or a parent page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct NotePage {
    /// UUID v7.
    pub id: String,
    /// The book the page documents, if it lives on a book's notes tab.
    pub book_id: Option<String>,
    /// The page this page nests under; `None` at the top level.
    pub parent_page_id: Option<String>,
    /// Title as written; may be empty (an untitled page is a real state).
    pub title: String,
    /// Normalized mirror of the title (search), same contract as `books.title_norm`.
    pub title_norm: String,
    /// Optional icon identifier.
    pub icon: Option<String>,
    /// `true` for pages cloned from a template; v1 never sets it.
    pub is_template: bool,
    /// Which template the page came from (when [`NotePage::is_template`]).
    pub template_key: Option<String>,
    /// Pinned pages float to the top of the notes screen.
    pub is_pinned: bool,
    /// Sibling ordering.
    pub position: i64,
    /// Bumped every write, so a later sync can detect concurrent edits.
    pub rev: i64,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last edit timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or editing a [`NotePage`].
///
/// `book_id` and `parent_page_id` only mean anything on **create** — the editor never
/// re-anchors an existing page, and the storage layer keeps the stored values on update,
/// so an input that carries them there is ignored rather than a silent surprise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct NotePageInput {
    /// Page title (may be blank — an untitled page shows its first block's text later).
    pub title: String,
    /// Icon identifier.
    pub icon: Option<String>,
    /// The book this page documents (create only).
    pub book_id: Option<String>,
    /// The page this page nests under (create only).
    pub parent_page_id: Option<String>,
    /// Pin the page to the top of the notes screen.
    pub is_pinned: bool,
}

impl NotePage {
    /// Build a new page, deriving id, the normalized mirror and timestamps.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::TooLong`] when the title or icon exceeds its cap.
    pub fn new(input: NotePageInput, now_ms: i64) -> Result<Self> {
        Ok(Self {
            position: 0,
            rev: 1,
            ..Self::apply_parts(input, now_ms, now_ms)?
        })
    }

    /// Apply an edit in place, re-validating and stamping `updated_at`.
    ///
    /// Identity (`id`, `book_id`, `parent_page_id`, template marks, position) is what
    /// makes a page *this* page; only content, icon, pin and rev change on an edit.
    ///
    /// # Errors
    ///
    /// Same as [`NotePage::new`].
    pub fn apply(&mut self, input: NotePageInput, now_ms: i64) -> Result<()> {
        self.is_pinned = input.is_pinned;
        self.rev += 1;
        let (title, title_norm, icon, _) = Self::validate(input)?;
        self.title = title;
        self.title_norm = title_norm;
        self.icon = icon;
        self.updated_at = now_ms;
        Ok(())
    }

    /// Build the create-time struct, sharing the field-wise work with [`Self::apply`].
    ///
    /// # Errors
    ///
    /// Same as [`NotePage::new`].
    fn apply_parts(input: NotePageInput, created_at: i64, updated_at: i64) -> Result<Self> {
        let (title, title_norm, icon, _) = Self::validate(input.clone())?;
        Ok(Self {
            id: new_id(),
            book_id: None,
            parent_page_id: None,
            title,
            title_norm,
            icon,
            is_template: false,
            template_key: None,
            is_pinned: input.is_pinned,
            position: 0,
            rev: 1,
            created_at,
            updated_at,
        })
    }

    /// Validate and normalize the editable parts — shared by create and edit.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::TooLong`] on title or icon overflow.
    fn validate(input: NotePageInput) -> Result<(String, String, Option<String>, bool)> {
        let title = input.title.trim().to_owned();
        check_len("title", &title, limits::TITLE)?;
        let icon = input
            .icon
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_owned());
        check_opt_len("icon", icon.as_deref(), limits::ICON)?;
        let title_norm = normalize(&title);
        Ok((title, title_norm, icon, input.is_pinned))
    }
}

/// A note block: one typed piece of content in a [`NotePage`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct NoteBlock {
    /// UUID v7.
    pub id: String,
    /// The page the block belongs to (fixed for the block's life).
    pub page_id: String,
    /// Reserved for nested blocks; v1 always writes `None`.
    pub parent_block_id: Option<String>,
    /// Sequential ordering among the page's blocks. The column is TEXT (an upgrade path
    /// to fractional indexing), v1 stores decimal integers and orders by numeric value.
    pub position: String,
    /// The block's kind (the 16-value schema enum).
    pub block_type: NoteBlockType,
    /// The open-ended payload per kind — at minimum `{"text": …}` for every text kind.
    pub payload: Json,
    /// Bumped every write.
    pub rev: i64,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last edit timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or editing a [`NoteBlock`].
///
/// The payload is an open object so richer kinds stay forward-compatible; the only shape
/// the domain enforces is "an object" plus "a `text` member, when present, is a string"
/// — every text kind the v1 editor writes, and nothing the validator could wrong-guess.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct NoteBlockInput {
    /// The block's kind.
    pub block_type: NoteBlockType,
    /// The payload (an object; `text` when present is the content).
    pub payload: Json,
}

impl NoteBlockInput {
    /// Validate and normalize the payload.
    ///
    /// The `text` member, when present, must be a string (trimmed here, so the stored
    /// payload is the one the editor meant); absent or richer payload members ride along
    /// untouched. The block kinds the v1 editor writes all fit this shape, and letting
    /// arbitrary members through keeps the richer six kinds forward-compatible.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::TooLong`] when `text` overflows its cap.
    /// * [`crate::DomainError::InvalidTransition`] when the payload is not an object or
    ///   its `text` member is not a string.
    pub fn prepare(self) -> Result<(NoteBlockType, Json)> {
        let NoteBlockInput {
            block_type,
            payload,
        } = self;
        let Json::Object(map) = payload else {
            return Err(crate::DomainError::InvalidTransition {
                detail: "note block payload must be a JSON object".to_owned(),
            });
        };
        let mut map = map;
        if let Some(text) = map.remove("text") {
            let Json::String(s) = text else {
                return Err(crate::DomainError::InvalidTransition {
                    detail: "note block payload `text` must be a string".to_owned(),
                });
            };
            let text = s.trim().to_owned();
            check_len("text", &text, limits::TEXT)?;
            // An empty text stays legal: "add a block, then type" is how editors work,
            // and an empty block is still visible (its kind shows) and deletable —
            // refusing it here would only force the UI to fake content.
            map.insert("text".to_owned(), Json::String(text));
        }
        Ok((block_type, Json::Object(map)))
    }
}

impl NoteBlock {
    /// Build a new block on `page_id` at `position`.
    ///
    /// # Errors
    ///
    /// Same as [`NoteBlockInput::prepare`].
    pub fn new(
        page_id: impl Into<String>,
        position: impl ToString,
        input: NoteBlockInput,
        now_ms: i64,
    ) -> Result<Self> {
        let (block_type, payload) = input.prepare()?;
        Ok(Self {
            id: new_id(),
            page_id: page_id.into(),
            parent_block_id: None,
            position: position.to_string(),
            block_type,
            payload,
            rev: 1,
            created_at: now_ms,
            updated_at: now_ms,
        })
    }

    /// Apply an edit in place, re-validating and stamping `updated_at`.
    ///
    /// # Errors
    ///
    /// Same as [`NoteBlockInput::prepare`].
    pub fn apply(&mut self, input: NoteBlockInput, now_ms: i64) -> Result<()> {
        let (block_type, payload) = input.prepare()?;
        self.block_type = block_type;
        self.payload = payload;
        self.rev += 1;
        self.updated_at = now_ms;
        Ok(())
    }
}

// The payload rules now live directly in [`NoteBlockInput::prepare`].

#[cfg(test)]
use crate::DomainError;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn obj(pairs: &[(&str, serde_json::Value)]) -> Json {
        Json::from(serde_json::Value::Object(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
        ))
    }

    fn text_input(text: &str) -> NoteBlockInput {
        NoteBlockInput {
            block_type: NoteBlockType::Paragraph,
            payload: obj(&[("text", serde_json::Value::String(text.into()))]),
        }
    }

    #[test]
    fn creates_a_page_with_defaults_and_normalizes_the_title() {
        let page = NotePage::new(
            NotePageInput {
                title: "  ملاحظات في الفلسفة ".into(),
                icon: Some("book".into()),
                is_pinned: true,
                ..Default::default()
            },
            10,
        )
        .unwrap();
        assert_eq!(page.title, "ملاحظات في الفلسفة");
        assert_eq!(page.title_norm, normalize("ملاحظات في الفلسفة"));
        assert_eq!(page.icon.as_deref(), Some("book"));
        assert!(page.is_pinned);
        assert!(!page.is_template);
        assert!(page.template_key.is_none());
        assert!(page.book_id.is_none());
        assert_eq!(page.rev, 1);
        assert_eq!(page.created_at, 10);
        assert_eq!(page.updated_at, 10);
    }

    #[test]
    fn an_untitled_page_is_a_valid_state_but_a_long_one_is_not() {
        let untitled = NotePage::new(NotePageInput::default(), 1).unwrap();
        assert_eq!(untitled.title, "");
        assert_eq!(untitled.title_norm, "");

        let err = NotePage::new(
            NotePageInput {
                title: "أ".repeat(limits::TITLE + 1),
                ..Default::default()
            },
            1,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DomainError::TooLong {
                max: limits::TITLE,
                ..
            }
        ));
    }

    #[test]
    fn apply_edits_content_but_never_anchors_or_bumps_rev_out_of_turn() {
        let mut page = NotePage::new(
            NotePageInput {
                book_id: Some("b1".into()),
                ..Default::default()
            },
            10,
        )
        .unwrap();
        assert!(page.book_id.is_none(), "book anchoring is the repo's job");
        page.book_id = Some("b1".into());
        page.position = 3;

        page.apply(
            NotePageInput {
                title: "جديد".into(),
                is_pinned: true,
                ..Default::default()
            },
            20,
        )
        .unwrap();
        assert_eq!(page.title, "جديد");
        assert!(page.is_pinned);
        assert_eq!(page.book_id.as_deref(), Some("b1"), "anchors are immutable");
        assert_eq!(page.position, 3);
        assert_eq!(page.rev, 2);
        assert_eq!(page.updated_at, 20);
        assert_eq!(page.created_at, 10);
    }

    #[test]
    fn a_text_payload_is_validated_and_stored_verbatim_otherwise() {
        let (block_type, payload) = text_input("شقّ التعريفات").prepare().unwrap();
        assert_eq!(block_type, NoteBlockType::Paragraph);
        assert_eq!(
            payload,
            obj(&[("text", serde_json::Value::String("شقّ التعريفات".into()))])
        );

        // A blank text stays legal — an empty block is visible and deletable, the same
        // state "add a block" creates before the user types.
        let (_, empty) = text_input("   ").prepare().unwrap();
        assert_eq!(
            empty,
            obj(&[("text", serde_json::Value::String(String::new()))])
        );

        let err = text_input(&"أ".repeat(limits::TEXT + 1))
            .prepare()
            .unwrap_err();
        assert!(matches!(
            err,
            DomainError::TooLong {
                max: limits::TEXT,
                ..
            }
        ));
    }

    #[test]
    fn the_payload_must_be_an_object_and_text_a_string() {
        let err = NoteBlockInput {
            block_type: NoteBlockType::Paragraph,
            payload: Json::String("just text".into()),
        }
        .prepare()
        .unwrap_err();
        assert!(
            matches!(err, DomainError::InvalidTransition { detail } if detail.contains("object"))
        );

        let err = NoteBlockInput {
            block_type: NoteBlockType::Paragraph,
            payload: obj(&[("text", serde_json::Value::Bool(true))]),
        }
        .prepare()
        .unwrap_err();
        assert!(
            matches!(err, DomainError::InvalidTransition { detail } if detail.contains("string"))
        );

        // Extra members ride along — forward compatibility for the richer kinds.
        let (_, payload) = NoteBlockInput {
            block_type: NoteBlockType::Todo,
            payload: obj(&[
                ("text", serde_json::Value::String("قاعدة".into())),
                ("checked", serde_json::Value::Bool(true)),
            ]),
        }
        .prepare()
        .unwrap();
        assert!(matches!(payload, Json::Object(ref m) if m.contains_key("checked")));
    }

    #[test]
    fn a_block_carries_the_position_it_is_told() {
        let block = NoteBlock::new("p1", "12", text_input("نص"), 10).unwrap();
        assert_eq!(block.page_id, "p1");
        assert_eq!(block.position, "12");
        assert!(block.parent_block_id.is_none());
        assert_eq!(block.block_type, NoteBlockType::Paragraph);
        assert_eq!(block.created_at, 10);
    }
}
