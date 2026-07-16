/**
 * QA-3 (#8908): oracles over the COMMITTED baselines in `visual-baselines/`.
 * The manifest and PNGs must agree (names, hashes, dimensions, capture
 * geometry), and the diff gate must catch a deliberate drift introduced into
 * a real committed baseline image — the issue's "one drift deliberately
 * introduced proving the gate catches it".
 */
import { readFileSync } from "node:fs"
import { createHash } from "node:crypto"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"
import {
  decodePng,
  decodeVisualBaselineManifest,
  diffImages,
  encodePng,
} from "./visual-baseline-diff.ts"
import {
  VISUAL_BASELINE_DEVICE_SCALE_FACTOR,
  VISUAL_BASELINE_STATES,
  VISUAL_BASELINE_WINDOW,
} from "./visual-baseline-contract.ts"

const baselinesDir = path.resolve(import.meta.dirname, "..", "visual-baselines")

const loadManifest = () =>
  decodeVisualBaselineManifest(JSON.parse(readFileSync(path.join(baselinesDir, "manifest.json"), "utf8")))

describe("committed visual baselines", () => {
  test("manifest decodes and covers exactly the fixed capture set", () => {
    const manifest = loadManifest()
    expect(manifest).not.toBeNull()
    expect(manifest!.states.map(state => state.name)).toEqual([...VISUAL_BASELINE_STATES])
    expect(manifest!.window).toEqual({
      width: VISUAL_BASELINE_WINDOW.width,
      height: VISUAL_BASELINE_WINDOW.height,
      deviceScaleFactor: VISUAL_BASELINE_DEVICE_SCALE_FACTOR,
    })
    expect(manifest!.timezone).toBe("UTC")
  })

  test("every baseline PNG matches its manifest hash and dimensions", () => {
    const manifest = loadManifest()!
    for (const state of manifest.states) {
      const bytes = new Uint8Array(readFileSync(path.join(baselinesDir, state.file)))
      expect(createHash("sha256").update(bytes).digest("hex")).toBe(state.sha256)
      const image = decodePng(bytes)
      expect({ width: image.width, height: image.height }).toEqual({
        width: state.width,
        height: state.height,
      })
    }
  })

  test("the gate catches a deliberate drift in a real committed baseline", () => {
    const manifest = loadManifest()!
    const entry = manifest.states[0]!
    const bytes = new Uint8Array(readFileSync(path.join(baselinesDir, entry.file)))
    const baseline = decodePng(bytes)
    // Synthetic alteration: repaint a 64x64 block — a badge-sized UI drift.
    const altered = { ...baseline, pixels: new Uint8Array(baseline.pixels) }
    for (let y = 100; y < 164; y++) {
      for (let x = 100; x < 164; x++) {
        const offset = (y * baseline.width + x) * 4
        altered.pixels[offset] = 255
        altered.pixels[offset + 1] = 0
        altered.pixels[offset + 2] = 0
        altered.pixels[offset + 3] = 255
      }
    }
    // Roundtrip through PNG bytes so this is the gate's real decode path.
    const result = diffImages(baseline, decodePng(encodePng(altered)), manifest.thresholds)
    expect(result.ok).toBe(false)
    expect(result.reason).toBe("pixel_drift")
    expect(result.differentPixels).toBeGreaterThanOrEqual(64 * 64)
    // And the unaltered image still passes the same gate.
    expect(diffImages(baseline, decodePng(bytes), manifest.thresholds).ok).toBe(true)
  })
})
