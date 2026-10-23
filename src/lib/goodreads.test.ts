import { describe, expect, it } from "vitest";

import {
  DEFAULT_STATUS,
  STATUS_MAP,
  bindingToFormat,
  cleanIsbn,
  ratingToHalfStars,
} from "./goodreads";

describe("STATUS_MAP", () => {
  it("maps every shelf name Goodreads writes", () => {
    expect(STATUS_MAP.read).toBe("finished");
    expect(STATUS_MAP.reading).toBe("reading");
    expect(STATUS_MAP.to_read).toBe("to_read");
    expect(STATUS_MAP.unread).toBe("to_read");
  });

  it("leaves no shelf unmapped", () => {
    // The parser's doc contract: only these four strings ever appear in `status`.
    for (const shelf of ["read", "reading", "to_read", "unread"]) {
      expect(STATUS_MAP[shelf]).toBeDefined();
    }
  });

  it("falls back to to_read for a shelf it does not name", () => {
    expect(DEFAULT_STATUS).toBe("to_read");
  });
});

describe("bindingToFormat — mirrors metadata::goodreads::binding_to_format", () => {
  it("maps paper spellings onto print", () => {
    expect(bindingToFormat("Hardcover")).toBe("print");
    expect(bindingToFormat("Paperback")).toBe("print");
    expect(bindingToFormat("Mass Market Paperback")).toBe("print");
    expect(bindingToFormat("Spiral-bound")).toBe("print");
  });

  it("maps electronic and audio spellings onto their format", () => {
    expect(bindingToFormat("Kindle Edition")).toBe("ebook");
    expect(bindingToFormat("Nook Book")).toBe("ebook");
    expect(bindingToFormat("PDF eBook")).toBe("pdf");
    expect(bindingToFormat("Audible Audio CD")).toBe("audiobook");
    expect(bindingToFormat("Audio Cassette")).toBe("audiobook");
  });

  it("keeps the Rust helper's None cases", () => {
    expect(bindingToFormat(null)).toBe(null);
    expect(bindingToFormat("")).toBe(null);
    expect(bindingToFormat("   ")).toBe(null);
  });
});

describe("ratingToHalfStars", () => {
  it("doubles Goodreads' 0..5 into half-star units", () => {
    expect(ratingToHalfStars(5)).toBe(10);
    expect(ratingToHalfStars(4)).toBe(8);
    expect(ratingToHalfStars(2)).toBe(4);
    expect(ratingToHalfStars(1)).toBe(2);
  });

  it("treats zero and null as unrated, not as a zero opinion", () => {
    expect(ratingToHalfStars(0)).toBe(null);
    expect(ratingToHalfStars(null)).toBe(null);
  });

  it("clamps past the scale so a rogue export cannot corrupt a row", () => {
    expect(ratingToHalfStars(6 as never)).toBe(10);
  });
});

describe("cleanIsbn", () => {
  it("strips the spreadsheet guard quotes and separators Goodreads ships", () => {
    expect(cleanIsbn('="9780306406157"')).toBe("9780306406157");
    expect(cleanIsbn("978-0-306-40615-7")).toBe("9780306406157");
    expect(cleanIsbn(" 0-8044-2957-X")).toBe("080442957X");
  });

  it("keeps null out of the DB dedup query", () => {
    expect(cleanIsbn(null)).toBe(null);
    expect(cleanIsbn('=""')).toBe(null);
    expect(cleanIsbn("")).toBe(null);
  });
});
