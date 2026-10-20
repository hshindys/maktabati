// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AppErrorDto } from "@/bindings";
import { EMPTY_SUMMARY, getLibrarySummary } from "./api";

/**
 * The read path must never lie (P0-5): a backend failure arrives as `{ status: "error" }`
 * with the `AppErrorDto` intact, so the dashboard can render an error state. Only a real
 * "there is no shell here" resolves to the designed empty fallback.
 */

const librarySummary = vi.fn();

vi.mock("@/bindings", () => ({ commands: { librarySummary } }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({}) }));

const ERR: AppErrorDto = {
  code: "storage.corrupt",
  message: "database disk image is malformed",
  i18n_key: "error.storage.corrupt",
  details: null,
};

beforeEach(() => {
  librarySummary.mockReset();
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
