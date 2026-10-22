//! # maktabati-domain
//!
//! Pure domain layer for **Maktabati / مكتبتي**: entities, value objects, business rules
//! and the Arabic text-normalization contract shared by storage, search and duplicate detection.
//!
//! This crate performs **no I/O** — no database, no filesystem, no HTTP, no Tauri. That keeps
//! every rule here trivially unit-testable and lets `storage`/`search`/`stats` depend on it
//! without dragging in a runtime.
//!
//! ## Modules
//!
//! | Module | Contains |
//! |---|---|
//! | [`error`] | `DomainError` — typed validation failures |
//! | [`ids`] | UUIDv7 identifier generation |
//! | [`isbn`] | `Isbn` value object with checksum validation and 10↔13 conversion |
//! | [`text`] | Arabic-aware normalization, light stemming, duplicate keys |
//! | [`money`] | Exact monetary amounts (integer minor units) |
//! | [`json`] | `Json` — open-ended JSON that is safe to export to TypeScript |
//! | [`rating`] | Half-star ratings stored as `0..=10` |
//! | [`enums`] | All stored enumerations, mirroring the SQL `CHECK` constraints |
//! | [`book`], [`author`], [`file`], … | Entities and their write inputs |
//! | [`rules`] | Cross-entity business rules (status transitions, aggregates, dedupe) |

#![forbid(unsafe_code)]

pub mod author;
pub mod book;
pub mod category;
pub mod enums;
pub mod error;
pub mod file;
pub mod ids;
pub mod isbn;
pub mod json;
pub mod language;
pub mod money;
pub mod publisher;
pub mod rating;
pub mod rules;
pub mod series;
pub mod tag;
pub mod text;

/// Field validation shared by every entity input (private to the crate: it exists so the
/// entities cannot drift apart, not as part of the public API).
mod validate;

pub use author::{Author, AuthorInput};
pub use book::{AuthorCredit, Book, BookInput, BookStatus};
pub use category::{Category, CategoryInput};
pub use enums::*;
pub use error::DomainError;
pub use file::{BookFile, BookFileInput};
pub use isbn::Isbn;
pub use json::Json;
pub use language::{Language, LanguageInput};
pub use money::Money;
pub use publisher::{Publisher, PublisherInput};
pub use rating::Rating;
pub use series::{Series, SeriesInput};
pub use tag::{Tag, TagInput};

/// Result alias used throughout the domain layer.
pub type Result<T> = std::result::Result<T, DomainError>;
