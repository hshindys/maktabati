#!/usr/bin/env python3
"""Generate the Maktabati design-system sheet as a FigCraft command script.

FigCraft is driven headlessly: `figcraft-cli run` executes a JSON array of
`{command, params}` steps, where `{"$step": n}` refers to the layer the nth step
created. Writing that by hand does not scale past a few dozen layers, so the
sheet is generated here from the same tokens `src/app.css` ships — the tokens
stay single-sourced (docs/04-ux-design.md §5–§7) instead of being retyped into a
design file.

    python3 design/gen_design.py > design/design-system.json
    figcraft-cli run design/design-system.json --out design/maktabati.figcraft
    figcraft-cli export design/maktabati.figcraft -o design/export/ --scale 2
"""

from __future__ import annotations

import json
import sys

# ── Tokens (mirrors src/app.css) ─────────────────────────────────────────────

LIGHT = {
    "surface-0": "#fbf8f1",
    "surface-1": "#ffffff",
    "surface-2": "#f4efe4",
    "surface-3": "#eae3d5",
    "border-1": "#e3dbcb",
    "border-2": "#cfc4ae",
    "text-1": "#2a2621",
    "text-2": "#6b6153",
    "text-3": "#948a79",
    "star-empty": "#d8d0be",
}
DARK = {
    "surface-0": "#12110f",
    "surface-1": "#1a1917",
    "surface-2": "#22211e",
    "surface-3": "#2c2a26",
    "border-1": "#302e2a",
    "border-2": "#423f39",
    "text-1": "#ede7db",
    "text-2": "#ada697",
    "text-3": "#7e7768",
    "star-empty": "#45413a",
}
SEPIA = {
    "surface-0": "#f3e7cf",
    "surface-1": "#f7eedc",
    "surface-2": "#eee0c2",
    "surface-3": "#e4d3af",
    "border-1": "#ddcaa6",
    "border-2": "#c6ae83",
    "text-1": "#463b2a",
    "text-2": "#6f6049",
    "text-3": "#958568",
    "star-empty": "#d6c7a5",
}

# name → (accent, accent-soft, accent-soft-ink)
ACCENTS = {
    "green": ("#2f6b4f", "#e3efe8", "#24523c"),
    "indigo": ("#3b4cca", "#e6e8fa", "#2c3796"),
    "plum": ("#7a3b69", "#f6e7f1", "#5f2d51"),
    "terracotta": ("#b4552f", "#fbe9df", "#8c4122"),
    "ocean": ("#1f6a8a", "#dfeef5", "#17526b"),
    "gold": ("#8a6a1f", "#f6eed9", "#6a5117"),
}
SEMANTIC = [
    ("success", "#2f7a4f", "#e2f2e9"),
    ("warning", "#9a6b12", "#fbf0d8"),
    ("danger", "#a93226", "#fbe7e5"),
    ("info", "#2a5f8f", "#e4eef7"),
    ("star", "#c99a17", LIGHT["star-empty"]),
]

# label → (px, weight)
TYPE_SCALE = [
    ("3xl · 38", 38, "Bold"),
    ("2xl · 30", 30, "Bold"),
    ("xl · 24", 24, "SemiBold"),
    ("lg · 20", 20, "SemiBold"),
    ("md · 17", 17, "SemiBold"),
    ("base · 15", 15, "Regular"),
    ("sm · 13.5", 13.5, "Regular"),
    ("xs · 12", 12, "Regular"),
]
RADII = [("r-sm", 6), ("r-md", 10), ("r-lg", 14), ("r-xl", 20)]
SPACING = [("sp-1", 4), ("sp-2", 8), ("sp-3", 12), ("sp-4", 16),
           ("sp-5", 24), ("sp-6", 32), ("sp-7", 48), ("sp-8", 64)]

UI = "Inter"
AR = "Cairo"
NASKH = "Amiri"

SHEET_W = 1440


# ── Script builder ───────────────────────────────────────────────────────────

