#!/usr/bin/env python3
"""Generate the Maktabati design-system sheet as a FigCraft command script.

FigCraft is driven headlessly: `figcraft-cli run` executes a JSON array of
`{command, params}` steps, where `{"$step": n}` refers to the layer the nth step
created. Writing that by hand does not scale past a few dozen layers, so the
sheet is generated here.

The tokens are not retyped: they are **parsed out of `src/app.css`**, the file the
app actually ships. A copied literal table eventually drifts from the product, and
this one did — the moment P0-3/P0-4 retuned the dark and sepia matrix, the sheet
was still showing the pre-fix greys. Every selector the sheet needs is looked up by
name and the script fails loudly if one disappears, so the next token change moves
the sheet with it.

    python3 design/gen_design.py > design/design-system.json
    figcraft-cli run design/design-system.json --out design/design-system.figcraft
    python3 design/gen_frames.py > design/frames.json
    figcraft-cli run design/frames.json --in design/design-system.figcraft \
        --out design/maktabati.figcraft
    figcraft-cli export design/maktabati.figcraft -o design/export/ --scale 2

`design/check_tokens.py` pins the parsed values against the ones the app ships
(the dark/sepia greys, the retuned accents, the per-theme statuses), so a token
rename in app.css that would empty a section here is caught before the sheet is
regenerated rather than after it silently renders blank.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
APP_CSS = ROOT / "src" / "app.css"


# ── Token extraction ─────────────────────────────────────────────────────────

def _strip_comments(css: str) -> str:
    return re.sub(r"/\*.*?\*/", "", css, flags=re.S)


def _decls(body: str) -> dict[str, str]:
    """`--name: value;` pairs of a rule body, in declaration order."""
    return dict(re.findall(r"(--[a-z0-9-]+)\s*:\s*([^;]+);", body))


def _top_level_rules(css: str) -> list[tuple[str, str]]:
    """`(selector, body)` for every top-level rule; at-rules are skipped whole.

    Token blocks are all flat and top-level, so a depth walk is enough: it also
    means a future `@media` wrapper around base styles cannot silently shadow a
    token lookup — the inner rules are skipped and the lookup then fails loudly.
    """
    rules: list[tuple[str, str]] = []
    sel_start, i, n = 0, 0, len(css)
    while i < n:
        if css[i] != "{":
            i += 1
            continue
        selector = " ".join(css[sel_start:i].split())
        depth, j = 1, i + 1
        while j < n and depth:
            depth += (css[j] == "{") - (css[j] == "}")
            j += 1
        if not selector.startswith("@"):
            rules.append((selector, css[i + 1 : j - 1]))
        sel_start, i = j, j
    return rules


def _pick(rules: list[tuple[str, str]], selector: str) -> dict[str, str]:
    for sel, body in rules:
        if sel == selector:
            return _decls(body)
    raise SystemExit(f"gen_design: `{selector}` not found in {APP_CSS}")


def _subset(block: dict[str, str], *prefixes: str) -> dict[str, str]:
    """The `--` off, only the wanted prefixes — e.g. `surface`, `border`, `text`."""
    return {
        key[2:]: value
        for key, value in block.items()
        if any(key == "--" + p or key.startswith("--" + p + "-") for p in prefixes)
    }


def _only(block: dict[str, str], *prefixes: str) -> dict[str, str]:
    """Like `_subset`, but for a dict whose keys are already `--`-stripped."""
    return {
        key: value
        for key, value in block.items()
        if any(key == p or key.startswith(p + "-") for p in prefixes)
    }


def _px(value: str) -> float:
    """`1.25rem` / `6px` → px on the 16px root the app assumes."""
    value = value.strip()
    return float(value[:-3]) * 16 if value.endswith("rem") else float(value[:-2])


def _alpha_hex(color: str) -> str:
    """`rgba(42, 38, 33, 0.06)` → `#2a26210f`; `#rrggbb` → `#rrggbbff`.

    FigCraft draws effects with `#rrggbbaa`, so the box-shadow strings the app
    ships have to be translated rather than approximated — an elevation spec that
    quietly invents its own alpha is worse than none.
    """
    color = color.strip()
    m = re.match(r"rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*([\d.]+)\s*\)", color)
    if m:
        r, g, b, a = int(m[1]), int(m[2]), int(m[3]), float(m[4])
        return f"#{r:02x}{g:02x}{b:02x}{round(a * 255):02x}"
    if re.fullmatch(r"#[0-9a-fA-F]{6}", color):
        return color.lower() + "ff"
    return color


def _shadow_effect(value: str) -> dict:
    m = re.match(r"(-?[\d.]+)(?:px)?\s+(-?[\d.]+)(?:px)?\s+(-?[\d.]+)(?:px)?\s+(rgba?\([^)]+\)|#[0-9a-fA-F]+)", value)
    if not m:
        raise SystemExit(f"gen_design: cannot parse shadow `{value}`")
    return {
        "type": "dropShadow",
        "offset": [float(m[1]), float(m[2])],
        "radius": float(m[3]),
        "spread": 0.0,
        "color": _alpha_hex(m[4]),
    }


THEMES = ("light", "sepia", "dark")
ACCENT_NAMES = ("green", "indigo", "plum", "terracotta", "ocean", "gold")
STATUS_NAMES = ("success", "warning", "danger", "info")

_rules = _top_level_rules(_strip_comments(APP_CSS.read_text(encoding="utf-8")))

# Surfaces/borders/text/shadows, keyed by theme. Light lives in the combined
# `:root, [data-theme="light"]` block; sepia inherits light's shadows.
_THEME_BLOCKS = {
    "light": _pick(_rules, ':root, [data-theme="light"]'),
    "sepia": _pick(_rules, '[data-theme="sepia"]'),
    "dark": _pick(_rules, '[data-theme="dark"]'),
}
THEME_TOKENS = {
    name: {
        **_subset(_THEME_BLOCKS["light"], "surface", "border", "text", "sh", "star-empty"),
        **_subset(block, "surface", "border", "text", "sh", "star-empty"),
    }
    for name, block in _THEME_BLOCKS.items()
}

# Accent presets per theme. Sepia overrides only accent/hover for the four
# presets that needed retuning on warm surfaces; dark ships all five values of
# its own (lighter accents, ink foreground).
def _optional(selector: str) -> dict[str, str]:
    """The declarations of `selector`, or nothing if the app doesn't override it."""
    for sel, body in _rules:
        if sel == selector:
            return _decls(body)
    return {}


