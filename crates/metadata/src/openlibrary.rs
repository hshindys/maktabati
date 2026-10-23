//! Open Library (`openlibrary.org`) response parsing — pure, no I/O.
//!
//! The HTTP fetch lives in `src-tauri`; this module only turns an already-fetched
//! JSON document into an [`EnrichedBook`] suggestion, so it is trivially testable
//! offline and the network layer stays thin. Two response shapes are accepted:
//!
//! * `jscmd=data` (documented "data" API): everything is at the top level and
//!   people/publishers are `{"name": …}` objects.
//! * `jscmd=details` / plain `details`: most strings sit inside a `details` object
//!   and values may arrive as `{"value": …}` wrappers.

use serde::{Deserialize, Serialize};
use specta::Type;

/// An enrichment *suggestion* parsed from an Open Library response.
///
/// Deliberately a projection — never a full `BookInput` — because `/enrich`
/// must not write: the caller answers a discriminative i18n prompt (import vs.
/// merge vs. discard) and maps these fields onto whatever it decides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct EnrichedBook {
    pub title: String,
    pub subtitle: Option<String>,
    /// Author names, in the order Open Library lists them.
    pub authors: Vec<String>,
    /// Publisher names, plain (the caller matches / creates rows).
    pub publishers: Vec<String>,
    /// Blurb or publisher's note, merged into one text when several are present.
    pub description: Option<String>,
    /// Largest available cover image URL (`None` when the record has none).
    pub cover_url: Option<String>,
    pub publication_year: Option<i32>,
    pub page_count: Option<i32>,
    /// Digits only, normalized.
    pub isbn13: Option<String>,
}

