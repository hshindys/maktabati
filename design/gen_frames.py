#!/usr/bin/env python3
"""The three screens Maktabati is judged on, as FigCraft frames.

Dashboard, Library and the per-book page — drawn at the shipped window size
(1320×880, `tauri.conf.json`) in the default locale (Arabic). The chrome, tokens
and copy come from the code that already ships, so this file is a faithful
picture of the product rather than a parallel invention:

**RTL mirroring.** `dir="rtl"` sits on `<html>` (index.html), so *every* flex
row in the shell reverses: the first child of a DOM element lands on the RIGHT.
FigCraft has no writing direction, so each row below is written in **visual
order, left to right** — i.e. exactly backwards from the DOM in
`src/lib/components`. Vertical containers are told the same thing with
`cross_align="max"` (their children hug the right edge), horizontal ones that
hold only hug children with `main_align="max"`.

    python3 design/gen_frames.py > design/frames.json
    figcraft-cli run design/frames.json --in design/design-system.figcraft \
        --out design/maktabati.figcraft
    figcraft-cli export design/maktabati.figcraft -o design/export/ --scale 2
"""
from __future__ import annotations

import json
import sys

from gen_design import ACCENTS, AR, LIGHT, Script, UI
from icons import paths

# ── Shell constants (src/lib/components/*.svelte, tauri.conf.json) ───────────
WIN_W, WIN_H = 1320, 880
TITLEBAR_H = 44          # h-11
RAIL = 240               # --drawer: 15rem
GREEN = ACCENTS["green"]
STAR = "#c99a17"

# Visual (left→right) — the DOM order is the reverse of each of these.
NAV_PRIMARY = [
    ("layout-dashboard", "لوحة التحكم"),
    ("library", "المكتبة"),
    ("book-open", "أقرأ الآن"),
]
NAV_SECONDARY = [
    ("sticky-note", "الملاحظات"),
    ("quote", "الاقتباسات"),
    ("users", "المؤلفون"),
    ("store", "الناشرون"),
    ("folder-tree", "التصنيفات"),
    ("hand-coins", "الإعارات"),
    ("target", "الأهداف"),
    ("arrow-left-right", "استيراد / تصدير"),
    ("settings", "الإعدادات"),
]

# CoverPlaceholder's six bookbinding tones.
COVERS = [
    ("#F2EDE1", "#2F6B4F", "#1D4231"),
    ("#F6EDF3", "#7A3B69", "#4E2642"),
    ("#F7F1DF", "#8A6A1F", "#564012"),
    ("#E9F2F6", "#1F6A8A", "#13485E"),
    ("#F8ECE4", "#B4552F", "#6E3419"),
    ("#EBEDF9", "#3B4CCA", "#26307F"),
]

ICONS = paths()
s = Script()

# Lucide draws at 1.75 inside a 24px box; the shell renders them at 18px
# (size-4.5), so the stroke that lands on screen is 1.75 × 18/24 ≈ 1.35.
ICON_STROKE = 1.35

COL_W = 193              # Library column: (content − padding − 4 gaps) ÷ 5


# ── Primitives ───────────────────────────────────────────────────────────────

def v(parent=None, **kw) -> int:
    return s.frame(parent, layout="vertical", **kw)


def h(parent=None, **kw) -> int:
    return s.frame(parent, layout="horizontal", **kw)


def plain(parent=None, **kw) -> int:
    """A frame with no auto layout — children land where `x`/`y` put them."""
    return s.frame(parent, **kw)


def t(text: str, parent=None, **kw) -> int:
    return s.text(text, parent, **kw)


def fill_text(parent, text: str, **kw) -> int:
    """Text that eats the rest of the row — `flex-1` in the shell. RTL then
    hugs it to the right edge of that space, next to what follows."""
    box = h(parent, sizing_h="fill", sizing_v="hug", cross_align="center",
            main_align="max")
    t(text, box, **kw)
    return box


