//! Goodreads CSV export parser.
//!
//! Goodreads' "Import and export → Export library" produces a CSV with a fixed
//! header row. This module turns that CSV into [`GoodreadsRow`] values that the
//! import command can preview and commit. Parsing is pure (no I/O, no DB) so it
//! can be unit-tested against fixture files.

use serde::{Deserialize, Serialize};
use specta::Type;

/// One row from a Goodreads export, normalised for Maktabati.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct GoodreadsRow {
    /// Goodreads' own book id (kept for de-dup / re-import).
    pub goodreads_id: String,
    pub title: String,
    pub author: String,
    pub isbn10: Option<String>,
    pub isbn13: Option<String>,
    /// 0..=5, `None` when unrated.
    pub rating: Option<u8>,
    /// Maktabati reading status: "read" | "reading" | "to_read" | "unread".
    pub status: String,
    pub page_count: Option<u32>,
    pub publication_year: Option<i32>,
    /// ISO-ish "YYYY-MM-DD" as exported; `None` when blank.
    pub date_read: Option<String>,
    /// Publishing house, exactly as Goodreads spells it (`None` when the column is absent).
    pub publisher: Option<String>,
    /// Physical binding as exported ("Hardcover", "Kindle Edition"…); map with
    /// [`binding_to_format`].
    pub binding: Option<String>,
    /// Bookshelves the user filed the row on (excluding the exclusive shelf characters).
    pub tags: Vec<String>,
    pub review: Option<String>,
    /// Human-readable problems with this row (non-fatal).
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("empty input")]
    Empty,
    #[error("missing required column: {0}")]
    MissingColumn(&'static str),
    #[error("malformed csv: {0}")]
    Csv(String),
}

