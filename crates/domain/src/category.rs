//! `Category` — a hierarchical, bilingual taxonomy.
//!
//! Categories form a tree (`parent_id`) with a **materialized `path`** so "everything under
//! Literature → Novels" is a single indexed prefix query instead of a recursive CTE.
//!
//! The seeded taxonomy is bilingual (`name_ar` / `name_en`) with a stable, *unlocalized* `key`
//! so renames and translations never break saved shelves or filters.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ids::new_id, text::normalize, DomainError, Result};

/// Maximum nesting depth (`/a/b/c`).
pub const MAX_DEPTH: usize = 3;

/// A node in the category tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Category {
    /// UUID v7.
    pub id: String,
    /// Parent node, `None` for a root category.
    pub parent_id: Option<String>,
    /// Stable machine key (never localized), e.g. `novels`.
    pub key: String,
    /// Arabic name.
    pub name_ar: String,
    /// English name.
    pub name_en: String,
    /// Normalized mirror (of the name matching the active locale is not decided here — both
    /// names are searchable through the projection).
    pub name_norm: String,
    /// Materialized path of `key`s: `/literature/novels/fantasy`.
    pub path: String,
    /// `0` for roots.
    pub depth: usize,
    /// Sibling ordering.
    pub position: i64,
    /// Optional icon identifier.
    pub icon: Option<String>,
    /// Optional accent colour (hex).
    pub color: Option<String>,
    /// `true` for seeded system categories (can be renamed/reordered, never silently deleted).
    pub is_system: bool,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or updating a [`Category`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct CategoryInput {
    /// Stable machine key.
    pub key: String,
    /// Arabic name.
    pub name_ar: String,
    /// English name.
    pub name_en: String,
    /// Icon.
    pub icon: Option<String>,
    /// Colour.
    pub color: Option<String>,
}

/// Slugify a display name into a stable key.
///
/// [`crate::text::slug`] cannot be reused as-is: it runs [`crate::text::normalize`] first, and
/// normalization drops punctuation — including the hyphen — so `Self-Development` would collapse
/// to `selfdevelopment` and the key would lose the word boundary it is meant to show. Splitting on
/// the separators *before* normalizing keeps them, while each piece still gets the full
/// normalization pipeline (diacritics, ta-marbuta fold, digit fold).
///
/// # Examples
///
/// ```
/// use maktabati_domain::category::key_from_name;
/// assert_eq!(key_from_name("Self-Development"), "self-development");
/// assert_eq!(key_from_name("  "), "category");
/// ```
#[must_use]
pub fn key_from_name(name: &str) -> String {
    let key = name
        .split(|c: char| c == '-' || c.is_whitespace())
        .map(crate::text::normalize)
        .filter(|part| !part.is_empty())
        .take(8) // same cap as `slug`
        .collect::<Vec<_>>()
        .join("-");
    if key.is_empty() {
        "category".to_owned()
    } else {
        key.to_ascii_lowercase()
    }
}

/// Build a category, validating depth and computing its materialized path.
///
/// # Errors
///
/// * [`DomainError::EmptyField`] when `key` or either name is empty.
/// * [`DomainError::TooLong`] when the resulting path exceeds 200 characters.
/// * [`DomainError::InvalidTransition`] when nesting would exceed [`MAX_DEPTH`].
pub fn new_category(
    input: CategoryInput,
    parent: Option<&Category>,
    position: i64,
    now_ms: i64,
    is_system: bool,
) -> Result<Category> {
    let key = input.key.trim().to_ascii_lowercase();
    if key.is_empty() {
        return Err(DomainError::EmptyField {
            field: "category.key".into(),
        });
    }
    if input.name_ar.trim().is_empty() || input.name_en.trim().is_empty() {
        return Err(DomainError::EmptyField {
            field: "category.name".into(),
        });
    }

    let (path, depth) = match parent {
        Some(p) => {
            let depth = p.depth + 1;
            if depth > MAX_DEPTH {
                return Err(DomainError::InvalidTransition {
                    detail: format!("categories may nest at most {MAX_DEPTH} levels"),
                });
            }
            (format!("{}/{}", p.path, key), depth)
        }
        None => (format!("/{key}"), 0),
    };

    if path.len() > 200 {
        return Err(DomainError::TooLong {
            field: "category.path".into(),
            got: path.len(),
            max: 200,
        });
    }

    Ok(Category {
        id: new_id(),
        parent_id: parent.map(|p| p.id.clone()),
        key,
        name_ar: input.name_ar.trim().to_owned(),
        name_en: input.name_en.trim().to_owned(),
        name_norm: normalize(&input.name_ar),
        path,
        depth,
        position,
        icon: input.icon,
        color: input.color,
        is_system,
        created_at: now_ms,
        updated_at: now_ms,
    })
}

impl Category {
    /// `true` when `other` is this category or one of its descendants.
    #[must_use]
    pub fn contains_path(&self, other: &str) -> bool {
        other == self.path || other.starts_with(&format!("{}/", self.path))
    }

    /// Localized name for the given key (`ar` or `en`).
    #[must_use]
    pub fn localized(&self, locale: &str) -> &str {
        if locale.starts_with("ar") {
            &self.name_ar
        } else {
            &self.name_en
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn root_category_path() {
        let c = new_category(
            CategoryInput {
                key: "Literature".into(),
                name_ar: "الأدب".into(),
                name_en: "Literature".into(),
                ..Default::default()
            },
            None,
            0,
            1,
            true,
        )
        .unwrap();
        assert_eq!(c.path, "/literature");
        assert_eq!(c.depth, 0);
        assert!(c.is_system);
    }

    #[test]
    fn nested_category_extends_path() {
        let root = new_category(
            CategoryInput {
                key: "literature".into(),
                name_ar: "الأدب".into(),
                name_en: "Literature".into(),
                ..Default::default()
            },
            None,
            0,
            1,
            true,
        )
        .unwrap();
        let child = new_category(
            CategoryInput {
                key: "novels".into(),
                name_ar: "روايات".into(),
                name_en: "Novels".into(),
                ..Default::default()
            },
            Some(&root),
            0,
            1,
            true,
        )
        .unwrap();
        assert_eq!(child.path, "/literature/novels");
        assert_eq!(child.depth, 1);
        assert!(root.contains_path("/literature/novels"));
    }

    #[test]
    fn depth_is_capped() {
        let mut node = new_category(
            CategoryInput {
                key: "a".into(),
                name_ar: "أ".into(),
                name_en: "A".into(),
                ..Default::default()
            },
            None,
            0,
            1,
            false,
        )
        .unwrap();
        for _ in 0..MAX_DEPTH {
            node = new_category(
                CategoryInput {
                    key: "n".into(),
                    name_ar: "ن".into(),
                    name_en: "N".into(),
                    ..Default::default()
                },
                Some(&node),
                0,
                1,
                false,
            )
            .unwrap();
        }
        assert!(new_category(
            CategoryInput {
                key: "too_deep".into(),
                name_ar: "x".into(),
                name_en: "X".into(),
                ..Default::default()
            },
            Some(&node),
            0,
            1,
            false,
        )
        .is_err());
    }

    #[test]
    fn rejects_empty_inputs() {
        assert!(new_category(CategoryInput::default(), None, 0, 1, false).is_err());
    }

    #[test]
    fn localized_names() {
        let c = new_category(
            CategoryInput {
                key: "history".into(),
                name_ar: "التاريخ".into(),
                name_en: "History".into(),
                ..Default::default()
            },
            None,
            0,
            1,
            false,
        )
        .unwrap();
        assert_eq!(c.localized("ar"), "التاريخ");
        assert_eq!(c.localized("en"), "History");
    }
}
