/**
 * Client-side ISBN validation — the mirror of `crates/domain/src/isbn.rs`.
 *
 * The Rust validator is the authority; this copy exists so the wizard can flag a bad
 * checksum while the user types instead of only after a rejected save. Both implement the
 * same ISO 2108 rules and are pinned by the same fixtures (`isbn.test.ts` here, the
 * property tests in `domain/src/isbn.rs` there), so the two cannot drift apart silently.
 */

/** Strip spaces, hyphens and underscores; upper-case the ISBN-10 check character. */
export function canonicalIsbn(input: string): string {
  return input.replace(/[\s_-]/g, "").toUpperCase();
}

function isDigits(s: string): boolean {
  return /^[0-9]+$/.test(s);
}

/** ISBN-10: nine digits + a check digit that is `0–9` or `X`; `∑(11−i)·dᵢ ≡ 0 (mod 11)`. */
function isValid10(d: string): boolean {
  const head = d.slice(0, 9);
  if (!isDigits(head)) return false;
  const last = d[9];
  if (!/^[0-9X]$/.test(last)) return false;
  let sum = 0;
  for (let i = 0; i < 9; i++) sum += (10 - i) * Number(d[i]);
  sum += last === "X" ? 10 : Number(last);
  return sum % 11 === 0;
}

/** ISBN-13: thirteen digits; alternating weights `1,3,1,3,…`; `∑ ≡ 0 (mod 10)`. */
function isValid13(d: string): boolean {
  if (!isDigits(d)) return false;
  let sum = 0;
  for (let i = 0; i < 13; i++) sum += Number(d[i]) * (i % 2 === 0 ? 1 : 3);
  return sum % 10 === 0;
}

/** True when `input` is a valid ISBN-10 or ISBN-13; spaces and hyphens are accepted. */
export function isValidIsbn(input: string): boolean {
  const d = canonicalIsbn(input);
  if (d.length === 10) return isValid10(d);
  if (d.length === 13) return isValid13(d);
  return false;
}
