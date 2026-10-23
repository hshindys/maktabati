//! Bundles the Tauri assets (icons, `tauri.conf.json`, capabilities) and validates the config.

use tauri_build::{Attributes, WindowsAttributes};

fn main() {
    // Re-run when the configuration or a capability changes, so a `cargo build` in CI cannot
    // silently ship a stale allowlist.
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=capabilities");
    println!("cargo:rerun-if-changed=icons");

    // The app manifest is supplied below, for *every* binary this crate links (see the
    // comment there), so tauri_build's default manifest must be suppressed — two manifests
    // in one binary make MSVC's linker fail with CVT1100.
    if let Err(e) = tauri_build::try_build(
        Attributes::new().windows_attributes(WindowsAttributes::new_without_app_manifest()),
    ) {
        panic!("tauri-build failed: {e}");
    }

    // Windows (MSVC): every cargo test binary this crate builds (the lib unit tests most
    // visibly) would otherwise get no application manifest, and its loader then hands it
    // comctl32 v5 — which lacks `TaskDialogIndirect`, a symbol `tauri-plugin-dialog` pulls
    // in. The process dies at load with 0xc0000139 (STATUS_ENTRYPOINT_NOT_FOUND) before
    // a single test runs. The manifest below adds the Common-Controls v6 dependency — the
    // same one tauri_build normally gives the main binary, which is why the released app
    // does not die this way.
    //
    // Sent through bare `rustc-link-arg` rather than `rustc-link-arg-tests`: the `-tests`
    // form demands that the package declare an explicit test target, and this crate keeps
    // its tests inside the lib.
    //
    // https://github.com/tauri-apps/tauri/issues/13419
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        embed_manifest();
    }
}

/// Link-argument instructions that put `windows-manifest.xml` inside every binary this
/// crate links. The file lives next to this script because that is the working directory
/// a build script runs in.
fn embed_manifest() {
    // A failing `current_dir` cannot be reported from a build script beyond the hard
    // failure this would cause, so skip — the missing manifest surfaces as the same
    // entrypoint failure this workaround exists to fix.
    let Ok(package_dir) = std::env::current_dir() else {
        return;
    };
    let manifest = package_dir.join("windows-manifest.xml");

    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    // `Display` of a `Path` is lossy but valid for the linker, so no Unicode case is special.
    let arg = format!("/MANIFESTINPUT:{}", manifest.display());
    println!("cargo:rustc-link-arg={arg}");
}
