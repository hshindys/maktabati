#!/usr/bin/env python3
"""Verify dashboard row geometry against the render, not against guesses.

`gen_frames.py` gives its two fixed-height rows their heights from constants
(`KPI_ROW_H`, `STATS_ROW_H`) because a horizontal FigCraft frame cannot derive
its cross-axis size from `sizing_v="fill"` children. This script checks, from
the exported PNG:

    height    the card's border-to-border height, vs the constant that produced it
    gap       distance from the card's bottom border to its last ink — i.e. how
              much of the card is empty below the content
    excess    a card's gap minus the smallest gap in its row: the dead space the
              tallest sibling forces onto it, which no spacing token can remove

Usage: python3 design/measure_spacing.py
"""
from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))

from gen_frames import KPI_ROW_H, SP_CARD, STATS_ROW_H  # noqa: E402

EXPORT = HERE / "export"
BORDER = (227, 219, 203)  # --border-1 as it lands on the export
TOL = 12                  # antialiasing around the border stroke

Bands = dict[str, tuple[int, int, list[tuple[str, int, int]]]]


def card_box_and_ink(
    im: Image.Image, x0: int, x1: int, y0: int, y1: int
) -> tuple[int, int, int]:
    """Return (top, bottom, last_ink) for one card's vertical band."""
    px = im.load()
    width = x1 - x0
    top = bottom = None
    for y in range(y0, y1):
        border_px = sum(
            1 for x in range(x0, x1)
            if all(abs(px[x, y][i] - BORDER[i]) <= TOL for i in range(3))
        )
        if border_px >= width * 0.5:  # a full horizontal border line, not text
            if top is None:
                top = y
            bottom = y
    assert top is not None and bottom is not None, f"no card borders in {x0}..{x1}"

    last_ink = None
    for y in range(top, bottom):  # inside the card only
        if any(sum(px[x, y]) / 3 < 210 for x in range(x0, x1)):
            last_ink = y
    assert last_ink is not None
    return top, bottom, last_ink


def report(path: Path, expected: int, rows: Bands) -> None:
    im = Image.open(path).convert("RGB")
    print(f"\n{path.name}  ({im.width}x{im.height})   expected row height: {expected}")
    for row_name, (y0, y1, cards) in rows.items():
        measured = [
            card_box_and_ink(im, x0, x1, y0, y1) for _, (x0, x1) in cards
        ]
        gaps = [bottom - ink for _, bottom, ink in measured]
        print(f"  {row_name}")
        for (name, _), (top, bottom, ink), gap in zip(cards, measured, gaps):
            height = bottom - top + 1
            flag = "OK" if height == expected else f"!! expected {expected}"
            print(
                f"    {name:<14} y={top:<4}..{bottom:<4} height={height:<4} {flag:<20}"
                f" gap={gap:<3} excess={gap - min(gaps)}"
            )


# Card bands: the rail occupies x >= 1060, so the content area is x 40..1024,
# and each row's cards split it with 16px gaps (RTL: first card at the right).
if __name__ == "__main__":
    dash = EXPORT / "1 _ لوحة التحكم _ Dashboard.png"
    report(
        dash,
        KPI_ROW_H,
        {
            "kpi row (right to left)": (
                # Cards span y 196..(196 + KPI_ROW_H - 1); stop well before the
                # stats row's top border so its line is not mistaken for ours.
                185, 185 + KPI_ROW_H + 20,
                list(zip(
                    ["al-ijmal", "qaraaha", "safha", "hadaf"],
                    [(790, 1024), (540, 774), (290, 524), (40, 274)],
                )),
            ),
        },
    )
    stats_top = 196 + KPI_ROW_H + 32  # KPI row + SP_SECTION
    report(
        dash,
        STATS_ROW_H,
        {
            "stats row (right to left)": (
                stats_top - 10, stats_top + STATS_ROW_H + 10,
                list(zip(["fi-maktabati", "al-lughat"], [(545, 1024), (40, 529)])),
            ),
        },
    )
