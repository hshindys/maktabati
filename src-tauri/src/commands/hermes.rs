//! Hermes — the optional loopback HTTP API (Phase "Hermes").
//!
//! `127.0.0.1:17321`, opt-in via the Settings toggle (`hermes.server.enabled`), read-only
//! over the catalogue **except** `POST`/`PATCH /books`, which mirror the IPC `book_create`
//! / `book_update` semantics exactly (full [`BookInput`] replace). Enrichment
//! (`POST /enrich`) is a pure suggestion — never a write.
//!
//! Endpoints (per the desktop-integration brief):
//! | `GET /health`        | liveness + version                                  |
//! | `GET /books?query=`  | search the catalogue (empty query = recent books)   |
//! | `POST /books`        | create from a `BookInput` (201)                     |
//! | `GET /books/:id`     | one book (404 when unknown)                         |
//! | `PATCH /books/:id`   | **replace the input in full**, like `book_update`   |
//! | `POST /enrich`       | Open Library suggestion, nothing written            |
//!
//! The webview toggles it through [`hermes_start`]/[`hermes_stop`]; the server lives on
//! the same tokio runtime `tauri::async_runtime` runs commands on, with a graceful
//! shutdown driven by a `watch` channel owned by [`HermesState`].

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State as AxumState};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Json;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use maktabati_domain::BookInput;
use maktabati_storage::StorageError;

use super::enrich::{self, EnrichRequest, EnrichSuggestion};
use crate::error::{AppErrorDto, CmdResult};
use crate::state::AppState;

/// Where the server listens. Loopback **only** — this is a local admin API; binding to a
/// public interface would hand the catalogue to the LAN. Compile-check the value with the
/// parse at [`server_addr`].
const LOOPBACK_PORT: u16 = 17321;

/// The address the server binds, for the status DTO and tooltips.
#[must_use]
pub fn server_addr() -> SocketAddr {
    // Loopback literal + fixed port; the parse is const-safe but written as a function so
    // a future "random port" preference has one place to change.
    SocketAddr::from(([127, 0, 0, 1], LOOPBACK_PORT))
}

/// Server-side handle for the graceful shutdown. The sender is owned by the state;
/// dropping/sending closes the server on the next accept loop tick.
#[derive(Debug, Default)]
pub struct HermesState {
    /// `Some` while the server owns a bound listener. `Arc` so the auto-start spawn in
    /// `lib.rs` no longer has to borrow from the managed state.
    pub inner: Arc<Mutex<Inner>>,
}

/// The mutable half of [`HermesState`].
#[derive(Debug, Default)]
pub struct Inner {
    shutdown: Option<tokio::sync::watch::Sender<bool>>,
}

impl HermesState {
    /// Is a server up right now?
    #[must_use]
    pub fn running(&self) -> bool {
        self.inner.lock().is_ok_and(|g| g.shutdown.is_some())
    }
}

/// What the IPC status/toggle endpoints answer with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct HermesStatus {
    /// Server currently accepting connections.
    pub running: bool,
    /// The loopback address, as text, e.g. `127.0.0.1:17321`.
    pub addr: String,
}

fn status_of(state: &HermesState) -> HermesStatus {
    HermesStatus {
        running: state.running(),
        addr: server_addr().to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HTTP surface
// ─────────────────────────────────────────────────────────────────────────────

type DbState = AxumState<Arc<maktabati_storage::Db>>;

/// Status-code map for [`StorageError`] at the HTTP boundary. Mirrors `error.rs`'s
/// stable codes — `message` is developer English, the caller never parses it.
fn http_error(err: StorageError) -> (StatusCode, Json<AppErrorDto>) {
    let dto = AppErrorDto {
        code: err.code().to_owned(),
        message: err.to_string(),
        i18n_key: Some(format!("error.{}", err.code())),
        details: None,
    };
    let status = match err {
        StorageError::NotFound { .. } => StatusCode::NOT_FOUND,
        StorageError::Duplicate { .. } => StatusCode::CONFLICT,
        StorageError::Domain(_) => StatusCode::UNPROCESSABLE_ENTITY,
        StorageError::Busy => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(dto))
}

/// Run synchronous work on the blocking pool, like the IPC commands do.
async fn db_blocking<T, F>(
    db: Arc<maktabati_storage::Db>,
    f: F,
) -> Result<T, (StatusCode, Json<AppErrorDto>)>
where
    T: Send + 'static,
    F: FnOnce(&maktabati_storage::Db) -> maktabati_storage::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || f(&db))
        .await
        .map_err(|join_err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AppErrorDto::new(
                    "internal",
                    format!("background task failed: {join_err}"),
                )),
            )
        })?
        .map_err(http_error)
}