def _clean(block: dict[str, str]) -> dict[str, str]:
    """`--accent: …` → `accent: …`, so sheet code reads `acc["accent"]`."""
    return {key[2:]: value for key, value in block.items()}


_ACCENT_BLOCKS = {
    "light": {n: _pick(_rules, f'[data-accent="{n}"]') for n in ACCENT_NAMES},
    "sepia": {n: _optional(f'[data-theme="sepia"][data-accent="{n}"]') for n in ACCENT_NAMES},
    "dark": {n: _pick(_rules, f'[data-theme="dark"][data-accent="{n}"]') for n in ACCENT_NAMES},
}
ACCENT_MATRIX = {
    "light": {n: _clean(blk) for n, blk in _ACCENT_BLOCKS["light"].items()},
    # Sepia layers its retuned accent/hover over the light preset's full set.
    "sepia": {
        n: {**_clean(_ACCENT_BLOCKS["light"][n]), **_clean(_ACCENT_BLOCKS["sepia"][n])}
        for n in ACCENT_NAMES
    },
    "dark": {n: _clean(blk) for n, blk in _ACCENT_BLOCKS["dark"].items()},
}

# Status colours: accent-independent, resolved per theme (dark flips lighter,
# sepia overrides success/warning only). star-filled is global; star-empty is a
# theme token and rides along from THEME_TOKENS.
_BASE = _pick(_rules, ":root")
_STATUS_BLOCKS = {
    "light": _BASE,
    "sepia": {**_BASE, **_pick(_rules, ':root[data-theme="sepia"]')},
    "dark": {**_BASE, **_pick(_rules, ':root[data-theme="dark"]')},
}
STATUS_MATRIX = {
    name: _subset(block, "success", "warning", "danger", "info", "star-filled")
    for name, block in _STATUS_BLOCKS.items()
}

# A `--`-stripped view of `:root` for the scale tokens below (colour tokens go
# through `_subset`, which strips too).
_ROOT = _clean(_pick(_rules, ":root"))

