# Performance baseline

Measured before Task 6 on 2026-11-03. Repeat the same setup and measurement method after
the memory work to make the comparison meaningful.

## Results

| Metric | Baseline |
|---|---:|
| Idle app RSS (Dashboard) | 376,892 KiB (368.1 MiB) |
| Library RSS (164 books) | 526,380 KiB (514.0 MiB) |
| Cold start to visible app window | 0.572 s |
| Release executable size | 19,542,712 bytes (18.64 MiB) |

The Dashboard was visibly rendered in a capture taken 2 seconds after the window appeared
(within 2.572 seconds of launch). The cold-start figure is process launch to the main window
being mapped; OS file caches were not flushed.

## Method

- Built the production executable and embedded frontend with
  `cargo build -p maktabati-app --release --features tauri/custom-protocol`.
- Ran the release app on Linux / WebKitGTK under Niri, using XWayland
  (`GDK_BACKEND=x11`) to make window timing and interaction measurable.
- Used an isolated copy of the local library database and covers; the original app data was
  not modified. The copied database contained 164 books and passed SQLite `integrity_check`.
- Summed resident set size (RSS) for the Rust app and every descendant process via `/proc`.
  The measured process tree contained the app, `WebKitNetworkProcess`, and
  `WebKitWebProcess`. RSS is reported rather than PSS, so shared pages may be counted in
  more than one process.
- Sampled idle RSS on the rendered Dashboard after the app had settled, then opened Library
  and sampled again 4 seconds after navigation. The same 164-book data set was used for both.
- Executable size is the on-disk size of `target/release/maktabati-app`, not an installer or
  bundle size.

Host: Arch Linux, kernel 7.2.14, Intel Core i5-3570 (4 CPUs), 7.6 GiB RAM. Source was based
on `26820fc` with pre-existing local changes; those changes were not edited for this task.
