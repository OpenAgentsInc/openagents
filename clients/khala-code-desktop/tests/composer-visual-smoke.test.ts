import { describe, expect, test } from "bun:test"

import {
  assertCanvasProbe,
  assertCommandPaletteProbe,
  COMPOSER_VISUAL_SMOKE_HARNESS,
  assertComposerGeometry,
  assertFocusProbe,
  assertReducedMotionProbe,
  composerVisualPlan,
  validatePublicSafeComposerPrompt,
} from "../scripts/composer-visual-smoke"

describe("composer visual smoke", () => {
  test("covers the desktop composer, public chat composer, and HUD scene", () => {
    const plan = composerVisualPlan()

    expect(validatePublicSafeComposerPrompt(plan.prompt)).toBe(true)
    expect(plan.prompt).toContain("\n")
    expect(plan.prompt.length).toBeGreaterThan(100)
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
    expect(plan.targets[0]?.commandPaletteSelector).toBe(".khala-code-command-palette")
    expect(plan.targets[1]?.commandPaletteSelector).toBeNull()
    expect(plan.targets[2]?.canvasSelector).toBe("oa-landing-squares")
    expect(COMPOSER_VISUAL_SMOKE_HARNESS).toBe("preview_ui_codex_harness_shell")
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
      footerChildren: [
        { x: 22, y: 682, width: 110, height: 28 },
        { x: 610, y: 682, width: 94, height: 28 },
        { x: 712, y: 682, width: 32, height: 28 },
      ],
      footer: { x: 22, y: 682, width: 740, height: 28 },
      input: { x: 22, y: 612, width: 740, height: 54 },
      viewport: { x: 0, y: 0, width: 1280, height: 800 },
    })

    expect(() =>
      assertComposerGeometry({
        composer: { x: 12, y: 600, width: 760, height: 120 },
        footerChildren: [],
        footer: { x: 22, y: 642, width: 740, height: 28 },
        input: { x: 22, y: 612, width: 740, height: 54 },
        viewport: { x: 0, y: 0, width: 1280, height: 800 },
      }),
    ).toThrow("overlap")

    expect(() =>
      assertComposerGeometry({
        composer: { x: 0, y: 700, width: 392, height: 120 },
        footerChildren: [
          { x: 12, y: 780, width: 96, height: 28 },
          { x: 100, y: 780, width: 96, height: 28 },
        ],
        footer: { x: 12, y: 780, width: 360, height: 28 },
        input: { x: 12, y: 712, width: 360, height: 54 },
        viewport: { x: 0, y: 0, width: 390, height: 844 },
      }),
    ).toThrow("viewport")

    expect(() =>
      assertComposerGeometry({
        composer: { x: 12, y: 600, width: 760, height: 120 },
        footerChildren: [
          { x: 22, y: 682, width: 120, height: 28 },
          { x: 120, y: 682, width: 120, height: 28 },
        ],
        footer: { x: 22, y: 682, width: 740, height: 28 },
        input: { x: 22, y: 612, width: 740, height: 54 },
        viewport: { x: 0, y: 0, width: 1280, height: 800 },
      }),
    ).toThrow("footer controls overlap")
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

  test("requires command palette visual probes to be visible and selected", () => {
    assertCommandPaletteProbe("khala-code-desktop", {
      visible: true,
      panel: { x: 320, y: 80, width: 640, height: 360 },
      input: { x: 344, y: 104, width: 592, height: 44 },
      list: { x: 344, y: 164, width: 592, height: 240 },
      resultCount: 2,
      selectedResultId: "view.chat",
      screenshot: "khala-code-desktop-desktop-command-palette.png",
      viewport: { x: 0, y: 0, width: 1280, height: 800 },
    })
    expect(() =>
      assertCommandPaletteProbe("khala-code-desktop", {
        visible: false,
        panel: { x: 320, y: 80, width: 640, height: 360 },
        input: { x: 344, y: 104, width: 592, height: 44 },
        list: { x: 344, y: 164, width: 592, height: 240 },
        resultCount: 2,
        selectedResultId: "view.chat",
        screenshot: "khala-code-desktop-desktop-command-palette.png",
        viewport: { x: 0, y: 0, width: 1280, height: 800 },
      }),
    ).toThrow("visible")
    expect(() =>
      assertCommandPaletteProbe("khala-code-desktop", {
        visible: true,
        panel: { x: 0, y: 0, width: 260, height: 120 },
        input: { x: 12, y: 12, width: 236, height: 40 },
        list: { x: 12, y: 64, width: 236, height: 40 },
        resultCount: 0,
        selectedResultId: null,
        screenshot: "",
        viewport: { x: 0, y: 0, width: 390, height: 844 },
      }),
    ).toThrow("too small")
    expect(() => assertCommandPaletteProbe("khala-code-desktop", null)).not.toThrow()
  })

  test("requires focus framing and reduced-motion-safe transitions", () => {
    assertFocusProbe("khala-code-desktop", {
      activeElementMatchesInput: true,
      borderColor: "rgb(79, 208, 255)",
      boxShadow: "rgb(79, 208, 255) 0px 0px 0px 1px inset",
      focusedBorderColor: "rgb(79, 208, 255)",
      hasVisibleFrame: true,
    })
    expect(() =>
      assertFocusProbe("khala-code-desktop", {
        activeElementMatchesInput: false,
        borderColor: "rgb(79, 208, 255)",
        boxShadow: "none",
        focusedBorderColor: "rgb(79, 208, 255)",
        hasVisibleFrame: true,
      }),
    ).toThrow("focus")
    expect(() =>
      assertFocusProbe("khala-code-desktop", {
        activeElementMatchesInput: true,
        borderColor: "rgba(0, 0, 0, 0)",
        boxShadow: "none",
        focusedBorderColor: "rgb(79, 208, 255)",
        hasVisibleFrame: true,
      }),
    ).toThrow("transparent")

    assertReducedMotionProbe(
      "openagents-khala-chat",
      { matchesMedia: true, transitionDurationMs: 0 },
      true,
    )
    assertReducedMotionProbe(
      "openagents-khala-chat",
      { matchesMedia: false, transitionDurationMs: 160 },
      false,
    )
    expect(() =>
      assertReducedMotionProbe(
        "openagents-khala-chat",
        { matchesMedia: true, transitionDurationMs: 160 },
        true,
      ),
    ).toThrow("reduced motion")
  })
})
