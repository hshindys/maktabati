// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";

import { clearDraft, EMPTY_DRAFT, loadDraft, reconcile, saveDraft, type BookDraft } from "./draft";

beforeEach(() => {
  localStorage.clear();
});

describe("reconcile", () => {
  it("returns the empty draft for garbage input", () => {
    for (const input of [null, undefined, 42, "nope", []]) {
      expect(reconcile(input)).toEqual(EMPTY_DRAFT);
    }
  });

  it("discards an out-of-range step and rating", () => {
    expect(reconcile({ step: 9, rating: 40 }).step).toBe(1);
    expect(reconcile({ step: 0, rating: -3 }).rating).toBe(0);
    expect(reconcile({ step: 3, rating: 8 }).step).toBe(3);
    expect(reconcile({ step: 3, rating: 8 }).rating).toBe(8);
  });

  it("drops name-less entity refs and non-string lists", () => {
    const draft = reconcile({
      authors: [{ id: null, name: "أحمد فكري" }, { name: "   " }, "junk"],
      tags: ["تاريخ", 7, null],
      categories: ["id-1", 2],
    });
    expect(draft.authors).toEqual([{ id: null, name: "أحمد فكري" }]);
    expect(draft.tags).toEqual(["تاريخ"]);
    expect(draft.categories).toEqual(["id-1"]);
  });

  it("keeps a typed but uncreated name with a null id, and a picked id with it", () => {
    const draft = reconcile({
      publisher: { id: null, name: "دار المعارف" },
      series: { id: "s-1", name: "كنوز المعرفة" },
    });
    expect(draft.publisher).toEqual({ id: null, name: "دار المعارف" });
    expect(draft.series).toEqual({ id: "s-1", name: "كنوز المعرفة" });
  });

  it("defaults the language and status rather than leaving them blank", () => {
    expect(reconcile({ languageCode: "", status: "" })).toMatchObject({
      languageCode: "ar",
      status: "to_read",
    });
  });
});

describe("persistence", () => {
  it("round-trips a draft through localStorage", () => {
    const draft: BookDraft = { ...EMPTY_DRAFT, title: "مقدمة", step: 2, rating: 6 };
    saveDraft(draft);
    expect(loadDraft()).toEqual(draft);
  });

  it("degrades corrupt JSON to a fresh draft", () => {
    localStorage.setItem("maktabati.draft.book.v1", "{not json");
    expect(loadDraft()).toEqual(EMPTY_DRAFT);
  });

  it("clears the draft on demand", () => {
    saveDraft({ ...EMPTY_DRAFT, title: "مقدمة" });
    clearDraft();
    expect(loadDraft()).toEqual(EMPTY_DRAFT);
  });
});
