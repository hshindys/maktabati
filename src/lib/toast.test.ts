// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";

import { clearToasts, dismissToast, pushToast, toast, toasts } from "./toast";

function snapshot() {
  let current: unknown = [];
  const unsub = toasts.subscribe((v) => (current = v));
  unsub();
  return current as { id: string; tone: string; message: string; durationMs: number | null }[];
}

beforeEach(() => {
  toasts.set([]);
  vi.useRealTimers();
});

describe("pushToast", () => {
  it("appends a toast with the default duration", () => {
    pushToast("saved");
    const list = snapshot();
    expect(list).toHaveLength(1);
    expect(list[0].tone).toBe("info");
    expect(list[0].durationMs).toBe(4000);
  });

  it("gives each toast a distinct id", () => {
    pushToast("a");
    pushToast("b");
    expect(new Set(snapshot().map((t) => t.id)).size).toBe(2);
  });

  it("caps the visible stack at four", () => {
    for (let i = 0; i < 8; i += 1) pushToast(`m${i}`);
    expect(snapshot()).toHaveLength(4);
    // The newest four survive.
    expect(snapshot().at(-1)?.message).toBe("m7");
  });

  it("does not auto-dismiss an error, because it carries an action", () => {
    pushToast("boom", { tone: "error" });
    expect(snapshot()[0].durationMs).toBeNull();
  });

  it("honours an explicit duration override", () => {
    pushToast("fast", { durationMs: 10 });
    expect(snapshot()[0].durationMs).toBe(10);
  });

  it("carries an action label through", () => {
    pushToast("deleted", { actionLabel: "Undo" });
    expect(snapshot()[0]).toHaveProperty("actionLabel", "Undo");
  });
});

describe("dismissToast", () => {
  it("removes only the named toast", () => {
    const a = pushToast("a");
    pushToast("b");
    dismissToast(a);
    expect(snapshot().map((t) => t.message)).toEqual(["b"]);
  });

  it("is a no-op for an unknown id", () => {
    pushToast("a");
    dismissToast("nope");
    expect(snapshot()).toHaveLength(1);
  });
});

describe("clearToasts", () => {
  it("empties the stack", () => {
    pushToast("a");
    pushToast("b");
    clearToasts();
    expect(snapshot()).toHaveLength(0);
  });
});

describe("toast helpers", () => {
  it("set the right tone", () => {
    toast.success("s");
    toast.error("e");
    toast.warning("w");
    toast.info("i");
    expect(snapshot().map((t) => t.tone)).toEqual(["success", "error", "warning", "info"]);
  });
});