# The px sizes are the app's own tokens; the weight per step is a markup decision
# (headings SemiBold/Bold, body Regular) rather than a CSS token, so it lives
# here. `:root` declares them smallest-first; the sheet reads largest-first.
_WEIGHTS = {"3xl": "Bold", "2xl": "Bold", "xl": "SemiBold", "lg": "SemiBold",
            "md": "SemiBold", "base": "Regular", "sm": "Regular", "xs": "Regular"}


def _fmt_px(px: float) -> str:
    """13.504 → `13.5`, 38.0 → `38` (the rem tokens round at the pixel)."""
    px = round(px, 1)
    return str(int(px)) if px == int(px) else f"{px:g}"


TYPE_SCALE = []
for key, value in _ROOT.items():
    if key.startswith("fs-"):
        step = key[3:]
        TYPE_SCALE.append((f"{step} · {_fmt_px(_px(value))}", _px(value),
                           _WEIGHTS.get(step, "Regular")))
TYPE_SCALE.reverse()

RADII = [(key, int(_px(value))) for key, value in _ROOT.items() if key.startswith("r-")]

# Spacing: `app.css` only *aliases* --sp-N (in @theme); the literal 4/8/12/16/24/
# 32/48/64 rhythm those aliases point at is the documented scale in
# docs/04-ux-design.md §6, so it is read from there rather than invented here.
SPACING = [("sp-1", 4), ("sp-2", 8), ("sp-3", 12), ("sp-4", 16),
           ("sp-5", 24), ("sp-6", 32), ("sp-7", 48), ("sp-8", 64)]

MOTION = [
    (key, value.strip())
    for key, value in _ROOT.items()
    if key.startswith("dur-") or key in ("ease", "stagger")
]

# Shadows are per-theme (dark's are pure black at a higher alpha); sepia declares
# none and inherits light's, exactly as the cascade does.
SHADOWS = {name: _only(THEME_TOKENS[name], "sh") for name in THEMES}

# ── Compat surface for gen_frames.py, which imports these by name ────────────
LIGHT, SEPIA, DARK = THEME_TOKENS["light"], THEME_TOKENS["sepia"], THEME_TOKENS["dark"]
ACCENTS = {n: (m["accent"], m["accent-soft"], m["accent-soft-ink"])
           for n, m in ACCENT_MATRIX["light"].items()}
SEMANTIC = [
    (name, STATUS_MATRIX["light"][name], STATUS_MATRIX["light"][f"{name}-soft"])
    for name in STATUS_NAMES
] + [("star", STATUS_MATRIX["light"]["star-filled"], LIGHT["star-empty"])]

# The two font families the sheet draws in (the app's --font-ui / --font-read
# stacks; the sheet pins one concrete family per role so renders are stable).
UI = "Inter"
AR = "Cairo"

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
               stroke_dash=None, effects=None, opacity=None,
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
            if stroke_dash is not None:
                # `node.create`'s dash pattern prop (see `figcraft-cli commands`).
                props["dashes"] = list(stroke_dash)
        if effects is not None:
            # Real drop shadows: `props` accepts every `node.set` key, effects
            # included, so elevation renders as the shipped box-shadow rather
            # than as a grey rectangle pretending to be one.
            props["effects"] = list(effects)
        if opacity is not None:
            props["opacity"] = opacity
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
              fill=None, radius=None, stroke=None, stroke_weight=1,
              stroke_dash=None, effects=None, opacity=None, width=None,
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
                            stroke_dash=stroke_dash, effects=effects,
                            opacity=opacity,
                            sizing_h=sizing_h, sizing_v=sizing_v,
                            main_align=main_align, cross_align=cross_align,
                            clip=clip)
        if props:
            params["props"] = props
        return self.add("node.create", params)

    def rect(self, parent=None, *, name="", width=None, height=None, fill=None,
             radius=None, stroke=None, stroke_weight=1, x=None, y=None,
             sizing_h=None, sizing_v=None, opacity=None, stroke_dash=None,
             effects=None) -> int:
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
                            stroke_weight=stroke_weight, stroke_dash=stroke_dash,
                            effects=effects,
                            sizing_h=sizing_h, sizing_v=sizing_v, opacity=opacity)
        if props:
            params["props"] = props
        return self.add("node.create", params)

    def ellipse(self, parent=None, *, name="", width=None, height=None, fill=None,
                stroke=None, stroke_weight=1, stroke_dash=None, x=None,
                y=None) -> int:
        """A circle — the loading spinner and the focus ring's round cousin."""
        params: dict = {"type": "ellipse", "name": name, "select": False}
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
        props = self._props(fill=fill, stroke=stroke, stroke_weight=stroke_weight,
                            stroke_dash=stroke_dash)
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
    s.text(subtitle, box, size=13.5, color=LIGHT["text-3"], width=SHEET_W - 176)
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


