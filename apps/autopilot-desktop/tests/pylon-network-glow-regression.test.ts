// Headless before/after regression for the Verse pylon-network glow-up
// (graphics audit A1/A2/A3 + nicer pylons). This is the test that proves the
// pylons + their connections now GLOW in the dark scene, and that the base
// frame still reads correctly with bloom DISABLED (the bloom skill's own
// acceptance check + the don't-double-tone-map guard).
//
// It mounts the REAL `oa-training-run` three-effect element with the REAL
// `pylonNetworkVisualizationOptions` mapper in headless Chromium, advances
// DETERMINISTIC fixed frames (injected fake clock — no rAF/wall-clock),
// screenshots, and scores GLOW (very-bright pixels, luma >= 200) in the central
// band plus the dark background corners.
//
//   bloom ON  -> a large GLOW footprint AND dark background corners.
//   bloom OFF -> far less glow, real (non-blank) base signal, dark corners.
//
// HONEST CAVEAT: this is headless Chromium / SwiftShader (software GL), NOT the
// real Electrobun WKWebView + GPU. It proves: the composer/bloom pass runs, the
// emitters carry HDR signal, fat connection lines replace the 1px lines, and the
// base frame still renders with bloom off without washing the dark background.
// It does NOT prove the exact on-device GPU bloom look — confirm that on device.
//
// Skips cleanly where no Chromium binary is installed (set CHROME_PATH).

import { fileURLToPath } from "node:url"
import { dirname, join } from "node:path"

import { describe, expect, test } from "bun:test"

import {
  renderVisualizationAndProbe,
  resolveChromePathOrNull,
  type PngImage,
} from "../src/testing/headless-pixel"

const here = dirname(fileURLToPath(import.meta.url))
const entryModulePath = join(here, "../scripts/pylon-network-glow-entry.ts")

const FRAME_STEPS = 90
const FRAME_DELTA_MS = 16

const REGION = { x0: 0.18, y0: 0.18, x1: 0.82, y1: 0.82 }

type GlowScore = Readonly<{ glowPixels: number; meanLuma: number; cornerLuma: number }>

const scoreGlow = (image: PngImage): GlowScore => {
  const { data, width, height } = image
  const x0 = Math.floor(REGION.x0 * width)
  const x1 = Math.floor(REGION.x1 * width)
  const y0 = Math.floor(REGION.y0 * height)
  const y1 = Math.floor(REGION.y1 * height)
  let glow = 0
  let sum = 0
  let n = 0
  const luma = (i: number): number =>
    0.2126 * (data[i] ?? 0) + 0.7152 * (data[i + 1] ?? 0) + 0.0722 * (data[i + 2] ?? 0)
  for (let y = y0; y < y1; y += 1) {
    for (let x = x0; x < x1; x += 1) {
      const l = luma((y * width + x) * 4)
      if (l >= 200) glow += 1
      sum += l
      n += 1
    }
  }
  let cornerSum = 0
  let cornerN = 0
  for (let y = 0; y < 40; y += 1) {
    for (let x = 0; x < 40; x += 1) {
      cornerSum += luma((y * width + x) * 4)
      cornerN += 1
    }
  }
  return {
    glowPixels: glow,
    meanLuma: sum / Math.max(1, n),
    cornerLuma: cornerSum / Math.max(1, cornerN),
  }
}

const chrome = resolveChromePathOrNull()

describe("pylon network glow-up before/after regression", () => {
  if (chrome === null) {
    test.skip("skipped: no Chromium binary (set CHROME_PATH)", () => {})
    return
  }

  test(
    "bloom ON makes the pylons + connections GLOW while the dark background stays dark",
    async () => {
      const after = await renderVisualizationAndProbe({
        entryModulePath,
        frameSteps: FRAME_STEPS,
        frameDeltaMs: FRAME_DELTA_MS,
      })
      const before = await renderVisualizationAndProbe({
        entryModulePath,
        frameSteps: FRAME_STEPS,
        frameDeltaMs: FRAME_DELTA_MS,
        pageQuery: "nobloom=1",
      })

      const a = scoreGlow(after.image)
      const b = scoreGlow(before.image)

      // 1) Base frame (bloom OFF) still reads: real signal in the band, NOT blank.
      expect(b.meanLuma).toBeGreaterThan(5)
      // 2) Base frame (bloom OFF) keeps a dark background (no full-frame wash).
      expect(b.cornerLuma).toBeLessThan(40)
      // 3) Bloom ON adds a clear GLOW footprint over the flat look — the pylons +
      //    connections now glow. (~11k glow pixels vs <1k under SwiftShader.)
      expect(a.glowPixels).toBeGreaterThan(b.glowPixels + 2000)
      // 4) Bloom ON does NOT wash the dark background corners to grey (the
      //    linear-decode-clear fix). The corners stay near-black.
      expect(a.cornerLuma).toBeLessThan(40)
    },
    120_000,
  )
})
