//! Cover commands — pick/clear a book's cover image and resolve its display path.
//!
//! The library owns copied cover bytes (`covers/<book_id>.<ext>` under the data dir),
//! the same policy as attachments in "copy" mode: a cover cannot break because the
//! file it was picked from is deleted. `cover_url` resolves a row-relative path to an
//! absolute one so the webview can load it through Tauri's asset protocol (`convertFileSrc`),
//! which the CSP already allows.

use std::sync::Arc;
use std::time::Duration;

use tauri::State;

use maktabati_domain::Book;
use maktabati_storage::repo;

use super::blocking;
use crate::error::{AppErrorDto, CmdResult};
use crate::state::AppState;

/// Copy a picked image into the library and set it as the book's cover.
///
/// `source_path` must be absolute (the file dialog returns one) and must end in one of
/// `jpg`, `jpeg`, `png`, `webp` — validated in the repo, not trusted from the webview.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the book is gone, the extension is not
/// an image one, or a copy/update failed.
#[specta::specta]
#[tauri::command]
pub async fn cover_set(
    state: State<'_, AppState>,
    book_id: String,
    source_path: String,
) -> CmdResult<Book> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| {
        repo::book::set_cover(db, &data_dir, &book_id, &source_path)
    })
    .await
}

/// Clear the book's cover and delete its copied bytes.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the book is gone or the update failed.
#[specta::specta]
#[tauri::command]
pub async fn cover_remove(state: State<'_, AppState>, book_id: String) -> CmdResult<Book> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| {
        repo::book::remove_cover(db, &data_dir, &book_id)
    })
    .await
}

/// Absolute filesystem path of a book's cover, for `convertFileSrc` in the webview.
///
/// `Ok(None)` when the book has no cover **or** the recorded file is gone from disk — the
/// honest miss renders the generated placeholder instead of a broken image.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the query fails.
#[specta::specta]
#[tauri::command]
pub async fn cover_url(state: State<'_, AppState>, book_id: String) -> CmdResult<Option<String>> {
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    blocking(db, move |db| -> maktabati_storage::Result<Option<String>> {
        let book = repo::book::get(db, &book_id)?;
        let Some(rel) = book.cover_path else {
            return Ok(None);
        };
        let path = data_dir.join(&rel);
        if !path.is_file() {
            // A vanished file is the same as no cover — and letting the caller know it was
            // suspect would need a log, so that lands here on purpose.
            tracing::warn!(book_id, rel, "cover recorded but missing on disk");
            return Ok(None);
        }
        Ok(Some(path.display().to_string()))
    })
    .await
}

/// Base URL for the Open Library images host; overridden with wiremock in tests.
const OPENLIBRARY_BASE: &str = "https://covers.openlibrary.org";
/// Base URL for the Open Library search API; overridden with wiremock in tests.
const OPENLIBRARY_API_BASE: &str = "https://openlibrary.org";
/// Base URL for the Google Books API host; overridden with wiremock in tests.
const GOOGLEBOOKS_BASE: &str = "https://www.googleapis.com";
/// Base URL for Amazon's ISBN-keyed product-image host; overridden with wiremock in tests.
const AMAZON_BASE: &str = "https://images-na.ssl-images-amazon.com";
/// Base URL for the Internet Archive; overridden with wiremock in tests.
const ARCHIVE_BASE: &str = "https://archive.org";

fn client() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("maktabati/0.1 (+cover)")
        .timeout(Duration::from_secs(15))
        .build()
        .ok()
}

fn cover_network(detail: String) -> AppErrorDto {
    AppErrorDto::new("cover.network", detail).with_i18n_key("error.cover.network")
}

/// An answer that only ever *carries* data into the app: the bytes plus the extension
/// detected from magic numbers. Never trusts a `Content-Type` or a filename.
pub(crate) type CoverHit = (Vec<u8>, &'static str);

/// Identify an image's extension from its leading magic bytes. Providers answer with
/// JPG, PNG or WebP; anything else is not a cover we can store.
///
/// Open Library serves a blank 1×1 GIF placeholder for misses, so "unknown magic" maps
/// to `None` rather than an error — a miss, not a failure.
pub(crate) fn image_ext_from(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if bytes.len() > 12 && bytes[..4] == *b"RIFF" && bytes[8..12] == *b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Download the image at `image_url` (Open Library's covers host) and validate magic bytes.
///
/// `?default=false` makes a miss answer 404 instead of the placeholder image; a body
/// that still is not a real image (blank GIF, HTML error page) is treated the same way.
/// Search-record URLs arrive without the flag, so it is appended when missing.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on request failure,
/// or a cut-off body.
pub(crate) async fn download_cover_image(
    client: &reqwest::Client,
    image_url: &str,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let url = if image_url.contains('?') {
        image_url.to_owned()
    } else {
        format!("{image_url}?default=false")
    };
    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return Err(cover_network(format!("cover image request failed: {e}"))),
    };
    // A miss is 404 — a miss, not an outage.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(cover_network(format!(
            "cover image answered HTTP {}",
            response.status()
        )));
    }
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => return Err(cover_network(format!("cover image body failed: {e}"))),
    };
    Ok(image_ext_from(&bytes).map(|ext| (bytes.to_vec(), ext)))
}

/// Download the largest Open Library cover available for `isbn` digits.
pub(crate) async fn grab_openlibrary_isbn(
    client: &reqwest::Client,
    base_url: &str,
    isbn: &str,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let url = format!("{base_url}/b/isbn/{isbn}-L.jpg?default=false");
    download_cover_image(client, &url).await
}

/// Find a cover by title (+ optional author hint) through Open Library's search
/// index — the fallback for books without a usable ISBN. The search answers a
/// numeric `cover_i`, from which the covers host serves the largest image through
/// the same magic-byte download as the ISBN path.
///
/// The search API is treated tolerantly, like Google Books: a non-200 answer is a
/// miss rather than an outage, because the index errors verbosely on odd titles.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on transport
/// failure, or a body that is not JSON.
pub(crate) async fn grab_openlibrary_search(
    client: &reqwest::Client,
    api_base: &str,
    covers_base: &str,
    title: &str,
    author: Option<&str>,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let title = title.trim();
    if title.is_empty() {
        return Ok(None); // nothing to search with
    }
    let mut request = client
        .get(format!("{api_base}/search.json"))
        .query(&[("limit", "1"), ("fields", "cover_i")]);
    if let Some(author) = author.filter(|name| !name.trim().is_empty()) {
        request = request.query(&[("author", author)]);
    }
    let response = match request.query(&[("title", title)]).send().await {
        Ok(r) => r,
        Err(e) => return Err(cover_network(format!("Open Library search failed: {e}"))),
    };
    if response.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let payload: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return Err(cover_network(format!(
                "Open Library search body was not JSON: {e}"
            )))
        }
    };
    let Some(cover_i) = payload
        .pointer("/docs/0/cover_i")
        .and_then(serde_json::Value::as_i64)
    else {
        return Ok(None);
    };
    download_cover_image(client, &format!("{covers_base}/b/id/{cover_i}-L.jpg")).await
}

/// Pixel dimensions of a JPEG or PNG body, parsed from the format's own headers.
///
/// Only the two formats `image_ext_from` already trusts are measured, so a hit that is
/// not measurable is precisely a non-image.
pub(crate) fn image_dims(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) && bytes.len() >= 24 {
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return Some((width, height));
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                break;
            }
            let marker = bytes[i + 1];
            if marker == 0xFF {
                i += 1; // a stuffing byte; re-inspect from here
                continue;
            }
            // SOF0–SOF15 carry the frame header, except DHT (C4), JPG (C8) and DAC
            // (CC), which are markers whose next bytes are not a frame header.
            if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                let height = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]);
                let width = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]);
                return Some((u32::from(width), u32::from(height)));
            }
            let len = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
            i += 2 + len.max(2);
        }
    }
    None
}