def icon(parent, name: str, *, size=18, color=LIGHT["text-2"],
         stroke=ICON_STROKE) -> int:
    """A Lucide glyph as a single stroked vector layer."""
    return s.add("node.create", {
        "type": "vector",
        "name": name,
        "parent": {"$step": parent},
        "path": " ".join(ICONS[name]),
        "props": {
            "width": size,
            "height": size,
            "fills": [],
            "stroke": color,
            "strokeWeight": stroke,
            "strokeCap": "round",
            "strokeJoin": "round",
        },
    })


def rule(parent) -> int:
    """A 1px `border-*` line. The shell gets it from CSS logical borders; a
    design file has no such property, so it is drawn explicitly — and only ever
    inside a *vertical* container, where it spans the width instead of taking a
    slot beside the content."""
    return s.rect(parent, width=100, height=1, fill=LIGHT["border-1"],
                  sizing_h="fill")


def button(parent, label: str, *, variant="secondary", size="md", ico=None) -> int:
    if variant == "primary":
        fill, fg, stroke = GREEN[0], GREEN[1], None
    elif variant == "ghost":
        fill, fg, stroke = None, GREEN[0], None
    else:
        fill, fg, stroke = LIGHT["surface-1"], LIGHT["text-1"], LIGHT["border-2"]
    pad_y, pad_x, fs = (10, 18, 15) if size == "md" else (6, 12, 13.5)
    b = h(parent, gap=8, padding=[pad_y, pad_x, pad_y, pad_x], fill=fill, radius=8,
          stroke=stroke, sizing_h="hug", sizing_v="hug", cross_align="center")
    # DOM is [iconStart, children] → RTL puts the icon on the right.
    t(label, b, size=fs, style="SemiBold", color=fg, font=AR)
    if ico:
        icon(b, ico, size=16, color=fg)
    return b


def chip(parent, label: str, *, fill=None, fg=LIGHT["text-2"], border=None,
         font=AR) -> int:
    c = h(parent, padding=[5, 11, 5, 11], fill=fill, radius=999, stroke=border,
          sizing_h="hug", sizing_v="hug", cross_align="center")
    t(label, c, size=13.5, color=fg, font=font)
    return c


def progress(parent, pct: float, *, width, height=8) -> int:
    """ProgressBar.svelte — in RTL the fill grows from the right edge, which
    here means the empty part is laid down first."""
    track = h(parent, width=width, height=height, sizing_h="fixed",
              sizing_v="fixed", fill=LIGHT["surface-3"], radius=999, clip=True,
              gap=0)
    s.rect(track, width=width - round(width * pct), height=height, fill=[])
    s.rect(track, width=round(width * pct), height=height, fill=GREEN[0],
           radius=999)
    return track


# ── Book cover (mirrors CoverPlaceholder.svelte exactly) ─────────────────────

def cover(parent, w: int, title: str, author: str, idx: int) -> int:
    page, cloth, ink = COVERS[idx % len(COVERS)]
    height = round(w * 1.5)
    c = plain(parent, width=w, height=height, fill=page, radius=6, clip=True,
              sizing_h="fixed", sizing_v="fixed")

    spine = round(w * 0.09)
    # `start-0` in an RTL document is the right edge — the spine follows the
    # reading direction, which is exactly why this screen is drawn in Arabic.
    s.rect(c, x=w - spine, y=0, width=spine, height=height, fill=cloth)
    s.rect(c, x=w - spine - 1, y=0, width=1, height=height, fill=cloth)

    inset = round(w * 0.18)
    s.rect(c, x=inset, y=round(height * 0.26), width=w - 2 * inset, height=1,
           fill=cloth)

    title_size = max(11, round(height * 0.072))
    title_y = round(height * 0.34)
    s.add("text.create", {
        "parent": {"$step": c},
        "x": inset, "y": title_y, "width": w - 2 * inset,
        "text": title,
        "props": {
            "fontFamily": AR, "fontStyle": "SemiBold", "fontSize": title_size,
            "lineHeight": "145%", "textAlign": "center", "textAutoResize": "height",
            "fills": [ink],
        },
    })
    s.add("text.create", {
        "parent": {"$step": c},
        "x": inset, "y": title_y + round(title_size * 1.45 * 3) + 8,
        "width": w - 2 * inset,
        "text": author,
        "props": {
            "fontFamily": AR, "fontSize": max(9, round(height * 0.05)),
            "textAlign": "center", "textAutoResize": "height", "fills": [ink],
            "opacity": 0.75,
        },
    })
    return c


