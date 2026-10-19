// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";

import {
  DEFAULT_PREFERENCES,
  directionFor,
  reconcile,
  resolveTheme,
  setPreferences,
  type Preferences,
} from "./settings";

function read(): Preferences {
  const raw = localStorage.getItem("maktabati.preferences.v1");
  return raw === null ? DEFAULT_PREFERENCES : (JSON.parse(raw) as Preferences);
}

beforeEach(() => {
  localStorage.clear();
});

describe("reconcile", () => {
  it("returns the defaults for garbage input", () => {
    for (const input of [null, undefined, 42, "nope", []]) {
      expect(reconcile(input)).toEqual(DEFAULT_PREFERENCES);
    }
  });

  it("keeps valid values", () => {
    const out = reconcile({
      locale: "en",
      theme: "sepia",
      accent: "ocean",
      digits: "western",
      calendar: "hijri",
      sidebarCollapsed: true,
    });
    expect(out).toEqual({
      locale: "en",
      theme: "sepia",
      accent: "ocean",
      digits: "western",
      calendar: "hijri",
      sidebarCollapsed: true,
    });
  });

  it("discards unknown enum members one field at a time", () => {
    const out = reconcile({
      locale: "fr",
      theme: "neon",
      accent: "chartreuse",
      digits: "romanesque",
      calendar: "mayan",
      sidebarCollapsed: "yes",
    });
    // Every bad field falls back independently; nothing cascades.
    expect(out).toEqual(DEFAULT_PREFERENCES);
  });

  it("accepts a partially-filled object", () => {
    expect(reconcile({ theme: "dark" }).theme).toBe("dark");
    expect(reconcile({ theme: "dark" }).locale, "locale keeps its default").toBe("ar");
  });
});

describe("directionFor", () => {
  it("maps Arabic to RTL and English to LTR", () => {
    expect(directionFor("ar")).toBe("rtl");
    expect(directionFor("en")).toBe("ltr");
  });
});

describe("resolveTheme", () => {
  it("passes concrete themes through untouched", () => {
    expect(resolveTheme("light")).toBe("light");
    expect(resolveTheme("dark")).toBe("dark");
    expect(resolveTheme("sepia")).toBe("sepia");
  });

  it("resolves `system` without throwing, whatever the host supports", () => {
    // jsdom does not implement matchMedia; the helper must degrade, not throw.
    const out = resolveTheme("system");
    expect(["light", "dark", "sepia"]).toContain(out);
  });
});

describe("applyToDocument", () => {
  it("mirrors preferences onto <html> so CSS can key off them", async () => {
    const { applyToDocument } = await import("./settings");
    applyToDocument({
      ...DEFAULT_PREFERENCES,
      theme: "dark",
      accent: "plum",
      locale: "en",
    });
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.dataset.accent).toBe("plum");
    expect(document.documentElement.lang).toBe("en");
    expect(document.documentElement.dir).toBe("ltr");
  });

  it("sets dir=rtl for the default Arabic locale", async () => {
    const { applyToDocument } = await import("./settings");
    applyToDocument(DEFAULT_PREFERENCES);
    expect(document.documentElement.dir).toBe("rtl");
    expect(document.documentElement.lang).toBe("ar");
  });
});

describe("setPreferences", () => {
  it("applies a partial patch and persists it", () => {
    setPreferences({ accent: "gold" });
    expect(read().accent).toBe("gold");
    setPreferences({ locale: "en" });
    expect(read().locale).toBe("en");
    expect(read().accent, "a later patch must not clear earlier fields").toBe("gold");
  });

  it("reflects the change on the document immediately", () => {
    setPreferences({ theme: "sepia" });
    expect(document.documentElement.dataset.theme).toBe("sepia");
  });
});
