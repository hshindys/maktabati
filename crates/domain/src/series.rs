//! `Series` entity and its write input.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ids::new_id, text::normalize};

/// A book series (السلسلة).
///
/// A book belongs to at most one series in v1 — a deliberate simplification documented in
/// `docs/03-data-model.md`; multi-series membership would complicate the add form for a case
/// that is rare in practice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Series {
    /// UUID v7.
    pub id: String,
    /// Name as written.
    pub name: String,
    /// Normalized mirror.
    pub name_norm: String,
    /// Description.
    pub description: Option<String>,
    /// Creation timestamp, epoch ms UTC.
    pub created_at: i64,
    /// Last update timestamp, epoch ms UTC.
    pub updated_at: i64,
}

/// Input for creating or updating a [`Series`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Default)]
pub struct SeriesInput {
    /// Name (required).
    pub name: String,
    /// Description.
    pub description: Option<String>,
}

impl Series {
    /// Build a new series.
    #[must_use]
    pub fn new(input: SeriesInput, now_ms: i64) -> Self {
        let name_norm = normalize(&input.name);
        Self {
            id: new_id(),
            name: input.name.trim().to_owned(),
            name_norm,
            description: input.description.filter(|s| !s.trim().is_empty()),
            created_at: now_ms,
            updated_at: now_ms,
        }
    }

    /// Recompute derived fields after an edit.
    pub fn apply(&mut self, input: SeriesInput, now_ms: i64) {
        self.name = input.name.trim().to_owned();
        self.name_norm = normalize(&self.name);
        self.description = input.description.filter(|s| !s.trim().is_empty());
        self.updated_at = now_ms;
    }
}