/// Liveness record — also where clients read the version against which the API shape
/// is stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// App identifier.
    pub app: String,
    /// Bundle version.
    pub version: String,
    /// Always `ok` — the server answers, that is the whole check.
    pub status: String,
}

#[allow(clippy::unused_async)] // handler signature uniformity with axum
async fn health(AxumState(db): DbState) -> Json<Health> {
    // The db handle is not used today; versioning is compile-time. Kept in the signature
    // so the state wiring stays exercised.
    let _ = db;
    Json(Health {
        app: "maktabati".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        status: "ok".to_owned(),
    })
}

/// Search result listing (flattened through `Book`, the same shape IPC returns).
async fn books_list(
    AxumState(db): DbState,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<maktabati_domain::Book>>, (StatusCode, Json<AppErrorDto>)> {
    let query = params.get("query").cloned().unwrap_or_default();
    let books = db_blocking(db, move |db| {
        maktabati_storage::repo::book::search(db, &query, 50)
    })
    .await?;
    Ok(Json(books))
}

async fn book_get(
    AxumState(db): DbState,
    Path(id): Path<String>,
) -> Result<Json<maktabati_domain::Book>, (StatusCode, Json<AppErrorDto>)> {
    let book = db_blocking(db, move |db| maktabati_storage::repo::book::get(db, &id)).await?;
    Ok(Json(book))
}

async fn book_create(
    AxumState(db): DbState,
    Json(input): Json<BookInput>,
) -> Result<(StatusCode, Json<maktabati_domain::Book>), (StatusCode, Json<AppErrorDto>)> {
    let book = db_blocking(db, move |db| {
        maktabati_storage::repo::book::create(db, input)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(book)))
}

/// `PATCH` replaces the whole `BookInput` — the same semantics as IPC `book_update`
/// (relation lists must be sent complete). Deliberately not partial-merge: two divergent
/// notions of "which fields were set" is how update bugs happen.
async fn book_patch(
    AxumState(db): DbState,
    Path(id): Path<String>,
    Json(input): Json<BookInput>,
) -> Result<Json<maktabati_domain::Book>, (StatusCode, Json<AppErrorDto>)> {
    let book = db_blocking(db, move |db| {
        maktabati_storage::repo::book::update(db, &id, input)
    })
    .await?;
    Ok(Json(book))
}

#[allow(clippy::unused_async)] // uniform with the other handlers
async fn enrich_endpoint(
    Json(request): Json<EnrichRequest>,
) -> Result<Json<EnrichSuggestion>, (StatusCode, Json<AppErrorDto>)> {
    let suggestion = enrich::suggest(&request)
        .await
        .map_err(|dto| match dto.code.as_str() {
            "enrich.invalid_isbn" | "enrich.empty_request" => {
                (StatusCode::UNPROCESSABLE_ENTITY, Json(dto))
            }
            _ => (StatusCode::BAD_GATEWAY, Json(dto)),
        })?;
    // The "nothing found" answer is a 200 with `{"suggestion": null}` — not an error.
    Ok(Json(suggestion))
}

/// Assemble the router. Test-runnable without any listener.
pub fn build_router(db: Arc<maktabati_storage::Db>) -> axum::Router {
    axum::Router::new()
        .route("/health", get(health))
        .route("/books", get(books_list).post(book_create))
        .route("/books/{id}", get(book_get).patch(book_patch))
        .route("/enrich", post(enrich_endpoint))
        .with_state(db)
}

/// Bind and serve. The bind happens *before* the state is marked running, so "running"
/// means "accepting". Any failure (port taken, permissions) is returned as an error
/// string — the toggle UI then shows what went wrong via an i18n-keyed dto.
///
/// Lock discipline: the `std` mutex is never held across an await. Order is
/// bind → lock → hand the listener to the serve task → tag running; the check-and-set
/// window between bind and lock is closed by re-checking `Inner` (the only way another
/// launch could have got there is an earlier `launch` that failed its own bind anyway).
async fn launch(db: Arc<maktabati_storage::Db>, hermes: &Arc<Mutex<Inner>>) -> Result<(), String> {
    if hermes.lock().is_ok_and(|inner| inner.shutdown.is_some()) {
        return Err("already running".to_owned());
    }

    let addr = server_addr();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("could not bind {addr}: {e}"))?;

    let (tx, mut rx) = tokio::sync::watch::channel(false);
    let router = build_router(db);

    let mut inner = hermes.lock().map_err(|e| format!("poisoned state: {e}"))?;
    if inner.shutdown.is_some() {
        // Lost a race to a concurrent launch; leave *its* server alone.
        drop(listener);
        return Err("already running".to_owned());
    }
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                // Any change (the `send(true)` in `stop`) closes the server; a dropped
                // sender (state torn down) closes it too — `changed()` errors then.
                let _ = rx.changed().await;
            })
            .await;
    });
    inner.shutdown = Some(tx);
    tracing::info!(addr = %addr, "hermes server starting");
    Ok(())
}