/// Parse an Open Library JSON document into a suggestion.
///
/// Returns `None` when the document has no usable title (`{}`, an "invoke
/// fallback" response, a `nothing` marker, …) — the caller then falls back to
/// title+author search.
#[must_use]
pub fn parse_openlibrary_data(value: &serde_json::Value) -> Option<EnrichedBook> {
    // `jscmd=details` nests everything under "details"; accept both layouts.
    let obj = value
        .get("details")
        .filter(|details| details.is_object())
        .and_then(serde_json::Value::as_object)
        .or_else(|| value.as_object())?;

    let title = text(obj, "title")?;

    let authors = name_list(obj, "authors");
    let publishers = name_list(obj, "publishers");

    let description = description_from(obj);
    let cover_url = cover_from(obj);
    let publication_year = text(obj, "publish_date").as_deref().and_then(extract_year);
    let page_count = obj
        .get("number_of_pages")
        .and_then(serde_json::Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .or_else(|| {
            obj.get("pagination")
                .and_then(serde_json::Value::as_str)
                .and_then(pages_from_pagination)
        });
    let isbn13 = isbn13_from(obj);

    Some(EnrichedBook {
        title,
        subtitle: text(obj, "subtitle"),
        authors,
        publishers,
        description,
        cover_url,
        publication_year,
        page_count,
        isbn13,
    })
}

/// Extract a human-readable string from a field that may be
/// `"text"` or `{"value": "text"}`; `None` for missing/blank/non-text.
fn text(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    clean(obj.get(key))
}

fn clean(value: Option<&serde_json::Value>) -> Option<String> {
    let raw = match value? {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => map.get("value")?.as_str()?.to_string(),
        _ => return None,
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `{"name": "X"}` objects *or* plain `"X"` strings (details responses use
/// strings for publishers, data responses use objects; real data is
/// inconsistent, so accept both) → `["X", …]`, unnamed entries skipped.
fn name_list(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Vec<String> {
    match obj.get(key).and_then(serde_json::Value::as_array) {
        Some(items) => items
            .iter()
            .filter_map(|item| {
                let name = match item {
                    serde_json::Value::String(name) => Some(name.as_str()),
                    serde_json::Value::Object(_) => {
                        item.get("name").and_then(serde_json::Value::as_str)
                    }
                    _ => None,
                }?;
                let name = name.trim();
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            })
            .collect(),
        None => Vec::new(),
    }
}

/// Blurb sources accepted, in priority order:
/// `description` → `notes` → `excerpt` → `first_sentence` (each string or
/// `{"value": …}`); multiple sources are joined with a blank line.
fn description_from(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let parts: Vec<String> = ["description", "notes", "excerpt", "first_sentence"]
        .iter()
        .filter_map(|key| clean(obj.get(*key)))
        .collect();
    match parts.len() {
        0 => None,
        _ => Some(parts.join("\n\n")),
    }
}

/// Largest cover URL available (`large` > `medium` > `small`).
fn cover_from(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let cover = obj.get("cover")?.as_object()?;
    ["large", "medium", "small"]
        .iter()
        .find_map(|size| cover.get(*size).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(ToOwned::to_owned)
}

/// First plausible year in a free-form publish date ("October 1, 1988",
/// "2nd ed. 1997", "edio 2003", …). Arabic-era dates like "١٤٤٥" are ignored
/// because we only look for ASCII four-digit years.
fn extract_year(date: &str) -> Option<i32> {
    let bytes = date.as_bytes();
    let mut start = 0;
    while start + 4 <= bytes.len() {
        if !bytes[start].is_ascii_digit() {
            start += 1;
            continue;
        }
        let run: &str = &date[start..start + 4];
        if run.bytes().all(|b| b.is_ascii_digit()) {
            if let Ok(year) = run.parse::<i32>() {
                if (1000..=2100).contains(&year) {
                    return Some(year);
                }
            }
        }
        start += 1;
    }
    None
}

/// `"352 p."`, `"114 pages ; 20 cm"`, `"xv, 12 p."` (details-format
/// `pagination`) → the page count of the *first* all-digit run. Roman-numeral
/// preliminaries ("xv") are skipped because they carry no digit.
fn pages_from_pagination(pagination: &str) -> Option<i32> {
    let digits: &str = pagination
        .split(|c: char| !c.is_ascii_digit())
        .find(|run| !run.is_empty())?;
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i32>().ok().filter(|pages| *pages > 0)
}

/// Collect an ISBN-13 preference: `identifiers.isbn_13` first, then the
/// top-level `isbn_13` used by details responses from search results.
fn isbn13_from(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let list = obj
        .get("identifiers")
        .and_then(|ids| ids.get("isbn_13"))
        .or_else(|| obj.get("isbn_13"))
        .and_then(serde_json::Value::as_array)?;
    list.iter()
        .filter_map(serde_json::Value::as_str)
        .map(|raw| raw.chars().filter(char::is_ascii_digit).collect::<String>())
        .find(|digits| digits.len() == 13)
}

/// Parse one `docs` entry of a `https://openlibrary.org/search.json` response.
///
/// The search endpoint matches by title **and/or** author, so it is the fallback
/// behind [`parse_openlibrary_data`] when the caller has no ISBN — and its doc
/// shape differs enough to deserve its own parser: arrays of plain strings, a
/// numeric `cover_i`, a `first_publish_year`, and no description.
#[must_use]
pub fn parse_openlibrary_search(value: &serde_json::Value) -> Option<EnrichedBook> {
    let doc = value.get("docs")?.as_array()?.first()?.as_object()?;

    let title = text(doc, "title")?;
    let authors = string_list(doc, "author_name");
    let publishers = string_list(doc, "publisher");
    let cover_url = doc
        .get("cover_i")
        .and_then(serde_json::Value::as_i64)
        .filter(|id| *id > 0)
        .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg"));
    let publication_year = doc
        .get("first_publish_year")
        .and_then(serde_json::Value::as_i64)
        .and_then(|y| i32::try_from(y).ok());
    let page_count = doc
        .get("number_of_pages_median")
        .and_then(serde_json::Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .filter(|n| *n > 0);
    let isbn13 = doc
        .get("isbn")
        .and_then(serde_json::Value::as_array)
        .and_then(|list| {
            list.iter()
                .filter_map(serde_json::Value::as_str)
                .find_map(|raw| {
                    let digits = raw.chars().filter(char::is_ascii_digit).collect::<String>();
                    (digits.len() == 13).then_some(digits)
                })
        });

    Some(EnrichedBook {
        title,
        subtitle: text(doc, "subtitle"),
        authors,
        publishers,
        // Search docs never carry a blurb; an `/enrich` by title+author yields none rather
        // than costing a second round-trip to fetch the edition record.
        description: None,
        cover_url,
        publication_year,
        page_count,
        isbn13,
    })
}

/// `{"foo": ["a", …]}` → `["a", …]` (blank entries skipped, others ignored).
fn string_list(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Vec<String> {
    match obj.get(key).and_then(serde_json::Value::as_array) {
        Some(items) => items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_the_documented_data_shape() {
        let doc = json!({
            "bib_key": "ISBN:9780140328721",
            "info_url": "https://openlibrary.org/books/OL2732960M",
            "title": "Fantastic Mr. Fox",
            "subtitle": "and other tricky animals",
            "authors": [
                {"name": "Roald Dahl", "url": "https://openlibrary.org/authors/OL26492A"}
            ],
            "number_of_pages": 94,
            "publishers": [{"name": "Puffin"}, {"name": ""}],
            "publish_date": "October 1, 1988",
            "cover": {"small": "https://covers.openlibrary.org/b/id/207-s.jpg",
                      "medium": "https://covers.openlibrary.org/b/id/207-m.jpg",
                      "large": "https://covers.openlibrary.org/b/id/207-l.jpg"},
            "notes": "Publisher's note about the edition.",
            "excerpt": "Boggis was an enormously fat man.",
            "identifiers": {"isbn_13": ["978-0-14-032872-1"], "isbn_10": ["0140328726"]}
        });

        let parsed = parse_openlibrary_data(&doc).expect("should parse");
        assert_eq!(parsed.title, "Fantastic Mr. Fox");
        assert_eq!(parsed.subtitle.as_deref(), Some("and other tricky animals"));
        assert_eq!(parsed.authors, vec!["Roald Dahl".to_string()]);
        assert_eq!(parsed.publishers, vec!["Puffin".to_string()]);
        assert_eq!(parsed.page_count, Some(94));
        assert_eq!(parsed.publication_year, Some(1988));
        assert_eq!(
            parsed.cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/207-l.jpg")
        );
        assert_eq!(parsed.isbn13.as_deref(), Some("9780140328721"));
        // Notes and excerpt merge, newest not lost.
        assert!(parsed
            .description
            .as_deref()
            .unwrap()
            .contains("Publisher's note"));
        assert!(parsed.description.as_deref().unwrap().contains("Boggis"));
    }

    #[test]
    fn parses_the_details_shape() {
        // `jscmd=details` puts strings inside "details" and wraps some in {"value"}.
        let doc = json!({
            "bib_key": "ISBN:9781843104812",
            "details": {
                "title": "Aspects of the Novel",
                "publishers": ["Martino Fine Books"],
                "publish_date": "2012",
                "description": {"value": "A famous discussion of the novel form."},
                "pagination": "114 pages",
                "cover": {"medium": "https://covers.openlibrary.org/b/id/12345-m.jpg"},
                "number_of_pages": null,
                "isbn_13": ["9781843104812"]
            }
        });

        let parsed = parse_openlibrary_data(&doc).expect("should parse");
        assert_eq!(parsed.title, "Aspects of the Novel");
        assert_eq!(parsed.publishers, vec!["Martino Fine Books".to_string()]);
        assert_eq!(parsed.publication_year, Some(2012));
        assert_eq!(parsed.page_count, Some(114));
        assert_eq!(
            parsed.description.as_deref(),
            Some("A famous discussion of the novel form.")
        );
        assert_eq!(
            parsed.cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/12345-m.jpg")
        );
        assert_eq!(parsed.isbn13.as_deref(), Some("9781843104812"));
    }

    #[test]
    fn unusable_documents_return_none() {
        for doc in [
            json!({}),
            json!({"bib_key": "ISBN:123"}),
            json!({"title": ""}),
            json!("not an object"),
            json!(null),
        ] {
            assert!(parse_openlibrary_data(&doc).is_none(), "doc = {doc}");
        }
    }

    #[test]
    fn title_is_required_but_everything_else_is_optional() {
        let doc = json!({"title": "A poem without a name"});
        let parsed = parse_openlibrary_data(&doc).expect("should parse");
        assert_eq!(parsed.title, "A poem without a name");
        assert_eq!(parsed.authors, Vec::<String>::new());
        assert_eq!(parsed.publishers, Vec::<String>::new());
        assert_eq!(parsed.description, None);
        assert_eq!(parsed.cover_url, None);
        assert_eq!(parsed.publication_year, None);
        assert_eq!(parsed.page_count, None);
        assert_eq!(parsed.isbn13, None);
    }

    #[test]
    fn years_are_extracted_from_messy_publish_dates() {
        /* We take the FIRST plausible 4-digit year in the string — edition and
         * original-publication years are indistinguishable at this layer, and callers
         * that care (Goodreads import) have their own columns. */
        assert_eq!(extract_year("October 1, 1988"), Some(1988));
        assert_eq!(extract_year("2nd ed. 1997, 1st 1954"), Some(1997));
        assert_eq!(extract_year("2012"), Some(2012));
        assert_eq!(extract_year(""), None);
        assert_eq!(extract_year("no year here"), None);
        // Plausible years only.
        assert_eq!(extract_year("12"), None);
        assert_eq!(extract_year("999"), None);
        assert_eq!(extract_year("99999"), None); // every 4-digit window is 9999 → rejected.
    }

    #[test]
    fn pagination_loses_trap_characters() {
        assert_eq!(pages_from_pagination("352 p."), Some(352));
        assert_eq!(pages_from_pagination("114 pages ; 20 cm"), Some(114));
        assert_eq!(pages_from_pagination("xv, 12 p."), Some(12));
        assert_eq!(pages_from_pagination("no digits"), None);
        assert_eq!(pages_from_pagination("0 p."), None);
    }

    #[test]
    fn parses_the_search_doc_shape() {
        let doc = json!({
            "numFound": 1,
            "docs": [{
                "key": "/works/OL45804W",
                "title": "Fantastic Mr. Fox",
                "subtitle": "and other tricky animals",
                "author_name": ["Roald Dahl", "Donald Chaffin"],
                "publisher": ["Puffin Books", "Knight"],
                "first_publish_year": 1970,
                "number_of_pages_median": 94,
                "cover_i": 8739161,
                "isbn": ["0140328726", "9780140328721", "9780142327689"]
            }]
        });

        let parsed = parse_openlibrary_search(&doc).expect("should parse");
        assert_eq!(parsed.title, "Fantastic Mr. Fox");
        assert_eq!(
            parsed.authors,
            vec!["Roald Dahl".to_string(), "Donald Chaffin".to_string()]
        );
        assert_eq!(
            parsed.publishers,
            vec!["Puffin Books".to_string(), "Knight".to_string()]
        );
        assert_eq!(parsed.publication_year, Some(1970));
        assert_eq!(parsed.page_count, Some(94));
        assert_eq!(parsed.isbn13.as_deref(), Some("9780140328721"));
        assert_eq!(
            parsed.cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/8739161-L.jpg")
        );
        assert_eq!(parsed.description, None);
    }

    #[test]
    fn empty_or_hitless_searches_return_none() {
        assert!(parse_openlibrary_search(&json!({"numFound": 0, "docs": []})).is_none());
        assert!(parse_openlibrary_search(&json!({})).is_none());
    }
}