def theme_group(box: int, label: str) -> int:
    grp = s.frame(box, layout="vertical", gap=10, sizing_h="fill", sizing_v="hug")
    s.text(label, grp, size=15, style="SemiBold", color=LIGHT["text-1"], font=AR)
    return grp


def build() -> Script:
    green = ACCENT_MATRIX["light"]["green"]

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
    s.text("Maktabati design tokens — parsed live from src/app.css, the file the "
           "app ships. Change a token there and this sheet follows.",
           head, size=15, color=LIGHT["text-2"], width=SHEET_W - 112)
    rule = s.frame(head, layout="horizontal", sizing_h="fill", height=4, fill=None)
    s.rect(rule, width=SHEET_W - 112, height=3, fill=green["accent"], radius=2)

    # ── 2. Surfaces, per theme ───────────────────────────────────────────────
    box = section(root, "الأسطح والحدود · Surfaces & borders",
                  "Four elevations and two border weights, in every theme.")
    for theme_name in ("light", "sepia", "dark"):
        label = {"light": "light · ورقي", "sepia": "sepia · تربيع", "dark": "dark · حبر"}[theme_name]
        grp = theme_group(box, label)
        theme = THEME_TOKENS[theme_name]
        swatch_row(grp, [(k, v) for k, v in theme.items() if k.startswith(("surface", "border"))])
        swatch_row(grp, [(k, v) for k, v in theme.items() if k.startswith("text")])
        swatch_row(grp, [(k, v) for k, v in theme.items() if k.startswith("star")])

    # ── 3. Accent matrix — six presets × three themes ────────────────────────
    box = section(root, "لون التمييز · Accent matrix",
                  "Six presets in every theme: accent, the hover step, the "
                  "foreground painted on the accent, and the soft pill with its "
                  "own ink. Dark accents are deliberately lighter — they are read "
                  "as text on dark surfaces — and flip the foreground to ink.")
    for theme_name in ("light", "sepia", "dark"):
        label = {"light": "light · ورقي", "sepia": "sepia · تربيع", "dark": "dark · حبر"}[theme_name]
        grp = theme_group(box, label)
        row = s.frame(grp, layout="horizontal", gap=16, sizing_h="fill",
                      sizing_v="hug", cross_align="min")
        for name, acc in ACCENT_MATRIX[theme_name].items():
            cell = s.frame(row, layout="vertical", gap=0, radius=14, clip=True,
                           stroke=LIGHT["border-1"], sizing_h="fill", sizing_v="hug")
            band = s.frame(cell, layout="vertical", padding=[14, 16, 14, 16],
                           fill=acc["accent"], sizing_h="fill", sizing_v="hug")
            s.text("مكتبتي  Aa", band, size=17, style="SemiBold",
                   color=acc["accent-fg"], font=AR)
            body = s.frame(cell, layout="vertical", gap=3, padding=[12, 16, 14, 16],
                           fill=acc["accent-soft"], sizing_h="fill", sizing_v="hug")
            s.text(name, body, size=15, style="SemiBold", color=acc["accent-soft-ink"])
            s.text(acc["accent"].upper(), body, size=12, color=acc["accent-soft-ink"], font=UI)
            meta = s.frame(body, layout="horizontal", gap=6, sizing_h="fill",
                           sizing_v="hug", cross_align="center")
            s.rect(meta, width=11, height=11, radius=3, fill=acc["accent-fg"],
                   stroke=LIGHT["border-2"])
            s.text(f"fg {acc['accent-fg'].upper()} · ink {acc['accent-soft-ink'].upper()}",
                   meta, size=10, color=acc["accent-soft-ink"], font=UI)

    # ── 4. Status colours, per theme ─────────────────────────────────────────
    box = section(root, "الدلالات · Status colours",
                  "Accent-independent, resolved per theme: the dark inks flip "
                  "lighter to hold 4.5:1, sepia overrides success and warning "
                  "only. Every pair below is pinned by `npm run lint:contrast`.")
    for theme_name in ("light", "sepia", "dark"):
        label = {"light": "light · ورقي", "sepia": "sepia · تربيع", "dark": "dark · حبر"}[theme_name]
        grp = theme_group(box, label)
        row = s.frame(grp, layout="horizontal", gap=16, sizing_h="fill",
                      sizing_v="hug", cross_align="min")
        theme = THEME_TOKENS[theme_name]
        status = STATUS_MATRIX[theme_name]
        for name in STATUS_NAMES:
            cell = s.frame(row, layout="vertical", gap=4, padding=[14, 16, 14, 16],
                           fill=status[f"{name}-soft"], radius=10,
                           stroke=LIGHT["border-1"], sizing_h="fill", sizing_v="hug")
            s.text(name, cell, size=15, style="SemiBold", color=status[name], font=UI)
            s.text(status[name].upper(), cell, size=12, color=status[name], font=UI)
        star = s.frame(row, layout="vertical", gap=4, padding=[14, 16, 14, 16],
                       fill=theme["star-empty"], radius=10, stroke=LIGHT["border-1"],
                       sizing_h="fill", sizing_v="hug")
        s.text("star", star, size=15, style="SemiBold", color=status["star-filled"], font=UI)
        s.text(f"★ {status['star-filled'].upper()} · ☆ {theme['star-empty'].upper()}",
               star, size=12, color=status["star-filled"], font=UI)

    # ── 5. Elevation ─────────────────────────────────────────────────────────
    box = section(root, "الظلال · Elevation",
                  "Four steps — --sh-1 for a card at rest through --sh-lift for a "
                  "cover under the cursor — rendered as the actual box-shadows "
                  "from src/app.css. Dark uses pure black at a higher alpha; "
                  "sepia inherits the light ramp.")
    for theme_name in ("light", "dark"):
        label = {"light": "light · ورقي", "dark": "dark · حبر"}[theme_name]
        grp = theme_group(box, label)
        panel = s.frame(grp, layout="horizontal", gap=24, padding=[28, 32, 28, 32],
                        fill=THEME_TOKENS[theme_name]["surface-0"], radius=14,
                        sizing_h="fill", sizing_v="hug", cross_align="min")
        for key, value in SHADOWS[theme_name].items():
            cell = s.frame(panel, layout="vertical", gap=8, sizing_h="fill",
                           sizing_v="hug", cross_align="center")
            card = s.frame(cell, layout="vertical", padding=[18, 20, 18, 20],
                           fill=THEME_TOKENS[theme_name]["surface-1"], radius=14,
                           stroke=THEME_TOKENS[theme_name]["border-1"],
                           sizing_h="fill", sizing_v="hug",
                           effects=[_shadow_effect(value)])
            s.text(key, card, size=15, style="SemiBold",
                   color=THEME_TOKENS[theme_name]["text-1"], font=UI)
            s.text(value, cell, size=11, color=THEME_TOKENS[theme_name]["text-3"],
                   font=UI, width=250, align="center")

    # ── 6. Type scale ────────────────────────────────────────────────────────
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

    # ── 7. Radii + spacing + motion ──────────────────────────────────────────
    box = section(root, "الزوايا والمسافات · Radii, spacing & motion",
                  "A radius ramp on an 8px rhythm (4/8/12/16/24/32/48/64), plus the "
                  "three durations and one easing curve every transition uses.")
    row = s.frame(box, layout="horizontal", gap=20, sizing_h="fill", sizing_v="hug")
    for label, px in RADII:
        cell = s.frame(row, layout="vertical", gap=8, sizing_h="hug", sizing_v="hug")
        s.rect(cell, width=96, height=64, fill=LIGHT["surface-2"], radius=min(px, 32),
               stroke=LIGHT["border-2"])
        s.text(f"{label} · {px}px", cell, size=13.5, color=LIGHT["text-2"])

    row = s.frame(box, layout="horizontal", gap=20, padding=[8, 0, 0, 0],
                  sizing_h="fill", sizing_v="hug", cross_align="max")
    for label, px in SPACING:
        cell = s.frame(row, layout="vertical", gap=8, sizing_h="hug", sizing_v="hug",
                       cross_align="center")
        s.rect(cell, width=max(px, 4), height=px, fill=green["accent"])
        s.text(f"{label} · {px}", cell, size=12, color=LIGHT["text-3"])

    motion = s.frame(box, layout="horizontal", gap=10, padding=[8, 0, 0, 0],
                     sizing_h="fill", sizing_v="hug", cross_align="center")
    s.text("الحركة · Motion", motion, size=13.5, color=LIGHT["text-2"], font=AR)
    for key, value in MOTION:
        if key.startswith("dur-") or key == "stagger":
            pill = s.frame(motion, layout="horizontal", padding=[5, 12, 5, 12],
                           fill=LIGHT["surface-2"], radius=999,
                           sizing_h="hug", sizing_v="hug")
            s.text(f"{key} {value}", pill, size=12, color=LIGHT["text-2"], font=UI)
        else:
            s.text(f"{key} {value}", motion, size=12, color=LIGHT["text-3"], font=UI)

    # ── 8. Components ────────────────────────────────────────────────────────
    box = section(root, "المكوّنات · Components",
                  "The base kit: buttons, field, chip, progress, rating, stat card.")

    row = s.frame(box, layout="horizontal", gap=12, sizing_h="fill", sizing_v="hug",
                  cross_align="center")
    for label, fill, fg, stroke in (
        ("أساسي · Primary", green["accent"], green["accent-fg"], None),
        ("ثانوي · Secondary", LIGHT["surface-1"], LIGHT["text-1"], LIGHT["border-2"]),
        ("شبح · Ghost", None, green["accent"], None),
        ("خطر · Danger", STATUS_MATRIX["light"]["danger"], "#ffffff", None),
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
    for name in ("green", "indigo"):
        acc = ACCENT_MATRIX["light"][name]
        chip = s.frame(row, layout="horizontal", padding=[6, 12, 6, 12],
                       fill=acc["accent-soft"], radius=999,
                       sizing_h="hug", sizing_v="hug")
        label = "قراءة الآن" if name == "green" else "أنهيت"
        s.text(label, chip, size=13.5, color=acc["accent-soft-ink"], font=AR)

    # progress
    prog = s.frame(box, layout="vertical", gap=8, sizing_h="fill", sizing_v="hug")
    s.text("62% · صفحة ٢٦٣ من ٤٢٤", prog, size=13.5, color=LIGHT["text-2"], font=AR)
    track = s.frame(prog, layout="horizontal", height=8, width=520, fill=LIGHT["surface-3"],
                    radius=999, clip=True, sizing_h="fixed", sizing_v="fixed")
    s.rect(track, width=322, height=8, fill=green["accent"], radius=999)

    # rating
    stars = s.frame(box, layout="horizontal", gap=4, sizing_h="hug", sizing_v="hug",
                    cross_align="center")
    s.text("★★★★☆  (٤)", stars, size=24, color=STATUS_MATRIX["light"]["star-filled"], font=AR)
    s.text("نجمة واحدة · star / star-empty (theme token)", stars, size=13.5,
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

    # ── 9. State matrix ──────────────────────────────────────────────────────
    box = section(root, "الحالات · Component states",
                  "The shipped state model (src/lib/components/*.svelte): hover "
                  "swaps to the hover step, focus draws a 2px accent outline at "
                  "2px offset and is never removed, disabled drops to 45% "
                  "opacity, loading disables the button and swaps in the "
                  "spinner, and a field's error border is danger at 60%.")

    def state_cell(parent: int, state: str, state_en: str, caption: str) -> int:
        cell = s.frame(parent, layout="vertical", gap=8, padding=[4, 0, 0, 0],
                       sizing_h="fill", sizing_v="hug", cross_align="center")
        s.text(state, cell, size=13.5, style="SemiBold", color=LIGHT["text-1"], font=AR)
        return cell

    def button_in(cell: int, *, fill, fg, stroke=None, opacity=None,
                  spin=False, focus=False) -> None:
        host = cell
        if focus:
            # outline: 2px accent, 2px offset — drawn as a ring around the button.
            host = s.frame(cell, layout="horizontal", padding=[2, 2, 2, 2], radius=12,
                           stroke=green["accent"], stroke_weight=2, fill=None,
                           sizing_h="hug", sizing_v="hug")
        btn = s.frame(host, layout="horizontal", gap=8, padding=[10, 18, 10, 18],
                      fill=fill, radius=8, stroke=stroke, opacity=opacity,
                      sizing_h="hug", sizing_v="hug", cross_align="center")
        if spin:
            s.ellipse(btn, width=15, height=15, fill=None, stroke=fg,
                      stroke_weight=1.5, stroke_dash=[14, 9])
        s.text("إضافة كتاب", btn, size=15, style="SemiBold", color=fg, font=AR)

    # Primary: default / hover / focus / disabled / loading
    row = s.frame(box, layout="horizontal", gap=24, sizing_h="fill", sizing_v="hug",
                  cross_align="min")
    c = state_cell(row, "أساسي", "primary", "")
    button_in(c, fill=green["accent"], fg=green["accent-fg"])
    s.text(green["accent"].upper(), c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "عند المرور", "hover", "")
    button_in(c, fill=ACCENT_MATRIX["light"]["green"]["accent-hover"], fg=green["accent-fg"])
    s.text(ACCENT_MATRIX["light"]["green"]["accent-hover"].upper(), c, size=11,
           color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "التركيز", "focus", "")
    button_in(c, fill=green["accent"], fg=green["accent-fg"], focus=True)
    s.text("outline 2px accent · offset 2px", c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "معطّل", "disabled", "")
    button_in(c, fill=green["accent"], fg=green["accent-fg"], opacity=0.45)
    s.text("opacity 45%", c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "يحمّل", "loading", "")
    button_in(c, fill=green["accent"], fg=green["accent-fg"], opacity=1, spin=True)
    s.text("disabled + spinner", c, size=11, color=LIGHT["text-3"], font=UI)

    # Secondary: default / hover / focus / disabled
    row = s.frame(box, layout="horizontal", gap=24, sizing_h="fill", sizing_v="hug",
                  cross_align="min")
    c = state_cell(row, "ثانوي", "secondary", "")
    button_in(c, fill=LIGHT["surface-1"], fg=LIGHT["text-1"], stroke=LIGHT["border-2"])
    s.text("surface-1 · border-2", c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "عند المرور", "hover", "")
    button_in(c, fill=LIGHT["surface-2"], fg=LIGHT["text-1"], stroke=LIGHT["border-2"])
    s.text("→ surface-2", c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "التركيز", "focus", "")
    button_in(c, fill=LIGHT["surface-1"], fg=LIGHT["text-1"], stroke=LIGHT["border-2"],
              focus=True)
    s.text("outline 2px accent · offset 2px", c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "معطّل", "disabled", "")
    button_in(c, fill=LIGHT["surface-1"], fg=LIGHT["text-1"], stroke=LIGHT["border-2"],
              opacity=0.45)
    s.text("opacity 45%", c, size=11, color=LIGHT["text-3"], font=UI)

    # Field: default / focus / error / disabled
    danger = STATUS_MATRIX["light"]["danger"]
    row = s.frame(box, layout="horizontal", gap=24, sizing_h="fill", sizing_v="hug",
                  cross_align="min")

    def field_in(cell: int, *, stroke, stroke_weight=1, opacity=None,
                 focus=False, error=None) -> None:
        host = cell
        if focus:
            host = s.frame(cell, layout="horizontal", padding=[2, 2, 2, 2], radius=12,
                           stroke=green["accent"], stroke_weight=2, fill=None,
                           sizing_h="hug", sizing_v="hug")
        box_ = s.frame(host, layout="horizontal", padding=[10, 14, 10, 14], gap=8,
                       fill=LIGHT["surface-1"], radius=8, stroke=stroke,
                       stroke_weight=stroke_weight, opacity=opacity, width=240,
                       sizing_h="fixed", sizing_v="hug", cross_align="center")
        s.text("ابحث في كل شيء…", box_, size=15, color=LIGHT["text-3"], font=AR)
        if error:
            s.text(error, cell, size=12, color=danger, font=AR)

    c = state_cell(row, "حقل", "field", "")
    field_in(c, stroke=LIGHT["border-2"])
    s.text("border-2", c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "التركيز", "focus", "")
    field_in(c, stroke=green["accent"], focus=True)
    s.text("border-accent + outline", c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "خطأ", "error", "")
    field_in(c, stroke=danger + "99")
    s.text("danger/60 · danger 12px", c, size=11, color=LIGHT["text-3"], font=UI)
    c = state_cell(row, "معطّل", "disabled", "")
    field_in(c, stroke=LIGHT["border-2"], opacity=0.45)
    s.text("opacity 45%", c, size=11, color=LIGHT["text-3"], font=UI)

    return s


if __name__ == "__main__":
    build()
    json.dump(s.steps, sys.stdout, ensure_ascii=False, indent=1)
    print()
