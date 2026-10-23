//! Binary entry point. All wiring lives in the `maktabati-app` library crate so it can be
//! exercised by integration tests.

// Windows: prefer performance-oriented GPU policies and avoid a console window on release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write;

/// Enrichment is the one headless mode: `maktabati enrich <isbn>` fetches an Open
/// Library suggestion and prints it, before any Tauri app comes up. Kept here (not in a
/// separate binary) so the bundle ships exactly one executable.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("enrich") {
        let isbn = args
            .get(1)
            .map(String::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if isbn.is_empty() {
            // Not a console window on Windows release, but on Linux/macOS this is the
            // user's terminal — plain stderr is fine.
            let mut err = std::io::stderr().lock();
            let _ = writeln!(err, "usage: maktabati enrich <isbn>");
            std::process::exit(2);
        }
        std::process::exit(maktabati_app_lib::commands::enrich::run_cli_isbn(isbn));
    }

    if let Err(err) = maktabati_app_lib::run() {
        // Deliberately plain: this runs before the tracing subscriber exists, and on Windows
        // there is no console to write to in a release build.
        eprintln!("Maktabati could not start: {err:#}");
        std::process::exit(1);
    }
}
