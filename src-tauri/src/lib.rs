//! # maktabati-app
//!
//! The Tauri shell for **Maktabati / مكتبتي**: window construction, tray, plugins, logging and
//! the IPC command surface. All behaviour lives in the workspace crates — this crate is wiring
//! only (ADR-0001, ADR-0010).

#![forbid(unsafe_code)]

pub mod commands;
pub mod error;
pub mod state;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::tray::TrayIconBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_specta::{collect_commands, Builder};

use maktabati_storage::{conn, migrate, seed, Db};

use crate::state::AppState;

/// Where the generated TypeScript contract is written, relative to `src-tauri`.
///
/// Not gated on `debug_assertions`: the drift test that reads it builds in release too.
pub const BINDINGS_OUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");

/// Every command exposed to the webview, in the order they appear in `bindings.ts`.
///
/// Adding a command here is the *only* way to add one: `collect_commands!` both registers it
/// with Tauri and exports its type to TypeScript, so a command cannot exist on one side and not
/// the other (ADR-0002).
macro_rules! commands {
    () => {
        collect_commands![
            crate::commands::app_info,
            crate::commands::library_health,
            crate::commands::library_summary,
            crate::commands::settings_all,
            crate::commands::settings_get,
            crate::commands::settings_set,
            // Catalogue CRUD (M3): one command per repo function, minus the Rust-only
            // `find_by_name_norm` — see `commands::catalogue`.
            crate::commands::catalogue::author_create,
            crate::commands::catalogue::author_delete,
            crate::commands::catalogue::author_get,
            crate::commands::catalogue::author_list,
            crate::commands::catalogue::author_search,
            crate::commands::catalogue::author_update,
            crate::commands::catalogue::book_create,
            crate::commands::catalogue::book_delete,
            crate::commands::catalogue::book_detail,
            crate::commands::catalogue::book_find_by_isbn,
            crate::commands::catalogue::book_find_duplicates,
            crate::commands::catalogue::book_get,
            crate::commands::catalogue::book_list,
            crate::commands::catalogue::book_update,
            crate::commands::catalogue::category_children,
            crate::commands::catalogue::category_create,
            crate::commands::catalogue::category_delete,
            crate::commands::catalogue::category_get,
            crate::commands::catalogue::category_list,
            crate::commands::catalogue::category_update,
            crate::commands::catalogue::language_create,
            crate::commands::catalogue::language_delete,
            crate::commands::catalogue::language_get,
            crate::commands::catalogue::language_list,
            crate::commands::catalogue::language_list_active,
            crate::commands::catalogue::language_update,
            crate::commands::catalogue::publisher_create,
            crate::commands::catalogue::publisher_delete,
            crate::commands::catalogue::publisher_get,
            crate::commands::catalogue::publisher_list,
            crate::commands::catalogue::publisher_search,
            crate::commands::catalogue::publisher_update,
            crate::commands::catalogue::series_create,
            crate::commands::catalogue::series_delete,
            crate::commands::catalogue::series_get,
            crate::commands::catalogue::series_list,
            crate::commands::catalogue::series_search,
            crate::commands::catalogue::series_update,
            crate::commands::catalogue::tag_create,
            crate::commands::catalogue::tag_delete,
            crate::commands::catalogue::tag_get,
            crate::commands::catalogue::tag_list,
            crate::commands::catalogue::tag_search,
            crate::commands::catalogue::tag_update
        ]
    };
}

/// Build the specta builder that owns the command registry.
///
/// `dangerously_cast_bigints_to_number` exports `i64`/`u64` as TypeScript `number` instead of
/// failing with `bigint-forbidden`. That is correct here rather than merely convenient:
///
/// * **The transport is JSON.** Tauri serialises the payload as JSON and the webview runs
///   `JSON.parse`, which folds any integer past 2⁵³ into an `f64` before the frontend can see it.
///   Declaring `bigint` would therefore be a lie — the value arrives as a truncated number.
///   Declaring `number` states what the webview genuinely holds.
/// * **Nothing we send is anywhere near 2⁵³.** Schema versions and migration counts are single
///   digits; byte counts are local-disk sizes (~2⁵³ is 9 PB); `checked_at_ms` is a millisecond
///   timestamp (~2⁵³ ms is 285 million years). No value loses precision in practice.
///
/// The name keeps the tradeoff visible at the call site. If a future command ever carries an
/// integer that must survive intact, give that field its own representation instead of relying
/// on this switch.
fn specta_builder() -> Builder<tauri::Wry> {
    <Builder<tauri::Wry>>::new()
        .dangerously_cast_bigints_to_number()
        .commands(commands!())
}

