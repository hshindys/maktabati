#!/usr/bin/env python3
"""Derive the Maktabati brand variants from the VectorCraft master and render PNGs.

The vector masters (`design/brand/maktabati-mark.svg`, `maktabati-icon.svg`) are authored
in VectorCraft and committed. This script is the *mechanical* step that turns the master
into the rest of the kit, so the kit is reproducible from code rather than hand-drawn:

  maktabati-mark.svg            ink book + green ribbon, transparent  (VectorCraft master)
  maktabati-mark-reversed.svg   paper book + light-green ribbon, for dark surfaces
  maktabati-mark-mono.svg       one flat colour (currentColor), for etching/engraving
  maktabati-wordmark.svg        mark + «مكتبتي» + "Maktabati" lockup
  maktabati-mark-{1024,512,256,128,32}.png   PNG renders of the mark

The wordmark references Arabic/Latin fonts by family only (never embedded), rendered by
rsvg-convert/Pango, which shapes the Arabic correctly. `rsvg-convert` (librsvg) is already a
Tauri Linux dependency, so the pipeline has no extra requirement.

Reproducible: `python3 scripts/make-brand.py`. Requires `rsvg-convert` on PATH.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BRAND = ROOT / "design" / "brand"

RSVG = "/usr/sbin/rsvg-convert"

# Design tokens — must match src/app.css.
PAPER = "#fbf8f1"
INK = "#2a2621"
GREEN = "#2f6b4f"
GREEN_LIGHT = "#639f80"  # dark-theme accent green
ARABIC_FONT = "IBM Plex Sans Arabic, Cairo, Tajawal, sans-serif"

PATH_RE = re.compile(r"<path\b[^>]*/>")


def master_paths() -> list[str]:
    """The four <path> elements of the mark, in authoring order."""
    svg = (BRAND / "maktabati-mark.svg").read_text(encoding="utf-8")
    return PATH_RE.findall(svg)


def recolor(paths: list[str], mapping: dict[str, str]) -> list[str]:
    out = []
    for p in paths:
        for old, new in mapping.items():
            p = p.replace(old, new)
        out.append(p)
    return out


def svg_wrap(body: str, w: int, h: int, title: str) -> str:
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" '
        f'viewBox="0 0 {w} {h}">\n  <title>{title}</title>\n{body}\n</svg>\n'
    )


def indent(items: list[str]) -> str:
    return "\n".join("  " + i for i in items)


def build_variants(paths: list[str]) -> None:
    # Reversed: paper book + light-green ribbon, for #12110f surfaces.
    reversed_paths = recolor(
        paths, {INK: PAPER, GREEN: GREEN_LIGHT, "#2a2621": PAPER, "#2f6b4f": GREEN_LIGHT}
    )
    (BRAND / "maktabati-mark-reversed.svg").write_text(
        svg_wrap(indent(reversed_paths), 512, 512, "Maktabati Mark (reversed)"),
        encoding="utf-8",
    )

    # Mono: every fill and stroke collapses to currentColor (one flat colour).
    mono = []
    for p in paths:
        p = re.sub(r'fill="#[0-9a-fA-F]{6}"', 'fill="currentColor"', p)
        p = re.sub(r'stroke="#[0-9a-fA-F]{6}"', 'stroke="currentColor"', p)
        mono.append(p)
    (BRAND / "maktabati-mark-mono.svg").write_text(
        svg_wrap(indent(mono), 512, 512, "Maktabati Mark (mono)"),
        encoding="utf-8",
    )


def build_wordmark(paths: list[str]) -> None:
    """Mark + «مكتبتي» + "Maktabati" — the bilingual lockup.

    The mark spans x 72..440, y 64..404 in its own 512 space (368 x 340). We scale it and
    place it on the left; the Arabic name and the Latin name are stacked to its right, both
    centered on the same axis so the RTL Arabic (whose logical "start" is its right edge)
    never overflows back into the mark.
    """
    scale = 110.0 / 340.0
    tx, ty = 6.0, 2.0  # translate the scaled mark's (72,64) origin here

    mark = "\n".join(
        f'<g transform="translate({tx:.2f} {ty:.2f}) scale({scale:.4f})">{p}</g>' for p in paths
    )

    # Both text lines are centered on one axis to the right of the mark.
    cx = 330.0
    body = (
        f"  <g>\n{indent(mark.split(chr(10)))}\n  </g>\n"
        f'  <text x="{cx:.0f}" y="72" direction="rtl" text-anchor="middle" '
        f'font-family="{ARABIC_FONT}" font-size="62" font-weight="700" fill="{INK}">مكتبتي</text>\n'
        f'  <text x="{cx:.0f}" y="136" text-anchor="middle" '
        f'font-family="{ARABIC_FONT}" font-size="25" font-weight="500" '
        f'fill="{GREEN}" letter-spacing="2">Maktabati</text>'
    )
    (BRAND / "maktabati-wordmark.svg").write_text(
        svg_wrap(body, 560, 150, "Maktabati"), encoding="utf-8"
    )


def render(svg: str, out: pathlib.Path, w: int, h: int | None = None) -> None:
    # rsvg-convert scales the SVG's viewBox to the requested pixel size.
    subprocess.run([RSVG, "-w", str(w), "-h", str(h or w), str(svg), "-o", str(out)], check=True)


def main() -> int:
    if not pathlib.Path(RSVG).exists():
        print(f"make-brand: rsvg-convert not found at {RSVG}; cannot render", file=sys.stderr)
        return 1

    paths = master_paths()
    if len(paths) != 4:
        print(f"make-brand: expected 4 paths in the master, found {len(paths)}", file=sys.stderr)
        return 1

    build_variants(paths)
    build_wordmark(paths)

    mark = BRAND / "maktabati-mark.svg"
    for size in (1024, 512, 256, 128, 32):
        render(mark, BRAND / f"maktabati-mark-{size}.png", size)

    print("make-brand: wrote reversed, mono, wordmark and 5 PNG renders into design/brand/")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
