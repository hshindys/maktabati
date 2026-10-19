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

from gen_design import ACCENTS, AR, LIGHT, SEMANTIC, Script, UI
from icons import paths

# name → (foreground, soft background), e.g. SEM["danger"].
SEM = {name: (fg, soft) for name, fg, soft in SEMANTIC}

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

# ── Spacing scale ────────────────────────────────────────────────────────────
# Every page margin, section gap and card padding in one place. Padding is
# [top, right, bottom, left] in *visual* terms, like every other row here.
# Tune these and re-render; nothing below hard-codes a layout gap again.
SP_PAGE_TOP = 32         # content padding above the page heading (dashboard)
SP_PAGE_X = 40           # content padding, left and right (dashboard)
SP_PAGE_BOTTOM = 40      # content padding below the last section (dashboard)
SP_SECTION = 32          # vertical rhythm between page sections
SP_CARD = 24             # padding inside every card
SP_ROW_GAP = 16          # gap between the cards of one row

# Fixed row heights exist only because a horizontal FigCraft frame cannot derive
# its cross-axis size from `sizing_v="fill"` children (a hug parent has nothing
# to measure). Each is the *natural height of the tallest card in its row* —
# measured off the render with design/measure_spacing.py — plus ~2px of
# headroom, so shorter cards stretch (as CSS Grid's `align-items: stretch`
# does in the shipped dashboard) without a dead band under the whole row.
KPI_ROW_H = 170          # goal card: value carries "من 30" inline, bar below
STATS_ROW_H = 216        # "في مكتبتك" (4 stats): 213 natural


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
         stroke=ICON_STROKE, fill=None) -> int:
    """A Lucide glyph as a single stroked vector layer. `fill` paints it solid
    (the rating stars) instead of leaving the outline hollow."""
    return s.add("node.create", {
        "type": "vector",
        "name": name,
        "parent": {"$step": parent},
        "path": " ".join(ICONS[name]),
        "props": {
            "width": size,
            "height": size,
            "fills": [fill] if fill else [],
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
    root = v(main, name="content", gap=SP_SECTION,
             padding=[SP_PAGE_TOP, SP_PAGE_X, SP_PAGE_BOTTOM, SP_PAGE_X],
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
    # (glyph, label, value, suffix, caption, tone, pct). `suffix` rides on the
    # value line: the goal card reads "12 من 30" as one line, so it is no longer
    # 44px taller than its siblings — which is exactly the dead space that
    # equal-height rows would otherwise leave under each of them.
    kpis = [
        ("flame", "هدف", "12", "من 30", "84 منتهية · 2026", "success", 12 / 30),
        ("target", "صفحة هذا العام", "18,302", None, "24 تصنيفاً", "default",
         None),
        ("book-open", "أقرأها الآن", "47", None, "310 لم أبدأها", "accent",
         None),
        ("library", "الإجمالي", "1,284", None, "كتاب · 96 مؤلفاً", "default",
         None),
    ]
    # A fixed row height rather than `sizing_v="fill"` on the row: the row's
    # cross-axis size is what the cards stretch *to*, and a hug parent cannot
    # derive it from fill children. The height is the natural height of the
    # tallest card — the one carrying the progress bar — so the rest stretch to
    # match, exactly as `align-items: stretch` does in the shipped KpiCard grid,
    # while the row itself leaves no dead band under *any* card. Derived from
    # the render with design/measure_spacing.py (see KPI_ROW_H).
    row = h(root, sizing_h="fill", sizing_v="fixed", height=KPI_ROW_H,
            gap=SP_ROW_GAP)
    for glyph, label, value, suffix, caption, tone, pct in kpis:
        card = v(row, gap=8, padding=SP_CARD, radius=14, fill=LIGHT["surface-1"],
                 stroke=LIGHT["border-1"], sizing_h="fill", sizing_v="fill",
                 cross_align="max")
        # DOM [label, icon] → the label sits at the right.
        line = h(card, sizing_h="fill", sizing_v="hug", main_align="spaceBetween",
                 cross_align="center")
        icon(line, glyph, size=18, color=LIGHT["text-3"])
        t(label, line, size=13.5, color=LIGHT["text-2"], font=AR)
        tone_fg = {"default": LIGHT["text-1"], "accent": GREEN[0],
                   "success": "#2f7a4f"}[tone]
        # DOM [value, suffix] → RTL lands the value at the right, "من 30"
        # beside it, both sitting on one baseline.
        vr = h(card, sizing_h="fill", sizing_v="hug", main_align="max",
               cross_align="baseline", gap=6)
        if suffix is not None:
            t(suffix, vr, size=13.5, color=LIGHT["text-3"], font=AR)
        t(value, vr, size=38, style="Bold", color=tone_fg, font=UI)
        t(caption, card, size=12, color=LIGHT["text-3"], font=AR)
        if pct is not None:
            progress(card, pct, width=190, height=6)

    cols = h(root, sizing_h="fill", sizing_v="fixed", height=STATS_ROW_H,
             gap=SP_ROW_GAP)

    # Languages comes second in the DOM, therefore first — leftmost — here.
    # spaceBetween spreads title and rows over the row's height (its content is
    # 40px shorter than the sibling card), so no space pools under the last row;
    # the title still starts at the top, aligned with "في مكتبتك".
    langs = v(cols, gap=14, padding=SP_CARD, radius=14, fill=LIGHT["surface-1"],
              stroke=LIGHT["border-1"], sizing_h="fill", sizing_v="fill",
              cross_align="max", main_align="spaceBetween")
    t("اللغات", langs, size=17, style="SemiBold", color=LIGHT["text-1"], font=AR)
    for code, num in [("AR", "1,102"), ("EN", "174"), ("FR", "8")]:
        r = h(langs, sizing_h="fill", sizing_v="hug", main_align="spaceBetween",
              cross_align="center")
        t(num, r, size=15, color=LIGHT["text-1"], font=UI)
        b = h(r, padding=[3, 8, 3, 8], radius=6, fill=LIGHT["surface-2"],
              sizing_h="hug", sizing_v="hug")
        t(code, b, size=13.5, color=LIGHT["text-2"], font=UI, style="Medium")

    brk = v(cols, gap=16, padding=SP_CARD, radius=14, fill=LIGHT["surface-1"],
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


# ── Add-book wizard (M3) ─────────────────────────────────────────────────────
# A modal the shell renders over whatever route is open. The other three judged
# screens show the shell unobstructed, so these frames dim a flat stand-in
# instead of duplicating a page behind the card — what is judged here is the
# modal itself: the step rail, the field styles, the cover drop-zone, the
# review. Every string and value below is shipped copy (i18n keys, seeded
# taxonomy) or the flow's own example from docs/04 §4.5.

# 760 = tall enough for the longest step (2 · البيانات: six 67px rows plus the
# chrome, with ~20px of slack) without leaving the shorter steps hollow.
WIZ_W, WIZ_H = 860, 760
WIZ_X = (WIN_W - WIZ_W) // 2
WIZ_Y = (WIN_H - WIZ_H) // 2

# Logical order — the wizard walks them 1→4. Horizontal rows inside are written
# visual left→right, the same rule as `kpis` and the library toolbar: the code
# reads as the mirrored screen.
WIZARD_STEPS = [("1", "المصدر"), ("2", "البيانات"),
                ("3", "التصنيفات"), ("4", "المراجعة")]


def wiz_root(frame_name: str, x: int) -> int:
    root = plain(None, name=frame_name, x=x, width=WIN_W, height=WIN_H,
                 fill=LIGHT["surface-0"], sizing_h="fixed", sizing_v="fixed",
                 clip=True)
    # Dimmed stand-in for the route behind the modal (see above).
    s.rect(root, width=WIN_W, height=WIN_H, fill="#2a2621", opacity=0.5)
    return root


def wiz_label(parent, label: str, *, required: bool = False) -> int:
    g = h(parent, gap=3, sizing_h="hug", sizing_v="hug", cross_align="center")
    # Visual: the label hugs the right edge, the required mark trails it left.
    if required:
        t("*", g, size=13.5, style="SemiBold", color=SEM["danger"][0], font=AR)
    t(label, g, size=13.5, color=LIGHT["text-2"], font=AR)
    return g


def wiz_input(parent, *, value=None, placeholder=None, ltr=False,
              invalid=False) -> int:
    box = h(parent, height=36, padding=[0, 12, 0, 12], fill=LIGHT["surface-1"],
            radius=10, stroke=SEM["danger"][0] if invalid else LIGHT["border-2"],
            sizing_h="fill", sizing_v="fixed", cross_align="center", gap=8)
    if value is not None:
        if ltr:
            # `bidi-ltr` in the shell: identifiers start at the field's visual
            # left edge and never mirror (ISBN stays Western digits, like the
            # Gallery's input story).
            t(value, box, size=15, font=UI, color=LIGHT["text-1"])
        else:
            fill_text(box, value, size=15, font=AR, color=LIGHT["text-1"])
    elif placeholder:
        fill_text(box, placeholder, size=15, font=AR, color=LIGHT["text-3"])
    return box


def wiz_field(parent, label, *, required=False, value=None, placeholder=None,
              ltr=False, invalid=False, error=None, hint=None,
              width=None) -> int:
    f = v(parent, width=width, gap=5,
          sizing_h="fixed" if width else "fill", sizing_v="hug",
          cross_align="max")
    wiz_label(f, label, required=required)
    wiz_input(f, value=value, placeholder=placeholder, ltr=ltr, invalid=invalid)
    if error:
        t(error, f, size=12.5, font=AR, color=SEM["danger"][0])
    if hint:
        t(hint, f, size=12.5, font=AR, color=LIGHT["text-3"])
    return f


def wiz_select(parent, value: str) -> int:
    box = h(parent, height=36, padding=[0, 12, 0, 12], fill=LIGHT["surface-1"],
            radius=10, stroke=LIGHT["border-2"], sizing_h="fill",
            sizing_v="fixed", cross_align="center", main_align="spaceBetween")
    # Visual: the chevron sits at the field's end (left in RTL), the value at
    # its start — `spaceBetween` is what puts the label first in reading order.
    icon(box, "chevron-down", size=16, color=LIGHT["text-3"])
    t(value, box, size=15, font=AR, color=LIGHT["text-1"])
    return box


def wiz_select_field(parent, label, value, *, required=False) -> int:
    f = v(parent, gap=5, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_label(f, label, required=required)
    wiz_select(f, value)
    return f


def wiz_combobox(parent, label, *, required=False, value=None, placeholder=None,
                 add=None) -> int:
    """Label + text field with the inline-create button at the field's end
    (`[أحمد فكري ] [+ إضافة مؤلف]` in docs/04 §4.5)."""
    f = v(parent, gap=5, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_label(f, label, required=required)
    row = h(f, gap=8, sizing_h="fill", sizing_v="hug", cross_align="max")
    if add:
        button(row, add, size="sm", ico="plus")
    wiz_input(row, value=value, placeholder=placeholder)
    return f


def wiz_tag_pill(parent, label: str) -> int:
    p = h(parent, gap=6, padding=[5, 10, 5, 10], fill=GREEN[1], radius=999,
          sizing_h="hug", sizing_v="hug", cross_align="center")
    # Visual: × at the pill's end (left in RTL), the name at its start.
    icon(p, "x", size=12, color=GREEN[2])
    t(label, p, size=13, color=GREEN[2], font=AR)
    return p


def wiz_tags(parent, label, chips) -> int:
    f = v(parent, gap=5, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_label(f, label)
    box = h(f, height=36, padding=[0, 12, 0, 12], fill=LIGHT["surface-1"],
            radius=10, stroke=LIGHT["border-2"], sizing_h="fill",
            sizing_v="fixed", cross_align="center", gap=8, main_align="max")
    # Chips pack to the field's end (the right): the first typed reads rightmost.
    for name in reversed(chips):
        wiz_tag_pill(box, name)
    return box


def wiz_textarea(parent, label, *, placeholder, height=220) -> int:
    f = v(parent, gap=5, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_label(f, label)
    box = v(f, height=height, padding=[10, 12, 10, 12], fill=LIGHT["surface-1"],
            radius=10, stroke=LIGHT["border-2"], sizing_h="fill",
            sizing_v="fixed", cross_align="max")
    t(placeholder, box, size=15, font=AR, color=LIGHT["text-3"])
    return f


def wiz_rating(parent) -> int:
    f = v(parent, gap=5, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_label(f, "التقييم")
    row = h(f, gap=6, sizing_h="hug", sizing_v="hug", cross_align="center")
    # Visual: RTL counts from the right — the first star leads, three filled.
    for i in range(5, 0, -1):
        if i <= 3:
            icon(row, "star", size=17, color=STAR, fill=STAR)
        else:
            icon(row, "star", size=17, color=LIGHT["star-empty"])
    return f


def wiz_cover_zone(parent) -> int:
    zone = v(parent, gap=9, padding=[20, 20, 16, 20], fill=LIGHT["surface-2"],
             radius=14, stroke=GREEN[0], stroke_weight=1.5, stroke_dash=[7, 6],
             sizing_h="fill", sizing_v="fixed", height=250,
             main_align="center", cross_align="center")
    icon(zone, "upload", size=26, color=GREEN[0])
    t("اسحب صورة الغلاف هنا", zone, size=15, style="SemiBold",
      color=LIGHT["text-1"], font=AR)
    button(zone, "رفع صورة", size="sm")
    t("JPEG · PNG · WebP — حتى 4096 بكسل", zone, size=12.5,
      color=LIGHT["text-3"], font=AR)
    return zone


def wiz_card(root, *, active: int, primary: str, primary_ico) -> tuple[int, int]:
    """The modal chrome: header, step rail, body, footer. Returns (body, foot)."""
    card = v(root, x=WIZ_X, y=WIZ_Y, width=WIZ_W, height=WIZ_H,
             sizing_h="fixed", sizing_v="fixed", fill=LIGHT["surface-1"],
             radius=20, clip=True, stroke=LIGHT["border-1"], gap=0)
    # Visual: the close affordance left, the title hugging the right.
    head = h(card, padding=[18, 24, 14, 24], sizing_h="fill", sizing_v="hug",
             main_align="spaceBetween", cross_align="center")
    close = h(head, width=32, height=32, sizing_h="fixed", sizing_v="fixed",
              fill=LIGHT["surface-2"], radius=999, main_align="center",
              cross_align="center")
    icon(close, "x", size=16, color=LIGHT["text-2"])
    t("إضافة كتاب", head, size=19, style="SemiBold", color=LIGHT["text-1"],
      font=AR)
    rule(card)
    # Visual: step 4 leftmost, step 1 leading at the right (reads 1→4 RTL).
    rail = h(card, padding=[12, 24, 12, 24], sizing_h="fill", sizing_v="hug",
             gap=8, cross_align="center")
    for i in range(len(WIZARD_STEPS), 0, -1):
        num, label = WIZARD_STEPS[i - 1]
        state = "done" if i < active else ("now" if i == active else "next")
        fg = {"now": GREEN[2], "done": LIGHT["text-2"],
              "next": LIGHT["text-3"]}[state]
        pill = h(rail, gap=8, padding=[7, 14, 7, 14], radius=999,
                 fill=GREEN[1] if state == "now" else None,
                 sizing_h="hug", sizing_v="hug", cross_align="center")
        t(label, pill, size=14, font=AR,
          style="SemiBold" if state == "now" else "Regular", color=fg)
        badge_fill = {"now": GREEN[0], "done": GREEN[1],
                      "next": LIGHT["surface-3"]}[state]
        badge = h(pill, width=20, height=20, sizing_h="fixed",
                  sizing_v="fixed", radius=999, main_align="center",
                  cross_align="center", fill=badge_fill)
        if state == "done":
            icon(badge, "check", size=12, color=GREEN[0])
        else:
            t(num, badge, size=12, style="SemiBold", font=UI,
              color=GREEN[1] if state == "now" else LIGHT["text-3"])
    rule(card)
    body = v(card, padding=[20, 24, 18, 24], gap=14, sizing_h="fill",
             sizing_v="fill", cross_align="max")
    rule(card)
    # Visual: primary action at the left edge, إلغاء alone at the right — the
    # mirror of a [Cancel][Back][Next] footer (docs/04 §5's button rows).
    foot = h(card, padding=[14, 24, 16, 24], sizing_h="fill", sizing_v="hug",
             main_align="spaceBetween", cross_align="center")
    group = h(foot, gap=10, sizing_h="hug", sizing_v="hug",
              cross_align="center")
    button(group, primary, variant="primary", size="md", ico=primary_ico)
    if active > 1:
        button(group, "رجوع", size="md", ico="arrow-right")
    button(foot, "إلغاء", size="md")
    return body, foot


def wiz_kv(parent, label, value, *, ltr=False) -> None:
    r = h(parent, padding=[3, 0, 3, 0], sizing_h="fill", sizing_v="hug",
          main_align="spaceBetween", cross_align="center")
    # Visual: the value at the left, the term hugging the right — the meta
    # table's [value, term] order from the book page.
    t(value, r, size=15, font=UI if ltr else AR, color=LIGHT["text-1"])
    t(label, r, size=13.5, font=AR, color=LIGHT["text-3"])


def wiz_chips_row(parent, label, chips) -> None:
    r = h(parent, padding=[3, 0, 3, 0], sizing_h="fill", sizing_v="hug",
          main_align="spaceBetween", cross_align="center")
    vals = h(r, gap=6, sizing_h="hug", sizing_v="hug", cross_align="center")
    for name in reversed(chips):           # first picked reads rightmost
        chip(vals, name, fill=LIGHT["surface-2"], fg=LIGHT["text-2"],
             border=LIGHT["border-1"])
    t(label, r, size=13.5, font=AR, color=LIGHT["text-3"])


def wiz_step1(body) -> None:
    # The checksum rejecting the last digit — the localized error state.
    wiz_field(body, "ISBN", value="978-0-306-40615-8", ltr=True, invalid=True,
              error="رقم ISBN غير صالح — تحقّق من الأرقام ورمز التحقّق.")
    src = v(body, gap=5, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_label(src, "المصدر")
    row = h(src, gap=10, padding=[4, 0, 0, 0], sizing_h="fill",
            sizing_v="hug", cross_align="center", main_align="spaceBetween")
    t("يُطابق الرقم مع السجل عند الحفظ.", row, size=13, color=LIGHT["text-3"],
      font=AR)
    chip(row, "يدوي", fill=LIGHT["surface-2"], fg=LIGHT["text-2"],
         border=LIGHT["border-1"])
    wiz_cover_zone(body)


def wiz_step2(body) -> None:
    wiz_field(body, "العنوان", required=True, value="مقدمة في الحضارة الإسلامية")
    wiz_field(body, "العنوان الفرعي", placeholder="اختياري")
    # Visual: publisher left, author right (DOM: author, publisher).
    r = h(body, gap=16, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_combobox(r, "الناشر", value="دار المعارف", add="إضافة ناشر")
    wiz_combobox(r, "المؤلف", required=True, value="أحمد فكري",
                 add="إضافة مؤلف")
    # Visual: series order left (narrow), series combobox right.
    r = h(body, gap=16, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_field(r, "رقم السلسلة", placeholder="اختياري", width=150)
    wiz_combobox(r, "السلسلة", placeholder="اختياري", add="إضافة سلسلة")
    # Visual: language, pages, year — mirrored from the flow of docs/04 §4.5.
    r = h(body, gap=16, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_select_field(r, "اللغة", "العربية")
    wiz_field(r, "الصفحات", value="٤٢٤")
    wiz_field(r, "سنة النشر", value="١٩٧٨")
    # Visual: edition left, format right (DOM: format, edition).
    r = h(body, gap=16, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_field(r, "الطبعة", value="طبعة أولى")
    wiz_select_field(r, "التنسيق", "ورقي")


def wiz_step3(body) -> None:
    # Visual: tag input left, category combobox right (DOM: category, tags).
    r = h(body, gap=16, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_tags(r, "الوسوم", ["تاريخ", "حضارة"])
    wiz_combobox(r, "التصنيف", value="تاريخ العرب", add="تصنيف فرعي")
    # Visual: rating left, reading status right.
    r = h(body, gap=16, sizing_h="fill", sizing_v="hug", cross_align="max")
    wiz_rating(r)
    wiz_select_field(r, "الحالة", "لم أبدأها")
    wiz_textarea(body, "الوصف", placeholder="وصف مختصر أو ملاحظاتك عن الكتاب…")


def wiz_step4(body) -> None:
    # Visual: the cover thumb left, the summary filling the rest.
    top = h(body, gap=22, sizing_h="fill", sizing_v="hug", cross_align="min")
    cover(top, 116, "مقدمة في الحضارة الإسلامية", "أحمد فكري", 0)
    summary = v(top, gap=0, sizing_h="fill", sizing_v="hug",
                cross_align="max")
    wiz_kv(summary, "العنوان", "مقدمة في الحضارة الإسلامية")
    wiz_kv(summary, "المؤلف", "أحمد فكري")
    wiz_kv(summary, "الناشر", "دار المعارف")
    wiz_kv(summary, "ISBN", "978-0-306-40615-7", ltr=True)
    wiz_kv(summary, "السنة · الصفحات", "١٩٧٨ · ٤٢٤")
    wiz_kv(summary, "اللغة · التنسيق", "العربية · ورقي")
    wiz_chips_row(summary, "التصنيفات", ["التاريخ", "تاريخ العرب"])
    wiz_chips_row(summary, "الوسوم", ["تاريخ", "حضارة"])
    wiz_kv(summary, "الحالة", "لم أبدأها")
    # The save-time duplicate warning: warns, and offers the override.
    p = v(body, gap=10, padding=[14, 16, 14, 16], radius=12,
          fill=SEM["warning"][1], stroke=SEM["warning"][0],
          sizing_h="fill", sizing_v="hug", cross_align="max")
    head = h(p, gap=8, sizing_h="hug", sizing_v="hug", cross_align="center")
    t("قد يوجد كتاب متطابق", head, size=14, style="SemiBold",
      color=SEM["warning"][0], font=AR)
    icon(head, "triangle-alert", size=16, color=SEM["warning"][0])
    t("«مقدمة في الحضارة الإسلامية» لـ أحمد فكري — طابق ISBN والعناوان والمؤلف.",
      p, size=13.5, color=LIGHT["text-2"], font=AR)
    acts = h(p, gap=8, sizing_h="hug", sizing_v="hug", cross_align="center")
    # Visual: override left, "view match" leads right — the mirror of
    # docs/04 §4.5's `[عرض التطابق] [حفظ رغم ذلك]`.
    button(acts, "حفظ رغم ذلك", variant="primary", size="sm")
    button(acts, "عرض التطابق", size="sm")


def wizard(frame_name: str, x: int, step: int) -> None:
    root = wiz_root(frame_name, x)
    body, _ = wiz_card(root, active=step,
                       primary="حفظ" if step == 4 else "التالي",
                       primary_ico="check" if step == 4 else "arrow-left")
    {1: wiz_step1, 2: wiz_step2, 3: wiz_step3, 4: wiz_step4}[step](body)


# ── Build ────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    want = sys.argv[1] if len(sys.argv) > 1 else "all"
    if want in ("all", "dashboard"):
        dashboard(shell("1 · لوحة التحكم · Dashboard", "layout-dashboard", x=1600))
    if want in ("all", "library"):
        library(shell("2 · المكتبة · Library", "library", x=3000))
    if want in ("all", "book"):
        book_page(shell("3 · صفحة الكتاب · Book page", "book-open", x=4400))
    if want in ("all", "wizard"):
        wizard("4 · إضافة كتاب — المصدر · Wizard", 5800, 1)
        wizard("5 · إضافة كتاب — البيانات · Wizard", 7200, 2)
        wizard("6 · إضافة كتاب — التصنيفات · Wizard", 8600, 3)
        wizard("7 · إضافة كتاب — المراجعة · Wizard", 10000, 4)

    json.dump(s.steps, sys.stdout, ensure_ascii=False, indent=1)
    print()
