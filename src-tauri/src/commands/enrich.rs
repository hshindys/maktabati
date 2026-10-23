//! Open Library enrichment — the network half of Phase "Hermes".
//!
//! The pure parsing lives in [`maktabati_metadata::openlibrary`]; this module only
//! performs the HTTPS round-trips (reqwest, rustls) and exposes them three ways:
//!
//! * [`metadata_enrich`] / [`metadata_enrich_search`] — IPC commands for the webview.
//! * the Hermes server's `POST /enrich` endpoint (`hermes.rs`).
//! * the CLI (`main.rs` `maktabati enrich <isbn>`), via [`run_cli_isbn`].
//!
//! Nothing in this module ever writes: an enrichment is a *suggestion* the user
//! answers a prompt about (import / merge / discard).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::{AppErrorDto, CmdResult};

use maktabati_metadata::{parse_openlibrary_data, parse_openlibrary_search, EnrichedBook};

/// Polite identification + a hard wall-clock cap per request.
fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("maktabati/0.1 (+enrich)")
        .timeout(Duration::from_secs(15))
        .build()
        .ok()
}

/// Only digits survive (`978-0-14-…` and `=9780140328721` both become clean keys).
/// `None` when what remains is neither 10 nor 13 digits — i.e. not something the
/// Open Library "bibkey" will accept.
fn clean_isbn(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    match digits.len() {
        10 | 13 => Some(digits),
        _ => None,
    }
}

/// Fetch the Open Library `data` document for an ISBN and parse it.
///
/// `Ok(None)` = the request succeeded but Open Library has no such record (the
/// data API answers `{}` for unknown bibkeys).
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) on network failures, non-success HTTP
/// statuses, or a response body that is not JSON.
pub async fn fetch_by_isbn(isbn: &str) -> Result<Option<EnrichedBook>, AppErrorDto> {
    let digits = clean_isbn(isbn).ok_or_else(|| {
        AppErrorDto::new(
            "enrich.invalid_isbn",
            format!("`{isbn}` is not an ISBN-10 or ISBN-13"),
        )
        .with_i18n_key("error.enrich.invalid_isbn")
    })?;

    let client = client().ok_or_else(|| {
        AppErrorDto::new("enrich.network", "could not build the HTTP client")
            .with_i18n_key("error.enrich.network")
    })?;
    let url =
        format!("https://openlibrary.org/api/books?bibkeys=ISBN:{digits}&format=json&jscmd=data");
    let response = client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| {
            AppErrorDto::new(
                "enrich.network",
                format!("Open Library request failed: {e}"),
            )
            .with_i18n_key("error.enrich.network")
        })?;

    let payload: serde_json::Value = response.json().await.map_err(|e| {
        AppErrorDto::new(
            "enrich.network",
            format!("Open Library body was not JSON: {e}"),
        )
        .with_i18n_key("error.enrich.network")
    })?;

    // With a single bibkey the data API returns the record object itself; with several it
    // keys them. Accept both.
    let record = payload
        .as_object()
        .and_then(|map| {
            map.keys()
                .find(|key| key.starts_with("ISBN:"))
                .and_then(|key| map.get(key))
        })
        .unwrap_or(&payload);

    Ok(parse_openlibrary_data(record))
}

/// Fetch a suggestion by title + author via `search.json`; used when no ISBN is at hand.
pub async fn fetch_by_title_author(
    title: &str,
    author: &str,
) -> Result<Option<EnrichedBook>, AppErrorDto> {
    let title = title.trim();
    let author = author.trim();
    if title.is_empty() {
        return Err(AppErrorDto::new(
            "enrich.invalid_isbn",
            "a title is required for a search-based enrich",
        )
        .with_i18n_key("error.enrich.invalid_isbn"));
    }

    let client = client().ok_or_else(|| {
        AppErrorDto::new("enrich.network", "could not build the HTTP client")
            .with_i18n_key("error.enrich.network")
    })?;
    let mut request = client
        .get("https://openlibrary.org/search.json")
        .query(&[("limit", "1")]);
    if !author.is_empty() {
        request = request.query(&[("author", author)]);
    }
    let response = request
        .query(&[("title", title)])
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| {
            AppErrorDto::new("enrich.network", format!("Open Library search failed: {e}"))
                .with_i18n_key("error.enrich.network")
        })?;
    let payload: serde_json::Value = response.json().await.map_err(|e| {
        AppErrorDto::new(
            "enrich.network",
            format!("Open Library body was not JSON: {e}"),
        )
        .with_i18n_key("error.enrich.network")
    })?;

    Ok(parse_openlibrary_search(&payload))
}

