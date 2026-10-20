#!/usr/bin/env node
/**
 * Token guard — every `var(--x)` the app reads must have a definition somewhere.
 *
 * This exists because of a class of bug no type checker or test can see. `@theme` in
 * `app.css` *aliased* Tailwind's `--spacing-N` at `var(--sp-N)`, and the `--sp-N` values
 * were never written down anywhere. The build succeeded, svelte-check was clean, the
 * contrast gate passed — and every one of the 216 spacing declarations in the app
 * resolved to nothing, so the whole interface rendered with no padding or gaps at all.
 * A second token, `--color-accent-soft-ink`, was used in three components and never
 * mapped, so the utility silently emitted no CSS and the active nav item lost its ink.
 *
 * Both are invisible until someone looks at a rendered window. This gate makes them a
 * build failure instead.
 *
 * A reference with a fallback (`var(--x, 16px)`) is safe by construction and is skipped.
 * Definitions may live in `app.css`, in a component's scoped `<style>` block, or in a
 * `style="--x: …"` attribute — all three are real.
 *
 * Exit code 1 when anything is undefined.
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import process from "node:process";

const ROOT = new URL("..", import.meta.url).pathname;
const SCAN_DIRS = [join(ROOT, "src")];
const EXTENSIONS = new Set([".css", ".svelte", ".ts", ".js"]);

/** Vars supplied by the platform or by Tailwind's runtime, not by this codebase. */
const PROVIDED = new Set([
  "--tw-gradient-from-position",
  "--tw-gradient-to-position",
  "--tw-gradient-via-position",
]);

/** `--name:` — a declaration, in CSS or in a `style="…"` attribute. */
const DEFINES = /--([a-z][a-z0-9-]*)\s*:/g;
/** `var(--name` … capturing whether a fallback follows. */
const USES = /var\(\s*--([a-z][a-z0-9-]*)(\s*,)/g;

/** Strip block and line comments so prose does not count as a definition or a use. */
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

const defined = new Set(PROVIDED);
/** `--name` → first place it was referenced without a fallback. */
const uses = new Map();

for (const dir of SCAN_DIRS) {
  for (const file of walk(dir)) {
    const source = stripComments(readFileSync(file, "utf8"));
    const here = relative(ROOT, file);

    for (const m of source.matchAll(DEFINES)) defined.add(`--${m[1]}`);

    // Second pass: collect the bare references, so a definition later in the file (or in
    // another file) still satisfies one that appears before it.
    for (const m of source.matchAll(USES)) uses.set(`--${m[1]}`, here);

    // A `var(--x)` with no fallback anywhere on the line needs the same treatment.
    for (const [i, line] of source.split("\n").entries()) {
      for (const m of line.matchAll(/var\(\s*--([a-z][a-z0-9-]*)\s*(\))/g)) {
        const name = `--${m[1]}`;
        if (!uses.has(name)) uses.set(name, `${here}:${i + 1}`);
      }
    }
  }
}

const missing = [...uses.entries()]
  .filter(([name]) => !defined.has(name))
  .sort();

if (missing.length > 0) {
  for (const [name, site] of missing) {
    console.error(`${site}\n  \`${name}\` is read but never defined\n`);
  }
  console.error(
    `\n${missing.length} custom propert${missing.length === 1 ? "y" : "ies"} read without a definition.\n` +
      `An undefined custom property resolves to nothing and the build still succeeds, so this\n` +
      `is silent: declare it in src/app.css (or the component's own <style> block).\n`,
  );
  process.exit(1);
}

console.log(`tokens-defined: ok — every var(--x) in src/ resolves (${uses.size} references)`);
