// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AppErrorDto } from "@/bindings";
import { EMPTY_SUMMARY, bookHits, getLibrarySummary, searchLibrary, setBookPage } from "./api";

/**
 * The read path must never lie (P0-5): a backend failure arrives as `{ status: "error" }`
 * with the `AppErrorDto` intact, so the dashboard can render an error state. Only a real
 * "there is no shell here" resolves to the designed empty fallback.
 */

const librarySummary = vi.fn();
const search = vi.fn();
const bookPageSet = vi.fn();

vi.mock("@/bindings", () => ({ commands: { librarySummary, search, bookPageSet } }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({}) }));

const ERR: AppErrorDto = {
  code: "storage.corrupt",
  message: "database disk image is malformed",
  i18n_key: "error.storage.corrupt",
  details: null,
};

beforeEach(() => {
  librarySummary.mockReset();
  search.mockReset();
  bookPageSet.mockReset();
});

describe("searchLibrary", () => {
  it("uses the indexed search command with a bounded result count", async () => {
    const data = [
      {
        kind: "book",
        id: "book-1",
        title: "مقدمة",
        detail: null,
        book_id: null,
        status: "reading",
      },
    ];
    search.mockResolvedValue({ status: "ok", data });
    const res = await searchLibrary("مُقَدِّمَة", 8);

    expect(search).toHaveBeenCalledWith("مُقَدِّمَة", 8);
    expect(res).toEqual({ status: "ok", data });
  });

  it("preserves backend search errors", async () => {
    search.mockResolvedValue({ status: "error", error: ERR });
    expect(await searchLibrary("missing")).toEqual({ status: "error", error: ERR });
  });
});

describe("getLibrarySummary", () => {
  it("propagates a backend error instead of faking an empty library", async () => {
    librarySummary.mockResolvedValue({ status: "error", error: ERR });
    const res = await getLibrarySummary(2026);
    expect(res).toEqual({ status: "error", error: ERR });
  });

  it("passes a successful summary through untouched", async () => {
    librarySummary.mockResolvedValue({ status: "ok", data: { ...EMPTY_SUMMARY, books: 7 } });
    const res = await getLibrarySummary(2026);
    expect(res.status === "ok" && res.data.books).toBe(7);
  });

  it("reports a native invoke failure as an error, not as zeroes", async () => {
    librarySummary.mockRejectedValue(new Error("ipc down"));
    const res = await getLibrarySummary(2026);
    expect(res.status).toBe("error");
    expect(res.status === "error" && res.error.code).toBe("shell.invoke_failed");
  });
});

describe("bookHits", () => {
  const hit = (kind: "book" | "note" | "quote" | "author", id: string) => ({
    kind,
    id,
    title: `t-${id}`,
    detail: null,
    book_id: null,
    status: null,
  });

  it("keeps only books — the quote form may not name a note or an author", () => {
    const mixed = [hit("book", "1"), hit("note", "2"), hit("quote", "3"), hit("author", "4"), hit("book", "5")];
    expect(bookHits(mixed).map((h) => h.id)).toEqual(["1", "5"]);
  });

  it("passes an all-books result through untouched", () => {
    const books = [hit("book", "1"), hit("book", "2")];
    expect(bookHits(books)).toEqual(books);
  });

  it("returns an empty list for an empty search", () => {
    expect(bookHits([])).toEqual([]);
  });
});

describe("setBookPage", () => {
  it("sends the page alone — a mid-reading save never round-trips a whole BookInput", async () => {
    bookPageSet.mockResolvedValue({ status: "ok", data: { id: "b1", current_page: 120 } });
    const res = await setBookPage("b1", 120);

    expect(bookPageSet).toHaveBeenCalledWith("b1", 120);
    expect(res).toEqual({ status: "ok", data: { id: "b1", current_page: 120 } });
  });

  it("forwards null as an explicit clear", async () => {
    bookPageSet.mockResolvedValue({ status: "ok", data: { id: "b1", current_page: null } });
    const res = await setBookPage("b1", null);

    expect(bookPageSet).toHaveBeenCalledWith("b1", null);
    expect(res.status === "ok" && res.data.current_page).toBeNull();
  });

  it("keeps a range rejection intact so the form can show the real reason", async () => {
    bookPageSet.mockResolvedValue({ status: "error", error: { ...ERR, code: "validation.out_of_range" } });
    const res = await setBookPage("b1", 9999);

    expect(res.status).toBe("error");
    expect(res.status === "error" && res.error.code).toBe("validation.out_of_range");
  });
});
