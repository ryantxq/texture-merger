// src/store.test.ts
import { describe, it, expect } from "vitest";
import { reducer, initialState } from "./store";

describe("layer reducer", () => {
  it("adds layers to top", () => {
    const s = reducer(initialState, {
      type: "addLayers",
      items: [{ status: "ok", id: 1, path: "a.png", name: "a", width: 2, height: 2, hasAlpha: true, thumbnail: "data:," }],
    });
    expect(s.layers).toHaveLength(1);
    expect(s.layers[0].id).toBe(1);
  });

  it("removes layer", () => {
    const s1 = reducer(initialState, {
      type: "addLayers",
      items: [
        { status: "ok", id: 1, path: "a", name: "a", width: 2, height: 2, hasAlpha: true, thumbnail: "data:," },
        { status: "ok", id: 2, path: "b", name: "b", width: 2, height: 2, hasAlpha: true, thumbnail: "data:," },
      ],
    });
    const s2 = reducer(s1, { type: "removeLayer", id: 1 });
    expect(s2.layers.map((l) => l.id)).toEqual([2]);
  });

  it("toggles visibility", () => {
    const s1 = reducer(initialState, { type: "addLayers", items: [
      { status: "ok", id: 1, path: "a", name: "a", width: 2, height: 2, hasAlpha: true, thumbnail: "data:," },
    ]});
    const s2 = reducer(s1, { type: "toggleVisible", id: 1 });
    expect(s2.layers[0].visible).toBe(false);
  });

  it("reorders by dragging from to", () => {
    const s1 = reducer(initialState, { type: "addLayers", items: [1, 2, 3, 4].map((id) => ({
      status: "ok" as const, id, path: String(id), name: String(id), width: 2, height: 2, hasAlpha: true, thumbnail: "data:,",
    }))});
    const s2 = reducer(s1, { type: "moveLayer", from: 3, to: 0 });
    expect(s2.layers.map((l) => l.id)).toEqual([4, 1, 2, 3]);
  });

  it("rotates clockwise", () => {
    const s1 = reducer(initialState, { type: "addLayers", items: [
      { status: "ok", id: 1, path: "a", name: "a", width: 2, height: 2, hasAlpha: true, thumbnail: "data:," },
    ]});
    const s2 = reducer(s1, { type: "rotate", id: 1 });
    expect(s2.layers[0].rotate).toBe(1);
  });

  it("sets and clears import progress", () => {
    const s1 = reducer(initialState, { type: "setImportProgress", progress: { done: 2, total: 5 } });
    expect(s1.importProgress).toEqual({ done: 2, total: 5 });
    const s2 = reducer(s1, { type: "setImportProgress", progress: null });
    expect(s2.importProgress).toBeNull();
  });

  it("defaults preview background to checker", () => {
    expect(initialState.previewBg.mode).toBe("checker");
    expect(initialState.previewBg.checkerA).toBe("#e3e6ea");
  });

  it("sets preview background", () => {
    const bg = { mode: "solid" as const, checkerA: "#000", checkerB: "#fff", solid: "#112233" };
    const s1 = reducer(initialState, { type: "setPreviewBg", bg });
    expect(s1.previewBg).toEqual(bg);
  });
});
