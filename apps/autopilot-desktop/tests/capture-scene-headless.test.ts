import { describe, expect, test } from "bun:test"

import {
  captureOutputDir,
  parseSceneCaptureArgs,
} from "../scripts/isolated-scenes/capture-target"

describe("capture-scene-headless target parsing", () => {
  test("accepts a registered scene target", () => {
    const parsed = parseSceneCaptureArgs(["verse-arc", "tmp/arc.png"], "/repo")
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return
    expect(parsed.target.kind).toBe("registered-scene")
    expect(parsed.target.outputPath).toBe("/repo/tmp/arc.png")
    if (parsed.target.kind === "registered-scene") {
      expect(parsed.target.scene.name).toBe("verse-arc")
    }
  })

  test("threads registered scene query params through to the page", () => {
    const parsed = parseSceneCaptureArgs(["verse-arc?broken=1", "arc.png"], "/repo")
    expect(parsed.ok).toBe(true)
    if (!parsed.ok || parsed.target.kind !== "registered-scene") return
    expect(parsed.target.pageQuery).toBe("broken=1")
  })

  test("accepts an already-served URL target", () => {
    const parsed = parseSceneCaptureArgs(
      ["http://127.0.0.1:5188/?spawn=1", "out/scene.png"],
      "/repo",
    )
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return
    expect(parsed.target).toMatchObject({
      kind: "url",
      url: "http://127.0.0.1:5188/?spawn=1",
      outputPath: "/repo/out/scene.png",
    })
  })

  test("rejects unknown scene names with the registry usage", () => {
    const parsed = parseSceneCaptureArgs(["missing", "out.png"], "/repo")
    expect(parsed.ok).toBe(false)
    if (parsed.ok) return
    expect(parsed.message).toContain("Known scenes: verse-arc, pylon-network")
  })

  test("reports the output directory", () => {
    const parsed = parseSceneCaptureArgs(["pylon-network", "out/pylon.png"], "/repo")
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return
    expect(captureOutputDir(parsed.target)).toBe("/repo/out")
  })
})
