/**
 * QA-3 (#8908): unit oracles for the visual-baseline diff gate — PNG
 * encode/decode roundtrip, identical-image match, bounded-threshold pass,
 * drift failure (including a synthetically altered image, proving the gate
 * catches drift), side-by-side artifact geometry, and manifest handling.
 */
import { describe, expect, test } from "vite-plus/test"
import {
  DEFAULT_DIFF_THRESHOLDS,
  decodePng,
  decodeVisualBaselineManifest,
  diffImages,
  encodePng,
  sideBySideImage,
  type DecodedImage,
} from "./visual-baseline-diff.ts"

const solidImage = (width: number, height: number, rgba: readonly [number, number, number, number]): DecodedImage => {
  const pixels = new Uint8Array(width * height * 4)
  for (let pixel = 0; pixel < width * height; pixel++) pixels.set(rgba, pixel * 4)
  return { width, height, pixels }
}

const withAlteredBlock = (image: DecodedImage, count: number): DecodedImage => {
  const pixels = new Uint8Array(image.pixels)
  for (let pixel = 0; pixel < count; pixel++) {
    pixels[pixel * 4] = 255
    pixels[pixel * 4 + 1] = 0
    pixels[pixel * 4 + 2] = 0
  }
  return { ...image, pixels }
}

describe("visual-baseline PNG codec", () => {
  test("encode/decode roundtrips RGBA pixels exactly", () => {
    const image = solidImage(17, 9, [5, 7, 13, 255])
    // A gradient exercises every PNG filter recovery path meaningfully.
    for (let index = 0; index < image.pixels.length; index += 4) {
      image.pixels[index] = index % 251
      image.pixels[index + 1] = (index * 3) % 249
      image.pixels[index + 2] = (index * 7) % 253
    }
    const decoded = decodePng(encodePng(image))
    expect(decoded.width).toBe(17)
    expect(decoded.height).toBe(9)
    expect(Array.from(decoded.pixels)).toEqual(Array.from(image.pixels))
  })

  test("rejects non-PNG bytes", () => {
    expect(() => decodePng(new Uint8Array([1, 2, 3, 4]))).toThrow(/PNG signature/)
  })
})

describe("visual-baseline diff", () => {
  test("identical images match", () => {
    const image = solidImage(40, 30, [10, 20, 30, 255])
    const result = diffImages(image, solidImage(40, 30, [10, 20, 30, 255]))
    expect(result.ok).toBe(true)
    expect(result.reason).toBe("match")
    expect(result.differentPixels).toBe(0)
  })

  test("per-channel jitter within tolerance passes", () => {
    const baseline = solidImage(40, 30, [10, 20, 30, 255])
    const current = solidImage(
      40,
      30,
      [10 + DEFAULT_DIFF_THRESHOLDS.perChannelTolerance, 20, 30, 255],
    )
    const result = diffImages(baseline, current)
    expect(result.ok).toBe(true)
    expect(result.differentPixels).toBe(0)
  })

  test("a handful of drastically changed pixels under the ratio still passes", () => {
    const baseline = solidImage(100, 100, [10, 20, 30, 255])
    // 10 of 10_000 pixels = 0.1% = exactly the bounded default ratio.
    const result = diffImages(baseline, withAlteredBlock(baseline, 10))
    expect(result.ok).toBe(true)
    expect(result.differentPixels).toBe(10)
  })

  test("drift beyond the bounded ratio fails with a populated diff mask", () => {
    const baseline = solidImage(100, 100, [10, 20, 30, 255])
    const result = diffImages(baseline, withAlteredBlock(baseline, 500))
    expect(result.ok).toBe(false)
    expect(result.reason).toBe("pixel_drift")
    expect(result.differentPixels).toBe(500)
    expect(result.diffMask).not.toBeNull()
    // Drifted pixels are marked magenta-opaque in the mask.
    expect(result.diffMask!.pixels[0]).toBe(255)
    expect(result.diffMask!.pixels[3]).toBe(255)
    // Unchanged pixels stay transparent.
    expect(result.diffMask!.pixels[500 * 4 + 3]).toBe(0)
  })

  test("the gate catches a synthetically altered encoded image end to end", () => {
    // The committed-baseline flow in miniature: encode a baseline PNG, alter
    // the current render, decode both, and prove the diff fails nonzero-style.
    const baseline = solidImage(64, 64, [5, 7, 13, 255])
    const baselineBytes = encodePng(baseline)
    const altered = withAlteredBlock(baseline, 1_000)
    const alteredBytes = encodePng(altered)
    const result = diffImages(decodePng(baselineBytes), decodePng(alteredBytes))
    expect(result.ok).toBe(false)
    expect(result.differentPixels).toBe(1_000)
  })

  test("dimension mismatch fails closed", () => {
    const result = diffImages(solidImage(10, 10, [0, 0, 0, 255]), solidImage(11, 10, [0, 0, 0, 255]))
    expect(result.ok).toBe(false)
    expect(result.reason).toBe("dimensions_mismatch")
  })
})

describe("visual-baseline side-by-side artifact", () => {
  test("composes baseline, current, and mask panels horizontally", () => {
    const baseline = solidImage(10, 8, [1, 2, 3, 255])
    const current = solidImage(10, 8, [4, 5, 6, 255])
    const mask = solidImage(10, 8, [255, 0, 255, 255])
    const composed = sideBySideImage(baseline, current, mask)
    expect(composed.width).toBe(30)
    expect(composed.height).toBe(8)
    expect(composed.pixels[0]).toBe(1)
    expect(composed.pixels[10 * 4]).toBe(4)
    expect(composed.pixels[20 * 4]).toBe(255)
  })
})

describe("visual-baseline manifest", () => {
  const validManifest = {
    schema: "openagents-desktop.visual-baselines.v1",
    platform: "darwin-arm64",
    timezone: "UTC",
    window: { width: 1280, height: 800, deviceScaleFactor: 1 },
    thresholds: { perChannelTolerance: 3, maxDifferentPixelRatio: 0.001 },
    states: [{ name: "composer-idle", file: "composer-idle.png", sha256: "ab".repeat(32), width: 1280, height: 800 }],
  }

  test("decodes a valid manifest", () => {
    const manifest = decodeVisualBaselineManifest(validManifest)
    expect(manifest).not.toBeNull()
    expect(manifest!.states[0]!.name).toBe("composer-idle")
  })

  test("rejects a wrong schema tag and malformed shapes", () => {
    expect(decodeVisualBaselineManifest({ ...validManifest, schema: "wrong.v0" })).toBeNull()
    expect(decodeVisualBaselineManifest({ ...validManifest, states: [{ name: "x" }] })).toBeNull()
    expect(decodeVisualBaselineManifest(null)).toBeNull()
  })
})
