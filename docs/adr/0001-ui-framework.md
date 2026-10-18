# ADR-0001 — UI framework: Tauri 2 + Svelte 5 + Tailwind + Bits UI (webview) over Leptos

**Status:** Accepted · **Date:** 2026-10-18 · **Deciders:** principal architect + product designer

## Context

The brief asks for a polished desktop app with:

- full **Arabic + English** UI with live switching and whole-layout RTL/LTR mirroring,
- per-field automatic text direction,
- a **Notion-style block editor** with slash menu, tables, callouts,
- **in-app EPUB and PDF readers** with themes, font controls and highlights,
- a beautiful, modern, animated design system.

Two viable routes exist:

**A. Webview (Tauri 2 + a web framework)** — the UI is HTML/CSS/JS rendered by the OS webview
(WebView2 on Windows, WebKitGTK on Linux, WKWebView on macOS).

**B. Native Rust UI (Leptos/Slint/egui/Iced)** — the UI is drawn by Rust.

## Evidence from research (`00-research.md` §1.2, §4.8)

- **Readest**, one of the strongest modern open-source readers, is built on **Tauri v2 with a
  webview frontend** and ships highlights, notes, a full writing workspace and pdf/epub reading.
- **Foliate** renders e-book content in a **WebKitGTK WebView**; **Koodo Reader** is Electron.
  Essentially every serious e-reader renders book content in a webview — because that is where
  `epub.js`/`foliate-js`/`pdf.js` run.
- Arabic shaping is a solved, shipping problem in OS webviews (HarfBuzz + full UBA/bidi). A
  pure-Rust UI would require us to assemble shaping, bidi, fallback fonts and complex-script
  line breaking ourselves — a large, high-risk project whose only benefit is avoiding a webview.
- `rustybuzz`/`harfrust` exist, but using them correctly still leaves bidi, line breaking, font
  fallback and the editor/readers to build.

## Decision

Use **Tauri 2** as the shell and **Svelte 5 + TypeScript + Tailwind CSS + Bits UI** rendered in
the webview as a thin presentation layer. All business logic stays in Rust (see `02-architecture.md`
§1).

Svelte 5 was chosen over React/Vue because:

- its **runes** model gives fine-grained reactivity without virtual-DOM work — important for a
  50k-row virtualized grid targeting 60 fps;
- `bits-ui` is a **headless** component library built specifically for Svelte 5 with
  accessibility "baked in" (the brief requires ARIA/keyboard/AA contrast), while leaving full
  control over styling — which we need for a bespoke "bookish" design system;
- small bundle → fast cold start (< 2 s target).

## Consequences

- ✅ Best-in-class Arabic shaping, RTL, bidi, rich-text editing, pdf.js/epub.js readers.
- ✅ Design-system freedom (CSS logical properties mirror layout for free).
- ⚠️ We must police the boundary: no business logic in the webview; the webview gets **generated**
  typed bindings only (ADR-0002).
- ⚠️ Webview versions differ per OS → a documented browser-support matrix and CI builds on
  Windows + Linux.
- ⚠️ Slight memory overhead vs native (accepted; Kavita-class numbers are still reachable).

## Alternatives considered

| Option | Why rejected |
|---|---|
| **Leptos** | Real option for RTL and simple screens, but the block editor and two readers would have to be rebuilt from scratch in Rust; no equivalent of pdf.js/foliate-js. Would also require a custom shaping/bidi stack. Quality parity could not be *proven* — the brief requires proof. |
| **Slint / egui / Iced** | Excellent for tools, but weak for rich text, complex RTL, and web-standard readers; design-system polish would cost more. |
| **Electron** | 10× larger bundle and RAM; the brief specifies Tauri. |
| **Tauri 3 (alpha)** | `3.0.0-alpha.5` exists but is alpha; `2` is the stable line (verified 2026-10-18). Revisit at GA. |

## Compliance

- Frontend is a **thin presentation layer**: it may hold view state, but never business rules,
  validation logic (beyond instant feedback), persistence, or HTTP.
- The brief's "Leptos is acceptable if you can prove equal quality" clause is why this ADR
  records the proof-by-precedent argument explicitly.
