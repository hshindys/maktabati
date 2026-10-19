//! Bundles the Tauri assets (icons, `tauri.conf.json`, capabilities) and validates the config.

fn main() {
    // Re-run when the configuration or a capability changes, so a `cargo build` in CI cannot
    // silently ship a stale allowlist.
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=capabilities");
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