class Script:
    def __init__(self) -> None:
        self.steps: list[dict] = []

    def add(self, command: str, params: dict | None = None) -> int:
        self.steps.append({"command": command, "params": params or {}})
        return len(self.steps) - 1

    def _props(self, *, layout=None, gap=None, cross_gap=None, padding=None,
               fill=None, radius=None, stroke=None, stroke_weight=1,
               sizing_h=None, sizing_v=None, main_align=None, cross_align=None,
               clip=None) -> dict:
        props: dict = {}
        if layout is not None:
            props["layout"] = layout
        if gap is not None:
            props["gap"] = gap
        if cross_gap is not None:
            props["crossGap"] = cross_gap
        if padding is not None:
            props["padding"] = padding
        if fill is not None:
            props["fills"] = [fill] if isinstance(fill, str) else list(fill)
        if radius is not None:
            props["cornerRadius"] = radius
        if stroke is not None:
            props["stroke"] = stroke
            props["strokeWeight"] = stroke_weight
            props["strokeAlign"] = "inside"
        if sizing_h is not None:
            props["sizingH"] = sizing_h
        if sizing_v is not None:
            props["sizingV"] = sizing_v
        if main_align is not None:
            props["mainAlign"] = main_align
        if cross_align is not None:
            props["crossAlign"] = cross_align
        if clip is not None:
            props["clip"] = clip
        return props

    def frame(self, parent=None, *, name="", layout=None, gap=None, padding=None,
              fill=None, radius=None, stroke=None, stroke_weight=1, width=None,
              height=None, sizing_h=None, sizing_v=None, main_align=None,
              cross_align=None, clip=None, cross_gap=None,
              x=None, y=None) -> int:
        params: dict = {"type": "frame", "name": name, "select": False}
        if parent is not None:
            params["parent"] = {"$step": parent}
        # `x`/`y` only take effect when the parent has no auto layout — they are how the
        # absolute children inside a cover or a title bar are placed.
        if x is not None:
            params["x"] = x
        if y is not None:
            params["y"] = y
        if width is not None:
            params["width"] = width
        if height is not None:
            params["height"] = height
        props = self._props(layout=layout, gap=gap, cross_gap=cross_gap,
                            padding=padding, fill=fill, radius=radius,
                            stroke=stroke, stroke_weight=stroke_weight,
                            sizing_h=sizing_h, sizing_v=sizing_v,
                            main_align=main_align, cross_align=cross_align,
                            clip=clip)
        if props:
            params["props"] = props
        return self.add("node.create", params)

    def rect(self, parent=None, *, name="", width=None, height=None, fill=None,
             radius=None, stroke=None, stroke_weight=1, x=None, y=None,
             sizing_h=None, sizing_v=None, opacity=None) -> int:
        params: dict = {"type": "rectangle", "name": name, "select": False}
        if parent is not None:
            params["parent"] = {"$step": parent}
        if x is not None:
            params["x"] = x
        if y is not None:
            params["y"] = y
        if width is not None:
            params["width"] = width
        if height is not None:
            params["height"] = height
        props = self._props(fill=fill, radius=radius, stroke=stroke,
                            stroke_weight=stroke_weight, sizing_h=sizing_h,
                            sizing_v=sizing_v)
        if opacity is not None:
            props["opacity"] = opacity
        if props:
            params["props"] = props
        return self.add("node.create", params)

    def text(self, s: str, parent=None, *, size=15, color=None, font=UI,
             style="Regular", width=None, align=None, line_height=None,
             name="") -> int:
        params: dict = {
            "type": "text",
            "text": s,
            "select": False,
            "props": {
                "fontFamily": font,
                "fontSize": size,
                "fontStyle": style,
                "fills": [color or LIGHT["text-1"]],
                "textAutoResize": "widthAndHeight" if width is None else "height",
            },
        }
        if name:
            params["name"] = name
        if parent is not None:
            params["parent"] = {"$step": parent}
        if width is not None:
            params["width"] = width
        if align is not None:
            params["textAlign"] = align
        if line_height is not None:
            params["lineHeight"] = line_height
        return self.add("node.create", params)


s = Script()


# ── Sheet sections ───────────────────────────────────────────────────────────

def section(root: int, title: str, subtitle: str) -> int:
    """A titled, full-width block on the sheet."""
    box = s.frame(root, name=title, layout="vertical", gap=18, padding=[28, 32, 32, 32],
                  fill=LIGHT["surface-1"], radius=14, stroke=LIGHT["border-1"],
                  stroke_weight=1, sizing_h="fill", sizing_v="hug", clip=True)
    s.text(title, box, size=24, style="SemiBold", color=LIGHT["text-1"])
    s.text(subtitle, box, size=13.5, color=LIGHT["text-3"])
    return box


def swatch_row(parent: int, items: list[tuple[str, str]], *, label_color=None,
               size: tuple[int, int] = (132, 72)) -> None:
    row = s.frame(parent, layout="horizontal", gap=16, sizing_h="fill",
                  sizing_v="hug", cross_align="min")
    for label, hexv in items:
        cell = s.frame(row, layout="vertical", gap=8, sizing_h="hug", sizing_v="hug")
        s.rect(cell, width=size[0], height=size[1], fill=hexv, radius=10,
               stroke=LIGHT["border-1"] if hexv.lower() in ("#ffffff",) else None)
        s.text(hexv.upper(), cell, size=12, color=label_color or LIGHT["text-3"], font=UI)
        s.text(label, cell, size=13.5, color=LIGHT["text-2"])