/// Derive the ISBN-10 equivalent of a 978-family ISBN-13.
///
/// Amazon's image host is keyed by ISBN-10, and most books carry only the 13-digit form.
/// The 979 family has no ISBN-10 equivalent, so those digits stay unconverted.
pub(crate) fn isbn13_to_isbn10(digits: &str) -> Option<String> {
    if digits.len() != 13
        || !digits.chars().all(|c| c.is_ascii_digit())
        || !digits.starts_with("978")
    {
        return None;
    }
    let core = &digits[3..12];
    let sum: u32 = core
        .bytes()
        .enumerate()
        .map(|(n, b)| (u32::from(b - b'0')) * (10 - n as u32))
        .sum();
    let check = match (11 - (sum % 11)) % 11 {
        10 => 'X',
        other => char::from(b'0' + other as u8),
    };
    Some(format!("{core}{check}"))
}

/// Download a cover for `isbn10` from Amazon's ISBN-keyed product-image host.
///
/// The host has no miss answer: any ISBN gets the generic "no image available"
/// placeholder back as a *valid JPEG* whose dimensions vary by placeholder but is never
/// portrait for a book-style request, while real jackets always are. So the magic bytes
/// alone are not enough — a hit must additionally measure taller than wide. The endpoint
/// fires no HTTP error code for a miss, and it is a `200` even for a nonsense ISBN.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on request failure, or
/// a cut-off body. A placeholder — or any non-portrait image — is a miss, not a failure.
pub(crate) async fn grab_amazon_isbn(
    client: &reqwest::Client,
    base_url: &str,
    isbn10: &str,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let url = format!("{base_url}/images/P/{isbn10}.01.LZZZZZZZ.jpg");
    let Some((bytes, ext)) = download_cover_image(client, &url).await? else {
        return Ok(None);
    };
    let portrait = image_dims(&bytes).is_some_and(|(width, height)| height > width);
    Ok(match portrait {
        true => Some((bytes, ext)),
        false => None,
    })
}

/// Find the matching scanned item on the Internet Archive by an
/// `(isbn:…)` query and return its item image.
///
/// The search accepts `fl[]=identifier` only, `rows=1` narrowed by the
/// `mediatype:(texts)` and popular-first `sort[]`. The item image is served through the
/// same magic-byte-verified download as the Open Library path, but the miss answer here
/// is a redirect to archive.org's own "not found" image — followed by the client, so the
/// final URL URI must be examined to catch it (the image itself is a legitimate
/// PNG that would otherwise be stored as a cover).
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on a failing request
/// or a non-JSON body. A miss is silently `Ok(None)`: the search bails out on a non-200
/// response, a miss answer or a missing `identifier` field.
pub(crate) async fn grab_internetarchive_isbn(
    client: &reqwest::Client,
    base_url: &str,
    digits: &str,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let query = format!("(isbn:{digits}) AND mediatype:(texts)");
    let response = match client
        .get(format!("{base_url}/advancedsearch.php"))
        .query(&[
            ("q", query.as_str()),
            ("fl[]", "identifier"),
            ("rows", "1"),
            ("sort[]", "downloads desc"),
            ("output", "json"),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(cover_network(format!(
                "Internet Archive search failed: {e}"
            )))
        }
    };
    if response.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let payload: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return Err(cover_network(format!(
                "Internet Archive search body was not JSON: {e}"
            )))
        }
    };
    let Some(identifier) = payload
        .pointer("/response/docs/0/identifier")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    else {
        return Ok(None);
    };
    download_internetarchive_item_image(client, base_url, &identifier).await
}

/// Find the matching scanned item on the Internet Archive by title (+ optional
/// author hint) and return its item image — the **last** fallback of the title-search
/// chain, for books neither provider has ever indexed.
///
/// The query uses the `title:` and `creator:` operators so the archive matches the work
/// instead of every mention; the `sort[]=downloads desc` ordering prefers the scan people
/// actually read when the same edition exists more than once.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on a failing request or
/// a non-JSON body; misses are `Ok(None)`, like every other title search.
pub(crate) async fn grab_internetarchive_search(
    client: &reqwest::Client,
    base_url: &str,
    title: &str,
    author: Option<&str>,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let title = title.trim();
    if title.is_empty() {
        return Ok(None); // nothing to search with
    }
    // `quote_term` already wraps in ASCII quotes — do not wrap again.
    let mut query = format!("title:{} AND mediatype:(texts)", quote_term(title));
    if let Some(author) = author.filter(|name| !name.trim().is_empty()) {
        query.push_str(&format!(" AND creator:{}", quote_term(author.trim())));
    }
    let response = match client
        .get(format!("{base_url}/advancedsearch.php"))
        .query(&[
            ("q", query.as_str()),
            ("fl[]", "identifier"),
            ("rows", "1"),
            ("sort[]", "downloads desc"),
            ("output", "json"),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(cover_network(format!(
                "Internet Archive search failed: {e}"
            )))
        }
    };
    if response.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let payload: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return Err(cover_network(format!(
                "Internet Archive search body was not JSON: {e}"
            )))
        }
    };
    let Some(identifier) = payload
        .pointer("/response/docs/0/identifier")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    else {
        return Ok(None);
    };
    download_internetarchive_item_image(client, base_url, &identifier).await
}

/// Download one Internet Archive item's image (`/services/img/<identifier>`).
///
/// A nonexistent identifier redirects to the archive's own grey "not found" picture, and
/// the client follows that silently — so the *final URL* is what declares a miss.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on a failing request or
/// a cut-off body; the not-found redirect, a non-success status, or anything that is not
/// an image is a miss.
async fn download_internetarchive_item_image(
    client: &reqwest::Client,
    base_url: &str,
    identifier: &str,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let response = match client
        .get(format!("{base_url}/services/img/{identifier}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Err(cover_network(format!("cover image request failed: {e}"))),
    };
    if response.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    if response.url().path().ends_with("/images/notfound.png") {
        return Ok(None); // the grey "not found" picture followed silently
    }
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => return Err(cover_network(format!("cover image body failed: {e}"))),
    };
    Ok(image_ext_from(&bytes).map(|ext| (bytes.to_vec(), ext)))
}

/// Download a Google Books cover for `isbn` digits: one `volumes?q=isbn:` lookup for a
/// thumbnail URL, then the thumbnail itself.
///
/// This is the fallback for books Open Library has never heard of — small Arab-market
/// press runs mostly live here — so it never runs when Open Library already found one.
/// It is written to tolerate the API's quirks (absent `imageLinks` blocks, `zoom`
/// levels pinned at 1).
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on request failure,
/// or a body that is not JSON. A lookup that answers but has no thumbnail is `Ok(None)`.
pub(crate) async fn grab_googlebooks_isbn(
    client: &reqwest::Client,
    base_url: &str,
    isbn: &str,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let response = match client
        .get(format!("{base_url}/books/v1/volumes"))
        .query(&[("q", format!("isbn:{isbn}"))])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Err(cover_network(format!("Google Books lookup failed: {e}"))),
    };
    // The volumes API errors with a non-200 body that is still JSON; anything other
    // than a clean 200 is a miss to us, not a crash.
    if response.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let payload: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return Err(cover_network(format!(
                "Google Books body was not JSON: {e}"
            )))
        }
    };
    let Some(thumbnail) = payload
        .pointer("/items/0/volumeInfo/imageLinks/thumbnail")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    else {
        return Ok(None);
    };
    // Thumbnails come pinned at `zoom=1` (~128px); bump to `zoom=2` for something
    // readable on a book page. The scheme stays whatever the provider returned — the
    // bytes are downloaded here in the backend and stored, so no policy is needed.
    let image_url = thumbnail.replace("zoom=1", "zoom=2");
    let image = match client.get(&image_url).send().await {
        Ok(r) => r,
        Err(e) => return Err(cover_network(format!("cover image request failed: {e}"))),
    };
    if image.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let bytes = match image.bytes().await {
        Ok(b) => b,
        Err(e) => return Err(cover_network(format!("cover image body failed: {e}"))),
    };
    Ok(image_ext_from(&bytes).map(|ext| (bytes.to_vec(), ext)))
}

