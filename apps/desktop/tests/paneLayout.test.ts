import { describe, expect, it } from "@effect/vitest";
import { PaneStore } from "@openagentsinc/effuse-panes";

import {
  DESKTOP_PANE_SPEC_BY_ID,
  addInitialDesktopPanes,
  focusOrOpenDesktopPane,
  toggleDesktopPane,
} from "../src/effect/paneLayout";

const screen = { width: 1280, height: 800 } as const;

describe("desktop pane layout controller", () => {
  it("adds all expected panes with stable ids", () => {
    const store = new PaneStore();
    addInitialDesktopPanes(store, screen);

    expect(store.panes().length).toBe(Object.keys(DESKTOP_PANE_SPEC_BY_ID).length);
    expect(store.pane("desktop-overview")?.dismissable).toBe(false);
    expect(store.pane("desktop-transactions")?.dismissable).toBe(true);
  });

  it("focuses existing panes and opens missing panes", () => {
    const store = new PaneStore();
    addInitialDesktopPanes(store, screen);
    store.removePane("desktop-node", true);
    expect(store.pane("desktop-node")).toBeUndefined();

    focusOrOpenDesktopPane(store, screen, "desktop-wallet");
    expect(store.activePaneId).toBe("desktop-wallet");

    focusOrOpenDesktopPane(store, screen, "desktop-node");
    expect(store.pane("desktop-node")).toBeDefined();
    expect(store.activePaneId).toBe("desktop-node");
  });

  it("toggles a pane closed then re-opens it from stored rect", () => {
    const store = new PaneStore();
    addInitialDesktopPanes(store, screen);

    const before = store.pane("desktop-transactions");
    expect(before).toBeDefined();
    if (!before) return;

    toggleDesktopPane(store, screen, "desktop-transactions");
    expect(store.pane("desktop-transactions")).toBeUndefined();
    const stored = store.closedPositions.get("desktop-transactions");
    expect(stored).toBeDefined();

    toggleDesktopPane(store, screen, "desktop-transactions");
    const reopened = store.pane("desktop-transactions");
    expect(reopened).toBeDefined();
    expect(reopened?.rect).toEqual(before.rect);
  });
});
