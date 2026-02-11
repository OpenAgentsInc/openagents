import { describe, expect, it } from "vitest";

import { clampPaneRectToScreen, parseStoredPaneRect } from "../../src/effuse-app/controllers/home/overlayLifecycle";

describe("apps/web home overlay lifecycle helpers", () => {
  it("parses valid stored pane rect and rejects invalid shapes", () => {
    expect(parseStoredPaneRect({ x: 12, y: 18, width: 640, height: 480 })).toEqual({
      x: 12,
      y: 18,
      width: 640,
      height: 480,
    });
    expect(parseStoredPaneRect({ x: 12, y: 18, width: 0, height: 480 })).toBeNull();
    expect(parseStoredPaneRect({ x: "12", y: 18, width: 640, height: 480 })).toBeNull();
    expect(parseStoredPaneRect(null)).toBeNull();
  });

  it("clamps pane rect dimensions and position to screen", () => {
    const clamped = clampPaneRectToScreen(
      { x: 9999, y: -50, width: 2000, height: 5 },
      { width: 1024, height: 768 },
    );

    expect(clamped.width).toBe(1024);
    expect(clamped.height).toBe(220);
    expect(clamped.x).toBe(0);
    expect(clamped.y).toBe(0);
  });
});
