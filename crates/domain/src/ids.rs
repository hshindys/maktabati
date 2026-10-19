//! Identifier generation.
//!
//! All primary keys are **UUID v7**: time-ordered (so `ORDER BY id` roughly follows creation),
//! collision-free across devices, and safe to expose over IPC. Text IDs also keep the SQL simple
//! and avoid SQLite's `INTEGER PRIMARY KEY` churn during merges and imports.

use serde::{Deserialize, Serialize};

/// Generate a new identifier (UUID v7, hyphenated, lower-case).
///
/// # Examples
///
/// ```
/// use maktabati_domain::ids::new_id;
/// let id = new_id();
/// assert_eq!(id.len(), 36);
/// assert_eq!(id.matches('-').count(), 4);
/// ```
#[must_use]
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Marker wrapper used in tests to prove ids are monotonically sortable by creation time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(transparent)]
pub struct Id(pub String);

impl Id {
    /// Create a fresh v7 id.
    #[must_use]
    pub fn new() -> Self {
        Self(new_id())
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn ids_are_unique() {
        let mut seen = BTreeSet::new();
        for _ in 0..5_000 {
            assert!(seen.insert(new_id()));
        }
    }

    #[test]
    fn v7_ids_sort_by_creation_time() {
        let mut ids = Vec::new();
        for _ in 0..200 {
            ids.push(new_id());
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "v7 ids must be time-sortable");
    }
}
