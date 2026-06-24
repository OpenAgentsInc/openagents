import { describe, expect, test } from "bun:test"

import { isolatedSceneDefinitions } from "../scripts/isolated-scenes/registry"
import {
  assertSceneRendered,
  scoreSceneRenderSignature,
  type SceneRenderSignature,
} from "../scripts/isolated-scenes/render-gate"
import type { PngImage } from "../src/testing/headless-pixel"

const image = (
  width: number,
  height: number,
  paint: (x: number, y: number) => readonly [number, number, number, number],
): PngImage => {
  const data = new Uint8Array(width * height * 4)
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = (y * width + x) * 4
      const [r, g, b, a] = paint(x, y)
      data[offset] = r
      data[offset + 1] = g
      data[offset + 2] = b
      data[offset + 3] = a
    }
  }
  return { data, width, height }
}

const signature: SceneRenderSignature = {
  label: "test scene",
  minBrightPixels: 4,
  minDistinctLumaBuckets: 2,
}

describe("isolated scene render gate", () => {
  test("rejects a scene that mounts but renders a blank frame", () => {
    const blank = image(8, 8, () => [0, 0, 0, 255])

    expect(() => assertSceneRendered(blank, signature)).toThrow(
      "Scene render gate failed for test scene",
    )
  })

  test("accepts a scene with a real bright pixel footprint", () => {
    const rendered = image(8, 8, (x, y) =>
      x >= 2 && x < 6 && y >= 2 && y < 6
        ? [140, 190, 255, 255]
        : [4, 5, 8, 255],
    )

    const result = assertSceneRendered(rendered, signature)
    expect(result.ok).toBe(true)
    expect(result.brightPixels).toBe(16)
    expect(result.distinctLumaBuckets).toBeGreaterThanOrEqual(2)
  })

  test("respects per-scene signature regions", () => {
    const rendered = image(10, 10, (x, y) =>
      x >= 7 && y >= 7 ? [255, 255, 255, 255] : [0, 0, 0, 255],
    )
    const result = scoreSceneRenderSignature(rendered, {
      label: "top-left",
      minBrightPixels: 1,
      minDistinctLumaBuckets: 2,
      region: { x0: 0, y0: 0, x1: 0.5, y1: 0.5 },
    })

    expect(result.ok).toBe(false)
    expect(result.brightPixels).toBe(0)
  })

  test("every registered isolated scene declares a render signature", () => {
    expect(isolatedSceneDefinitions.every((definition) =>
      definition.renderSignature.minBrightPixels > 0 &&
      definition.renderSignature.minDistinctLumaBuckets > 1 &&
      definition.issueRefs.includes("github:OpenAgentsInc/openagents#6047"),
    )).toBe(true)
  })
})