/// Try the four providers, in the order the library trusts: Open Library covers first
/// (canonical `-L` covers), then the Google Books directory, then Amazon's ISBN-keyed
/// product-image host, then the Internet Archive's scans. A provider **error** still stops
/// the chain — silently continuing after a network failure would turn every outage into
/// "no cover found".
///
/// `candidates` are every ISBN form the book carries, in preference order and deduped
/// (13-digit first, the derived or stored 10-digit second) — a provider that keys by one
/// form gets every form before the chain moves to the next provider. The hit is returned
/// together with the provider it came from, because that is exactly what the cover's
/// provenance should say.
pub(crate) async fn grab_cover(
    client: &reqwest::Client,
    ol_base: &str,
    googlebooks_base: &str,
    amazon_base: &str,
    archive_base: &str,
    candidates: &[String],
) -> Result<Option<(Vec<u8>, &'static str, maktabati_domain::CoverSource)>, AppErrorDto> {
    // Open Library is keyed by either ISBN form, so every candidate gets asked there
    // before the chain moves on.
    for digits in candidates {
        if let Some((bytes, ext)) = grab_openlibrary_isbn(client, ol_base, digits).await? {
            return Ok(Some((
                bytes,
                ext,
                maktabati_domain::CoverSource::OpenLibrary,
            )));
        }
    }
    for digits in candidates {
        if let Some((bytes, ext)) = grab_googlebooks_isbn(client, googlebooks_base, digits).await? {
            return Ok(Some((
                bytes,
                ext,
                maktabati_domain::CoverSource::GoogleBooks,
            )));
        }
    }
    // Amazon answers only for ISBN-10 forms.
    for digits in candidates.iter().filter(|d| d.len() == 10) {
        if let Some((bytes, ext)) = grab_amazon_isbn(client, amazon_base, digits).await? {
            return Ok(Some((bytes, ext, maktabati_domain::CoverSource::Amazon)));
        }
    }
    for digits in candidates {
        if let Some((bytes, ext)) = grab_internetarchive_isbn(client, archive_base, digits).await? {
            return Ok(Some((
                bytes,
                ext,
                maktabati_domain::CoverSource::InternetArchive,
            )));
        }
    }
    Ok(None)
}

/// Find a cover by title (+ optional author hint) through the Google Books volumes API —
/// the **second** link of the title-search chain, and the one that usually answers for
/// Arabic titles: Open Library's index is thin for Arabic-market editions, while Google
/// Books carries most of them.
///
/// `q` gets the narrowing `intitle:` / `inauthor:` operators — unlike the ISBN path,
/// this is relevance-ranked and the first item wins. The thumbnail arrives pinned at
/// `zoom=1` (~128 px), so it is bumped to `zoom=2` before downloading, as the ISBN
/// path does.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on transport failure
/// or a body that is not JSON. A non-200 lookup, an empty result, or a hit without
/// imageLinks is a miss (`Ok(None)`), never a crash.
pub(crate) async fn grab_googlebooks_search(
    client: &reqwest::Client,
    base_url: &str,
    title: &str,
    author: Option<&str>,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let title = title.trim();
    if title.is_empty() {
        return Ok(None); // nothing to search with
    }
    // `intitle:`/`inauthor:` are the API's phrase operators; a bare `q` would OR the
    // words across titles, which matches much too loosely for a cover.
    let mut query = format!("intitle:{}", quote_term(title));
    if let Some(author) = author.filter(|name| !name.trim().is_empty()) {
        query.push_str(&format!(" inauthor:{}", quote_term(author.trim())));
    }
    let response = match client
        .get(format!("{base_url}/books/v1/volumes"))
        .query(&[("q", query)])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Err(cover_network(format!("Google Books search failed: {e}"))),
    };
    if response.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let payload: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return Err(cover_network(format!(
                "Google Books body was not JSON: {e}"
            )))
        }
    };
    let Some(thumbnail) = payload
        .pointer("/items/0/volumeInfo/imageLinks/thumbnail")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
    else {
        return Ok(None);
    };
    let image_url = thumbnail.replace("zoom=1", "zoom=2");
    let image = match client.get(&image_url).send().await {
        Ok(r) => r,
        Err(e) => return Err(cover_network(format!("cover image request failed: {e}"))),
    };
    if image.status() != reqwest::StatusCode::OK {
        return Ok(None);
    }
    let bytes = match image.bytes().await {
        Ok(b) => b,
        Err(e) => return Err(cover_network(format!("cover image body failed: {e}"))),
    };
    Ok(image_ext_from(&bytes).map(|ext| (bytes.to_vec(), ext)))
}

/// Wrap a search term in ASCII double quotes; embedded quotes are impossible in real
/// titles hitting this path, and the operator syntax wants the phrase quoted whole.
fn quote_term(term: &str) -> String {
    format!("\"{}\"", term.replace('"', ""))
}

/// The title-search chain for books no ISBN answers for: Open Library's search index
/// (canonical `-L` covers), then Google Books' volumes search — the usual answer for
/// small-run Arabic editions — and finally the Internet Archive's scanned shelf.
///
/// Author hint management: each provider is tried once **with** the author hint and,
/// on a miss, once without it — Arabic author names arrive transliterated any number of
/// ways, and a wrong-narrowing miss must not hide a right-title hit. A provider
/// **error** stops the chain, as everywhere else.
pub(crate) async fn grab_search_cover(
    client: &reqwest::Client,
    ol_api_base: &str,
    ol_covers_base: &str,
    googlebooks_base: &str,
    archive_base: &str,
    title: &str,
    author: Option<&str>,
) -> Result<Option<(Vec<u8>, &'static str, maktabati_domain::CoverSource)>, AppErrorDto> {
    // With the author first (precision), the title-only retry last (recall). No author
    // at all means the two passes would be identical, so it runs once.
    let hints: Vec<Option<&str>> = match author {
        Some(name) if !name.trim().is_empty() => vec![Some(name), None],
        other => vec![other],
    };
    for with_author in hints {
        if let Some((bytes, ext)) =
            grab_openlibrary_search(client, ol_api_base, ol_covers_base, title, with_author).await?
        {
            return Ok(Some((
                bytes,
                ext,
                maktabati_domain::CoverSource::OpenLibrary,
            )));
        }
        if let Some((bytes, ext)) =
            grab_googlebooks_search(client, googlebooks_base, title, with_author).await?
        {
            return Ok(Some((
                bytes,
                ext,
                maktabati_domain::CoverSource::GoogleBooks,
            )));
        }
        if let Some((bytes, ext)) =
            grab_internetarchive_search(client, archive_base, title, with_author).await?
        {
            return Ok(Some((
                bytes,
                ext,
                maktabati_domain::CoverSource::InternetArchive,
            )));
        }
    }
    Ok(None)
}

