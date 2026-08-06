import { describe, expect, it } from "vite-plus/test";

import { controllerLayout, CONTROLLER_CHAT_MAX_WIDTH } from "../src/controller/layout.ts";
import {
  initialFeedAnchorState,
  reduceFeedAnchor,
  shouldMaintainFeedEnd,
} from "../src/controller/feed-anchor.ts";
import { controllerLinking } from "../src/controller/routes.ts";

describe("mobile controller geometry and routes", () => {
  it("uses a stack for both phone orientations", () => {
    expect(controllerLayout(390, 844).mode).toBe("stack");
    expect(controllerLayout(844, 390).mode).toBe("stack");
  });

  it("uses a clamped split for both tablet orientations", () => {
    for (const [width, height] of [
      [1024, 1366],
      [1366, 1024],
    ] as const) {
      const layout = controllerLayout(width, height);
      expect(layout.mode).toBe("split");
      expect(layout.sidebarWidth).toBeGreaterThanOrEqual(280);
      expect(layout.sidebarWidth).toBeLessThanOrEqual(460);
      expect(layout.chatWidth).toBeLessThanOrEqual(CONTROLLER_CHAT_MAX_WIDTH);
    }
  });

  it("publishes every typed controller route as a deep link", () => {
    expect(Object.keys(controllerLinking.config.screens)).toEqual([
      "Home",
      "Inbox",
      "Thread",
      "Terminal",
      "Review",
      "Files",
      "Git",
      "Connections",
      "Intake",
      "NewTask",
      "Settings",
      "SarahVoice",
    ]);
  });
});

describe("bottom feed anchor laws", () => {
  it("starts at the end and follows row-height changes while the user remains there", () => {
    let state = initialFeedAnchorState;
    expect(shouldMaintainFeedEnd(state)).toBe(true);
    state = reduceFeedAnchor(state, { type: "initial_scroll" });
    state = reduceFeedAnchor(state, { type: "distance_from_end", distance: 24 });
    expect(shouldMaintainFeedEnd(state)).toBe(true);
  });

  it("retains visible content away from the end and freezes during disclosure", () => {
    let state = reduceFeedAnchor(initialFeedAnchorState, { type: "initial_scroll" });
    state = reduceFeedAnchor(state, { type: "distance_from_end", distance: 240 });
    expect(shouldMaintainFeedEnd(state)).toBe(false);
    state = reduceFeedAnchor(state, { type: "distance_from_end", distance: 0 });
    state = reduceFeedAnchor(state, { type: "disclosure", open: true });
    expect(shouldMaintainFeedEnd(state)).toBe(false);
    state = reduceFeedAnchor(state, { type: "disclosure", open: false });
    expect(shouldMaintainFeedEnd(state)).toBe(true);
  });
});