# ── Shell ────────────────────────────────────────────────────────────────────

def nav_item(parent, glyph: str, label: str, *, active=False, muted=False) -> int:
    fg = GREEN[2] if active else (LIGHT["text-3"] if muted else LIGHT["text-2"])
    it = h(parent, gap=12, padding=[8, 12, 8, 12], radius=10,
           fill=GREEN[1] if active else None, sizing_h="fill", sizing_v="hug",
           cross_align="center")
    # DOM is [Icon, span] → RTL puts the glyph on the right.
    t(label, it, size=13.5 if muted else 15, color=fg, font=AR,
      style="SemiBold" if active else "Regular")
    icon(it, glyph, size=18, color=fg)
    return it


def shell(frame_name: str, active: str, x: int = 0) -> int:
    """Title bar + rail + main, in RTL order. Returns the `<main>` container."""
    root = s.frame(None, name=frame_name, layout="vertical", gap=0, x=x,
                   width=WIN_W, height=WIN_H, sizing_h="fixed", sizing_v="fixed",
                   fill=LIGHT["surface-0"], clip=True)

    # ── Title bar. Visual left→right: window buttons, the three toggles,
    #    search (optically centred), then the mark at the right edge.
    tb = v(root, height=TITLEBAR_H, sizing_h="fill", sizing_v="fixed",
           fill=LIGHT["surface-2"], gap=0)
    row = h(tb, height=TITLEBAR_H - 1, sizing_h="fill", sizing_v="fixed",
            cross_align="center", gap=0, padding=[0, 8, 0, 12])

    left = h(row, sizing_h="fill", main_align="min", cross_align="center", gap=2)
    win = h(left, gap=0, sizing_h="hug", sizing_v="hug", cross_align="center")
    # DOM [min, max, close] → visual [close, max, min].
    for glyph in ("x", "square", "minus"):
        b = h(win, width=44, height=32, main_align="center", cross_align="center",
              radius=6)
        icon(b, glyph, size=16, color=LIGHT["text-2"])
    h(left, width=8, sizing_h="fixed")   # ms-2
    # DOM [language, theme, lock] → visual [lock, theme, language].
    ctrl = h(left, gap=2, sizing_h="hug", sizing_v="hug", cross_align="center")
    for glyph in ("lock", "sun", "languages"):
        b = h(ctrl, width=32, height=32, main_align="center", cross_align="center",
              radius=6)
        icon(b, glyph, size=16, color=LIGHT["text-2"])

    sw = h(row, width=544, height=40, sizing_h="fixed", sizing_v="fixed",
           padding=[0, 12, 0, 12], cross_align="center")
    search = h(sw, height=28, sizing_h="fill", sizing_v="fixed",
               padding=[0, 12, 0, 12], gap=8, radius=10, fill=LIGHT["surface-1"],
               stroke=LIGHT["border-1"], cross_align="center")
    # DOM [icon, label, hint] → visual [hint, label, icon].
    kbd = h(search, padding=[2, 6, 2, 6], radius=6, fill=LIGHT["surface-2"],
            stroke=LIGHT["border-1"], sizing_h="hug", sizing_v="hug")
    t("Ctrl+K", kbd, size=10, color=LIGHT["text-3"], font=UI, style="Medium")
    fill_text(search, "ابحث في كل شيء", size=13.5, color=LIGHT["text-3"], font=AR)
    icon(search, "search", size=14, color=LIGHT["text-3"])

    right = h(row, sizing_h="fill", main_align="max", cross_align="center", gap=8)
    t("مكتبتي", right, size=15, style="SemiBold", color=LIGHT["text-1"], font=AR)
    mark = h(right, width=24, height=24, sizing_h="fixed", sizing_v="fixed",
             main_align="center", cross_align="center", radius=6, fill=GREEN[0])
    t("م", mark, size=13, style="Bold", color=GREEN[1], font=AR)
    rule(tb)

    # ── Body: main first (visual left), rail second (visual right).
    body = h(root, sizing_h="fill", sizing_v="fill", gap=0)
    main = v(body, name="main", sizing_h="fill", sizing_v="fill", gap=0)

    rail = h(body, name="sidebar", width=RAIL, sizing_h="fixed", sizing_v="fill",
             fill=LIGHT["surface-1"], gap=0)
    s.rect(rail, width=1, height=100, fill=LIGHT["border-1"], sizing_v="fill")
    inner = v(rail, sizing_h="fill", sizing_v="fill", padding=8, gap=4)

    items = v(inner, sizing_h="fill", sizing_v="fill", gap=4)
    for glyph, label in NAV_PRIMARY:
        nav_item(items, glyph, label, active=(glyph == active))
    rule(items)
    for glyph, label in NAV_SECONDARY:
        nav_item(items, glyph, label, active=(glyph == active))

    foot = v(inner, sizing_h="fill", sizing_v="hug", gap=4, padding=[4, 0, 0, 0])
    rule(foot)
    nav_item(foot, "library-big", "معرض المكوّنات", muted=True)
    nav_item(foot, "panel-left-close", "طيّ الشريط الجانبي", muted=True)

    return main