/// The lookup chain a book's cover goes down: an ISBN when the row carries one,
/// otherwise a title (+ first credited author) search.
enum CoverLookup {
    /// Every ISBN form the book carries, deduped and in preference order (13-digit
    /// first, the derived or stored 10-digit second) — providers are asked form by form.
    Isbn(Vec<String>),
    /// Open Library search index, narrowed by the optional author name.
    Search(Option<String>),
}

/// Every ISBN form a book's two raw fields yield: a 13-digit form of the 978 family
/// converts to its 10-digit twin, and a stored 10-digit form joins the candidates in its
/// own right.
///
/// Deduped, because the chain asks each provider for every candidate.
pub(crate) fn isbn_candidates(isbn13_raw: Option<&str>, isbn10_raw: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |digits: Option<String>| {
        if let Some(d) = digits {
            if !out.contains(&d) {
                out.push(d);
            }
        }
    };
    let thirteen = isbn13_raw
        .filter(|raw| !raw.trim().is_empty())
        .and_then(crate::commands::enrich::clean_isbn);
    let ten = isbn10_raw
        .filter(|raw| !raw.trim().is_empty())
        .and_then(crate::commands::enrich::clean_isbn);
    push(thirteen.clone());
    push(ten);
    if let Some(thirteen) = thirteen {
        push(isbn13_to_isbn10(&thirteen));
    }
    out
}

/// Decide a book's cover lookup chain and run it to a hit.
///
/// An ISBN — even junk-jacketed — is trusted when it cleans to 10/13 digits; anything
/// else falls back to the title search with the first credited author as the hint, so
/// an ISBN-less book is *askable* instead of permanently coverless. A provider miss on
/// an ISBN book does **not** chain into the title search: the ISBN answer is the honest
/// "this edition has no cover", and a title hit might be a different edition.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) `cover.network` (or `enrich.network` on
/// the search path) at the transport level; misses are `Ok(None)`.
async fn grab_book_cover(
    client: &reqwest::Client,
    lookup: CoverLookup,
    title: &str,
) -> Result<Option<(Vec<u8>, &'static str, maktabati_domain::CoverSource)>, AppErrorDto> {
    match lookup {
        CoverLookup::Isbn(candidates) => {
            grab_cover(
                client,
                OPENLIBRARY_BASE,
                GOOGLEBOOKS_BASE,
                AMAZON_BASE,
                ARCHIVE_BASE,
                &candidates,
            )
            .await
        }
        CoverLookup::Search(author) => {
            grab_search_cover(
                client,
                OPENLIBRARY_API_BASE,
                OPENLIBRARY_BASE,
                GOOGLEBOOKS_BASE,
                ARCHIVE_BASE,
                title,
                author.as_deref(),
            )
            .await
        }
    }
}

/// Fetch a cover for a book from the internet and install it as the book's cover.
///
/// Chains: an ISBN-bearing book goes Open Library covers → Google Books volumes →
/// Amazon's product images → the Internet Archive's scans, each provider asked for every
/// ISBN form the row carries; a book without a usable ISBN first goes through Open
/// Library's title search, then Google Books' title search, then the Internet Archive's —
/// the usual answers for small-run Arabic editions the canonical indexes never heard of.
///
/// The image is validated by magic numbers, saved as `covers/<book_id>.<ext>` exactly
/// like a picked cover (old bytes replaced, library-owned copy), and its provenance is
/// recorded ([`CoverSource::OpenLibrary`], [`CoverSource::GoogleBooks`],
/// [`CoverSource::Amazon`] or [`CoverSource::InternetArchive`]) so the settings disk
/// report stays honest about where each cover came from.
///
/// `Ok(None)` = every provider on the chain answered but none had a cover — a miss,
/// which the UI reports as "no cover found online", distinct from a network error.
///
/// # Errors
///
/// * `cover.network` / `enrich.network` when the lookup chain failed at the
///   transport level.
/// * Storage errors from [`repo::book::set_cover_bytes`].
#[specta::specta]
#[tauri::command]
pub async fn cover_fetch(state: State<'_, AppState>, book_id: String) -> CmdResult<Option<Book>> {
    let db = Arc::clone(&state.db);
    let got_id = book_id.clone();
    let book = blocking(Arc::clone(&db), move |db| repo::book::get(db, &got_id)).await?;
    let candidates = isbn_candidates(book.isbn13.as_deref(), book.isbn10.as_deref());
    let lookup = if candidates.is_empty() {
        let id = book_id.clone();
        let author = blocking(Arc::clone(&db), move |db| {
            repo::book::first_author_name(db, &id)
        })
        .await?;
        CoverLookup::Search(author)
    } else {
        CoverLookup::Isbn(candidates)
    };

    let client =
        client().ok_or_else(|| cover_network("could not build the HTTP client".to_owned()))?;
    let Some((bytes, ext, source)) = grab_book_cover(&client, lookup, &book.title).await? else {
        return Ok(None);
    };

    let data_dir = state.data_dir.clone();
    let id = book_id.clone();
    blocking(db, move |db| {
        repo::book::set_cover_bytes(db, &data_dir, &id, &bytes, ext, source)
    })
    .await
    .map(Some)
}

/// Pause between two books in the bulk pass — providers are free, not ours to hammer.
const BULK_PAUSE: Duration = Duration::from_millis(250);

/// Live progress of the bulk pass, pushed to the webview as it moves.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct CoverBulkProgress {
    /// Books processed so far (installed *and* misses count: both were "handled").
    pub done: u32,
    /// Candidates at the start of the pass.
    pub total: u32,
    /// Covers installed so far.
    pub fetched: u32,
    /// Provider "no cover for this ISBN" so far — a miss, not a failure.
    pub missed: u32,
}

/// The end-of-pass answer: counters over the *whole pass*, not one book.
#[derive(Debug, serde::Serialize, specta::Type)]
pub struct CoverBulkReport {
    /// Covers downloaded and installed this pass.
    pub fetched: u32,
    /// Books whose chain answered outright "no cover" — they stay coverless, honestly.
    pub missed: u32,
    /// `Some(detail)` when the pass stopped early — a transport failure or a storage
    /// error. Counts above it cover the books processed *before* the stop; a re-pass
    /// simply continues from the head of the queue.
    pub stopped: Option<String>,
}

