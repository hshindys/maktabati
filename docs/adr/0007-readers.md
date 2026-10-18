# ADR-0007 — Readers: pdf.js + foliate-js in the webview; Rust reads files, never renders them

**Status:** Accepted · **Date:** 2026-10-18

## Context

Required: in-app EPUB and PDF reading with themes, font controls, RTL support, bookmarks,
highlights that create Quotes, and last-position memory synced to `current_page`/progress.

Two places to render: native Rust (e.g. `pdfium-render`, `lopdf` + a custom renderer) or the
webview (pdf.js / epub.js / foliate-js).

## Decision

Render in the **webview**:

| Format | Library | Rationale |
|---|---|---|
| **PDF** | **pdf.js** (Mozilla, vendored) | The de-facto PDF renderer in browsers; handles text extraction (needed for highlights), selection, and RTL/Arabic text order better than anything we'd build. |
| **EPUB** | **foliate-js** (primary), `epub.js` (fallback) | `foliate-js` is small, modular, dependency-free, operates directly on `Range` objects with **bisecting for the visible range** (more accurate than epub.js fractions), anchors survive resize, supports >2 columns and **switching scrolled↔paginated without a reload**. Its author's README also candidly documents the CSS-multi-column limitations we inherit — a known-risk position beats an unknown one. |

**Rust's role** (`crates/files`): open the file safely (path validation, size/type/magic checks),
compute SHA-256, read **embedded metadata** (EPUB OPF, PDF XMP/info dict), extract/generate
covers, and serve **bytes to the webview** through a scoped, id-addressed command
(`files::read_chunk(book_file_id, offset, len)` or a `convertFileSrc`-style asset scope) — the
webview never passes arbitrary filesystem paths.

### Position & highlight contract

- **Canonical position** is stored in Rust/SQLite: `current_page`, `progress_pct`,
  `position_ref` (format-specific: PDF page + viewport offset; EPUB CFI-ish `Range` anchor +
  chapter href), `updated_at`.
- The reader sends `reading::save_position { book_id, position_ref, page, progress }` on a
  debounce and on close; it is what drives the progress bar and the reading-log chart.
- **Highlights** are created in the reader (`selection → colour → optional comment`) and sent as
  `quotes::create_from_highlight { book_id, file_id, position_ref, text, page, chapter, colour, comment }`.
  The stored highlight anchor lets `quotes::jump_to_position` re-open the reader at that spot.

## Consequences

- ✅ Arabic shaping, bidi, selection and complex pagination come for free — the same reason
  Readest/Foliate/Koodo all render in a webview (`00-research.md` §1.2).
- ✅ Themes (light/dark/sepia), font family/size, margins and RTL are plain CSS, matching the
  app's design tokens exactly instead of a second styling universe.
- ✅ No heavyweight native PDF engine in the binary (`pdfium-render` would ship a C++ library and
  a licence/bundling story for no benefit, since we don't need to *render to pixels* in Rust).
- ⚠️ Large PDFs can be memory-heavy in the webview. Mitigation: **lazy page rendering** (render
  viewport ± N pages), which pdf.js supports natively, and chunked byte delivery.
- ⚠️ pdf.js/foliate-js are vendored into `src/` (not CDN) to honour the offline-first and CSP
  rules (`script-src 'self'`).

## Alternatives considered

| Option | Why rejected |
|---|---|
| **`pdfium-render` (0.8.27 verified)** | Renders to pixels in Rust — we'd have to build selection, text extraction, search, RTL, zoom, and a viewport ourselves; adds a C++ dependency. Correct choice for *rendering in Rust*, which we don't do. |
| **`lopdf` (0.45.0)** | Structure/metadata only, no rendering. **Kept** — used by `crates/files` to read PDF metadata. |
| **`epub` crate (2.1.5)** | Parses EPUB for metadata/TOC/cover. **Kept** for `crates/files`; not used for rendering. |
| **Native GTK/Qt widget** | Two rendering stacks, two theme systems, poor RTL story compared to CSS logical properties. |