/// Parse a Goodreads export CSV. Returns one row per data line (header skipped).
pub fn parse_goodreads_csv(text: &str) -> Result<Vec<GoodreadsRow>, ParseError> {
    let text = text.trim_start_matches('\u{feff}'); // strip BOM
    if text.trim().is_empty() {
        return Err(ParseError::Empty);
    }
    let rows = parse_csv(text).map_err(ParseError::Csv)?;
    if rows.is_empty() {
        return Err(ParseError::Empty);
    }
    let header = &rows[0];
    let idx = |name: &str| -> Option<usize> {
        header
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let col_title = idx("Title").ok_or(ParseError::MissingColumn("Title"))?;
    let col_author = idx("Author").ok_or(ParseError::MissingColumn("Author"))?;
    let col_id = idx("Book Id");
    let col_isbn = idx("ISBN");
    let col_isbn13 = idx("ISBN13");
    let col_rating = idx("My Rating");
    let col_pages = idx("Number of Pages");
    let col_year = idx("Year Published");
    let col_orig_year = idx("Original Publication Year");
    let col_date_read = idx("Date Read");
    let col_shelves = idx("Bookshelves");
    let col_exclusive = idx("Exclusive Shelf");
    let col_review = idx("My Review");
    let col_publisher = idx("Publisher");
    let col_binding = idx("Binding");

    let mut out = Vec::new();
    for (lineno, r) in rows.iter().skip(1).enumerate() {
        if r.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let get =
            |i: Option<usize>| -> String { i.and_then(|i| r.get(i)).cloned().unwrap_or_default() };
        let title = get(Some(col_title)).trim().to_string();
        let author = get(Some(col_author)).trim().to_string();
        if title.is_empty() {
            // skip rows with no title but record nothing
            continue;
        }
        let mut warnings = Vec::new();
        let goodreads_id = get(col_id).trim().to_string();
        let isbn10 = clean_isbn(&get(col_isbn));
        let isbn13 = clean_isbn(&get(col_isbn13));
        let rating = parse_rating(&get(col_rating), &mut warnings);
        let status = map_status(&get(col_exclusive));
        let page_count = parse_u32(&get(col_pages));
        let publication_year = parse_i32(&get(col_year)).or_else(|| parse_i32(&get(col_orig_year)));
        let date_read = nonempty(get(col_date_read));
        let tags: Vec<String> = get(col_shelves)
            .split([',', ';'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let review = nonempty(get(col_review));
        let publisher = nonempty(get(col_publisher));
        let binding = nonempty(get(col_binding));
        if author.is_empty() {
            warnings.push(format!("row {}: missing author", lineno + 2));
        }
        out.push(GoodreadsRow {
            goodreads_id,
            title,
            author,
            isbn10,
            isbn13,
            rating,
            status,
            page_count,
            publication_year,
            date_read,
            publisher,
            binding,
            tags,
            review,
            warnings,
        });
    }
    Ok(out)
}

fn clean_isbn(s: &str) -> Option<String> {
    // Goodreads exports ISBN as `="0123..."` or `=""`.
    let s = s.trim();
    let s = s.strip_prefix("=\"").unwrap_or(s);
    let s = s.strip_suffix('"').unwrap_or(s);
    let s = s.trim_matches('=').trim_matches('"').trim();
    if s.is_empty() || s == "0" {
        None
    } else {
        Some(s.to_string())
    }
}

fn parse_rating(s: &str, warnings: &mut Vec<String>) -> Option<u8> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Goodreads exports ratings as decimals ("4.0", "5.0"); tolerate both.
    let n: f64 = match s.parse() {
        Ok(v) => v,
        Err(_) => {
            warnings.push(format!("bad rating: {s}"));
            return None;
        }
    };
    let rounded = n.round();
    if !(0.0..=5.0).contains(&rounded) {
        warnings.push(format!("rating out of range: {n}"));
        return None;
    }
    Some(rounded as u8)
}

fn map_status(shelf: &str) -> String {
    match shelf.trim().to_lowercase().as_str() {
        "read" => "read".into(),
        "currently-reading" => "reading".into(),
        "to-read" => "to_read".into(),
        "" => "unread".into(),
        other => other.to_string(),
    }
}

fn parse_u32(s: &str) -> Option<u32> {
    s.trim().parse().ok()
}

fn parse_i32(s: &str) -> Option<i32> {
    s.trim().parse().ok()
}

fn nonempty(s: String) -> Option<String> {
    let t = s.trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// Map a Goodreads `Binding` value onto a Maktabati book-format literal
/// (one of `domain::BookFormat`'s stored strings, `None` when unmappable).
///
/// Goodreads knows several paper bindings (Hardcover, Paperback, Mass Market Paperback…) —
/// all of them are the same *print* format to Maktabati, which keeps the physical detail in
/// a note the caller may add. Anything audiobook/ebook-like maps to its own format.
#[must_use]
pub fn binding_to_format(binding: Option<&str>) -> Option<&'static str> {
    let b = binding?.trim().to_lowercase();
    if b.is_empty() {
        return None;
    }
    let audio = b.contains("audio") || b.contains("cd") || b.contains("cassette");
    let ebook = b.contains("kindle")
        || b.contains("nook")
        || b.contains("ebook")
        || b.contains("e-book")
        || b.contains("digital");
    let pdf = b.contains("pdf");
    if audio {
        Some("audiobook")
    } else if pdf {
        Some("pdf")
    } else if ebook {
        Some("ebook")
    } else {
        // Hardcover, Paperback, Mass Market Paperback, Spiral-bound… all print.
        Some("print")
    }
}

/// Minimal RFC-4180-ish CSV parser. Handles quoted fields with embedded commas/newlines
/// and doubled-quote escapes.
fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut field = String::new();
    let mut row: Vec<String> = Vec::new();
    let mut in_quotes = false;
    let mut field_started = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let n = chars.len();
    // We process char by char; track whether we've seen any content in current field.
    while i < n {
        let c = chars[i];
        if in_quotes {
            if c == '"' {
                if i + 1 < n && chars[i + 1] == '"' {
                    field.push('"');
                    i += 2;
                    continue;
                } else {
                    in_quotes = false;
                    i += 1;
                    continue;
                }
            } else {
                field.push(c);
                i += 1;
                continue;
            }
        } else {
            match c {
                '"' => {
                    in_quotes = true;
                    field_started = true;
                    i += 1;
                }
                ',' => {
                    row.push(std::mem::take(&mut field));
                    field_started = false;
                    i += 1;
                }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    field_started = false;
                    // only push non-empty trailing? push always; trailing empties handled by skip
                    rows.push(std::mem::take(&mut row));
                    i += 1;
                }
                '\r' => {
                    i += 1;
                }
                _ => {
                    field.push(c);
                    field_started = true;
                    i += 1;
                }
            }
        }
    }
    // trailing field/row
    if field_started || !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    // drop a trailing empty row
    if rows
        .last()
        .map(|r| r.iter().all(|c| c.is_empty()))
        .unwrap_or(false)
    {
        rows.pop();
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "Book Id,Title,Author,ISBN,ISBN13,My Rating,Average Rating,Publisher,Binding,Number of Pages,Year Published,Original Publication Year,Date Read,Date Added,Bookshelves,Bookshelves with positions,Exclusive Shelf,My Review\n\
123,The Pragmatic Programmer,Andrew Hunt,=\"020161622X\",=\"9780201616224\",5,4.2,Addison-Wesley,Hardcover,352,1999,1999,2024/01/15,2023/12/01,tech; favorites,,read,Great book\n\
456,Empty Author Book,,=\"\",=\"\",0,0.0,,,,, ,,,to-read,\n";

    #[test]
    fn parses_basic() {
        let rows = parse_goodreads_csv(FIXTURE).unwrap();
        assert_eq!(rows.len(), 2);
        let r = &rows[0];
        assert_eq!(r.title, "The Pragmatic Programmer");
        assert_eq!(r.author, "Andrew Hunt");
        assert_eq!(r.isbn10.as_deref(), Some("020161622X"));
        assert_eq!(r.isbn13.as_deref(), Some("9780201616224"));
        assert_eq!(r.rating, Some(5));
        assert_eq!(r.status, "read");
        assert_eq!(r.page_count, Some(352));
        assert_eq!(r.publication_year, Some(1999));
        assert_eq!(r.date_read.as_deref(), Some("2024/01/15"));
        assert_eq!(r.publisher.as_deref(), Some("Addison-Wesley"));
        assert_eq!(r.binding.as_deref(), Some("Hardcover"));
        assert_eq!(r.tags, vec!["tech".to_string(), "favorites".to_string()]);
        assert_eq!(r.review.as_deref(), Some("Great book"));
    }

    #[test]
    fn publisher_and_binding_are_optional() {
        let rows = parse_goodreads_csv(FIXTURE).unwrap();
        let r = &rows[1];
        assert_eq!(r.publisher, None);
        assert_eq!(r.binding, None);
    }

    #[test]
    fn binding_maps_onto_book_formats() {
        assert_eq!(binding_to_format(Some("Hardcover")), Some("print"));
        assert_eq!(
            binding_to_format(Some("Mass Market Paperback")),
            Some("print")
        );
        assert_eq!(binding_to_format(Some("Kindle Edition")), Some("ebook"));
        assert_eq!(
            binding_to_format(Some("Audible Audio CD")),
            Some("audiobook")
        );
        assert_eq!(binding_to_format(Some("PDF eBook")), Some("pdf"));
        assert_eq!(binding_to_format(Some("")), None);
        assert_eq!(binding_to_format(None), None);
    }

    #[test]
    fn empty_input_errors() {
        assert!(matches!(parse_goodreads_csv(""), Err(ParseError::Empty)));
    }

    #[test]
    fn missing_title_column_errors() {
        assert!(matches!(
            parse_goodreads_csv("Author,ISBN\na,b"),
            Err(ParseError::MissingColumn("Title"))
        ));
    }

    #[test]
    fn status_mapping() {
        let csv = "Title,Author,Exclusive Shelf\nA,B,currently-reading\nC,D,to-read\nE,F,\n";
        let rows = parse_goodreads_csv(csv).unwrap();
        assert_eq!(rows[0].status, "reading");
        assert_eq!(rows[1].status, "to_read");
        assert_eq!(rows[2].status, "unread");
    }
}
