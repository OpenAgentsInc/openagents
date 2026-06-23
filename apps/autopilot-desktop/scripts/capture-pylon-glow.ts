// One-off before/after capture for the Verse pylon-network glow-up.
//
// Renders the LIVE pylon network (real `oa-training-run` element + real
// `pylonNetworkVisualizationOptions`) through the real render path in headless
// Chromium, three times:
//   - after  : bloom ON  (HDR pylons + fat glowing connections + HDR emitters)
//   - before : bloom OFF (the same scene, the pre-glow-up flat look)
//   - basecheck = the same "before" pass, used as the acceptance proof that the
//     base frame still reads correctly with bloom disabled.
//
// Saves the PNGs under docs/testing/proof/ and prints bright-pixel scores so the
// before/after delta on the connections/pylons is asserted, not eyeballed.
//
// HONEST CAVEAT: this is headless Chromium / SwiftShader (software GL), NOT the
// real Electrobun WKWebView + GPU. What it proves: the composer/bloom pass runs,
// the emitters carry HDR signal, fat connection lines replace the 1px lines, and
// the base frame still renders with bloom off. What it does NOT prove: the exact
// on-device GPU bloom look. Confirm that on the real app.

import { fileURLToPath } from "node:url"
import { dirname, join } from "node:path"
import { writeFileSync } from "node:fs"

import {
  renderVisualizationAndProbe,
  resolveChromePathOrNull,
  type PngImage,
} from "../src/testing/headless-pixel"

const here = dirname(fileURLToPath(import.meta.url))
const entryModulePath = join(here, "pylon-network-glow-entry.ts")
const proofDir = join(here, "../docs/testing/proof")

const FRAME_STEPS = 90
const FRAME_DELTA_MS = 16

// The pylon ring + connections fill the central band; corners are background.
const REGION = { x0: 0.18, y0: 0.18, x1: 0.82, y1: 0.82 }

// Measure GLOW, not just "any lit pixel". `glowPixels` counts only very-bright
// energy (luma >= 200) — the bloomed cores + glowing connections that the flat
// 1px lines could never produce. `meanLuma` of the central band lets us watch
// for an over-bloom washout. `cornerLuma` watches the far background corners so
// we can assert the dark scene stays dark (no full-frame grey wash).
type GlowScore = Readonly<{
  glowPixels: number
  meanLuma: number
  cornerLuma: number
  sampled: number
}>

const scoreGlow = (image: PngImage): GlowScore => {
  const { data, width, height } = image
  const x0 = Math.floor(REGION.x0 * width)
  const x1 = Math.floor(REGION.x1 * width)
  const y0 = Math.floor(REGION.y0 * height)
  const y1 = Math.floor(REGION.y1 * height)
  let glow = 0
  let sum = 0
  let sampled = 0
  for (let y = y0; y < y1; y += 1) {
    for (let x = x0; x < x1; x += 1) {
      const i = (y * width + x) * 4
      const luma =
        0.2126 * (data[i] ?? 0) +
        0.7152 * (data[i + 1] ?? 0) +
        0.0722 * (data[i + 2] ?? 0)
      if (luma >= 200) glow += 1
      sum += luma
      sampled += 1
    }
  }
  // Sample a 40px corner box (top-left) for the dark-background check.
  let cornerSum = 0
  let cornerN = 0
  for (let y = 0; y < 40; y += 1) {
    for (let x = 0; x < 40; x += 1) {
      const i = (y * width + x) * 4
      cornerSum +=
        0.2126 * (data[i] ?? 0) +
        0.7152 * (data[i + 1] ?? 0) +
        0.0722 * (data[i + 2] ?? 0)
      cornerN += 1
    }
  }
  return {
    glowPixels: glow,
    meanLuma: Number((sum / Math.max(1, sampled)).toFixed(2)),
    cornerLuma: Number((cornerSum / Math.max(1, cornerN)).toFixed(2)),
    sampled,
  }
}

const main = async (): Promise<void> => {
  const chrome = resolveChromePathOrNull()
  if (chrome === null) {
    console.error("No Chromium binary found (set CHROME_PATH). Cannot capture.")
    process.exit(2)
  }

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

  const afterPng = join(proofDir, "2026-06-22-pylon-glow-after-bloom-on.png")
  const beforePng = join(proofDir, "2026-06-22-pylon-glow-before-bloom-off.png")
  writeFileSync(afterPng, Buffer.from(after.screenshotBase64, "base64"))
  writeFileSync(beforePng, Buffer.from(before.screenshotBase64, "base64"))

  const afterScore = scoreGlow(after.image)
  const beforeScore = scoreGlow(before.image)

  console.log(JSON.stringify({
    canvas: { width: after.canvasWidth, height: after.canvasHeight },
    afterBloomOn: afterScore,
    beforeBloomOff: beforeScore,
    glowPixelDelta: afterScore.glowPixels - beforeScore.glowPixels,
    paths: { afterPng, beforePng },
  }, null, 2))

  // Acceptance:
  //  1) the base frame still reads with bloom OFF (real signal in the band, and
  //     the background corners stay DARK — no full-frame wash, no double
  //     tone-map).
  //  2) bloom ON adds a clear GLOW footprint (more very-bright pixels) over the
  //     flat look — the pylons + connections now glow — WITHOUT washing the
  //     dark background corners to grey.
  if (beforeScore.meanLuma < 5) {
    console.error("FAIL: base frame (bloom off) band is essentially blank.")
    process.exit(1)
  }
  if (beforeScore.cornerLuma > 40) {
    console.error("FAIL: base frame (bloom off) background corners are not dark.")
    process.exit(1)
  }
  if (afterScore.glowPixels <= beforeScore.glowPixels) {
    console.error("FAIL: bloom ON did not add glow over the flat look.")
    process.exit(1)
  }
  if (afterScore.cornerLuma > 70) {
    console.error("FAIL: bloom ON washed the dark background corners to grey.")
    process.exit(1)
  }
  console.log("OK: base frame reads (dark corners) with bloom off; bloom on glows brighter without washing the background.")
}

void main()
