#!/usr/bin/env node
/**
 * Contrast gate — WCAG 2.1 AA (4.5:1) for every text-colour pair the app ships.
 *
 * Reads the *actual* custom properties from src/app.css, composes them the way the
 * DOM does (`:root` base → theme → accent → theme×status → theme×accent), and fails
 * with a table of the offending pairs. This is the regression lock for the P0-3/P0-4
 * token work: adding an accent or a theme that dips under 4.5:1 is now a build error,
 * not a review note.
 *
 * Non-text pairs (borders, shadows, stars, skeletons) are deliberately out of scope —
 * WCAG treats them under 1.4.11 (3:1) and this app keeps them decorative.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const CSS = readFileSync(fileURLToPath(new URL("../src/app.css", import.meta.url)), "utf8");

const THEMES = ["light", "dark", "sepia"];
const ACCENTS = ["green", "indigo", "plum", "terracotta", "ocean", "gold"];
const MIN = 4.5;

// ── parse top-level blocks ────────────────────────────────────────────────────

function blocks(css) {
  const out = [];
  let i = 0;
  while (i < css.length) {
    const open = css.indexOf("{", i);
    if (open === -1) break;
    const selector = css.slice(i, open).split("*/").pop().trim();
    let depth = 1;
    let j = open + 1;
    while (j < css.length && depth > 0) {
      if (css[j] === "{") depth++;
      else if (css[j] === "}") depth--;
      j++;
    }
    const body = css.slice(open + 1, j - 1);
    if (!selector.startsWith("@") && !selector.includes("@")) out.push({ selector, body });
    i = j;
  }
  return out;
}

function declarations(body) {
  const map = new Map();
  for (const line of body.split(";")) {
    const m = line.match(/(--[\w-]+)\s*:\s*([^;]+)/);
    if (m) map.set(m[1], m[2].trim());
  }
  return map;
}

const ALL = blocks(CSS).map((b) => ({ ...b, vars: declarations(b.body) }));

const pick = (pred) => ALL.filter(pred).flatMap((b) => [...b.vars]);

function compose(theme, accent) {
  const vars = new Map();
  const add = (list) => {
    for (const [k, v] of list) vars.set(k, v);
  };
  // Base: the :root alias/semantic blocks plus the light theme block.
  add(pick((b) => b.selector === ":root"));
  add(pick((b) => b.selector.includes('[data-theme="light"]')));
  if (theme !== "light") add(pick((b) => b.selector === `[data-theme="${theme}"]`));
  add(pick((b) => b.selector === `[data-accent="${accent}"]`));
  add(pick((b) => b.selector === `:root[data-theme="${theme}"]`));
  add(pick((b) => b.selector.includes(`[data-theme="${theme}"]`) && b.selector.includes(`[data-accent="${accent}"]`)));
  return vars;
}

// ── colour maths ──────────────────────────────────────────────────────────────

const lin = (c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
function luminance(hex) {
  const h = hex.replace("#", "").slice(0, 6);
  const n = (i) => lin(parseInt(h.slice(i, i + 2), 16) / 255);
  return 0.2126 * n(0) + 0.7152 * n(2) + 0.0722 * n(4);
}
function ratio(a, b) {
  const [x, y] = [luminance(a), luminance(b)].sort((m, n) => n - m);
  return (x + 0.05) / (y + 0.05);
}

// ── the fixed pair list ───────────────────────────────────────────────────────

const failures = [];
let checked = 0;

function checkPair(vars, fgToken, bgToken, label) {
  const fg = vars.get(fgToken);
  const bg = vars.get(bgToken);
  if (fg === undefined || bg === undefined) {
    failures.push(`${label}: token missing (${fgToken}=${fg} ${bgToken}=${bg})`);
    return;
  }
  const r = ratio(fg, bg);
  checked++;
  if (r < MIN) failures.push(`${label}: ${r.toFixed(2)}:1 (${fg} on ${bg})`);
}

for (const theme of THEMES) {
  const base = compose(theme, "green"); // text/status pairs are accent-independent
  for (const t of ["--text-1", "--text-2", "--text-3"])
    for (const s of ["--surface-0", "--surface-1", "--surface-2", "--surface-3"])
      checkPair(base, t, s, `${theme} ${t} on ${s}`);
  for (const s of ["--success", "--warning", "--danger", "--info"]) {
    checkPair(base, s, "--surface-0", `${theme} ${s} on --surface-0`);
    checkPair(base, s, `${s}-soft`, `${theme} ${s} on ${s}-soft`);
  }

  for (const accent of ACCENTS) {
    const v = compose(theme, accent);
    for (const s of ["--surface-0", "--surface-1", "--surface-2", "--surface-3"])
      checkPair(v, "--accent", s, `${theme}/${accent} --accent on ${s}`);
    checkPair(v, "--accent-hover", "--surface-0", `${theme}/${accent} --accent-hover on --surface-0`);
    checkPair(v, "--accent-fg", "--accent", `${theme}/${accent} --accent-fg on --accent`);
    checkPair(v, "--accent-soft-ink", "--accent-soft", `${theme}/${accent} --accent-soft-ink on --accent-soft`);
  }
}

if (failures.length > 0) {
  console.error(`contrast: ${failures.length} of ${checked} pairs below ${MIN}:1`);
  for (const f of failures) console.error(`  ✗ ${f}`);
  process.exit(1);
}
console.log(`contrast: ok — ${checked} pairs >= ${MIN}:1 across 3 themes × 6 accents`);