# ── Screen 1 · Dashboard ─────────────────────────────────────────────────────

def dashboard(main: int) -> None:
    root = v(main, name="content", gap=32, padding=[32, 40, 40, 40],
             sizing_h="fill", sizing_v="hug", cross_align="max")

    # DOM [greeting, action] → the action leads visually.
    head = h(root, sizing_h="fill", sizing_v="hug", main_align="spaceBetween",
             cross_align="max")
    button(head, "المكتبة", variant="primary")
    g = v(head, gap=4, sizing_h="hug", sizing_v="hug", cross_align="max")
    t("مرحباً حاتم", g, size=30, style="Bold", color=LIGHT["text-1"], font=AR)
    t("السبت ١٥ ربيع الآخر ١٤٤٨ · 2026-10-18", g, size=15, color=LIGHT["text-2"],
      font=AR)

    # DOM reads total → reading → pages → goal, so the goal is leftmost here.
    kpis = [
        ("flame", "هدف", "12", "84 منتهية · 2026", "success", 12 / 30),
        ("target", "صفحة هذا العام", "18,302", "24 تصنيفاً", "default", None),
        ("book-open", "أقرأها الآن", "47", "310 لم أبدأها", "accent", None),
        ("library", "الإجمالي", "1,284", "كتاب · 96 مؤلفاً", "default", None),
    ]
    # A fixed row height rather than `sizing_v="fill"` on the row: the row's
    # cross-axis size is what the cards stretch *to*, and a hug parent cannot
    # derive it from fill children. 200px is the natural height of the card
    # that carries a progress bar — the tallest — so the rest stretch to match,
    # exactly as `items-stretch` does in the shipped KpiCard row.
    row = h(root, sizing_h="fill", sizing_v="fixed", height=200, gap=16)
    for glyph, label, value, caption, tone, pct in kpis:
        card = v(row, gap=8, padding=24, radius=14, fill=LIGHT["surface-1"],
                 stroke=LIGHT["border-1"], sizing_h="fill", sizing_v="fill",
                 cross_align="max")
        # DOM [label, icon] → the label sits at the right.
        line = h(card, sizing_h="fill", sizing_v="hug", main_align="spaceBetween",
                 cross_align="center")
        icon(line, glyph, size=18, color=LIGHT["text-3"])
        t(label, line, size=13.5, color=LIGHT["text-2"], font=AR)
        tone_fg = {"default": LIGHT["text-1"], "accent": GREEN[0],
                   "success": "#2f7a4f"}[tone]
        t(value, card, size=38, style="Bold", color=tone_fg, font=UI)
        t(caption, card, size=12, color=LIGHT["text-3"], font=AR)
        if pct is not None:
            progress(card, pct, width=190, height=6)
            t("12 من 30", card, size=12, color=LIGHT["text-3"], font=AR)

    cols = h(root, sizing_h="fill", sizing_v="fixed", height=225, gap=16)

    # Languages comes second in the DOM, therefore first — leftmost — here.
    langs = v(cols, gap=14, padding=24, radius=14, fill=LIGHT["surface-1"],
              stroke=LIGHT["border-1"], sizing_h="fill", sizing_v="fill",
              cross_align="max")
    t("اللغات", langs, size=17, style="SemiBold", color=LIGHT["text-1"], font=AR)
    for code, num in [("AR", "1,102"), ("EN", "174"), ("FR", "8")]:
        r = h(langs, sizing_h="fill", sizing_v="hug", main_align="spaceBetween",
              cross_align="center")
        t(num, r, size=15, color=LIGHT["text-1"], font=UI)
        b = h(r, padding=[3, 8, 3, 8], radius=6, fill=LIGHT["surface-2"],
              sizing_h="hug", sizing_v="hug")
        t(code, b, size=13.5, color=LIGHT["text-2"], font=UI, style="Medium")

    brk = v(cols, gap=16, padding=24, radius=14, fill=LIGHT["surface-1"],
            stroke=LIGHT["border-1"], sizing_h="fill", sizing_v="fill",
            cross_align="max")
    t("في مكتبتك", brk, size=17, style="SemiBold", color=LIGHT["text-1"], font=AR)
    for pair in [((("الملاحظات", "48"), ("الاقتباسات", "312"))),
                 ((("الرفوف", "9"), ("الناشرون", "57")))]:
        r = h(brk, sizing_h="fill", sizing_v="hug", gap=16)
        for lbl, num in pair:
            cell = v(r, gap=4, sizing_h="fill", sizing_v="hug", cross_align="max")
            t(lbl, cell, size=13.5, color=LIGHT["text-3"], font=AR)
            t(num, cell, size=24, style="SemiBold", color=LIGHT["text-1"], font=UI)

    # Recently added — first item at the right edge.
    recent = v(root, gap=12, sizing_h="fill", sizing_v="hug", cross_align="max")
    t("أضيف مؤخراً", recent, size=17, style="SemiBold", color=LIGHT["text-1"],
      font=AR)
    strip = h(recent, sizing_h="fill", sizing_v="hug", gap=16, main_align="max")
    books = [
        ("ذاكرة الجسد", "أحلام مستغانمي"),
        ("حديث الصباح", "نجيب محفوظ"),
        ("عصير الكتب", "عبد الوهاب عيسى"),
        ("الخيميائي", "باولو كويلو"),
        ("تاريخ الطبري", "الطبري"),
        ("مقدمة في الحضارة", "أحمد فكري"),
    ]
    for i, (title, author) in enumerate(books):
        cover(strip, 120, title, author, i)


