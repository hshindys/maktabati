# ADR-0011 — Barcode scanning is COULD; typed/paste ISBN entry is the guaranteed path

**Status:** Accepted · **Date:** 2026-10-18

## Context

The brief asks for ISBN lookup *"typed or via webcam/barcode scan **if feasible**"*. Research
found barcode scanning is a headline feature of Libib, CLZ, BookBuddy and LibraryThing — but all
of them deliver it through **mobile apps**, not desktop apps. On desktop, physical USB barcode
scanners work by emulating keyboard input (they type the ISBN + Enter), which needs **no API at
all**.

## Decision

1. **Guaranteed (MUST):** typed ISBN input with live validation (ISBN-10/13 + checksum), plus
   **paste** support. A USB barcode scanner "just works" through this path because it is a keyboard.
2. **Webcam capture is COULD (post-v1).** If attempted, it will use a browser-side decoder
   (e.g. `zxing`-family WASM) purely for *decoding to a string* — no image ever leaves the machine
   — and feed the same `isbn_lookup` command.
3. Settings documents: *"USB barcode scanners work out of the box — they type into this field."*

## Consequences

- ✅ No camera permissions, no native capture dependency, no platform matrix risk.
- ✅ The scanner UX that most home-library users actually own (a USB wand) is supported on day one.
- ⚠️ Webcam users must type/paste for v1. Recorded as a COULD in `01-product-spec.md`.

## Alternatives considered

| Option | Why rejected now |
|---|---|
| Native camera plugin | Platform-specific, heavy test matrix (Win/macOS/Linux), no benefit over USB scanners for the target user. |
| Phone-as-scanner companion | Requires a device/network → violates offline-first simplicity. |