def build() -> Script:
    root = s.frame(
        None,
        name="Maktabati · Design System",
        layout="vertical",
        gap=40,
        padding=56,
        fill=LIGHT["surface-0"],
        width=SHEET_W,
        sizing_h="fixed",
        sizing_v="hug",
    )

    # ── 1. Masthead ──────────────────────────────────────────────────────────
    head = s.frame(root, layout="vertical", gap=10, sizing_h="fill", sizing_v="hug")
    s.text("مكتبتي · نظام التصميم", head, size=38, style="Bold",
           color=LIGHT["text-1"], font=AR, line_height="125%")
    s.text("Maktabati design tokens — single source of truth: docs/04-ux-design.md §5–§7, "
           "shipped as src/app.css", head, size=15, color=LIGHT["text-2"])
    rule = s.frame(head, layout="horizontal", sizing_h="fill", height=4, fill=None)
    s.rect(rule, width=SHEET_W - 112, height=3, fill="#2f6b4f", radius=2)

    # ── 2. Surfaces, per theme ───────────────────────────────────────────────
    box = section(root, "الأسطح والحدود · Surfaces & borders",
                  "Four elevations and two border weights, in every theme.")
    for theme_name, theme in (("light · ورقي", LIGHT), ("dark · حبر", DARK), ("sepia · تربيع", SEPIA)):
        grp = s.frame(box, layout="vertical", gap=10, sizing_h="fill", sizing_v="hug")
        s.text(theme_name, grp, size=15, style="SemiBold", color=LIGHT["text-1"], font=AR)
        swatch_row(grp, [(k, v) for k, v in theme.items() if k.startswith(("surface", "border"))])
        swatch_row(grp, [(k, v) for k, v in theme.items() if k.startswith("text")])

    # ── 3. Accents ───────────────────────────────────────────────────────────
    box = section(root, "لون التمييز · Accent presets",
                  "Six presets, each with accent / soft / soft-ink. Applied to buttons, links, "
                  "focus rings, progress bars, active nav and the heatmap ramp.")
    row = s.frame(box, layout="horizontal", gap=16, sizing_h="fill", sizing_v="hug")
    for name, (accent, soft, ink) in ACCENTS.items():
        cell = s.frame(row, layout="vertical", gap=0, width=200, radius=14, clip=True,
                       stroke=LIGHT["border-1"], sizing_h="fill", sizing_v="hug")
        s.rect(cell, width=186, height=72, fill=accent)
        body = s.frame(cell, layout="vertical", gap=4, padding=[12, 14, 14, 14],
                       fill=soft, sizing_h="fill", sizing_v="hug")
        s.text(name, body, size=15, style="SemiBold", color=ink)
        s.text(accent.upper(), body, size=12, color=ink, font=UI)

    # ── 4. Semantic ──────────────────────────────────────────────────────────
    box = section(root, "الدلالات · Semantic",
                  "Accent-independent status colours, each with a soft companion.")
    row = s.frame(box, layout="horizontal", gap=16, sizing_h="fill", sizing_v="hug")
    for name, solid, soft in SEMANTIC:
        cell = s.frame(row, layout="vertical", gap=8, padding=[14, 16, 14, 16],
                       fill=soft, radius=10, stroke=LIGHT["border-1"],
                       sizing_h="fill", sizing_v="hug")
        s.text(name, cell, size=15, style="SemiBold", color=solid, font=UI)
        s.text(solid.upper(), cell, size=12, color=LIGHT["text-2"], font=UI)

    # ── 5. Type scale ────────────────────────────────────────────────────────
    box = section(root, "مقياس الخط · Type scale",
                  "1.200 minor third on a 16px base. Arabic gets the looser --lh-read (1.85) "
                  "so diacritics and descenders never collide.")
    for label, px, weight in TYPE_SCALE:
        line = s.frame(box, layout="horizontal", gap=24, padding=[6, 0, 6, 0],
                       sizing_h="fill", sizing_v="hug", cross_align="baseline")
        s.text(label, line, size=13.5, color=LIGHT["text-3"], width=110)
        s.text("مقدمة في التاريخ الإسلامي", line, size=px, style=weight,
               color=LIGHT["text-1"], font=AR, line_height="150%")
        s.text("Reading a library", line, size=px, style=weight, color=LIGHT["text-2"], font=UI)

    # ── 6. Radii + spacing ───────────────────────────────────────────────────
    box = section(root, "الزوايا والمسافات · Radii & spacing",
                  "A four-step radius ramp on an 8px rhythm (4/8/12/16/24/32/48/64).")
    row = s.frame(box, layout="horizontal", gap=20, sizing_h="fill", sizing_v="hug")
    for label, px in RADII:
        cell = s.frame(row, layout="vertical", gap=8, sizing_h="hug", sizing_v="hug")
        s.rect(cell, width=96, height=64, fill=LIGHT["surface-2"], radius=px,
               stroke=LIGHT["border-2"])
        s.text(f"{label} · {px}px", cell, size=13.5, color=LIGHT["text-2"])

    row = s.frame(box, layout="horizontal", gap=20, padding=[8, 0, 0, 0],
                  sizing_h="fill", sizing_v="hug", cross_align="max")
    for label, px in SPACING:
        cell = s.frame(row, layout="vertical", gap=8, sizing_h="hug", sizing_v="hug",
                       cross_align="center")
        s.rect(cell, width=max(px, 4), height=px, fill="#3b4cca")
        s.text(f"{label} · {px}", cell, size=12, color=LIGHT["text-3"])

    # ── 7. Components ────────────────────────────────────────────────────────
    box = section(root, "المكوّنات · Components",
                  "The base kit: buttons, field, chip, progress, rating, stat card.")

    row = s.frame(box, layout="horizontal", gap=12, sizing_h="fill", sizing_v="hug",
                  cross_align="center")
    for label, fill, fg, stroke in (
        ("أساسي · Primary", "#2f6b4f", "#ffffff", None),
        ("ثانوي · Secondary", LIGHT["surface-1"], LIGHT["text-1"], LIGHT["border-2"]),
        ("شبح · Ghost", None, "#2f6b4f", None),
        ("خطر · Danger", "#a93226", "#ffffff", None),
    ):
        btn = s.frame(row, layout="horizontal", gap=8, padding=[10, 18, 10, 18],
                      fill=fill, radius=8, stroke=stroke, sizing_h="hug",
                      sizing_v="hug", cross_align="center")
        s.text(label, btn, size=15, style="SemiBold", color=fg, font=AR)

    row = s.frame(box, layout="horizontal", gap=12, sizing_h="fill", sizing_v="hug",
                  cross_align="center")
    field = s.frame(row, layout="horizontal", padding=[10, 14, 10, 14], gap=8,
                    fill=LIGHT["surface-1"], radius=8, stroke=LIGHT["border-2"],
                    width=360, sizing_h="fixed", sizing_v="hug", cross_align="center")
    s.text("ابحث في كل شيء…", field, size=15, color=LIGHT["text-3"], font=AR)
    for label, fill, fg in (("قراءة الآن", "#e3efe8", "#24523c"),
                            ("أنهيت", "#e6e8fa", "#2c3796")):
        chip = s.frame(row, layout="horizontal", padding=[6, 12, 6, 12], fill=fill,
                       radius=999, sizing_h="hug", sizing_v="hug")
        s.text(label, chip, size=13.5, color=fg, font=AR)

    # progress
    prog = s.frame(box, layout="vertical", gap=8, sizing_h="fill", sizing_v="hug")
    s.text("62% · صفحة ٢٦٣ من ٤٢٤", prog, size=13.5, color=LIGHT["text-2"], font=AR)
    track = s.frame(prog, layout="horizontal", height=8, width=520, fill=LIGHT["surface-3"],
                    radius=999, clip=True, sizing_h="fixed", sizing_v="fixed")
    s.rect(track, width=322, height=8, fill="#2f6b4f", radius=999)

    # rating
    stars = s.frame(box, layout="horizontal", gap=4, sizing_h="hug", sizing_v="hug",
                    cross_align="center")
    s.text("★★★★☆  (٤)", stars, size=24, color="#c99a17", font=AR)
    s.text("نجمة واحدة · star #c99a17 / star-empty #d8d0be", stars, size=13.5,
           color=LIGHT["text-3"], font=AR)

    # stat card
    cards = s.frame(box, layout="horizontal", gap=16, sizing_h="fill", sizing_v="hug")
    for value, label, hint in (("1,284", "كتاب · إجمالي", "dashboard stat"),
                               ("47", "أقرأها الآن", "dashboard stat"),
                               ("18,302", "صفحة هذا العام", "dashboard stat")):
        c = s.frame(cards, layout="vertical", gap=6, padding=[18, 20, 18, 20],
                    fill=LIGHT["surface-1"], radius=14, stroke=LIGHT["border-1"],
                    sizing_h="fill", sizing_v="hug")
        s.text(value, c, size=30, style="Bold", color=LIGHT["text-1"], font=AR)
        s.text(label, c, size=13.5, color=LIGHT["text-2"], font=AR)
        s.text(hint, c, size=12, color=LIGHT["text-3"], font=UI)

    return s


if __name__ == "__main__":
    build()
    json.dump(s.steps, sys.stdout, ensure_ascii=False, indent=1)
    print()
