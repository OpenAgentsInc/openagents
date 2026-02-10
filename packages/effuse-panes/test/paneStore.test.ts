import { describe, expect, test } from "bun:test";

import { PaneStore, normalizePaneRect } from "../src/paneStore.js";

describe("PaneStore", () => {
  test("addPane sets active and last position; duplicates bring to front", () => {
    const store = new PaneStore();
    store.addPane({
      id: "a",
      kind: "chat",
      title: "A",
      rect: { x: 10, y: 10, width: 200, height: 100 },
      dismissable: true,
    });
    expect(store.activePaneId).toBe("a");
    expect(store.lastPanePosition?.x).toBe(10);

    store.addPane({
      id: "b",
      kind: "events",
      title: "B",
      rect: { x: 20, y: 20, width: 200, height: 100 },
      dismissable: true,
    });
    expect(store.activePaneId).toBe("b");
    expect(store.panes().map((p) => p.id)).toEqual(["a", "b"]);

    const lastBefore = store.lastPanePosition;
    store.addPane({
      id: "a",
      kind: "chat",
      title: "A2",
      rect: { x: 999, y: 999, width: 999, height: 999 },
      dismissable: true,
    });
    expect(store.activePaneId).toBe("a");
    // Bring-to-front: a should now be last.
    expect(store.panes().map((p) => p.id)).toEqual(["b", "a"]);
    // Rust behavior: addPane for existing id does NOT update lastPanePosition.
    expect(store.lastPanePosition).toEqual(lastBefore);
  });

  test("removePane stores snapshot when requested and updates active to last", () => {
    const store = new PaneStore();
    store.addPane({
      id: "a",
      kind: "chat",
      title: "A",
      rect: { x: 10, y: 10, width: 200, height: 100 },
      dismissable: true,
    });
    store.addPane({
      id: "b",
      kind: "events",
      title: "B",
      rect: { x: 20, y: 20, width: 200, height: 100 },
      dismissable: true,
    });
    expect(store.activePaneId).toBe("b");
    store.removePane("b", true);
    expect(store.activePaneId).toBe("a");
    expect(store.closedPositions.get("b")?.rect.x).toBe(20);
  });

  test("togglePane closes when active, focuses when inactive, restores snapshot when reopening", () => {
    const store = new PaneStore();
    store.addPane({
      id: "events",
      kind: "events",
      title: "Events",
      rect: { x: 10, y: 10, width: 220, height: 120 },
      dismissable: true,
    });
    expect(store.activePaneId).toBe("events");

    store.togglePane("events", { width: 1000, height: 800 }, (snap) => {
      // Won't be called on close.
      return {
        id: "events",
        kind: "events",
        title: "Events",
        rect: snap?.rect ?? { x: 0, y: 0, width: 1, height: 1 },
        dismissable: true,
      };
    });
    expect(store.pane("events")).toBeUndefined();
    expect(store.closedPositions.has("events")).toBe(true);

    store.togglePane("events", { width: 1000, height: 800 }, (snap) => {
      return {
        id: "events",
        kind: "events",
        title: "Events (restored)",
        rect: snap?.rect ?? { x: 0, y: 0, width: 1, height: 1 },
        dismissable: true,
      };
    });
    expect(store.pane("events")?.title).toBe("Events (restored)");
    expect(store.pane("events")?.rect.x).toBe(10);
  });
});

describe("normalizePaneRect", () => {
  test("enforces min size and handles NaN", () => {
    expect(normalizePaneRect({ x: 0, y: 0, width: 10, height: 10 })).toEqual({
      x: 0,
      y: 0,
      width: 200,
      height: 100,
    });
    expect(normalizePaneRect({ x: 0, y: 0, width: Number.NaN, height: 10 })).toEqual({
      x: 0,
      y: 0,
      width: 200,
      height: 100,
    });
  });
});

