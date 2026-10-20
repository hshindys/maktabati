#!/usr/bin/env python3
"""Verify gen_design.py parses src/app.css to the values the app actually ships."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "design"))
import gen_design as g  # noqa: E402

fails = []


def check(label, got, want):
    ok = got == want
    print(f"{'ok ' if ok else 'FAIL'} {label}: {got!r}" + ("" if ok else f"  (want {want!r})"))
    if not ok:
        fails.append(label)


check("light text-2", g.LIGHT["text-2"], "#5a5344")
check("light text-3", g.LIGHT["text-3"], "#6e6554")
check("dark text-3", g.DARK["text-3"], "#979081")
check("sepia text-2", g.SEPIA["text-2"], "#57492e")
check("sepia text-3", g.SEPIA["text-3"], "#695a3e")
check("light surface-0", g.LIGHT["surface-0"], "#fbf8f1")
check("dark surface-0", g.DARK["surface-0"], "#12110f")

check("accents per theme", {t: len(g.ACCENT_MATRIX[t]) for t in ("light", "sepia", "dark")},
      {"light": 6, "sepia": 6, "dark": 6})
check("light terracotta", g.ACCENT_MATRIX["light"]["terracotta"]["accent"], "#a64821")
check("light gold", g.ACCENT_MATRIX["light"]["gold"]["accent"], "#7f5f0f")
check("sepia gold (override)", g.ACCENT_MATRIX["sepia"]["gold"]["accent"], "#755600")
check("sepia indigo (inherit)", g.ACCENT_MATRIX["sepia"]["indigo"]["accent"], "#3b4cca")
check("sepia green hover", g.ACCENT_MATRIX["sepia"]["green"]["accent-hover"], "#19583d")
check("dark green accent", g.ACCENT_MATRIX["dark"]["green"]["accent"], "#639f80")
check("dark green fg", g.ACCENT_MATRIX["dark"]["green"]["accent-fg"], "#12110f")
check("dark green soft-ink", g.ACCENT_MATRIX["dark"]["green"]["accent-soft-ink"], "#97d4b3")

check("light success", g.STATUS_MATRIX["light"]["success"], "#2d784d")
check("dark success", g.STATUS_MATRIX["dark"]["success"], "#87d1a2")
check("dark success-soft", g.STATUS_MATRIX["dark"]["success-soft"], "#183b26")
check("sepia success", g.STATUS_MATRIX["sepia"]["success"], "#287449")
check("sepia warning", g.STATUS_MATRIX["sepia"]["warning"], "#8c5e00")
check("sepia danger (inherit)", g.STATUS_MATRIX["sepia"]["danger"], "#a93226")
check("star-filled", g.STATUS_MATRIX["light"]["star-filled"], "#c99a17")

check("type scale labels", [t[0] for t in g.TYPE_SCALE],
      ["3xl · 38", "2xl · 30", "xl · 24", "lg · 20", "md · 17",
       "base · 15", "sm · 13.5", "xs · 12"])
check("radii", g.RADII, [("r-sm", 6), ("r-md", 10), ("r-lg", 14), ("r-xl", 20), ("r-full", 999)])
check("spacing", g.SPACING, [("sp-1", 4), ("sp-2", 8), ("sp-3", 12), ("sp-4", 16),
                             ("sp-5", 24), ("sp-6", 32), ("sp-7", 48), ("sp-8", 64)])
check("motion keys", [k for k, _ in g.MOTION],
      ["dur-fast", "dur-base", "dur-slow", "ease", "stagger"])
check("shadow keys", list(g.SHADOWS["light"]),
      ["sh-1", "sh-2", "sh-3", "sh-lift"])
check("sepia inherits light shadows", g.SHADOWS["sepia"], g.SHADOWS["light"])
check("dark sh-1 alpha", g._shadow_effect(g.SHADOWS["dark"]["sh-1"])["color"], "#00000066")

# gen_frames.py's import contract must keep holding.
check("ACCENTS green tuple", g.ACCENTS["green"], ("#2f6b4f", "#e3efe8", "#24523c"))
check("SEMANTIC names", [n for n, _, _ in g.SEMANTIC],
      ["success", "warning", "danger", "info", "star"])
check("SEMANTIC star", g.SEMANTIC[-1], ("star", "#c99a17", "#d8d0be"))

print()
print(f"{len(fails)} failures" if fails else "all token checks pass")
sys.exit(1 if fails else 0)
