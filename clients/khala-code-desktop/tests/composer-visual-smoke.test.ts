import { describe, expect, test } from "bun:test"

import {
  assertCanvasProbe,
  assertComposerGeometry,
  composerVisualPlan,
  validatePublicSafeComposerPrompt,
} from "../scripts/composer-visual-smoke"

describe("composer visual smoke", () => {
  test("covers the desktop composer, public chat composer, and HUD scene", () => {
    const plan = composerVisualPlan()

    expect(validatePublicSafeComposerPrompt(plan.prompt)).toBe(true)
    expect(plan.targets.map(target => target.name)).toEqual([
      "khala-code-desktop",
      "openagents-khala-chat",
      "openagents-autopilot-hud",
    ])
    expect(plan.viewports.map(viewport => viewport.name)).toEqual([
      "desktop",
      "mobile",
    ])
    expect(plan.targets[0]?.canvasSelector).toBe("#composer-hud canvas")
    expect(plan.targets[2]?.canvasSelector).toBe("oa-landing-squares")
  })

  test("rejects prompts that look like private material", () => {
    expect(validatePublicSafeComposerPrompt("token abc")).toBe(false)
    expect(validatePublicSafeComposerPrompt("open ~/.codex/auth.json")).toBe(false)
    expect(
      validatePublicSafeComposerPrompt(
        "Synthetic visual smoke prompt with aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      ),
    ).toBe(false)
  })

  test("accepts stable composer geometry and rejects overlapping footer controls", () => {
    assertComposerGeometry({
      composer: { x: 12, y: 600, width: 760, height: 120 },
      footer: { x: 22, y: 682, width: 740, height: 28 },
      input: { x: 22, y: 612, width: 740, height: 54 },
    })

    expect(() =>
      assertComposerGeometry({
        composer: { x: 12, y: 600, width: 760, height: 120 },
        footer: { x: 22, y: 642, width: 740, height: 28 },
        input: { x: 22, y: 612, width: 740, height: 54 },
      }),
    ).toThrow("overlap")
  })

  test("requires nonblank pixels for canvas-backed HUD captures", () => {
    assertCanvasProbe("desktop", {
      found: true,
      width: 760,
      height: 120,
      sampledPixels: 16,
      nonBlankPixels: 4,
    })
    expect(() =>
      assertCanvasProbe("desktop", {
        found: true,
        width: 760,
        height: 120,
        sampledPixels: 16,
        nonBlankPixels: 0,
      }),
    ).toThrow("blank")
    expect(() => assertCanvasProbe("desktop", null)).not.toThrow()
  })
})
