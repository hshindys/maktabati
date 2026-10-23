//! Bundles the Tauri assets (icons, `tauri.conf.json`, capabilities) and validates the config.

fn main() {
    // Re-run when the configuration or a capability changes, so a `cargo build` in CI cannot
    // silently ship a stale allowlist.
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=capabilities");
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();

    // Windows (MSVC): test binaries get no application manifest, so their loader hands them
    // comctl32 v5 — which lacks `TaskDialogIndirect`, a symbol `tauri-plugin-dialog` pulls
    // in. The test process dies at load with 0xc0000139 (STATUS_ENTRYPOINT_NOT_FOUND) before
    // a single test runs. Embed a Common-Controls v6 manifest into *test* binaries only:
    // the main binary already carries the same dependency via tauri_build, and a second
    // manifest there makes MSVC's linker fail with CVT1100.
    //
    // https://github.com/tauri-apps/tauri/issues/13419
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        embed_manifest_for_tests();
    }
}

/// Link-argument instructions that put `windows-test-manifest.xml` inside every test
/// binary. Relative to the package root, because that is the working directory a build
/// script runs in.
fn embed_manifest_for_tests() {
    // A failing `current_dir` is not survivable here and there is no error channel to the
    // compiler from a build script — skip, and the missing manifest surfaces as the same
    // entrypoint failure the workaround exists to fix.
    let Ok(package_dir) = std::env::current_dir() else {
        return;
    };
    let manifest = package_dir.join("windows-test-manifest.xml");

    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    // `Display` of a `Path` is lossy but valid for the linker, so no Unicode case is special.
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
