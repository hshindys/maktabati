#!/usr/bin/env python3
"""Generate the Maktabati app icon set from a single vector-ish description.

Reproducible: `python3 scripts/make-icons.py`. Outputs into `src-tauri/icons/`:
  icon.png          1024×1024  (source of truth, also used by the system tray)
  32x32.png, 128x128.png, 128x128@2x.png, 128x128@2x.png, icon.png (bundler)
  icon.ico          Windows
  icon.icns         macOS (only if `iconutil`/Pillow ICNS writer is available)

The mark: a warm "paper" rounded square, an open book in ink, and a green bookmark ribbon —
calm, bookish, and legible at 16 px.
"""

from __future__ import annotations

import pathlib

from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "icons"

# Design tokens — must match docs/04-ux-design.md.
PAPER = (251, 248, 241, 255)  # --surface-0 light
INK = (42, 38, 33, 255)  # --text-1
ACCENT = (47, 107, 79, 255)  # --accent (bookbinding green)
ACCENT_DARK = (39, 89, 63, 255)  # --accent-hover
RULE = (207, 196, 174, 255)  # --border-2


def draw_mark(size: int) -> Image.Image:
    s = size
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # Rounded background plate.
    d.rounded_rectangle([0, 0, s - 1, s - 1], radius=int(s * 0.22), fill=PAPER)

    # Open book: two pages meeting at a spine.
    m = s * 0.16  # margin
    top = s * 0.30
    bot = s * 0.80
    mid = s * 0.50
    # left page
    d.polygon(
        [(m, top + s * 0.02), (mid, top), (mid, bot), (m, bot + s * 0.04)],
        fill=(255, 255, 255, 255),
        outline=INK,
        width=max(1, int(s * 0.012)),
    )
    # right page
    d.polygon(
        [(s - m, top + s * 0.02), (mid, top), (mid, bot), (s - m, bot + s * 0.04)],
        fill=(255, 255, 255, 255),
        outline=INK,
        width=max(1, int(s * 0.012)),
    )
    # spine
    d.line([(mid, top), (mid, bot)], fill=INK, width=max(1, int(s * 0.014)))

    # Text rules on the pages (suggests writing without needing a font).
    for i in range(3):
        y = top + s * (0.10 + i * 0.075)
        d.line([(m + s * 0.06, y), (mid - s * 0.06, y - s * 0.01)], fill=RULE, width=max(1, int(s * 0.010)))
        d.line(
            [(mid + s * 0.06, y - s * 0.01), (s - m - s * 0.06, y)],
            fill=RULE,
            width=max(1, int(s * 0.010)),
        )

    # Bookmark ribbon hanging from the top-right of the right page.
    rw = s * 0.09
    rx = mid + s * 0.14
    d.polygon(
        [(rx, top - s * 0.02), (rx + rw, top - s * 0.06), (rx + rw, top + s * 0.16), (rx + rw / 2, top + s * 0.11), (rx, top + s * 0.16)],
        fill=ACCENT,
    )

    # Small accent rule under the book (grounding line).
    d.line([(m + s * 0.05, bot + s * 0.09), (s - m - s * 0.05, bot + s * 0.09)], fill=ACCENT_DARK, width=max(1, int(s * 0.016)))

    return img


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)

    master = draw_mark(1024)
    master.save(OUT / "icon.png")

    for name, size in (
        ("32x32.png", 32),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
    ):
        draw_mark(size).save(OUT / name)

    # Windows .ico with the sizes the bundler expects.
    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    master.save(OUT / "icon.ico", sizes=ico_sizes)

    # macOS .icns — Pillow can write ICNS directly.
    try:
        master.save(OUT / "icon.icns", sizes=[(16, 16), (32, 32), (64, 64), (128, 128), (256, 256), (512, 512)])
    except ValueError as exc:  # pragma: no cover - platform dependent
        print(f"skipping icon.icns: {exc}")

    for p in sorted(OUT.iterdir()):
        print(f"{p.relative_to(ROOT)}  {p.stat().st_size} bytes")


if __name__ == "__main__":
    main()