/// Stop the server if it is up. Returns whether a server was actually running.
fn stop(hermes: &Arc<Mutex<Inner>>) -> Result<bool, String> {
    let mut inner = hermes.lock().map_err(|e| format!("poisoned state: {e}"))?;
    let was_running = inner.shutdown.is_some();
    if let Some(tx) = inner.shutdown.take() {
        let _ = tx.send(true);
        tracing::info!("hermes server stopping");
    }
    Ok(was_running)
}

// ─────────────────────────────────────────────────────────────────────────────
// IPC commands
// ─────────────────────────────────────────────────────────────────────────────

/// IPC surface for the Settings → Hermes card.
///
/// Starts the loopback server. Fails with `hermes.bind` when the port is taken or the
/// server already runs.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) on bind/already-running failures.
#[specta::specta]
#[tauri::command]
pub async fn hermes_start(state: State<'_, AppState>) -> CmdResult<HermesStatus> {
    let result = launch(Arc::clone(&state.db), &state.hermes.inner).await;
    result.map_err(|message| {
        let code = if message == "already running" {
            "hermes.running"
        } else {
            "hermes.bind"
        };
        AppErrorDto::new(code, format!("hermes start failed: {message}"))
            .with_i18n_key(format!("error.{code}"))
    })?;
    Ok(status_of(&state.hermes))
}

/// Stops the server if it is up. Idempotent.
///
/// # Errors
///
/// [`AppErrorDto`](crate::error::AppErrorDto) when the internal state is poisoned.
#[specta::specta]
#[tauri::command]
pub async fn hermes_stop(state: State<'_, AppState>) -> CmdResult<HermesStatus> {
    let _ = stop(&state.hermes.inner).map_err(|message| AppErrorDto::new("internal", message))?;
    Ok(status_of(&state.hermes))
}

/// Report whether the server is running, for the Settings card.
///
/// # Errors
///
/// Never in practice; the signature matches its siblings.
#[specta::specta]
#[tauri::command]
pub async fn hermes_status(state: State<'_, AppState>) -> CmdResult<HermesStatus> {
    Ok(status_of(&state.hermes))
}