# ── Screen 2 · Library ───────────────────────────────────────────────────────

BOOKS = [
    ("مقدمة في الحضارة الإسلامية", "أحمد فكري"),
    ("تاريخ الطبري", "محمد بن جرير الطبري"),
    ("الخيميائي", "باولو كويلو"),
    ("The Pragmatic Programmer", "Andrew Hunt"),
    ("عصير الكتب", "عبد الوهاب عيسى"),
    ("حديث الصباح والمساء", "نجيب محفوظ"),
    ("Clean Code", "Robert C. Martin"),
    ("ذاكرة الجسد", "أحلام مستغانمي"),
    ("The Hobbit", "J.R.R. Tolkien"),
    ("مقدمات في علوم الحديث", "الشاطبي"),
]


def library(main: int) -> None:
    wrap = v(main, sizing_h="fill", sizing_v="hug", cross_align="max")
    bar_ = h(wrap, gap=12, padding=[12, 24, 12, 24], cross_align="center",
             sizing_h="fill", sizing_v="hug")
    # DOM [title, count, ms-auto tools] → tools lead visually.
    tools = h(bar_, gap=12, sizing_h="fill", sizing_v="hug", main_align="min",
              cross_align="center")
    button(tools, "إضافة", variant="primary", size="sm", ico="plus")
    # DOM [grid, list, table] → the selected grid mode ends up on the right.
    modes = h(tools, gap=0, padding=2, radius=10, fill=LIGHT["surface-1"],
              stroke=LIGHT["border-1"], sizing_h="hug", sizing_v="hug")
    for i, glyph in enumerate(("rows-3", "list", "grid-3x3")):
        on = i == 2
        m = h(modes, width=28, height=28, sizing_h="fixed", sizing_v="fixed",
              main_align="center", cross_align="center", radius=6,
              fill=GREEN[0] if on else None)
        icon(m, glyph, size=16, color=GREEN[1] if on else LIGHT["text-2"])
    sort = h(tools, gap=8, sizing_h="hug", sizing_v="hug", cross_align="center")
    sel = h(sort, gap=8, padding=[6, 10, 6, 10], radius=8, fill=LIGHT["surface-1"],
            stroke=LIGHT["border-2"], sizing_h="hug", sizing_v="hug",
            cross_align="center")
    icon(sel, "chevron-down", size=14, color=LIGHT["text-3"])
    t("الأحدث إضافة", sel, size=13.5, color=LIGHT["text-1"], font=AR)
    t("ترتيب", sort, size=13.5, color=LIGHT["text-3"], font=AR)

    cnt = h(bar_, padding=[3, 10, 3, 10], radius=999, fill=LIGHT["surface-2"],
            sizing_h="hug", sizing_v="hug")
    t("1,284 كتاباً", cnt, size=13.5, color=LIGHT["text-2"], font=AR)
    t("المكتبة", bar_, size=20, style="SemiBold", color=LIGHT["text-1"], font=AR)
    rule(wrap)

    # Filter row — first chip at the right edge.
    filters = h(main, gap=8, padding=[14, 24, 0, 24], sizing_h="fill",
                sizing_v="hug", cross_align="center", main_align="max")
    t("تصفية", filters, size=13.5, color=LIGHT["text-3"], font=AR)
    icon(filters, "chevron-down", size=14, color=LIGHT["text-3"])
    for label, n in [("متوقفة", "12"), ("لم أبدأها", "310"), ("منتهية", "84"),
                     ("أقرأها", "47")]:
        chip(filters, f"{label} {n}", fill=LIGHT["surface-2"],
             fg=LIGHT["text-2"])
    chip(filters, "الكل", fill=GREEN[1], fg=GREEN[2])

    # Grid — five across, first book of each row at the right.
    grid = v(main, gap=24, padding=[20, 24, 24, 24], sizing_h="fill",
             sizing_v="hug", cross_align="max")
    for start in range(0, len(BOOKS), 5):
        row_ = h(grid, sizing_h="fill", sizing_v="hug", gap=16,
                 main_align="max")
        for i, (title, author) in enumerate(reversed(BOOKS[start:start + 5])):
            card = v(row_, gap=8, sizing_h="fill", sizing_v="hug",
                     cross_align="max")
            cover(card, COL_W, title, author, start + i)
            # Fixed-width boxes need `align="right"`: FigCraft lays text out in
            # its own frame's direction, which has no `dir` to inherit from.
            t(title, card, width=COL_W, size=15, style="SemiBold",
              color=LIGHT["text-1"], font=AR, line_height="145%", align="right")
            t(author, card, width=COL_W, size=13.5, color=LIGHT["text-3"], font=AR,
              align="right")
            meta = h(card, sizing_h="fill", sizing_v="hug", gap=8,
                     cross_align="center")
            if (start + i) % 3 == 0:
                progress(meta, 0.62, width=64, height=5)
            t("★★★★☆", meta, size=13.5, color=STAR, font=AR)


