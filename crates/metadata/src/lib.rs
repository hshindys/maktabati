//! maktabati-metadata — metadata parsing and enrichment.

pub mod goodreads;
pub mod openlibrary;
pub use goodreads::{parse_goodreads_csv, GoodreadsRow, ParseError};
pub use openlibrary::{parse_openlibrary_data, parse_openlibrary_search, EnrichedBook};