/// What the HTTP `/enrich` endpoint accepts. `isbn` alone, or `title` (+ optional
/// `author`), decide which lookup runs — never both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct EnrichRequest {
    /// An ISBN-10 or ISBN-13 (junk tolerated around the digits).
    pub isbn: Option<String>,
    /// Title (required when `isbn` is absent).
    pub title: Option<String>,
    /// Author hint for the title search; optional even when `title` is present.
    pub author: Option<String>,
}

/// The HTTP response: a suggestion, or `{"suggestion": null}` — **never** a write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct EnrichSuggestion {
    /// `None` when Open Library has nothing matching.
    pub suggestion: Option<EnrichedBook>,
    /// Which lookup produced it: `isbn` or `search`.
    pub mode: String,
}

/// Join both flavours for the HTTP endpoint: ISBN first, then title+author.
pub async fn suggest(request: &EnrichRequest) -> Result<EnrichSuggestion, AppErrorDto> {
    let isbn = request.isbn.as_deref().filter(|s| !s.trim().is_empty());
    if let Some(isbn) = isbn {
        return Ok(EnrichSuggestion {
            suggestion: fetch_by_isbn(isbn).await?,
            mode: "isbn".to_owned(),
        });
    }
    let title = request.title.as_deref().unwrap_or("").trim();
    let author = request.author.as_deref().unwrap_or("");
    if title.is_empty() {
        return Err(AppErrorDto::new(
            "enrich.empty_request",
            "an enrich request needs an `isbn`, or a `title` (+ optional `author`)",
        )
        .with_i18n_key("error.enrich.empty_request"));
    }
    Ok(EnrichSuggestion {
        suggestion: fetch_by_title_author(title, author).await?,
        mode: "search".to_owned(),
    })
}

/// CLI branch (`maktabati enrich <isbn>`): fetch, parse, print. Runs a private
/// single-threaded runtime so no Tauri app has to exist. Exit codes: 0 with the
/// JSON (or `null` for "nothing found"), 1 for any failure — messages go to stderr
/// so stdout stays pipeable.
#[allow(clippy::needless_pass_by_value)]
pub fn run_cli_isbn(isbn: String) -> i32 {
    let Some(digits) = clean_isbn(&isbn) else {
        eprintln!("maktabati: `{isbn}` is not an ISBN-10 or ISBN-13");
        return 1;
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("maktabati: could not start the async runtime: {e}");
            return 1;
        }
    };
    let result = runtime.block_on(fetch_by_isbn(&digits));

    match result {
        Ok(Some(book)) => {
            match serde_json::to_string_pretty(&book) {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("maktabati: could not serialise the suggestion: {e}");
                    return 1;
                }
            }
            0
        }
        Ok(None) => {
            println!("null");
            0
        }
        Err(err) => {
            eprintln!("maktabati: {err}");
            1
        }
    }
}

/// Suggest book metadata for an ISBN, straight from Open Library.
///
/// **No writes.** The suggestion is returned to the webview, i18n-prompted
/// (import / merge / discard), and only then applied by whoever owns the decision.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) on network failures or an invalid ISBN.
/// `Ok(None)` = Open Library has no such record.
#[specta::specta]
#[tauri::command]
pub async fn metadata_enrich(isbn: String) -> CmdResult<Option<EnrichedBook>> {
    fetch_by_isbn(&isbn).await
}

/// Suggest metadata by title (+ optional author) when no ISBN is known.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) on network failures; an empty title.
#[specta::specta]
#[tauri::command]
pub async fn metadata_enrich_search(
    title: String,
    author: String,
) -> CmdResult<Option<EnrichedBook>> {
    fetch_by_title_author(&title, &author).await
}
