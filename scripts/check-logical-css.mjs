#!/usr/bin/env node
/**
 * RTL guard — enforces design rule 7.2 (`docs/04-ux-design.md`).
 *
 * Layout in this app is written with **CSS logical properties** so the whole interface mirrors
 * for Arabic without a second stylesheet. A physical `left`/`right` slips through review easily
 * and produces a bug that only shows up in one locale, so it is a build failure instead.
 *
 * Allowed escape hatch — put this comment on the line immediately before the offender:
 *
 *     /* logical-css-allow: mirrored icon, deliberate *\/
 *
 * Exit code 1 when anything is flagged.
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import process from "node:process";

const ROOT = new URL("..", import.meta.url).pathname;
const SCAN_DIRS = [join(ROOT, "src")];
const EXTENSIONS = new Set([".css", ".svelte", ".ts", ".js"]);

/** Patterns that are never allowed, with the logical replacement. */
const RULES = [
  ["\\bmargin-left\\b", "margin-inline-start"],
  ["\\bmargin-right\\b", "margin-inline-end"],
  ["\\bpadding-left\\b", "padding-inline-start"],
  ["\\bpadding-right\\b", "padding-inline-end"],
  ["\\btext-align\\s*:\\s*left\\b", "text-align: start"],
  ["\\btext-align\\s*:\\s*right\\b", "text-align: end"],
  ["\\bborder-left\\b", "border-inline-start"],
  ["\\bborder-right\\b", "border-inline-end"],
  ["\\bborder-left-width\\b", "border-inline-start-width"],
  ["\\bborder-right-width\\b", "border-inline-end-width"],
  ["\\bborder-left-color\\b", "border-inline-start-color"],
  ["\\bborder-right-color\\b", "border-inline-end-color"],
  ["\\bborder-left-style\\b", "border-inline-start-style"],
  ["\\bborder-right-style\\b", "border-inline-start-style"],
  ["\\bborder-top-left-radius\\b", "border-start-start-radius"],
  ["\\bborder-top-right-radius\\b", "border-start-end-radius"],
  ["\\bborder-bottom-left-radius\\b", "border-end-start-radius"],
  ["\\bborder-bottom-right-radius\\b", "border-end-end-radius"],
  ["\\bfloat\\s*:\\s*left\\b", "remove the float; flex/grid already follow direction"],
  ["\\bfloat\\s*:\\s*right\\b", "remove the float; flex/grid already follow direction"],
  ["\\binset-left\\b", "inset-inline-start"],
  ["\\binset-right\\b", "inset-inline-end"],
];

/** `left:` / `right:` position offsets, only when they look like a declaration. */
const POS_RULES = [
  ["^\\s*left\\s*:", "inset-inline-start (or `inset` with logical values)"],
  ["^\\s*right\\s*:", "inset-inline-end (or `inset` with logical values)"],
];

const ALLOW_MARKER = /logical-css-allow:/;

/** Strip block and line comments so prose does not trip a rule. */
function stripComments(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "));
}

function* walk(dir) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return;
  }
  for (const name of entries) {
    if (name === "node_modules" || name.startsWith(".")) continue;
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) yield* walk(full);
    else if (EXTENSIONS.has(name.slice(name.lastIndexOf(".")))) yield full;
  }
}

const compiled = RULES.map(([pattern, fix]) => [new RegExp(pattern, "gi"), fix]);
const compiledPos = POS_RULES.map(([pattern, fix]) => [new RegExp(pattern, "i"), fix]);

let failures = 0;

for (const dir of SCAN_DIRS) {
  for (const file of walk(dir)) {
    const raw = readFileSync(file, "utf8");
    const source = stripComments(raw);
    const lines = source.split("\n");

    for (let i = 0; i < lines.length; i += 1) {
      const line = lines[i];
      // The escape hatch lives on the preceding line (comments are already blanked above, so
      // consult the raw text for that line).
      const previous = i > 0 ? raw.split("\n")[i - 1] : "";
      if (ALLOW_MARKER.test(previous)) continue;

      for (const [regex, fix] of [...compiled, ...compiledPos]) {
        regex.lastIndex = 0;
        const hit = regex.exec(line);
        if (!hit) continue;
        failures += 1;
        console.error(
          `${relative(ROOT, file)}:${i + 1}\n` +
            `  physical property \`${hit[0].trim()}\`\n` +
            `  → use \`${fix}\` instead\n`,
        );
        break; // one report per line is enough to act on
      }
    }
  }
}

if (failures > 0) {
  console.error(
    `\n${failures} physical left/right declaration(s) found.\n` +
      `Layout must use CSS logical properties so RTL mirrors for free (docs/04-ux-design.md §7.2).\n` +
      `Add \`/* logical-css-allow: <reason> */\` on the previous line only if mirroring is genuinely wrong.\n`,
  );
  process.exit(1);
}

console.log("logical-css: ok — no physical left/right declarations in src/");
