//! Binary entry point. All wiring lives in the `maktabati-app` library crate so it can be
//! exercised by integration tests.

// Windows: prefer performance-oriented GPU policies and avoid a console window on release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(err) = maktabati_app_lib::run() {
        // Deliberately plain: this runs before the tracing subscriber exists, and on Windows
        // there is no console to write to in a release build.
        eprintln!("Maktabati could not start: {err:#}");
        std::process::exit(1);
    }
}