/// Fetch covers for **every** book that is missing one.
///
/// One politeness-paced pass over the whole queue (oldest book first, stable order):
/// ISBN-bearing books go Open Library covers -> Google Books volumes -> Amazon product
/// images -> Internet Archive scans, each provider asked for every ISBN form the row
/// carries; the rest — no ISBN, or only junk in the ISBN field — go through the title
/// searches (Open Library, then Google Books, then the Internet Archive), the first
/// credited author narrowing them. This is exactly per-book `cover_fetch`'s
/// chain, reused so the bulk path cannot drift from the single-book path its tests pin.
/// A book that already has a cover (a manual pick, a previous pass) is never touched,
/// and no provider is ever consulted about it again.
///
/// Progress runs over the `on_progress` channel (`done/total/fetched/missed`) so the
/// webview shows a live counter instead of a frozen button; the final
/// [`CoverBulkReport`] carries the whole-pass totals.
///
/// Per-book policy matches `cover_fetch`: a provider **miss** is a handled book (the
/// pass continues), a transport or storage failure stops the pass early and surfaces in
/// `stopped` — silently continuing after an outage would mark every later book "no
/// cover found" when the truth is "we could not ask".
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when listing the queue fails; per-book
/// failures are reported through `stopped`, not this result.
#[specta::specta]
#[tauri::command]
pub async fn cover_fetch_missing(
    state: State<'_, AppState>,
    on_progress: tauri::ipc::Channel<CoverBulkProgress>,
) -> CmdResult<CoverBulkReport> {
    let db = Arc::clone(&state.db);
    let queued = blocking(db, repo::book::list_missing_covers).await?;
    let mut report = CoverBulkProgress {
        done: 0,
        total: queued.len() as u32,
        fetched: 0,
        missed: 0,
    };

    let stopped = if queued.is_empty() {
        None
    } else {
        let client = match client() {
            Some(c) => c,
            None => return Err(cover_network("could not build the HTTP client".to_owned())),
        };
        tracing::info!(total = report.total, "bulk cover fetch starting");
        let mut stopped: Option<String> = None;
        let count = queued.len();
        for (i, book) in queued.into_iter().enumerate() {
            // The chain is per book: an ISBN lookup (every form the row carries), or a
            // title search with the first credited author as the narrowing hint. That
            // hint is a tiny read; the pass is paced at 250 ms per book, so a failed
            // hint read (local SQLite, absurdly unlikely) costs only the hint, not the
            // pass.
            let lookup = {
                let candidates = isbn_candidates(book.isbn13.as_deref(), book.isbn10.as_deref());
                if candidates.is_empty() {
                    let id = book.id.clone();
                    let author = blocking(Arc::clone(&state.db), move |db| {
                        repo::book::first_author_name(db, &id)
                    })
                    .await
                    .unwrap_or(None);
                    CoverLookup::Search(author)
                } else {
                    CoverLookup::Isbn(candidates)
                }
            };
            match grab_book_cover(&client, lookup, &book.title).await {
                Ok(Some((bytes, ext, source))) => {
                    let db = Arc::clone(&state.db);
                    let data_dir = state.data_dir.clone();
                    let id = book.id.clone();
                    match blocking(db, move |db| {
                        repo::book::set_cover_bytes(db, &data_dir, &id, &bytes, ext, source)
                    })
                    .await
                    {
                        Ok(_) => report.fetched += 1,
                        Err(e) => {
                            tracing::warn!(error = %e.message, "bulk cover fetch stopped (storage)");
                            stopped = Some(e.message);
                            break;
                        }
                    }
                }
                Ok(None) => report.missed += 1,
                Err(e) => {
                    tracing::warn!(error = %e.message, "bulk cover fetch stopped (network)");
                    stopped = Some(e.message);
                    break;
                }
            }
            report.done += 1;
            let _ = on_progress.send(report.clone());
            if i + 1 < count {
                tokio::time::sleep(BULK_PAUSE).await;
            }
        }
        stopped
    };

    tracing::info!(
        fetched = report.fetched,
        missed = report.missed,
        stopped = stopped.is_some(),
        "bulk cover fetch finished"
    );
    Ok(CoverBulkReport {
        fetched: report.fetched,
        missed: report.missed,
        stopped,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        grab_amazon_isbn, grab_cover, grab_googlebooks_isbn, grab_googlebooks_search,
        grab_internetarchive_isbn, grab_internetarchive_search, grab_openlibrary_isbn,
        grab_openlibrary_search, grab_search_cover, image_dims, image_ext_from, isbn13_to_isbn10,
        isbn_candidates, quote_term, AMAZON_BASE, ARCHIVE_BASE, GOOGLEBOOKS_BASE,
        OPENLIBRARY_API_BASE, OPENLIBRARY_BASE,
    };
    use crate::error::AppErrorDto;
    use maktabati_domain::CoverSource;
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
    static PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
    static GIF_PLACEHOLDER: &[u8] = &[b'G', b'I', b'F', b'8', b'7', b'a', 0x01, 0x01];

    /// A minimal but measurably-sized JPEG: SOI, an APP0/JFIF stub, then an SOF0 frame
    /// header carrying `width`×`height` — exactly what `image_dims` reads its sizes from.
    fn jpeg_with_dims(width: u16, height: u16) -> Vec<u8> {
        let mut v =
            b"\xFF\xD8\xFF\xE0\x00\x10\x4A\x46\x49\x46\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00"
                .to_vec();
        v.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        v.extend_from_slice(&height.to_be_bytes());
        v.extend_from_slice(&width.to_be_bytes());
        v.extend_from_slice(&[0x01, 0x01, 0x11, 0x00]);
        v
    }

    /// A minimal PNG carrying an IHDR with `width`×`height` (chunk layout: 8 signature
    /// bytes, length + `IHDR`, then the two big-endian dimensions).
    fn png_with_dims(width: u32, height: u32) -> Vec<u8> {
        let mut v = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A].to_vec();
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]);
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&width.to_be_bytes());
        v.extend_from_slice(&height.to_be_bytes());
        v.extend_from_slice(&[0x08, 0x06, 0x00, 0x00, 0x00]);
        v
    }

    /// `grab_cover` with the wiremock bases injected — the command itself pins the real
    /// constants; only the tests inject, so the chain logic exists exactly once.
    /// `candidates` is the book's ISBN forms, already deduped by `isbn_candidates`.
    async fn grab_cover_at(
        client: &reqwest::Client,
        ol: &str,
        gg: &str,
        amazon: &str,
        archive: &str,
        candidates: &[String],
    ) -> Result<Option<(Vec<u8>, &'static str, CoverSource)>, AppErrorDto> {
        grab_cover(client, ol, gg, amazon, archive, candidates).await
    }

    #[test]
    fn magic_bytes_decide_the_extension_not_the_response_headers() {
        assert_eq!(image_ext_from(JPEG), Some("jpg"));
        assert_eq!(image_ext_from(PNG), Some("png"));
        // A RIFF container with the WAVE form is not a WebP even though it starts RIFF.
        let wav = [
            b'R', b'I', b'F', b'F', 0, 0, 0, 0, b'W', b'A', b'V', b'E', 0,
        ];
        assert_eq!(image_ext_from(&wav), None);
        assert_eq!(image_ext_from(GIF_PLACEHOLDER), None); // the blank OL placeholder
        assert_eq!(image_ext_from(b"<html>not an image</html>"), None);
        // Truncated WebP header (RIFF without 12 bytes) is not confidently an image.
        assert_eq!(image_ext_from(b"RIFF"), None);
    }

    #[test]
    fn dims_come_from_the_format_headers() {
        assert_eq!(image_dims(&jpeg_with_dims(307, 475)), Some((307, 475)));
        assert_eq!(
            image_dims(&jpeg_with_dims(u16::MAX, 392)),
            Some((u32::from(u16::MAX), 392))
        );
        assert_eq!(image_dims(&png_with_dims(500, 392)), Some((500, 392)));
        // The tiny static fixtures carry no measurable frame header: not measurable.
        assert_eq!(image_dims(JPEG), None);
        assert_eq!(image_dims(PNG), None);
        assert_eq!(image_dims(GIF_PLACEHOLDER), None);
        assert_eq!(image_dims(b"<html>not an image</html>"), None);
        // A WAVE-in-RIFF body is not an image, so no dims either.
        let wav = [
            b'R', b'I', b'F', b'F', 0, 0, 0, 0, b'W', b'A', b'V', b'E', 0,
        ];
        assert_eq!(image_dims(&wav), None);
    }

    #[test]
    fn isbn13_to_isbn10_recomputes_the_check_digit() {
        assert_eq!(
            isbn13_to_isbn10("9780140328721").as_deref(),
            Some("0140328726")
        );
        // Wrong check digit 10 renders as 'X' (the 978-13 twin of 123456789X).
        assert_eq!(
            isbn13_to_isbn10("9781234567893").as_deref(),
            Some("123456789X")
        );
        // The 979 family has no ISBN-10 twin, and junk does not convert.
        assert_eq!(isbn13_to_isbn10("9791234567890"), None);
        assert_eq!(isbn13_to_isbn10("0140328726"), None);
        assert_eq!(isbn13_to_isbn10("9780140328"), None);
        assert_eq!(isbn13_to_isbn10("9780X40328721"), None);
    }

    #[test]
    fn isbn_candidates_dedupe_and_extend() {
        // Only 13: the derived 10 joins.
        assert_eq!(
            isbn_candidates(Some("9780140328721"), None),
            vec!["9780140328721", "0140328726"]
        );
        // Both forms present: nothing derived, nothing duplicated.
        assert_eq!(
            isbn_candidates(Some("9780140328721"), Some("0140328726")),
            vec!["9780140328721", "0140328726"]
        );
        // Only 10: no invented 13.
        assert_eq!(
            isbn_candidates(None, Some("0140328726")),
            vec!["0140328726"]
        );
        // Junk in both fields: no candidates at all (the title search takes over).
        assert!(isbn_candidates(Some("978-0-14-"), Some("  ")).is_empty());
        assert!(isbn_candidates(None, None).is_empty());
    }

    #[tokio::test]
    async fn openlibrary_misses_are_none_not_errors() {
        let server = MockServer::start().await;

        // JPEG found (the happy path).
        let jpg_space = Mock::given(method("GET"))
            .and(path("/b/isbn/9780140328721-L.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG));
        server.register(jpg_space).await;
        let client = reqwest::Client::new();
        let hit = grab_openlibrary_isbn(&client, &server.uri(), "9780140328721")
            .await
            .unwrap()
            .expect("cover hit");
        assert_eq!(hit.1, "jpg");
        assert_eq!(hit.0, JPEG);

        // 404 = provider miss.
        server
            .register(
                Mock::given(method("GET"))
                    .and(path("/b/isbn/9999999999999-L.jpg"))
                    .respond_with(ResponseTemplate::new(404)),
            )
            .await;
        assert!(
            grab_openlibrary_isbn(&client, &server.uri(), "9999999999999")
                .await
                .unwrap()
                .is_none()
        );

        // The blank 1×1 GIF placeholder is a miss too, not a cover.
        server
            .register(
                Mock::given(method("GET"))
                    .and(path("/b/isbn/8888888888888-L.jpg"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(GIF_PLACEHOLDER)),
            )
            .await;
        assert!(
            grab_openlibrary_isbn(&client, &server.uri(), "8888888888888")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn googlebooks_lookup_follows_the_thumbnail_url() {
        let api = MockServer::start().await;
        let files = MockServer::start().await;
        let client = reqwest::Client::new();

        // The volumes API returns an absolute thumbnail URL on a *second* host; the
        // client must follow it there. It arrives pinned at `zoom=1`, so the command
        // should bump it to `zoom=2` before fetching.
        let thumbnail = format!("{}/books/content?id=x&img=1&zoom=1", files.uri());
        api.register(
            Mock::given(method("GET"))
                .and(query_param("q", "isbn:9780328721410"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [{
                        "volumeInfo": { "imageLinks": { "thumbnail": thumbnail } }
                    }]
                }))),
        )
        .await;
        files
            .register(
                Mock::given(method("GET"))
                    .and(path("/books/content"))
                    .and(query_param("zoom", "2"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG)),
            )
            .await;
        let hit = grab_googlebooks_isbn(&client, &api.uri(), "9780328721410")
            .await
            .unwrap()
            .expect("cover hit");
        assert_eq!(hit.1, "png");
        assert_eq!(hit.0, PNG);
    }

    #[tokio::test]
    async fn googlebooks_tolerates_quirk() {
        let api = MockServer::start().await;
        let client = reqwest::Client::new();

        // No items at all (unknown ISBN): a miss.
        api.register(Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"items": []})),
        ))
        .await;
        assert!(grab_googlebooks_isbn(&client, &api.uri(), "9780000000002")
            .await
            .unwrap()
            .is_none());

        // An item with no imageLinks block (Google is happy to omit it): a miss.
        api.register(
            Mock::given(method("GET")).respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"items": [{"volumeInfo": {"title": "x"}}]})),
            ),
        )
        .await;
        assert!(grab_googlebooks_isbn(&client, &api.uri(), "9780000000003")
            .await
            .unwrap()
            .is_none());

        // A 400 (the volumes API errors verbosely but is still JSON): a miss, not a
        // crash — the command treats "no cover" as the answer.
        api.register(
            Mock::given(method("GET")).respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"error": {"message": "bad"}})),
            ),
        )
        .await;
        assert!(grab_googlebooks_isbn(&client, &api.uri(), "9780000000004")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn accepts_a_portrait_image_and_rejects_the_amazon_placeholder() {
        let amazon = MockServer::start().await;
        let client = reqwest::Client::new();

        // A portrait jacket: accepted.
        amazon
            .register(
                Mock::given(method("GET"))
                    .and(path("/images/P/0140328726.01.LZZZZZZZ.jpg"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_bytes(jpeg_with_dims(307, 475)),
                    ),
            )
            .await;
        let hit = grab_amazon_isbn(&client, &amazon.uri(), "0140328726")
            .await
            .unwrap()
            .expect("portrait cover hit");
        assert_eq!(hit.1, "jpg");

        // Landscape body: that is a placeholder shape, and a 500×392 miss image answers
        // 200 with valid JPEG magic — no HTTP signal. Only the portrait rule sees it.
        amazon
            .register(
                Mock::given(method("GET"))
                    .and(path("/images/P/9999999999.01.LZZZZZZZ.jpg"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_bytes(jpeg_with_dims(500, 392)),
                    ),
            )
            .await;
        assert!(
            grab_amazon_isbn(&client, &amazon.uri(), "9999999999")
                .await
                .unwrap()
                .is_none(),
            "the placeholder must not be stored as a cover"
        );

        // A square body (another placeholder shape, 300×300) is not portrait either.
        amazon
            .register(
                Mock::given(method("GET"))
                    .and(path("/images/P/8888888888.01.LZZZZZZZ.jpg"))
                    .respond_with(
                        ResponseTemplate::new(200).set_body_bytes(jpeg_with_dims(300, 300)),
                    ),
            )
            .await;
        assert!(grab_amazon_isbn(&client, &amazon.uri(), "8888888888")
            .await
            .unwrap()
            .is_none());

        // A misterious 404 (also a miss).
        amazon
            .register(
                Mock::given(method("GET"))
                    .and(path("/images/P/7777777777.01.LZZZZZZZ.jpg"))
                    .respond_with(ResponseTemplate::new(404)),
            )
            .await;
        assert!(grab_amazon_isbn(&client, &amazon.uri(), "7777777777")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn internetarchive_serves_the_matched_item_image() {
        let archive = MockServer::start().await;
        let client = reqwest::Client::new();

        // The search answers the item identifier; the item image follows.
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/advancedsearch.php"))
                    .and(query_param(
                        "q",
                        "(isbn:9780140328721) AND mediatype:(texts)",
                    ))
                    .and(query_param("sort[]", "downloads desc"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "response": {
                            "numFound": 1,
                            "docs": [{ "identifier": "fantasticmrfox" }]
                        }
                    }))),
            )
            .await;
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/services/img/fantasticmrfox"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG)),
            )
            .await;
        let hit = grab_internetarchive_isbn(&client, &archive.uri(), "9780140328721")
            .await
            .unwrap()
            .expect("cover hit");
        assert_eq!(hit.1, "jpg");
        assert_eq!(hit.0, JPEG);
    }

    #[tokio::test]
    async fn internetarchive_misses_are_none_not_errors() {
        let archive = MockServer::start().await;
        let client = reqwest::Client::new();

        // No items: a miss without an image request.
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/advancedsearch.php"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "response": { "numFound": 0, "docs": [] }
                    }))),
            )
            .await;
        assert!(
            grab_internetarchive_isbn(&client, &archive.uri(), "9780000000005")
                .await
                .unwrap()
                .is_none()
        );

        // The not-found item image: archive.org redirects to its grey "not found"
        // picture with a 200, so the *final URL* is the miss signal.
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/advancedsearch.php"))
                    .and(query_param(
                        "q",
                        "(isbn:9780000000006) AND mediatype:(texts)",
                    ))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "response": {
                            "numFound": 1,
                            "docs": [{ "identifier": "ghost-item" }]
                        }
                    }))),
            )
            .await;
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/services/img/ghost-item"))
                    .respond_with(
                        ResponseTemplate::new(302)
                            .insert_header("Location", "/images/notfound.png"),
                    ),
            )
            .await;
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/images/notfound.png"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG)),
            )
            .await;
        assert!(
            grab_internetarchive_isbn(&client, &archive.uri(), "9780000000006")
                .await
                .unwrap()
                .is_none(),
            "the not-found picture must not be stored as a cover"
        );

        // A non-200 search index (rate limit) is a miss, not a crash.
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/advancedsearch.php"))
                    .and(query_param(
                        "q",
                        "(isbn:9780000000007) AND mediatype:(texts)",
                    ))
                    .respond_with(ResponseTemplate::new(503)),
            )
            .await;
        assert!(
            grab_internetarchive_isbn(&client, &archive.uri(), "9780000000007")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn internetarchive_search_finds_a_cover_by_title() {
        let archive = MockServer::start().await;
        let client = reqwest::Client::new();

        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/advancedsearch.php"))
                    .and(query_param(
                        "q",
                        "title:\"مقدمة في الحضارة\" AND mediatype:(texts) AND creator:\"مؤلف\"",
                    ))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "response": {
                            "numFound": 1,
                            "docs": [{ "identifier": "ancient-scan" }]
                        }
                    }))),
            )
            .await;
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/services/img/ancient-scan"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG)),
            )
            .await;
        let hit =
            grab_internetarchive_search(&client, &archive.uri(), "مقدمة في الحضارة", Some("مؤلف"))
                .await
                .unwrap()
                .expect("cover hit");
        assert_eq!(hit.1, "jpg");
        assert_eq!(hit.0, JPEG);
    }

    #[tokio::test]
    async fn chain_picks_openlibrary_first_then_google() {
        let ol = MockServer::start().await;
        let gg = MockServer::start().await;
        let amazon = MockServer::start().await;
        let archive = MockServer::start().await;
        let client = reqwest::Client::new();
        // Candidate ISBN forms, exactly as `isbn_candidates` produces them.
        let candidates = ["9781111000009".to_owned(), "1111000001".to_owned()];

        // Open Library has it: the other three providers are never consulted. The 10-digit
        // twin is served by the same covers host, so register both candidates' misses.
        ol.register(
            Mock::given(method("GET"))
                .and(path("/b/isbn/9781111000009-L.jpg"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG)),
        )
        .await;
        let (bytes, ext, source) = grab_cover_at(
            &client,
            &ol.uri(),
            &gg.uri(),
            &amazon.uri(),
            &archive.uri(),
            &candidates,
        )
        .await
        .unwrap()
        .expect("cover hit");
        assert_eq!(ext, "jpg");
        assert_eq!(source, CoverSource::OpenLibrary);
        assert_eq!(bytes, JPEG);

        // A different book, whose every ISBN form misses on all four providers: the
        // chain's honest answer is no cover — and no provider along the way may turn
        // that into an error. (Distinct ISBNs so these 404 mocks never overlap the
        // JPEG hit mock above.)
        let missing = ["9780222000110".to_owned(), "0222000111".to_owned()];
        for digits in &missing {
            ol.register(
                Mock::given(method("GET"))
                    .and(path(format!("/b/isbn/{digits}-L.jpg").as_str()))
                    .respond_with(ResponseTemplate::new(404)),
            )
            .await;
        }
        gg.register(Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"items": []})),
        ))
        .await;
        amazon
            .register(Mock::given(method("GET")).respond_with(ResponseTemplate::new(404)))
            .await;
        archive
            .register(Mock::given(method("GET")).respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "response": { "numFound": 0, "docs": [] }
                })),
            ))
            .await;
        assert!(grab_cover_at(
            &client,
            &ol.uri(),
            &gg.uri(),
            &amazon.uri(),
            &archive.uri(),
            &missing
        )
        .await
        .unwrap()
        .is_none());
    }

    #[tokio::test]
    async fn googlebooks_search_follows_the_thumbnail_and_bumps_zoom() {
        let api = MockServer::start().await;
        let files = MockServer::start().await;
        let client = reqwest::Client::new();

        let thumbnail = format!("{}/books/content?id=y&img=1&zoom=1", files.uri());
        api.register(
            Mock::given(method("GET"))
                .and(query_param("q", "intitle:\"سؤال\" inauthor:\"كاتب\""))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "items": [{
                        "volumeInfo": { "imageLinks": { "thumbnail": thumbnail } }
                    }]
                }))),
        )
        .await;
        files
            .register(
                Mock::given(method("GET"))
                    .and(path("/books/content"))
                    .and(query_param("zoom", "2"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG)),
            )
            .await;
        let hit = grab_googlebooks_search(&client, &api.uri(), "سؤال", Some("كاتب"))
            .await
            .unwrap()
            .expect("cover hit");
        assert_eq!(hit.1, "png");
        assert_eq!(hit.0, PNG);
    }

    #[tokio::test]
    async fn googlebooks_search_misses_are_none_not_errors() {
        let api = MockServer::start().await;
        let client = reqwest::Client::new();

        // No items at all: a miss.
        api.register(Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "items": [] })),
        ))
        .await;
        assert!(grab_googlebooks_search(&client, &api.uri(), "مفقود", None)
            .await
            .unwrap()
            .is_none());

        // A hit with no imageLinks block: a miss.
        api.register(
            Mock::given(method("GET")).respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"items": [{"volumeInfo": {"title": "x"}}]})),
            ),
        )
        .await;
        assert!(
            grab_googlebooks_search(&client, &api.uri(), "بلا صورة", None)
                .await
                .unwrap()
                .is_none()
        );

        // A non-200 lookup: a miss, not a crash — like the ISBN path.
        api.register(
            Mock::given(method("GET")).respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"error": {"message": "bad"}})),
            ),
        )
        .await;
        assert!(grab_googlebooks_search(&client, &api.uri(), "خاطئ", None)
            .await
            .unwrap()
            .is_none());

        // A blank title has nothing to search with: a miss without a request.
        assert!(grab_googlebooks_search(&client, &api.uri(), "   ", None)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn search_chain_picks_openlibrary_then_google_then_archive() {
        let ol_api = MockServer::start().await;
        let ol_covers = MockServer::start().await;
        let gg = MockServer::start().await;
        let archive = MockServer::start().await;
        let client = reqwest::Client::new();

        // Open Library answers: nobody further down the chain is consulted.
        ol_api
            .register(
                Mock::given(method("GET"))
                    .and(path("/search.json"))
                    .and(query_param("title", "شرق"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "docs": [{ "cover_i": 42 }]
                    }))),
            )
            .await;
        ol_covers
            .register(
                Mock::given(method("GET"))
                    .and(path("/b/id/42-L.jpg"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG)),
            )
            .await;
        let (bytes, ext, source) = grab_search_cover(
            &client,
            &ol_api.uri(),
            &ol_covers.uri(),
            &gg.uri(),
            &archive.uri(),
            "شرق",
            None,
        )
        .await
        .unwrap()
        .expect("cover hit");
        assert_eq!(ext, "jpg");
        assert_eq!(source, CoverSource::OpenLibrary);
        assert_eq!(bytes, JPEG);

        // The first two miss, the archive carries the cover: the search answers the
        // identifier and the item image downloads.
        ol_api
            .register(
                Mock::given(method("GET"))
                    .and(query_param("title", "غريب"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "docs": []
                    }))),
            )
            .await;
        gg.register(Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"items": []})),
        ))
        .await;
        archive
            .register(
                Mock::given(method("GET"))
                    .and(query_param("q", "title:\"غريب\" AND mediatype:(texts)"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "response": {
                            "numFound": 1,
                            "docs": [{ "identifier": "stranger-scan" }]
                        }
                    }))),
            )
            .await;
        archive
            .register(
                Mock::given(method("GET"))
                    .and(path("/services/img/stranger-scan"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(PNG)),
            )
            .await;
        let (bytes, ext, source) = grab_search_cover(
            &client,
            &ol_api.uri(),
            &ol_covers.uri(),
            &gg.uri(),
            &archive.uri(),
            "غريب",
            None,
        )
        .await
        .unwrap()
        .expect("archive hit");
        assert_eq!(ext, "png");
        assert_eq!(source, CoverSource::InternetArchive);
        assert_eq!(bytes, PNG);
    }

    #[tokio::test]
    async fn search_path_finds_a_cover_by_title_then_downloads_it() {
        let api = MockServer::start().await;
        let covers = MockServer::start().await;

        // A search hit with `cover_i` on the primary host; the image itself lives on the
        // covers host and is fetched through the same magic-byte download as the ISBN path.
        api.register(
            Mock::given(method("GET"))
                .and(path("/search.json"))
                .and(query_param("title", "مقدمة في الحضارة"))
                .and(query_param("author", "مؤلف"))
                .and(query_param("limit", "1"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "docs": [{ "cover_i": 207 }]
                }))),
        )
        .await;
        covers
            .register(
                Mock::given(method("GET"))
                    .and(path("/b/id/207-L.jpg"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG)),
            )
            .await;

        let client = reqwest::Client::new();
        let hit = grab_openlibrary_search(
            &client,
            &api.uri(),
            &covers.uri(),
            "مقدمة في الحضارة",
            Some("مؤلف"),
        )
        .await
        .unwrap()
        .expect("cover hit");
        assert_eq!(hit.1, "jpg");
        assert_eq!(hit.0, JPEG);
    }

    #[tokio::test]
    async fn search_chain_retries_title_only_after_an_author_narrowed_miss() {
        let ol_api = MockServer::start().await;
        let ol_covers = MockServer::start().await;
        let gg = MockServer::start().await;
        let archive = MockServer::start().await;
        let client = reqwest::Client::new();

        // The author-narrowed search misses (transliterated names do that), the
        // title-only retry finds it — the exact ladder Arabic editions live on.
        ol_api
            .register(
                Mock::given(method("GET"))
                    .and(query_param("title", "سؤال"))
                    .and(query_param("author", "كاتب"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "docs": []
                    }))),
            )
            .await;
        ol_api
            .register(
                Mock::given(method("GET"))
                    .and(path("/search.json"))
                    .and(query_param("title", "سؤال"))
                    .and(query_param_is_missing("author"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "docs": [{ "cover_i": 7 }]
                    }))),
            )
            .await;
        ol_covers
            .register(
                Mock::given(method("GET"))
                    .and(path("/b/id/7-L.jpg"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG)),
            )
            .await;
        let (bytes, ext, source) = grab_search_cover(
            &client,
            &ol_api.uri(),
            &ol_covers.uri(),
            &gg.uri(),
            &archive.uri(),
            "سؤال",
            Some("كاتب"),
        )
        .await
        .unwrap()
        .expect("title-only retry hit");
        assert_eq!(ext, "jpg");
        assert_eq!(source, CoverSource::OpenLibrary);
        assert_eq!(bytes, JPEG);
    }

    #[tokio::test]
    async fn search_path_misses_are_none_not_errors() {
        let api = MockServer::start().await;
        let covers = MockServer::start().await;
        let client = reqwest::Client::new();

        // Empty result set: a miss.
        api.register(Mock::given(method("GET")).respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "docs": [] })),
        ))
        .await;
        assert!(
            grab_openlibrary_search(&client, &api.uri(), &covers.uri(), "غامض", None)
                .await
                .unwrap()
                .is_none()
        );

        // A hit without `cover_i` (search may return blocks without it): a miss.
        api.register(
            Mock::given(method("GET"))
                .and(query_param("title", "بدون صورة"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "docs": [{ "key": "/works/OL1W" }]
                }))),
        )
        .await;
        assert!(
            grab_openlibrary_search(&client, &api.uri(), &covers.uri(), "بدون صورة", None)
                .await
                .unwrap()
                .is_none()
        );

        // A 500 from the search API: treated like Google Books — a miss, not a crash.
        api.register(
            Mock::given(method("GET"))
                .and(query_param("title", "انكسر الخادم"))
                .respond_with(ResponseTemplate::new(500)),
        )
        .await;
        assert!(
            grab_openlibrary_search(&client, &api.uri(), &covers.uri(), "انكسر الخادم", None)
                .await
                .unwrap()
                .is_none()
        );

        // An ISBN-less book whose cover_i points at the placeholder GIF: a miss — and
        // the covers host gets its `default=false` appended by the downloader.
        api.register(
            Mock::given(method("GET"))
                .and(query_param("title", "غلاف ساخر"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "docs": [{ "cover_i": 888 }]
                }))),
        )
        .await;
        covers
            .register(
                Mock::given(method("GET"))
                    .and(path("/b/id/888-L.jpg"))
                    .respond_with(ResponseTemplate::new(200).set_body_bytes(GIF_PLACEHOLDER)),
            )
            .await;
        assert!(
            grab_openlibrary_search(&client, &api.uri(), &covers.uri(), "غلاف ساخر", None)
                .await
                .unwrap()
                .is_none()
        );

        // A blank title has nothing to search with: a miss without a request.
        assert!(
            grab_openlibrary_search(&client, &api.uri(), &covers.uri(), "   ", None)
                .await
                .unwrap()
                .is_none()
        );
    }

    /// Every real constant must still exist as written — a rename-safety net.
    #[test]
    fn provider_bases_are_what_the_docs_claim() {
        assert_eq!(OPENLIBRARY_BASE, "https://covers.openlibrary.org");
        assert_eq!(OPENLIBRARY_API_BASE, "https://openlibrary.org");
        assert_eq!(GOOGLEBOOKS_BASE, "https://www.googleapis.com");
        assert_eq!(AMAZON_BASE, "https://images-na.ssl-images-amazon.com");
        assert_eq!(ARCHIVE_BASE, "https://archive.org");
    }

    /// An embedded `"` is dropped, not escaped: the operators demarcate one phrase and a
    /// stray quote would break the whole query string.
    #[test]
    fn quote_term_wraps_and_strips_quotes() {
        assert_eq!(quote_term("حديقة"), "\"حديقة\"");
        assert_eq!(quote_term("ab\"cd"), "\"abcd\"");
    }
}
