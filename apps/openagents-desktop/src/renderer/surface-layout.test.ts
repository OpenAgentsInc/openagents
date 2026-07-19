import { describe, expect, test } from "vite-plus/test"

import {
  decodeDesktopSurfaceLayout,
  defaultDesktopSurfaceLayout,
  reduceDesktopSurfaceLayout,
} from "./surface-layout.ts"

describe("desktop surface layout", () => {
  test("adds, activates, and closes singleton capability surfaces predictably", () => {
    const review = reduceDesktopSurfaceLayout(defaultDesktopSurfaceLayout(), { type: "open", surface: "review" })
    const terminal = reduceDesktopSurfaceLayout(review, { type: "open", surface: "terminal" })
    expect(terminal).toMatchObject({ surfaces: ["review", "terminal"], active: "terminal" })
    expect(reduceDesktopSurfaceLayout(terminal, { type: "close", surface: "terminal" })).toMatchObject({ surfaces: ["review"], active: "review" })
  })

  test("toggles an admitted right-side surface without discarding its peers", () => {
    const opened = reduceDesktopSurfaceLayout(defaultDesktopSurfaceLayout(), { type: "toggle", surface: "review" })
    expect(opened).toMatchObject({ surfaces: ["review"], active: "review" })
    expect(reduceDesktopSurfaceLayout(opened, { type: "toggle", surface: "review" }))
      .toMatchObject({ surfaces: [], active: null })

    const terminal = reduceDesktopSurfaceLayout(defaultDesktopSurfaceLayout(), { type: "open", surface: "terminal" })
    expect(reduceDesktopSurfaceLayout(terminal, { type: "toggle", surface: "review" }))
      .toMatchObject({ surfaces: ["terminal", "review"], active: "review" })
  })

  test("supports close others, close right, close all, maximize, and bounded resize", () => {
    const both = { ...defaultDesktopSurfaceLayout(), surfaces: ["review", "terminal"] as const, active: "review" as const }
    expect(reduceDesktopSurfaceLayout(both, { type: "close_right", surface: "review" }).surfaces).toEqual(["review"])
    expect(reduceDesktopSurfaceLayout(both, { type: "close_others", surface: "terminal" })).toMatchObject({ surfaces: ["terminal"], active: "terminal" })
    expect(reduceDesktopSurfaceLayout(both, { type: "toggle_maximized" }).maximized).toBe(true)
    expect(reduceDesktopSurfaceLayout(both, { type: "resize", width: 4 }).width).toBe(320)
    expect(reduceDesktopSurfaceLayout(both, { type: "resize", width: 2_000 }).width).toBe(960)
    expect(reduceDesktopSurfaceLayout(both, { type: "close_all" })).toMatchObject({ surfaces: [], active: null, maximized: false })
  })

  test("decodes persisted presentation state without admitting unknown surfaces", () => {
    expect(decodeDesktopSurfaceLayout({
      surfaces: ["files", "unknown", "review", "terminal"],
      active: "files",
      maximized: true,
      width: Number.NaN,
    })).toEqual({ version: 1, surfaces: ["review", "terminal"], active: "terminal", maximized: true, width: 440 })
    expect(decodeDesktopSurfaceLayout("corrupt")).toEqual(defaultDesktopSurfaceLayout())
  })
})
