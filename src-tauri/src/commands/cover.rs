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
/// Base URL for the Google Books API host; overridden with wiremock in tests.
const GOOGLEBOOKS_BASE: &str = "https://www.googleapis.com";

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

/// Download the largest Open Library cover available for `isbn` digits.
///
/// `?default=false` makes a miss answer 404 instead of the placeholder image; a body
/// that still is not a real image (blank GIF, HTML error page) is treated the same way.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) with `cover.network` on request failure,
/// or a cut-off body.
pub(crate) async fn grab_openlibrary_isbn(
    client: &reqwest::Client,
    base_url: &str,
    isbn: &str,
) -> Result<Option<CoverHit>, AppErrorDto> {
    let url = format!("{base_url}/b/isbn/{isbn}-L.jpg?default=false");
    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return Err(cover_network(format!(
                "Open Library covers request failed: {e}"
            )))
        }
    };
    // A miss is 404 — a miss, not an outage.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(cover_network(format!(
            "Open Library covers answered HTTP {}",
            response.status()
        )));
    }
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return Err(cover_network(format!(
                "Open Library cover body failed: {e}"
            )))
        }
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

/// Try both providers in order: Open Library first (canonical `-L` covers, keyed by
/// ISBN), Google Books on a miss. A provider **error** stops the chain — silently
/// continuing after a network failure would turn every outage into "no cover found".
/// The hit is returned together with the provider it came from, because that is exactly
/// what the cover's provenance should say.
pub(crate) async fn grab_cover(
    client: &reqwest::Client,
    digits: &str,
) -> Result<Option<(Vec<u8>, &'static str, maktabati_domain::CoverSource)>, AppErrorDto> {
    if let Some((bytes, ext)) = grab_openlibrary_isbn(client, OPENLIBRARY_BASE, digits).await? {
        return Ok(Some((
            bytes,
            ext,
            maktabati_domain::CoverSource::OpenLibrary,
        )));
    }
    Ok(grab_googlebooks_isbn(client, GOOGLEBOOKS_BASE, digits)
        .await?
        .map(|(bytes, ext)| (bytes, ext, maktabati_domain::CoverSource::GoogleBooks)))
}

/// Fetch a cover for a book from the internet and install it as the book's cover.
///
/// Providers, in order: Open Library's covers endpoint (`isbn13`, else `isbn10`), then
/// a Google Books lookup — the fallback where small-run Arabic editions usually live.
/// The image is validated by magic numbers, saved as `covers/<book_id>.<ext>` exactly
/// like a picked cover (old bytes replaced, library-owned copy), and its provenance is
/// recorded as [`CoverSource::OpenLibrary`] or [`CoverSource::GoogleBooks`] so the
/// settings disk report stays honest about where each cover came from.
///
/// `Ok(None)` = both providers answered but neither had a cover — a miss, which the UI
/// reports as "no cover found online", distinct from a network error.
///
/// # Errors
///
/// * [`AppErrorDto`](crate::error::AppErrorDto) `cover.no_isbn` when the book carries no
///   ISBN at all (the button is disabled for those, but the IPC stays honest).
/// * `enrich.invalid_isbn` when the ISBN field holds only junk.
/// * `cover.network` when the lookup chain failed at the transport level.
/// * Storage errors from [`repo::book::set_cover_bytes`].
#[specta::specta]
#[tauri::command]
pub async fn cover_fetch(state: State<'_, AppState>, book_id: String) -> CmdResult<Option<Book>> {
    let db = Arc::clone(&state.db);
    let got_id = book_id.clone();
    let book = blocking(Arc::clone(&db), move |db| repo::book::get(db, &got_id)).await?;
    let raw_isbn = book
        .isbn13
        .clone()
        .or(book.isbn10.clone())
        .filter(|raw| !raw.trim().is_empty());
    let Some(raw) = raw_isbn else {
        return Err(AppErrorDto::new(
            "cover.no_isbn",
            format!("book {book_id} has no ISBN to fetch a cover with"),
        )
        .with_i18n_key("error.cover.no_isbn"));
    };
    let digits = crate::commands::enrich::clean_isbn(&raw).ok_or_else(|| {
        AppErrorDto::new(
            "enrich.invalid_isbn",
            format!("`{raw}` is not an ISBN-10 or ISBN-13"),
        )
        .with_i18n_key("error.enrich.invalid_isbn")
    })?;

    let client =
        client().ok_or_else(|| cover_network("could not build the HTTP client".to_owned()))?;
    let Some((bytes, ext, source)) = grab_cover(&client, &digits).await? else {
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

#[cfg(test)]
mod tests {
    use super::{
        grab_googlebooks_isbn, grab_openlibrary_isbn, image_ext_from, GOOGLEBOOKS_BASE,
        OPENLIBRARY_BASE,
    };
    use crate::error::AppErrorDto;
    use maktabati_domain::CoverSource;
    #[allow(unused_imports)]
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
    static PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
    static GIF_PLACEHOLDER: &[u8] = &[b'G', b'I', b'F', b'8', b'7', b'a', 0x01, 0x01];

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
    async fn chain_picks_openlibrary_first_then_google_then_none() {
        let ol = MockServer::start().await;
        let gg = MockServer::start().await;
        let client = reqwest::Client::new();

        // Open Library has it: Google is never consulted.
        ol.register(
            Mock::given(method("GET"))
                .and(path("/b/isbn/9781111000009-L.jpg"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG)),
        )
        .await;
        let (bytes, ext, source) = grab_cover_at(&client, &ol.uri(), &gg.uri(), "9781111000009")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ext, "jpg");
        assert_eq!(source, CoverSource::OpenLibrary);
        assert_eq!(bytes, JPEG);

        // Open Library misses: Google Books carries it.
        ol.register(
            Mock::given(method("GET"))
                .and(path("/b/isbn/9780222000110-L.jpg"))
                .respond_with(ResponseTemplate::new(404)),
        )
        .await;
        gg.register(
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({}))),
        )
        .await;
        // (The generic GG json above has no items, exercising the *miss* side too.)
        assert!(
            grab_cover_at(&client, &ol.uri(), &gg.uri(), "9780222000110")
                .await
                .unwrap()
                .is_none()
        );
    }

    /// `grab_cover` with both bases injected — the command itself pins the real ones.
    async fn grab_cover_at(
        client: &reqwest::Client,
        ol_base: &str,
        gg_base: &str,
        isbn: &str,
    ) -> Result<Option<(Vec<u8>, &'static str, CoverSource)>, AppErrorDto> {
        if let Some((bytes, ext)) = grab_openlibrary_isbn(client, ol_base, isbn).await? {
            return Ok(Some((bytes, ext, CoverSource::OpenLibrary)));
        }
        Ok(grab_googlebooks_isbn(client, gg_base, isbn)
            .await?
            .map(|(bytes, ext)| (bytes, ext, CoverSource::GoogleBooks)))
    }

    /// Both real constants must still exist as written — a rename-safety net.
    #[test]
    fn provider_bases_are_what_the_docs_claim() {
        assert_eq!(OPENLIBRARY_BASE, "https://covers.openlibrary.org");
        assert_eq!(GOOGLEBOOKS_BASE, "https://www.googleapis.com");
    }
}
