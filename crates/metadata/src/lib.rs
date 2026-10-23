//! maktabati-metadata — metadata parsing and enrichment.

pub mod goodreads;
pub use goodreads::{GoodreadsRow, ParseError, parse_goodreads_csv};
