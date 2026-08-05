import { describe, expect, test } from "vite-plus/test";

import { colorTokens, controlTokens, defaultTheme, khalaTheme, spacingTokens } from "./index.ts";
import type { Theme } from "./index.ts";

// The move out of the vendored Effect Native tokens package (openagents#9325
// packet 2) had to be value-preserving: the same token names, the same theme
// shape, and the same resolved hex for the roles other surfaces mirror by hand.
// These are the pins that make a drift visible in review.

describe("the theme covers the whole closed lattice", () => {
  const themes: ReadonlyArray<readonly [string, Theme]> = [
    ["khalaTheme", khalaTheme],
    ["defaultTheme", defaultTheme],
  ];

  test.each(themes)("%s names every color token exactly once", (_name, theme) => {
    expect(Object.keys(theme.color).toSorted()).toEqual([...colorTokens].toSorted());
  });

  test.each(themes)("%s names every spacing token exactly once", (_name, theme) => {
    expect(Object.keys(theme.spacing).toSorted()).toEqual([...spacingTokens].toSorted());
  });

  test.each(themes)("%s names every control-lattice step exactly once", (_name, theme) => {
    expect(Object.keys(theme.control).toSorted()).toEqual([...controlTokens].toSorted());
  });
});

describe("khalaTheme resolves the pinned hex values", () => {
  // The roles `apps/aiur/src/effect-native-theme.css` mirrors as static
  // `--en-color-*` custom properties, and that its parity test asserts against.
  const pins: ReadonlyArray<readonly [keyof typeof khalaTheme.color, string]> = [
    ["background", "#05070d"],
    ["surface", "#0b1220"],
    ["surfaceRaised", "#141f36"],
    ["border", "#1f2b45"],
    ["borderStrong", "#2c3d63"],
    ["accent", "#3b82f6"],
    ["accentHover", "#5c96f8"],
    ["info", "#38bdf8"],
    ["textPrimary", "#eef3ff"],
    ["textMuted", "#93a4c3"],
    ["textFaint", "#6b7ca1"],
    ["success", "#22c55e"],
    ["warning", "#f59e0b"],
    ["danger", "#f87171"],
  ];

  test.each(pins)("color.%s is %s", (role, value) => {
    expect(khalaTheme.color[role]).toBe(value);
  });

  test("state overlays stay translucent overlays of one base hue", () => {
    expect(khalaTheme.color.stateHover).toBe("#8fb3ff14");
    expect(khalaTheme.color.stateActive).toBe("#8fb3ff21");
    expect(khalaTheme.color.stateSelected).toBe("#3b82f629");
  });
});

describe("the theme carries the parts token lowering reads", () => {
  test("every typeScale step carries size, line height and weight", () => {
    for (const step of Object.values(khalaTheme.typeScale)) {
      expect(step.fontSize).toBeGreaterThan(0);
      expect(step.lineHeight).toBeGreaterThan(0);
      expect([400, 500, 600, 700]).toContain(step.fontWeight);
    }
  });

  test("every control step sizes height, gutter, radius, label and icon together", () => {
    for (const step of Object.values(khalaTheme.control)) {
      expect(step.height).toBeGreaterThan(0);
      expect(step.gutter).toBeGreaterThanOrEqual(0);
      expect(step.radius).toBeGreaterThanOrEqual(0);
      expect(step.fontSize).toBeGreaterThan(0);
      expect(step.icon).toBeGreaterThan(0);
    }
  });

  test("motion carries the named duration and easing vocabulary", () => {
    expect(khalaTheme.motion.durationFastMs).toBeGreaterThan(0);
    expect(khalaTheme.motion.easeBasic.length).toBeGreaterThan(0);
    expect(khalaTheme.motion.easeEnter.length).toBeGreaterThan(0);
    expect(khalaTheme.motion.easeExit.length).toBeGreaterThan(0);
  });

  test("elevation carries the overlay shadow and hairline width", () => {
    expect(khalaTheme.elevation.overlayShadow.length).toBeGreaterThan(0);
    expect(khalaTheme.elevation.hairlineWidth).toBeGreaterThan(0);
  });

  test("the khalaUi lattice survives the move", () => {
    expect(khalaTheme.khalaUi.edgeWidth.hairline).toBe(1);
    expect(khalaTheme.khalaUi.cutSize.medium).toBe(8);
    expect(khalaTheme.khalaUi.focusClearance).toBe(4);
  });
});
