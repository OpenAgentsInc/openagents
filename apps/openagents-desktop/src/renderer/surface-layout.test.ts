import { describe, expect, test } from "vite-plus/test"

import {
  decodeDesktopSurfaceLayout,
  defaultDesktopSurfaceLayout,
  reduceDesktopSurfaceLayout,
} from "./surface-layout.ts"

describe("desktop surface layout", () => {
  test("adds, activates, and closes singleton capability surfaces predictably", () => {
    const files = reduceDesktopSurfaceLayout(defaultDesktopSurfaceLayout(), { type: "open", surface: "files" })
    const review = reduceDesktopSurfaceLayout(files, { type: "open", surface: "review" })
    expect(review).toMatchObject({ surfaces: ["files", "review"], active: "review" })
    expect(reduceDesktopSurfaceLayout(review, { type: "close", surface: "review" })).toMatchObject({ surfaces: ["files"], active: "files" })
  })

  test("supports close others, close right, close all, maximize, and bounded resize", () => {
    const both = { ...defaultDesktopSurfaceLayout(), surfaces: ["files", "review"] as const, active: "files" as const }
    expect(reduceDesktopSurfaceLayout(both, { type: "close_right", surface: "files" }).surfaces).toEqual(["files"])
    expect(reduceDesktopSurfaceLayout(both, { type: "close_others", surface: "review" })).toMatchObject({ surfaces: ["review"], active: "review" })
    expect(reduceDesktopSurfaceLayout(both, { type: "toggle_maximized" }).maximized).toBe(true)
    expect(reduceDesktopSurfaceLayout(both, { type: "resize", width: 4 }).width).toBe(320)
    expect(reduceDesktopSurfaceLayout(both, { type: "resize", width: 2_000 }).width).toBe(960)
    expect(reduceDesktopSurfaceLayout(both, { type: "close_all" })).toMatchObject({ surfaces: [], active: null, maximized: false })
  })

  test("decodes persisted presentation state without admitting unknown surfaces", () => {
    expect(decodeDesktopSurfaceLayout({
      surfaces: ["files", "browser", "files", "review"],
      active: "browser",
      maximized: true,
      width: Number.NaN,
    })).toEqual({ version: 1, surfaces: ["files", "review"], active: "review", maximized: true, width: 440 })
    expect(decodeDesktopSurfaceLayout("corrupt")).toEqual(defaultDesktopSurfaceLayout())
  })
})
