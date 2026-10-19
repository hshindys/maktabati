// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import { canonicalIsbn, isValidIsbn } from "./isbn";

describe("canonicalIsbn", () => {
  it("strips separators and upper-cases the check character", () => {
    expect(canonicalIsbn("0-8044-2957-x")).toBe("080442957X");
    expect(canonicalIsbn(" 978 0 306 40615 7 ")).toBe("9780306406157");
  });
});

describe("isValidIsbn", () => {
  // The same fixtures the Rust doc-tests use (domain/src/isbn.rs), so a change in one
  // validator that the other misses fails here.
  it("accepts the canonical valid examples", () => {
    expect(isValidIsbn("978-0-306-40615-7")).toBe(true); // ISBN-13 (Bookland)
    expect(isValidIsbn("9780306406157")).toBe(true);
    expect(isValidIsbn("0-8044-2957-X")).toBe(true); // ISBN-10 with an X check digit
  });

  it("rejects a single flipped digit", () => {
    expect(isValidIsbn("9780306406158")).toBe(false); // the valid 13 above, last digit flipped
    expect(isValidIsbn("0804429571")).toBe(false); // the valid 10 above, check digit off
  });

  it("rejects wrong lengths and illegal characters", () => {
    for (const bad of ["", "978", "978030640615", "97803064061577", "ABCDEFGHIJ", "080442957Y"]) {
      expect(isValidIsbn(bad), bad).toBe(false);
    }
  });

  it("ignores spaces, hyphens and underscores", () => {
    expect(isValidIsbn("978_0_306_40615_7")).toBe(true);
  });
});
