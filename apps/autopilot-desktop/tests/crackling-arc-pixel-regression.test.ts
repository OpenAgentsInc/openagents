// Headless pixel regression for the spawned crackling arc (Mode 2: model-shape
// green, screen blank). This is the test that would have caught "the beam is in
// the model but nothing renders."
//
// It mounts the REAL three-effect `oa-training-run` element with the REAL
// spawned-scene visualization in headless Chromium, advances DETERMINISTIC fixed
// frames (no rAF/wall-clock — injected fake clock), screenshots, and scores
// pixels in the arc's REGION. Then it does the same for a deliberately-broken
// variant (evidence refs stripped so the renderer's evidence:required gate
// suppresses the arc) and asserts that one is dark.
//
// FIXED variant  → bright pixels in the arc region (PASS).
// BROKEN variant → far fewer bright pixels (the regression that fails on broken).
//
// Determinism: the fake frame clock means identical input → identical pixels.
// Skips cleanly (with a logged note) where no Chromium binary is installed, so
// the per-file runner (scripts/run-tests.sh) never goes red for lack of a
// browser.

import { fileURLToPath } from "node:url"
import { dirname, join } from "node:path"

import { describe, expect, test } from "bun:test"

import {
  renderVisualizationAndProbe,
  resolveChromePathOrNull,
  type PixelRegion,
} from "../src/testing/headless-pixel"

const here = dirname(fileURLToPath(import.meta.url))
const entryModulePath = join(here, "../scripts/crackling-arc-entry.ts")

// In the real perspective_walk frame the arc hangs at chest height in FRONT of
// the avatar, centre-left of the frame. Score that centre-left band specifically
// — a full-frame score could be fooled by base-scene chrome, and the old upper
// band predated the coordinate-frame fix that moved the arc into the camera's
// actual view.
const ARC_REGION: PixelRegion = { x0: 0.28, y0: 0.22, x1: 0.62, y1: 0.58 }

const FRAME_STEPS = 120
const FRAME_DELTA_MS = 16

const chrome = resolveChromePathOrNull()

describe("crackling arc headless pixel regression (Mode 2)", () => {
  if (chrome === null) {
    test.skip("skipped: no Chromium binary (set CHROME_PATH)", () => {})
    return
  }

  test(
    "the spawned crackling arc renders bright pixels in its region",
    async () => {
      const fixed = await renderVisualizationAndProbe({
        entryModulePath,
        frameSteps: FRAME_STEPS,
        frameDeltaMs: FRAME_DELTA_MS,
      })
      expect(fixed.canvasWidth).toBeGreaterThanOrEqual(480)
      expect(fixed.canvasHeight).toBeGreaterThanOrEqual(270)

      const score = fixed.score(ARC_REGION)
      // The arc region must paint bright, varied pixels — not a blank region.
      expect(score.brightPixels).toBeGreaterThan(300)
      expect(score.distinctLumaBuckets).toBeGreaterThanOrEqual(3)
    },
    60_000,
  )

  test(
    "the arc's pixel contribution vanishes when the renderer suppresses it",
    async () => {
      // The arc's scene-station ICONS render in both variants (entities are not
      // evidence-gated), so the region's absolute brightness is dominated by the
      // base scene + icons. The arc's signal is its CONTRIBUTION: the bright
      // pixels the crackling beam adds OVER the no-arc baseline. The broken
      // variant (evidence refs stripped → evidence:required gate suppresses the
      // beam) is exactly that no-arc baseline. So the faithful Mode-2 assertion
      // is: the fixed arc contributes a clear positive footprint, and the broken
      // one contributes essentially nothing.
      const fixed = await renderVisualizationAndProbe({
        entryModulePath,
        frameSteps: FRAME_STEPS,
        frameDeltaMs: FRAME_DELTA_MS,
      })
      const broken = await renderVisualizationAndProbe({
        entryModulePath,
        frameSteps: FRAME_STEPS,
        frameDeltaMs: FRAME_DELTA_MS,
        pageQuery: "broken=1",
      })

      const fixedScore = fixed.score(ARC_REGION)
      const brokenScore = broken.score(ARC_REGION)
      const arcContribution = fixedScore.brightPixels - brokenScore.brightPixels

      // The arc adds a clear, visible footprint when it renders. This is the
      // assertion that FAILS if a future change drops the arc geometry while
      // leaving the beam "in the model" — the exact Mode-2 bug. (~1100+ bright
      // pixels of crackling beam under SwiftShader; floor well below that.)
      expect(arcContribution).toBeGreaterThan(400)
      // And the broken variant is genuinely the no-arc baseline: it is dimmer
      // than the fixed render, never brighter.
      expect(brokenScore.brightPixels).toBeLessThan(fixedScore.brightPixels)
    },
    90_000,
  )

  test(
    "deterministic: rendering the same visualization twice yields the same score",
    async () => {
      const a = await renderVisualizationAndProbe({
        entryModulePath,
        frameSteps: FRAME_STEPS,
        frameDeltaMs: FRAME_DELTA_MS,
      })
      const b = await renderVisualizationAndProbe({
        entryModulePath,
        frameSteps: FRAME_STEPS,
        frameDeltaMs: FRAME_DELTA_MS,
      })
      const sa = a.score(ARC_REGION)
      const sb = b.score(ARC_REGION)
      // Same fixed frames + injected clock ⇒ bright-pixel count is stable. Allow
      // a tiny tolerance for SwiftShader rounding at the bright-pixel threshold.
      const delta = Math.abs(sa.brightPixels - sb.brightPixels)
      expect(delta).toBeLessThanOrEqual(Math.max(8, sa.brightPixels * 0.02))
    },
    90_000,
  )
})
