//! `Tag` entity and its write input.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::validate::{check_len, check_required};
use crate::{ids::new_id, text::normalize, Result};

/// Length caps for tag fields.
pub mod limits {
    /// Tag name cap — long enough for `#الحضارة_الإسلامية`, short enough to stay scannable.
    pub const NAME: usize = 60;
}

/// A free-form label (الوسم) the user attaches to books.
///
/// Tags are flat and unordered by design: a label that needs a hierarchy wants to be a
/// category instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Tag {
    /// UUID v7.
    pub id: String,
    /// Name as written.
    pub name: String,
    /// Normalized mirror; uniqueness comes from `ux_tags_name_norm`.
    pub name_norm: String,
    /// Optional colour, `#RRGGBB`.
    pub color: Option<String>,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
}

/// Input for creating or renaming a [`Tag`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct TagInput {
    /// Name (required).
    pub name: String,
    /// Optional colour.
    pub color: Option<String>,
}

impl Tag {
    /// Build a new tag, deriving `id`, the normalized mirror and `created_at`.
    ///
    /// # Errors
    ///
    /// * [`crate::DomainError::EmptyField`] when the name is blank.
    /// * [`crate::DomainError::TooLong`] when the name exceeds [`limits::NAME`].
    pub fn new(input: TagInput, now_ms: i64) -> Result<Self> {
        let (name, name_norm, color) = Self::prepare(input)?;
        Ok(Self {
            id: new_id(),
            name,
            name_norm,
            color,
            created_at: now_ms,
        })
    }

    /// Apply an edit in place.
    ///
    /// The `tags` table carries no `updated_at` — renaming a label is not a semantic
    /// change to it — so this deliberately touches no timestamp.
    ///
    /// # Errors
    ///
    /// Same as [`Tag::new`].
    pub fn apply(&mut self, input: TagInput) -> Result<()> {
        let (name, name_norm, color) = Self::prepare(input)?;
        self.name = name;
        self.name_norm = name_norm;
        self.color = color;
        Ok(())
    }

    /// Validate and normalize a raw input — shared by create and edit.
    fn prepare(input: TagInput) -> Result<(String, String, Option<String>)> {
        let name = input.name.trim().to_owned();
        check_required("name", &name)?;
        check_len("name", &name, limits::NAME)?;
        let name_norm = normalize(&name);
        let color = input
            .color
            .filter(|c| !c.trim().is_empty())
            .map(|c| c.trim().to_owned());
        Ok((name, name_norm, color))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DomainError;
    use pretty_assertions::assert_eq;

    #[test]
    fn new_tag_trims_and_normalizes() {
        let t = Tag::new(
            TagInput {
                name: " #الحضارة ".into(),
                color: None,
            },
            7,
        )
        .unwrap();
        assert_eq!(t.name, "#الحضارة");
        assert_eq!(t.name_norm, normalize("#الحضارة"));
        assert_eq!(t.created_at, 7);
        assert_eq!(t.color, None);
    }

    #[test]
    fn blank_tag_is_rejected() {
        let err = Tag::new(
            TagInput {
                name: "   ".into(),
                color: None,
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
    fn overlong_tag_is_rejected() {
        let err = Tag::new(
            TagInput {
                name: "x".repeat(limits::NAME + 1),
                color: None,
            },
            1,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            DomainError::TooLong {
                max: limits::NAME,
                ..
            }
        ));
    }

    #[test]
    fn apply_renames_and_recolours() {
        let mut t = Tag::new(
            TagInput {
                name: "a".into(),
                color: None,
            },
            1,
        )
        .unwrap();
        let before = t.clone();
        t.apply(TagInput {
            name: "  b ".into(),
            color: Some("  #ff0000 ".into()),
        })
        .unwrap();
        assert_eq!(t.id, before.id);
        assert_eq!(t.created_at, before.created_at);
        assert_eq!(t.name, "b");
        assert_eq!(t.color.as_deref(), Some("#ff0000"));
    }
}
