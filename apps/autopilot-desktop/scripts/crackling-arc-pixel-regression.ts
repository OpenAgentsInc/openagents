// Runnable headless pixel regression for the spawned crackling arc.
//
// Mounts the REAL three-effect element with the REAL spawned-scene
// visualization in headless Chromium, advances DETERMINISTIC fixed frames, and
// asserts the crackling arc actually paints pixels — and that suppressing it
// (evidence refs stripped → evidence:required gate) collapses its footprint.
//
// This is the script form of tests/crackling-arc-pixel-regression.test.ts, for
// running outside `bun test` (e.g. a proof gate). Exits non-zero on failure.

import { fileURLToPath } from "node:url"
import { dirname, join } from "node:path"

import {
  renderVisualizationAndProbe,
  resolveChromePathOrNull,
} from "../src/testing/headless-pixel"

const ARC_REGION = { x0: 0.2, y0: 0.0, x1: 0.85, y1: 0.6 } as const
const FRAME_STEPS = 120
const FRAME_DELTA_MS = 16

const main = async (): Promise<void> => {
  if (resolveChromePathOrNull() === null) {
    console.log(
      "crackling-arc-pixel-regression: no Chromium binary found (set CHROME_PATH); skipping.",
    )
    return
  }
  const here = dirname(fileURLToPath(import.meta.url))
  const entryModulePath = join(here, "./crackling-arc-entry.ts")

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

  const result = {
    canvasWidth: fixed.canvasWidth,
    canvasHeight: fixed.canvasHeight,
    framesAdvanced: fixed.framesAdvanced,
    fixedBrightPixels: fixedScore.brightPixels,
    brokenBrightPixels: brokenScore.brightPixels,
    arcContribution,
    ok: arcContribution > 400 && brokenScore.brightPixels < fixedScore.brightPixels,
  }
  console.log(JSON.stringify(result, null, 2))
  if (!result.ok) {
    throw new Error(
      `crackling arc did not render its expected pixel footprint (contribution=${arcContribution})`,
    )
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
