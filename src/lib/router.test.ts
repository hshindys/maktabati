// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";

import { currentPath, go, isActive, match, replace, resolve } from "./router";

beforeEach(() => {
  window.location.hash = "#/";
});

describe("match", () => {
  it("matches a static path exactly", () => {
    expect(match("/library", "/library")).toEqual({});
  });

  it("rejects a different length", () => {
    expect(match("/library", "/library/extra")).toBeNull();
    expect(match("/library", "/")).toBeNull();
  });

  it("rejects a different static segment", () => {
    expect(match("/library", "/settings")).toBeNull();
  });

  it("captures a param", () => {
    expect(match("/book/:id", "/book/abc")).toEqual({ id: "abc" });
  });

  it("captures multiple params", () => {
    expect(match("/a/:x/b/:y", "/a/1/b/2")).toEqual({ x: "1", y: "2" });
  });

  it("percent-decodes captured params so Arabic ids survive the URL", () => {
    // "كتاب" percent-encoded
    expect(match("/book/:id", "/book/%D9%83%D8%AA%D8%A7%D8%A8")).toEqual({ id: "كتاب" });
  });

  it("does not match an empty param segment", () => {
    expect(match("/book/:id", "/book/")).toBeNull();
  });
});

describe("resolve", () => {
  it("resolves the dashboard", () => {
    expect(resolve("/").pattern).toBe("/");
  });

  it("resolves a concrete route", () => {
    const m = resolve("/settings/backup");
    expect(m.pattern).toBe("/settings/backup");
    expect(m.params).toEqual({});
  });

  it("resolves a param route", () => {
    const m = resolve("/book/0193ab");
    expect(m.pattern).toBe("/book/:id");
    expect(m.params).toEqual({ id: "0193ab" });
  });

  it("prefers the most specific pattern", () => {
    // `/settings/backup` must not be swallowed by a hypothetical `/settings/:tab`.
    expect(resolve("/settings/backup").pattern).toBe("/settings/backup");
    expect(resolve("/settings").pattern).toBe("/settings");
  });

  it("falls back to / for an unknown path", () => {
    expect(resolve("/nope/nope/nope").pattern).toBe("/");
  });

  it("normalises a path without a leading slash", () => {
    expect(resolve("library").pattern).toBe("/library");
  });
});

describe("isActive", () => {
  it("treats / as exact so the dashboard is not active everywhere", () => {
    expect(isActive("/", "/")).toBe(true);
    expect(isActive("/", "/library")).toBe(false);
  });

  it("matches a route and its children", () => {
    expect(isActive("/settings", "/settings")).toBe(true);
    expect(isActive("/settings", "/settings/backup")).toBe(true);
  });

  it("does not match a prefix that is not a path boundary", () => {
    expect(isActive("/settings", "/settingsomething")).toBe(false);
  });
});

describe("navigation", () => {
  it("go() writes the hash and the store follows", () => {
    go("/authors");
    expect(currentPath()).toBe("/authors");
  });

  it("go() is a no-op for the current path", () => {
    go("/authors");
    const before = window.location.hash;
    go("/authors");
    expect(window.location.hash).toBe(before);
  });

  it("replace() moves without adding a history entry", () => {
    replace("/quotes");
    expect(currentPath()).toBe("/quotes");
  });

  it("a hashchange event updates the store", async () => {
    const { route } = await import("./router");
    let seen = "";
    const unsub = route.subscribe((r) => (seen = r.path));
    window.location.hash = "#/goals";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(seen).toBe("/goals");
    unsub();
  });
});
