import { describe, expect, it } from "vitest";

import {
  calendarDayKey,
  containsArabicIndicDigits,
  formatCompact,
  formatDate,
  formatHijri,
  formatNumber,
  formatPercent,
  gregorianYear,
  toDigits,
  toWesternDigits,
} from "./format";

describe("digits", () => {
  it("transliterates Western digits to Arabic-Indic", () => {
    expect(toDigits("1234", "arabic-indic")).toBe("١٢٣٤");
  });

  it("leaves non-digit characters untouched", () => {
    expect(toDigits("978-977-09-1234-5", "arabic-indic")).toBe("٩٧٨-٩٧٧-٠٩-١٢٣٤-٥");
    expect(toDigits("2026/10/18", "arabic-indic")).toBe("٢٠٢٦/١٠/١٨");
  });

  it("is the identity for Western output", () => {
    expect(toDigits("1234", "western")).toBe("1234");
    expect(toDigits("", "western")).toBe("");
  });

  it("accepts numbers directly", () => {
    expect(toDigits(42, "arabic-indic")).toBe("٤٢");
  });

  it("round-trips back to Western", () => {
    expect(toWesternDigits(toDigits("05920-12345", "arabic-indic"))).toBe("05920-12345");
  });

  it("detects Arabic-Indic digits so input can be normalised before validation", () => {
    expect(containsArabicIndicDigits("١٢٣")).toBe(true);
    expect(containsArabicIndicDigits("123")).toBe(false);
    expect(containsArabicIndicDigits("abc")).toBe(false);
  });
});

describe("formatNumber", () => {
  const w = { digits: "western" as const };
  const a = { digits: "arabic-indic" as const };

  it("groups thousands", () => {
    expect(formatNumber(1284, w)).toBe("1,284");
  });

  it("uses Arabic grouping and decimal separators when rendering Arabic-Indic", () => {
    // U+066C ARABIC THOUSANDS SEPARATOR, U+066B ARABIC DECIMAL SEPARATOR
    expect(formatNumber(1284, a)).toBe("١٬٢٨٤");
    expect(formatNumber(1234.5, { ...a, maximumFractionDigits: 1 })).toBe("١٬٢٣٤٫٥");
  });

  it("honours fixed fraction digits", () => {
    expect(formatNumber(4, { ...w, minimumFractionDigits: 1, maximumFractionDigits: 1 })).toBe("4.0");
  });

  it("handles zero and negatives", () => {
    expect(formatNumber(0, w)).toBe("0");
    expect(formatNumber(-1284, w)).toBe("-1,284");
  });
});

describe("formatCompact", () => {
  const w = { digits: "western" as const };

  it("leaves small numbers alone", () => {
    expect(formatNumber(999, w)).toBe("999");
  });

  it("abbreviates thousands and millions", () => {
    expect(formatCompact(1284, w)).toBe("1.3k");
    expect(formatCompact(18_302, w)).toBe("18k");
    expect(formatCompact(1_250_000, w)).toBe("1.3M");
  });
});

describe("formatPercent", () => {
  it("appends a percent sign to an already-0–100 value", () => {
    expect(formatPercent(73, { digits: "western" })).toBe("73%");
    expect(formatPercent(73, { digits: "arabic-indic" })).toBe("٧٣%");
  });
});

describe("dates", () => {
  // 2026-10-18T00:00:00Z
  const ts = Date.UTC(2026, 9, 18);

  it("formats a Gregorian date", () => {
    const out = formatDate(ts, "en", "gregorian", "western");
    expect(out).toContain("18");
    expect(out).toContain("2026");
    expect(out).toContain("October");
  });

  it("formats Arabic-Indic digits in a Gregorian date", () => {
    const out = formatDate(ts, "ar", "gregorian", "arabic-indic");
    expect(out).toMatch(/[٠-٩]/);
    expect(out).not.toMatch(/[0-9]/);
  });

  it("formats Hijri on its own", () => {
    const out = formatDate(ts, "en", "hijri", "western");
    // 18 October 2026 is in Jumada al-Akhirah 1448.
    expect(out).toMatch(/1448/);
  });

  it("shows both calendars joined by a middot", () => {
    const out = formatDate(ts, "en", "both", "western");
    expect(out.split(" · ")).toHaveLength(2);
  });

  it("falls back rather than throwing when the Hijri calendar is unavailable", () => {
    // The helper catches and degrades; assert it returns something non-empty regardless of the
    // host ICU build.
    expect(formatHijri(new Date(ts), "en").length).toBeGreaterThan(0);
  });

  it("derives the calendar day key used by reading_sessions.session_date", () => {
    expect(calendarDayKey(ts)).toBe("2026-10-18");
  });

  it("derives the Gregorian year for goals", () => {
    expect(gregorianYear(ts)).toBe(2026);
  });
});
