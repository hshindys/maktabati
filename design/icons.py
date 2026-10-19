#!/usr/bin/env python3
"""Turn the Lucide icons the shell uses into SVG paths FigCraft can draw.

`@lucide/svelte` stores each icon as `iconData.node`, a list of
`[tag, attrs]` primitives (rect / circle / path / line / polyline / polygon) in
a 24×24 box. FigCraft's `vector.create` wants a single path, so this converts
the primitives into one `d` string — Lucide draws with a 2px stroke and no
fill, which maps onto `stroke` + `fills: []` in `node.set`.

    from icons import paths
    paths()["library"]   # 'M16 7l-4-4-4 4M20 21a8 8 0 0 0-16 0' …
"""
from __future__ import annotations

import json
import re
from pathlib import Path

ICONS_DIR = Path(__file__).resolve().parents[1] / "node_modules/@lucide/svelte/dist/icons"

# Everything the shell and the three screens draw.
WANTED = [
    "layout-dashboard", "library", "library-big", "book-open", "sticky-note",
    "quote", "users", "store", "folder-tree", "hand-coins", "target",
    "arrow-left-right", "settings", "panel-left-close", "search", "languages",
    "sun", "moon", "lock", "minus", "square", "x", "grid-3x3", "list",
    "rows-3", "flame", "plus", "chevron-down", "arrow-left", "arrow-right", "star",
]


def _n(v: str | float) -> str:
    """Trim floats so the path stays short and parses as decimal."""
    f = float(v)
    return str(int(f)) if f == int(f) else f"{f:.4g}"


def _rounded_rect(a: dict) -> str:
    x, y = float(a.get("x", 0)), float(a.get("y", 0))
    w, h = float(a["width"]), float(a["height"])
    rx = min(float(a.get("rx", 0)), w / 2, h / 2)
    if rx <= 0:
        return f"M{_n(x)} {_n(y)}h{_n(w)}v{_n(h)}h{_n(-w)}Z"
    ry = min(float(a.get("ry", rx)), h / 2)
    return (
        f"M{_n(x + rx)} {_n(y)}"
        f"H{_n(x + w - rx)}A{_n(rx)} {_n(ry)} 0 0 1 {_n(x + w)} {_n(y + ry)}"
        f"V{_n(y + h - ry)}A{_n(rx)} {_n(ry)} 0 0 1 {_n(x + w - rx)} {_n(y + h)}"
        f"H{_n(x + rx)}A{_n(rx)} {_n(ry)} 0 0 1 {_n(x)} {_n(y + h - ry)}"
        f"V{_n(y + ry)}A{_n(rx)} {_n(ry)} 0 0 1 {_n(x + rx)} {_n(y)}Z"
    )


def _circle(a: dict) -> str:
    cx, cy, r = float(a["cx"]), float(a["cy"]), float(a["r"])
    return (
        f"M{_n(cx - r)} {_n(cy)}"
        f"A{_n(r)} {_n(r)} 0 1 0 {_n(cx + r)} {_n(cy)}"
        f"A{_n(r)} {_n(r)} 0 1 0 {_n(cx - r)} {_n(cy)}Z"
    )


def _ellipse(a: dict) -> str:
    cx, cy = float(a["cx"]), float(a["cy"])
    rx, ry = float(a["rx"]), float(a["ry"])
    return (
        f"M{_n(cx - rx)} {_n(cy)}"
        f"A{_n(rx)} {_n(ry)} 0 1 0 {_n(cx + rx)} {_n(cy)}"
        f"A{_n(rx)} {_n(ry)} 0 1 0 {_n(cx - rx)} {_n(cy)}Z"
    )


def _points(raw: str) -> str:
    nums = [float(v) for v in re.split(r"[\s,]+", raw.strip()) if v]
    pairs = list(zip(nums[0::2], nums[1::2]))
    if not pairs:
        return ""
    head, rest = pairs[0], pairs[1:]
    body = "".join(f"L{_n(x)} {_n(y)}" for x, y in rest)
    return f"M{_n(head[0])} {_n(head[1])}{body}"


def _shape(tag: str, a: dict) -> str:
    if tag == "rect":
        return _rounded_rect(a)
    if tag == "circle":
        return _circle(a)
    if tag == "ellipse":
        return _ellipse(a)
    if tag == "path":
        return a.get("d", "")
    if tag == "line":
        return f"M{_n(a['x1'])} {_n(a['y1'])}L{_n(a['x2'])} {_n(a['y2'])}"
    if tag == "polyline":
        return _points(a["points"])
    if tag == "polygon":
        return _points(a["points"]) + "Z"
    return ""


def _read(name: str) -> list[str]:
    """Parse `const iconData = {...}` out of one Lucide component."""
    src = (ICONS_DIR / f"{name}.svelte").read_text(encoding="utf-8")
    m = re.search(r"const iconData = (\{.*?\});", src, re.S)
    if not m:
        raise SystemExit(f"iconData not found in {name}.svelte")
    data = json.loads(m.group(1))
    out: list[str] = []
    for tag, attrs in data["node"]:
        d = _shape(tag, attrs).lstrip()
        if not d:
            continue
        # Every primitive is authored against its own (0, 0) origin. The shell
        # draws them as *separate* paths, but FigCraft takes one `d` — so a
        # relative leading command (`m6 6 12 12` in `x`) would otherwise be
        # measured from the previous primitive's end point. At the origin the
        # relative and absolute forms are identical, so promoting it is exact.
        if d[0].islower():
            d = d[0].upper() + d[1:]
        out.append(d)
    return out


def paths(names: list[str] | None = None) -> dict[str, list[str]]:
    """`{icon name: [subpath, …]}` — each subpath drawn as one vector layer."""
    wanted = names if names is not None else WANTED
    return {name: _read(name) for name in wanted}


if __name__ == "__main__":
    p = paths()
    print(f"{len(p)} icons")
    for k, v in list(p.items())[:3]:
        print(f"  {k}: {len(v)} subpath(s) — {v[0][:70]}…")