/// Read the Settings→Hermes auto-start preference and spawn a server start if it is on.
///
/// Called once from the setup hook with the already-opened database; `tauri::async_runtime`
/// is already up by then, so the spawn lands on the same runtime commands use. Failures
/// (port taken, mostly) are logged — a failed auto-start must not take the app down, the
/// toggle remains available in Settings.
pub fn autostart_if_enabled(db: Arc<maktabati_storage::Db>, hermes: Arc<Mutex<Inner>>) {
    let enabled: Option<bool> =
        maktabati_storage::repo::settings::get(&db, "hermes.server.enabled").unwrap_or(None);
    if enabled != Some(true) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Err(e) = launch(db, &hermes).await {
            tracing::error!("hermes auto-start failed: {e}");
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — an ephemeral-port server against a fresh migrated database
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use maktabati_storage::Db;
    use maktabati_storage::{migrate, seed};
    use pretty_assertions::assert_eq;

    async fn serve_test_router() -> (tempfile::TempDir, String, tokio::task::JoinHandle<()>) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("t.db")).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.write(seed::apply).unwrap();
        let db = Arc::new(db);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let router = build_router(db);
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        (dir, format!("http://{addr}"), server)
    }

    #[tokio::test]
    async fn health_reports_app_and_version() {
        let (dir, base, server) = serve_test_router().await;
        let health: serde_json::Value = reqwest::get(format!("{base}/health"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(health["app"], "maktabati");
        assert_eq!(health["status"], "ok");
        assert_eq!(health["version"], env!("CARGO_PKG_VERSION"));
        drop(dir);
        server.abort();
    }

    #[tokio::test]
    async fn books_lists_searches_and_404s() {
        let (dir, base, server) = serve_test_router().await;
        let client = reqwest::Client::new();

        // Empty query = recent list; the fresh library has none.
        let response = client
            .get(format!("{base}/books?query="))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        let books: serde_json::Value = response.json().await.unwrap();
        assert_eq!(books.as_array().unwrap().len(), 0);

        // Unknown id → 404 with the stable error code.
        let missing = client
            .get(format!("{base}/books/nope-id"))
            .send()
            .await
            .unwrap();
        assert_eq!(missing.status().as_u16(), 404);
        let body: serde_json::Value = missing.json().await.unwrap();
        assert_eq!(body["code"], "storage.not_found");
        drop(dir);
        server.abort();
    }

    #[tokio::test]
    async fn create_validates_like_the_ipc_surface() {
        let (dir, base, server) = serve_test_router().await;
        let client = reqwest::Client::new();

        // Blank title → the same validation error the wizard gets.
        let response = client
            .post(format!("{base}/books"))
            .json(&BookInput::default())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 422);
        let body: serde_json::Value = response.json().await.unwrap();
        assert!(
            body["code"].as_str().unwrap().contains("validation"),
            "got {body}"
        );
        drop(dir);
        server.abort();
    }

    #[tokio::test]
    async fn enrich_suggests_without_writing() {
        let (dir, base, server) = serve_test_router().await;
        let client = reqwest::Client::new();

        // A malformed ISBN is rejected *locally* (no network, no feasibility).
        let response = client
            .post(format!("{base}/enrich"))
            .json(&serde_json::json!({ "isbn": "not-an-isbn" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 422);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["code"], "enrich.invalid_isbn");

        // An empty request (no isbn, no title) also fails locally.
        let response = client
            .post(format!("{base}/enrich"))
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 422);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["code"], "enrich.empty_request");
        drop(dir);
        server.abort();
    }

    #[tokio::test]
    async fn status_marks_shutdown_and_relies_on_watch() {
        // Lifecycle without a network: start on a state, stop, start again.
        let hermes = HermesState::default();
        let inner = hermes.inner.clone();
        let (tx, mut rx) = tokio::sync::watch::channel(false);
        inner.lock().unwrap().shutdown = Some(tx);
        assert!(hermes.running());
        assert!(!*rx.borrow());

        let _ = stop(&hermes.inner).unwrap();
        assert!(!hermes.running());
        // The graceful-shutdown signal fired.
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), true);
        // Double stop is a no-op.
        assert!(!stop(&hermes.inner).unwrap());
    }

    #[test]
    fn loopback_address_is_fixed() {
        assert_eq!(server_addr().to_string(), "127.0.0.1:17321");
        assert!(server_addr().ip().is_loopback());
    }

    #[tokio::test]
    async fn lifecycle_marks_running_only_between_launch_and_stop() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("t.db")).unwrap();
        migrate::apply(&db, || Ok(())).unwrap();
        db.write(seed::apply).unwrap();
        let hermes = HermesState::default();

        assert!(!hermes.running());
        launch(Arc::new(db), &hermes.inner).await.unwrap();
        assert!(hermes.running());
        // A second launch is refused.
        assert_eq!(
            launch(
                Arc::new(Db::open(&dir.path().join("other.db")).unwrap()),
                &hermes.inner
            )
            .await
            .unwrap_err(),
            "already running"
        );
        assert!(stop(&hermes.inner).unwrap());
        assert!(!hermes.running());
    }
}
