# Maktabati brand

The Maktabati mark: an **open book** in ink with a **green bookmark ribbon** down its spine,
topped by a small **ring**. The ring-plus-descending-tail reads, gently, as the Arabic letter
**«م» (meem)** — the first letter of مكتبتي — while staying a plain, bookish bookmark. Restraint
over cleverness: it must read at 16 px and survive on both the paper plate and the deep-ink
dark surface.

## Provenance & licensing

Every shape is original geometry, authored in **VectorCraft** (the clean-room vector editor at
`~/Projects/vectorcraft`) from first principles — nothing is traced, embedded, or approximated
from any third-party logo, icon set, or font. The mark is licensable under **MIT OR
Apache-2.0**, the same terms as the app.

The vector masters are committed here; the rest of the kit is derived from them by
`scripts/make-brand.py`, so the whole set is reproducible from code:

| File | What it is |
|---|---|
| `maktabati-mark.svg` | **Master** — ink book + green ribbon, transparent, on-paper colours |
| `maktabati-icon.svg` | App icon — the mark on a warm-paper rounded plate (source for `src-tauri/icons`) |
| `maktabati-mark-reversed.svg` | Paper book + light-green ribbon, for `#12110f` surfaces |
| `maktabati-mark-mono.svg` | One flat colour (`currentColor`), for etching / single-colour use |
| `maktabati-wordmark.svg` | Bilingual lockup: mark + «مكتبتي» + "Maktabati" |
| `maktabati-mark-{1024,512,256,128,32}.png` | PNG renders of the mark |

Regenerate the derived kit and PNGs with:

```sh
python3 scripts/make-brand.py     # variants + wordmark + PNG renders
python3 scripts/make-icons.py     # app icon set into src-tauri/icons (via rsvg-convert)
```

Both need `rsvg-convert` (librsvg), which is already a Tauri Linux dependency. `make-icons.py`
keeps a Pillow-drawn fallback and warns loudly if librsvg is missing — the vector master stays
the source of truth.

## Colour

The mark uses the app's own tokens, so it themes itself: the book is `--text-1`, the ribbon is
`--accent`. In the light theme that is ink on warm paper with bookbinding green; in the dark
theme the same inline mark becomes paper-on-ink with the light-green accent (see
`src/lib/components/Mark.svelte`, which inlines the master paths and points them at the
tokens). The app icon bakes in the light "warm paper" treatment.

## The wordmark

`maktabati-wordmark.svg` references the Arabic and Latin fonts **by family only**
(`IBM Plex Sans Arabic, Cairo, Tajawal, sans-serif`) — never embedded, per the no-third-party-
font constraint. rsvg-convert/Pango shapes the Arabic correctly on render. If an
identical-everywhere render is ever required, outline the text before release.