# ── Screen 3 · Book page ─────────────────────────────────────────────────────

TABS = ["نظرة عامة", "ملاحظات", "اقتباسات", "سجل القراءة", "تقييم", "ملفات",
        "مرتبط"]


def book_page(main: int) -> None:
    root = v(main, gap=22, padding=[20, 32, 24, 32], sizing_h="fill",
             sizing_v="hug", cross_align="max")

    # DOM [icon, label] → RTL puts the arrow on the right, pointing right.
    back = h(root, gap=8, sizing_h="hug", sizing_v="hug", cross_align="center")
    t("رجوع", back, size=15, color=LIGHT["text-2"], font=AR)
    icon(back, "arrow-right", size=16, color=LIGHT["text-2"])

    hero = h(root, sizing_h="fill", sizing_v="hug", gap=28, cross_align="min")
    info = v(hero, gap=16, sizing_h="fill", sizing_v="hug", cross_align="max")
    t("مقدمة في الحضارة الإسلامية", info, size=30, style="Bold",
      color=LIGHT["text-1"], font=AR, width=660, line_height="130%",
      align="right")

    # DOM [author, language chip] → the chip leads.
    by = h(info, gap=12, sizing_h="hug", sizing_v="hug", cross_align="center")
    chip(by, "عربي", fill=LIGHT["surface-2"], fg=LIGHT["text-2"])
    t("أحمد فكري · ترجمة: محمد عبدالله", by, size=17, color=LIGHT["text-2"],
      font=AR)

    t("دار المعارف · ١٩٧٨ · ٤٢٤ صفحة · طبعة أولى", info, size=15,
      color=LIGHT["text-3"], font=AR)

    prog = v(info, gap=8, sizing_h="hug", sizing_v="hug", cross_align="max")
    progress(prog, 0.62, width=560, height=8)
    t("62% · صفحة ٢٦٣ من ٤٢٤", prog, size=13.5, color=LIGHT["text-2"], font=AR)

    tags = h(info, gap=8, sizing_h="hug", sizing_v="hug", cross_align="center")
    chip(tags, "#إسلام", fill=LIGHT["surface-2"], fg=LIGHT["text-2"])
    chip(tags, "#حضارة", fill=LIGHT["surface-2"], fg=LIGHT["text-2"])
    chip(tags, "#تاريخ", fill=LIGHT["surface-2"], fg=LIGHT["text-2"])
    t("★★★★☆  (٤)", tags, size=17, color=STAR, font=AR)

    acts = h(info, gap=12, sizing_h="hug", sizing_v="hug", cross_align="center")
    more = h(acts, width=40, height=40, sizing_h="fixed", sizing_v="fixed",
             main_align="center", cross_align="center", radius=8,
             fill=LIGHT["surface-1"], stroke=LIGHT["border-2"])
    t("⋯", more, size=17, color=LIGHT["text-2"], font=UI)
    button(acts, "ملاحظات", variant="secondary")
    button(acts, "متابعة القراءة", variant="primary")

    cover(hero, 320, "مقدمة في الحضارة الإسلامية", "أحمد فكري", 0)

    # Tabs — first one at the right, carrying the accent rule.
    tabs = h(root, sizing_h="fill", sizing_v="hug", gap=4, cross_align="max",
             main_align="max")
    for i, label in enumerate(reversed(TABS)):
        on = i == len(TABS) - 1
        tb = v(tabs, gap=8, padding=[8, 14, 8, 14], sizing_h="hug",
               sizing_v="hug", cross_align="center")
        t(label, tb, size=15, color=LIGHT["text-1"] if on else LIGHT["text-2"],
          font=AR, style="SemiBold" if on else "Regular")
        s.rect(tb, width=40, height=2, fill=GREEN[0] if on else [],
               sizing_h="fill", radius=2)
    rule(root)

    # DOM [content, aside] → the aside ends up on the left.
    cols = h(root, sizing_h="fill", sizing_v="hug", gap=24)
    side = v(cols, width=280, sizing_h="fixed", sizing_v="hug", gap=16, padding=20,
             radius=14, fill=LIGHT["surface-1"], stroke=LIGHT["border-1"],
             cross_align="max")
    t("الحالة", side, size=13.5, color=LIGHT["text-3"], font=AR)
    stat = h(side, gap=8, padding=[8, 12, 8, 12], radius=8, fill=GREEN[1],
             sizing_h="fill", sizing_v="hug", cross_align="center")
    t("أقرأها", stat, size=15, style="SemiBold", color=GREEN[2], font=AR)
    icon(stat, "book-open", size=16, color=GREEN[2])
    t("التقييم", side, size=13.5, color=LIGHT["text-3"], font=AR)
    t("★★★★☆", side, size=24, color=STAR, font=AR)
    t("أضيف في", side, size=13.5, color=LIGHT["text-3"], font=AR)
    t("2024-03-11", side, size=15, color=LIGHT["text-1"], font=UI)
    t("مصدر الاقتناء", side, size=13.5, color=LIGHT["text-3"], font=AR)
    t("مكتبة المدينة · ٣٥ ر.س", side, size=15, color=LIGHT["text-1"], font=AR)

    body_ = v(cols, gap=16, sizing_h="fill", sizing_v="hug", cross_align="max")
    t("نظرة عامة", body_, size=17, style="SemiBold", color=LIGHT["text-1"], font=AR)
    t("عرض تأريخي شامل لنشأة الحضارة الإسلامية ونموّها في السياسة والفكر والفنون، "
      "بأسلوب ميسّر يجمع بين الرواية والتحليل، مع فهرس مفصّل ومصادر مراجعة.",
      body_, size=15, color=LIGHT["text-2"], font=AR, width=640,
      line_height="185%", align="right")

    table = v(body_, gap=0, sizing_h="fill", sizing_v="hug", radius=10,
              stroke=LIGHT["border-1"], clip=True, fill=LIGHT["surface-1"])
    rows = [("المؤلف", "أحمد فكري"), ("المترجم", "محمد عبدالله"),
            ("الناشر", "دار المعارف"), ("سنة النشر", "١٩٧٨"),
            ("اللغة", "العربية"), ("الصفحات", "٤٢٤"),
            ("الموقع المادي", "الرف ٣ · القسم B")]
    for i, (k, val) in enumerate(rows):
        rw = v(table, sizing_h="fill", sizing_v="hug")
        if i:
            rule(rw)
        cells = h(rw, gap=16, padding=[10, 16, 10, 16], cross_align="center",
                  sizing_h="fill", sizing_v="hug")
        # DOM [term, value] → the term sits at the right, value fills the rest.
        fill_text(cells, val, size=15, color=LIGHT["text-1"], font=AR)
        t(k, cells, width=140, size=13.5, color=LIGHT["text-3"], font=AR,
          align="right")


# ── Build ────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    want = sys.argv[1] if len(sys.argv) > 1 else "all"
    if want in ("all", "dashboard"):
        dashboard(shell("1 · لوحة التحكم · Dashboard", "layout-dashboard", x=1600))
    if want in ("all", "library"):
        library(shell("2 · المكتبة · Library", "library", x=3000))
    if want in ("all", "book"):
        book_page(shell("3 · صفحة الكتاب · Book page", "book-open", x=4400))

    json.dump(s.steps, sys.stdout, ensure_ascii=False, indent=1)
    print()
