#!/usr/bin/env python3
"""Generate the Maktabati app icon set from the VectorCraft vector master.

Reproducible: `python3 scripts/make-icons.py`. Outputs into `src-tauri/icons/`:
  icon.png          1024×1024  (source of truth, also used by the system tray)
  32x32.png, 128x128.png, 128x128@2x.png (bundler sizes)
  icon.ico          Windows
  icon.icns         macOS (only if Pillow's ICNS writer is available)

The icon is no longer drawn in code: it is rasterized from `design/brand/maktabati-icon.svg`,
which is authored in VectorCraft (see that file and `scripts/make-brand.py`). Each size is
rasterized straight from the vector at its own pixel size — crisper than downscaling a big
raster. `rsvg-convert` (librsvg) is already a Tauri Linux dependency.

Fallback: if `rsvg-convert` is unavailable, this falls back to a Pillow-drawn approximation of
the mark and says so loudly — the vector master stays the source of truth.
"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import sys

from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "icons"
ICON_SVG = ROOT / "design" / "brand" / "maktabati-icon.svg"
RSVG = "/usr/sbin/rsvg-convert"

# Design tokens — must match src/app.css (light "warm paper" theme).
PAPER = (251, 248, 241, 255)  # --surface-0
INK = (42, 38, 33, 255)  # --text-1
ACCENT = (47, 107, 79, 255)  # --accent (bookbinding green)


def rasterize(size: int) -> Image.Image:
    """Rasterize the vector icon at `size` px via rsvg-convert."""
    tmp = OUT / f".tmp-{size}.png"
    subprocess.run(
        [RSVG, "-w", str(size), "-h", str(size), str(ICON_SVG), "-o", str(tmp)],
        check=True,
    )
    img = Image.open(tmp).convert("RGBA")
    tmp.unlink(missing_ok=True)
    return img


def fallback_mark(size: int) -> Image.Image:
    """Pillow-drawn approximation of the mark, used only when rsvg-convert is missing."""
    s = size
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle([0, 0, s - 1, s - 1], radius=int(s * 0.22), fill=PAPER)
    m, top, bot, mid = s * 0.16, s * 0.30, s * 0.78, s * 0.50
    d.polygon([(m, top + s * 0.02), (mid, top), (mid, bot), (m, bot + s * 0.04)], fill=INK)
    d.polygon(
        [(s - m, top + s * 0.02), (mid, top), (mid, bot), (s - m, bot + s * 0.04)], fill=INK
    )
    rw, rx = s * 0.07, mid - s * 0.035
    d.polygon(
        [(rx, top - s * 0.04), (rx + rw, top - s * 0.04), (rx + rw, bot + s * 0.02),
         (rx + rw / 2, bot - s * 0.02), (rx, bot + s * 0.02)],
        fill=ACCENT,
    )
    return img


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)

    have_rsvg = pathlib.Path(RSVG).exists() and ICON_SVG.exists()
    if not have_rsvg:
        print(
            "make-icons: WARNING — rsvg-convert or the vector icon is missing; "
            "falling back to the Pillow-drawn approximation. Regenerate with librsvg "
            "for the real mark.",
            file=sys.stderr,
        )

    make = rasterize if have_rsvg else fallback_mark

    make(1024).save(OUT / "icon.png")
    for name, size in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256)):
        make(size).save(OUT / name)

    # Windows .ico with the sizes the bundler expects.
    master = Image.open(OUT / "icon.png")
    master.save(
        OUT / "icon.ico",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    # macOS .icns — Pillow can write ICNS directly.
    try:
        master.save(
            OUT / "icon.icns",
            sizes=[(16, 16), (32, 32), (64, 64), (128, 128), (256, 256), (512, 512)],
        )
    except ValueError as exc:  # pragma: no cover - platform dependent
        print(f"skipping icon.icns: {exc}")

    for p in sorted(OUT.iterdir()):
        if p.name.startswith("."):
            continue
        print(f"{p.relative_to(ROOT)}  {p.stat().st_size} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
