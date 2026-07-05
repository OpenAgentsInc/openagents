import { describe, expect, test } from "bun:test"

import {
  assertEditorMonacoDomProbe,
  assertEditorPixelProbe,
  assertEditorVisualGeometry,
  EDITOR_VISUAL_SMOKE_HARNESS,
  editorVisualSmokeViewports,
} from "../scripts/editor-visual-smoke"

describe("Phase 1 editor visual smoke", () => {
  test("registers the fixture-only desktop editor visual smoke", async () => {
    const packageJson = await Bun.file(new URL("../package.json", import.meta.url)).text()
    const readme = await Bun.file(new URL("../README.md", import.meta.url)).text()

    expect(EDITOR_VISUAL_SMOKE_HARNESS).toBe("khala_code_phase1_editor_visual_smoke")
    expect(editorVisualSmokeViewports()).toEqual([
      { name: "desktop", width: 1280, height: 800 },
    ])
    expect(packageJson).toContain('"smoke:editor-visual"')
    expect(packageJson).toContain("scripts/editor-visual-smoke.ts")
    expect(readme).toContain("bun run smoke:editor-visual")
  })

  test("accepts split editor geometry and rejects blank or overlapping renders", () => {
    assertEditorVisualGeometry({
      editorPanel: { x: 84, y: 0, width: 1196, height: 800 },
      monacoHost: { x: 390, y: 49, width: 890, height: 751 },
      sourcePane: { x: 390, y: 0, width: 890, height: 800 },
      treePane: { x: 84, y: 0, width: 306, height: 800 },
      treeRow: { x: 102, y: 90, width: 260, height: 28 },
      viewport: { x: 0, y: 0, width: 1280, height: 800 },
    })
    assertEditorMonacoDomProbe({
      hasFixtureSource: true,
      lineCount: 5,
      text: "export const answer = 42",
    })
    assertEditorPixelProbe({
      height: 640,
      nonBlankPixels: 12,
      sampledPixels: 100,
      width: 860,
    })

    expect(() =>
      assertEditorVisualGeometry({
        editorPanel: { x: 84, y: 0, width: 1196, height: 800 },
        monacoHost: { x: 200, y: 49, width: 200, height: 200 },
        sourcePane: { x: 240, y: 0, width: 360, height: 800 },
        treePane: { x: 84, y: 0, width: 306, height: 800 },
        treeRow: { x: 102, y: 90, width: 260, height: 28 },
        viewport: { x: 0, y: 0, width: 1280, height: 800 },
      }),
    ).toThrow("too narrow")

    expect(() =>
      assertEditorMonacoDomProbe({
        hasFixtureSource: false,
        lineCount: 0,
        text: "",
      }),
    ).toThrow("too few lines")

    expect(() =>
      assertEditorPixelProbe({
        height: 640,
        nonBlankPixels: 0,
        sampledPixels: 100,
        width: 860,
      }),
    ).toThrow("blank")
  })
})