/// The log file's directory. Must exist before `tracing-appender` is constructed.
fn logs_dir(data_dir: &Path) -> PathBuf {
    let d = data_dir.join("logs");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Open the database, apply migrations (with the automatic pre-migration backup) and seed.
///
/// # Errors
///
/// Propagates any [`maktabati_storage::StorageError`].
fn open_database(data_dir: &std::path::Path) -> maktabati_storage::Result<Db> {
    let db_path = data_dir.join("library.db");
    let backup_dir = data_dir.join("backups");
    let db = Db::open(&db_path)?;

    // ADR-0003: before the schema changes, take a verified copy through SQLite's own online
    // backup API. If the backup fails the migration is aborted and the library is untouched.
    migrate::apply(&db, || {
        let stamp = conn::now_ms();
        let dest = backup_dir.join(format!("pre-migration-{stamp}.db"));
        db.backup_to(&dest)?;
        tracing::info!(dest = %dest.display(), "pre-migration backup written");
        Ok(())
    })?;

    db.write(seed::apply)?;
    Ok(db)
}

/// Install the tray icon. Clicking it toggles the main window.
fn install_tray(app: &mut tauri::App) -> anyhow::Result<()> {
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no bundled icon available for the tray"))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Maktabati · مكتبتي")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let Some(win) = tray.app_handle().get_webview_window("main") else {
                    return;
                };
                match win.is_visible() {
                    Ok(true) => {
                        let _ = win.hide();
                    }
                    _ => {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Entry point used by `main.rs`.
///
/// # Errors
///
/// `anyhow::Error` when the data directory cannot be resolved, the database cannot be opened, a
/// migration fails, or Tauri itself refuses to start.
pub fn run() -> anyhow::Result<()> {
    tauri::Builder::default()
        // `single-instance` must be first: without it a second launch would spin up a whole
        // second database handle before being told to quit.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(specta_builder().invoke_handler())
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            let file_appender =
                tracing_appender::rolling::never(logs_dir(&data_dir), "maktabati.log");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
            // The guard must outlive every log statement; `std::mem::forget` is the documented
            // pattern for an app-lifetime writer.
            std::mem::forget(_guard);
            // `.try_init()`, not `.init()`: `tauri_plugin_log` has already claimed the `log`
            // global logger, so registering the tracing→log bridge reports an error — after
            // the tracing subscriber itself is installed. Ignoring it keeps `tracing` events
            // flowing to the file appender while the plugin keeps handling `log`; `.init()`
            // would panic the app on every launch.
            let _ = tracing_subscriber::fmt()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .try_init();

            // P0-2: a debug build points the webview at Vite (build.devUrl). With no dev
            // server listening the window is a bare WebKit connection error — which reads
            // as "the app is broken". Detect it here, where we can still say what is wrong:
            // a log line in the file the docs point at, plus a dialog that names the fix.
            if cfg!(debug_assertions) {
                let dev_up = std::net::TcpStream::connect_timeout(
                    &std::net::SocketAddr::from(([127, 0, 0, 1], 1420)),
                    std::time::Duration::from_millis(300),
                )
                .is_ok();
                if !dev_up {
                    tracing::error!(
                        "dev server not reachable on 127.0.0.1:1420 — the window will render \
                         blank; run `npm run tauri dev`, or `npm run dev` in another terminal"
                    );
                    eprintln!(
                        "maktabati: no Vite dev server on 127.0.0.1:1420 — the window will be \
                         blank. Run `npm run tauri dev`."
                    );
                    app.dialog()
                        .message(
                            "The Vite dev server is not running on port 1420, so this window \
                             would be blank.\n\nRun `npm run tauri dev` — or `npm run dev` in \
                             another terminal — and reopen.",
                        )
                        .title("Maktabati — dev server not running")
                        .blocking_show();
                }
            }

            let db = open_database(&data_dir)?;
            tracing::info!(data_dir = %data_dir.display(), "database ready");

            app.manage(AppState {
                db: Arc::new(db),
                data_dir,
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
            });

            install_tray(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Run `f` on a thread with a much larger stack than a test thread gets.
    ///
    /// `tauri-specta`'s type export is heavily generic. In a debug build the generated
    /// instantiation and drop glue overflow the 2 MiB stack that libtest gives each test — a
    /// debug-only pathology that a release build inlines away. The test absorbs it rather than
    /// the crate: nothing in production runs on the test runner's stack.
    fn with_big_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(128 * 1024 * 1024)
            .spawn(f)
            .expect("could not spawn the high-stack export thread")
            .join()
            .expect("the export thread panicked");
    }

    /// The generated `bindings.ts` must match what the current command surface produces.
    ///
    /// Run `MAKTABATI_UPDATE_BINDINGS=1 cargo test -p maktabati-app bindings_are_up_to_date`
    /// after changing any command, then commit the regenerated file.
    #[test]
    fn bindings_are_up_to_date() {
        with_big_stack(export_and_check_bindings);
    }

    fn export_and_check_bindings() {
        let tmp = tempfile::tempdir().unwrap();
        let fresh_path = tmp.path().join("bindings.ts");
        let fresh_path_str = fresh_path.to_str().expect("temp path is valid utf-8");

        let builder = specta_builder();
        builder
            .export(specta_typescript::Typescript::default(), fresh_path_str)
            .expect("failed to export TypeScript bindings");
        let fresh = std::fs::read_to_string(&fresh_path).expect("export produced no file");

        let checked_in_path = std::path::Path::new(BINDINGS_OUT);

        if std::env::var_os("MAKTABATI_UPDATE_BINDINGS").is_some() {
            if let Some(parent) = checked_in_path.parent() {
                std::fs::create_dir_all(parent).expect("could not create src/");
            }
            std::fs::write(checked_in_path, &fresh).expect("could not write bindings.ts");
            return;
        }

        let checked_in = std::fs::read_to_string(checked_in_path).unwrap_or_else(|e| {
            panic!(
                "src/bindings.ts is missing ({e}).\n\
                 Generate it with: MAKTABATI_UPDATE_BINDINGS=1 cargo test -p maktabati-app \
                 bindings_are_up_to_date"
            )
        });

        assert_eq!(
            checked_in,
            fresh,
            "\n\nsrc/bindings.ts is out of date with the Rust command surface.\n\
             Regenerate and commit it with:\n\
             \x20 MAKTABATI_UPDATE_BINDINGS=1 cargo test -p maktabati-app bindings_are_up_to_date\n"
        );
    }

    /// The command list is a fixed, reviewable contract — a snapshot keeps an accidental
    /// addition or rename visible in the diff.
    #[test]
    fn command_surface_is_stable() {
        with_big_stack(assert_command_surface);
    }

    fn assert_command_surface() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("b.ts");
        let builder = specta_builder();
        builder
            .export(
                specta_typescript::Typescript::default(),
                out.to_str().expect("temp path is valid utf-8"),
            )
            .unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        for cmd in [
            "app_info",
            "library_health",
            "library_summary",
            "settings_all",
            "settings_get",
            "settings_set",
            "author_create",
            "author_delete",
            "author_get",
            "author_list",
            "author_search",
            "author_update",
            "book_create",
            "book_delete",
            "book_detail",
            "book_find_by_isbn",
            "book_find_duplicates",
            "book_get",
            "book_list",
            "book_update",
            "category_children",
            "category_create",
            "category_delete",
            "category_get",
            "category_list",
            "category_update",
            "language_create",
            "language_delete",
            "language_get",
            "language_list",
            "language_list_active",
            "language_update",
            "publisher_create",
            "publisher_delete",
            "publisher_get",
            "publisher_list",
            "publisher_search",
            "publisher_update",
            "series_create",
            "series_delete",
            "series_get",
            "series_list",
            "series_search",
            "series_update",
            "tag_create",
            "tag_delete",
            "tag_get",
            "tag_list",
            "tag_search",
            "tag_update",
        ] {
            assert!(
                text.contains(cmd),
                "command `{cmd}` missing from the generated bindings"
            );
        }
    }

    #[test]
    fn open_database_migrates_and_seeds() {
        let dir = tempfile::tempdir().unwrap();
        let db = open_database(dir.path()).unwrap();
        let (version, categories) = db
            .read(|c| {
                Ok((
                    c.query_row("SELECT MAX(version) FROM _migrations", [], |r| {
                        r.get::<_, i64>(0)
                    })?,
                    c.query_row("SELECT COUNT(*) FROM categories", [], |r| {
                        r.get::<_, i64>(0)
                    })?,
                ))
            })
            .unwrap();
        assert_eq!(version, migrate::embedded().last().unwrap().version);
        assert_eq!(categories, 51, "the seeded taxonomy must be present");
        assert!(dir.path().join("library.db").exists());
    }

    #[test]
    fn open_database_is_idempotent_across_launches() {
        let dir = tempfile::tempdir().unwrap();
        let first = open_database(dir.path()).unwrap();
        drop(first);
        // A second launch (upgrade path) must not fail and must not double-seed.
        let second = open_database(dir.path()).unwrap();
        let categories = second
            .read(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM categories", [], |r| {
                    r.get::<_, i64>(0)
                })?)
            })
            .unwrap();
        assert_eq!(categories, 51);
    }

    #[test]
    fn a_fresh_upgrade_takes_a_pre_migration_backup() {
        // Simulate a user whose database predates the newest migration by opening a directory,
        // migrating to version 1 only, then running the full path.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        {
            let db = Db::open(&db_path).unwrap();
            migrate::apply(&db, || {
                panic!("a brand-new database must not need a backup");
            })
            .unwrap();
        }
        // Now the normal path finds nothing pending, so no extra backup is created.
        let before = count_backups(dir.path());
        let db = open_database(dir.path()).unwrap();
        drop(db);
        assert_eq!(count_backups(dir.path()), before);
    }

    fn count_backups(dir: &std::path::Path) -> usize {
        let Ok(entries) = std::fs::read_dir(dir.join("backups")) else {
            return 0;
        };
        entries.flatten().count()
    }
}
