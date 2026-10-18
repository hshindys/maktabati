# ADR-0009 — Frontend stack: Svelte 5 + TypeScript + Tailwind CSS + Vite + Bits UI, fonts bundled

**Status:** Accepted · **Date:** 2026-10-18 · Complements ADR-0001.

## Decision

| Layer | Choice |
|---|---|
| Framework | **Svelte 5** (runes) + **TypeScript** (strict) |
| Styling | **Tailwind CSS v4** (CSS-first config) with design tokens expressed as CSS custom properties |
| Headless components | **Bits UI** (Svelte 5, accessibility-first, unstyled) |
| Build | **Vite** (Tauri's default dev server) |
| Icons | **Lucide** (tree-shaken, 1.5px stroke, consistent) |
| Charts | **Layer Chart** or a tiny hand-rolled SVG chart set — *decided at Milestone 10;* requirement is SVG + themable via design tokens |
| Routing | Tiny in-app router (hash/history) — the app is a shell, not a website |
| State | Svelte runes (`$state`, `$derived`, `$effect`) + a small query cache keyed by command name |
| i18n | Small in-house runtime over `ar.json` / `en.json` (see below) |
| Fonts | **Bundled locally** (no network): Cairo, Tajawal, IBM Plex Sans Arabic (UI), **Amiri** (reading), **Inter** (Latin pairing) — OFL licensed |

### Why an in-house i18n runtime

Requirements are: two locales, live switching, ICU-lite interpolation, plural rules for `ar`
(6 plural forms), digit-style and Hijri/Gregorian date formatting, and **CI key-parity checking**.
A ~200-line runtime satisfies this with no dependency whose upgrade could break RTL. If the app
grows to >3 locales, revisit (options: `typesafe-i18n`, `paraglide-js`).

### Digit & date policy

- **Digits:** a setting (`western` | `arabic_indic`) applied at *format* time via a formatter
  helper — stored values always stay Western/ASCII in SQLite so sorting and search are stable.
- **Dates:** stored as ISO-8601 / epoch; displayed per setting as Gregorian, Hijri, or both.
  Hijri conversion lives in `crates/domain` (Umm al-Qura rules) with fixture tests, so it works
  offline.

### Why Tailwind + Bits UI

- Tailwind's **logical-property utilities** (`ms-`, `me-`, `ps-`, `pe-`, `start-`, `end-`) make
  RTL mirroring the default instead of a special case.
- Bits UI gives accessible primitives (dialog, menu, combobox, tabs, tooltip, popover) whose
  behaviour we'd otherwise re-implement — while letting our design system own every pixel.

## Consequences

- ✅ Consistent, token-driven theming (light/dark/sepia/reading) with a single source of colour.
- ✅ Accessibility work is concentrated in Bits UI + our own focus-ring/contrast rules.
- ⚠️ Tailwind v4's CSS-first config differs from v3's JS config — locked in the scaffold, not
  mixed.
- ⚠️ Charts library choice deferred to Milestone 10; the *token contract* is fixed now so any
  library must consume CSS variables.

## Alternatives considered

- **React + shadcn/ui** — larger bundles, more re-render cost for a 50k-row grid; not the brief's
  default.
- **UnoCSS** — equivalent, less familiar to contributors; no strong reason to switch.
- **A full UI kit (Smelte etc.)** — pre-styled kits fight a bespoke design system.
