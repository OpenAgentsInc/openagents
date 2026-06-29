// Regression proof for the headless acceptance runner (EPIC #6017).
//
// THIS IS THE PROOF THE VERIFIER NOW MEANS SOMETHING: the runner must FAIL the
// 4-bug crossy-road artifact (per-test) and PASS the fixed one. If this test passes,
// `verified` is execution-backed, not a regex over source.
//
// Requires a real headless chromium (Playwright). Run with:
//   bun run --cwd apps/openagents.com/workers/api test -- src/inference/acceptance-runner/
//   (chromium installed via `bunx playwright install chromium`)
//
// A single shared browser is reused across cases for speed.

import { afterAll, beforeAll, describe, expect, test } from 'vitest'
import { chromium, type Browser } from 'playwright'

import { crossyRoadAcceptanceSpec } from '../acceptance-spec'
import { runAcceptanceSuite } from './runner'
import { assembleAcceptanceVerdict } from './verdict'
import { CROSSY_ROAD_BROKEN_HTML } from './fixtures/crossy-road-broken.html'
import { CROSSY_ROAD_FIXED_HTML } from './fixtures/crossy-road-fixed.html'

let browser: Browser

beforeAll(async () => {
  browser = await chromium.launch({ headless: true })
}, 60_000)

afterAll(async () => {
  await browser.close().catch(() => undefined)
})

describe('headless acceptance runner — verdict assembly (pure)', () => {
  test('scalarReward is the fraction passing; verified only when all pass', () => {
    const spec = crossyRoadAcceptanceSpec()
    const allPass = assembleAcceptanceVerdict({
      checks: spec.checks.map(id => ({ detail: 'ok', id, passed: true })),
      consoleErrors: [],
      pageErrors: [],
      spec,
    })
    expect(allPass.verified).toBe(true)
    expect(allPass.scalarReward).toBe(1)

    const halfPass = assembleAcceptanceVerdict({
      checks: spec.checks.map((id, index) => ({
        detail: 'x',
        id,
        passed: index % 2 === 0,
      })),
      consoleErrors: [],
      pageErrors: [],
      spec,
    })
    expect(halfPass.verified).toBe(false)
    expect(halfPass.scalarReward).toBeLessThan(1)
    expect(halfPass.scalarReward).toBeGreaterThan(0)
  })
})

describe('headless acceptance runner — executes the artifact', () => {
  test('PASSES the fixed crossy-road artifact (every check green)', async () => {
    const verdict = await runAcceptanceSuite(
      { artifactHtml: CROSSY_ROAD_FIXED_HTML, spec: crossyRoadAcceptanceSpec() },
      { browser },
    )
    expect(verdict.executed).toBe(true)
    expect(verdict.consoleErrors).toEqual([])
    expect(verdict.pageErrors).toEqual([])
    expect(verdict.failedChecks, JSON.stringify(verdict.checks, null, 2)).toEqual(
      [],
    )
    expect(verdict.verified).toBe(true)
    expect(verdict.scalarReward).toBe(1)
  }, 60_000)

  test('FAILS the 4-bug crossy-road artifact, per-test', async () => {
    const verdict = await runAcceptanceSuite(
      {
        artifactHtml: CROSSY_ROAD_BROKEN_HTML,
        spec: crossyRoadAcceptanceSpec(),
      },
      { browser },
    )
    expect(verdict.executed).toBe(true)
    expect(verdict.verified).toBe(false)
    expect(verdict.scalarReward).toBeLessThan(1)

    // BUG #1 — crashed on load: a page error was captured.
    expect(verdict.pageErrors.length).toBeGreaterThan(0)
    expect(verdict.failedChecks).toContain('loads_without_errors')

    // BUG #2 / #1 — PLAY does not start the game.
    expect(verdict.failedChecks).toContain('play_starts_game')

    // PLAY dead -> forward input cannot advance the player.
    expect(verdict.failedChecks).toContain('forward_input_advances_player')

    // BUG #4 — world stopped generating ahead.
    expect(verdict.failedChecks).toContain('world_keeps_generating_ahead')
  }, 60_000)

  test('catches the 100x camera bug by MEASURED delta when moves do land', async () => {
    // Drive the broken game's camera directly through its exposed instance to prove
    // the camera-delta check FAILS on the 100x multiply even when input is wired:
    // we force forward moves via the restart+move path and read the camera delta.
    const page = await browser.newPage()
    try {
      await page.setContent(CROSSY_ROAD_BROKEN_HTML, { waitUntil: 'load' })
      await page.waitForTimeout(200)
      // Read camera before and after one forced forward move on the half-built game.
      const deltas = await page.evaluate(() => {
        const w = globalThis as unknown as {
          __openagentsCrossyRoadState?: () => {
            camera?: { position?: { x?: number; y?: number; z?: number } }
          }
          brokenGame?: unknown
        }
        // Reach into the broken instance the fixture left on window/global.
        const g = w.brokenGame as
          | {
              move: (d: string) => void
              camera: { position: { x: number; y: number; z: number } }
            }
          | undefined
        const read = () => {
          const s = w.__openagentsCrossyRoadState?.()
          return s?.camera?.position
        }
        const before = read()
        if (g !== undefined && typeof g.move === 'function') g.move('forward')
        const after = read()
        if (before === undefined || after === undefined) return null
        return {
          dz: Math.abs((after.z ?? 0) - (before.z ?? 0)),
        }
      })
      // The 100x bug moves the camera by ~TILE_SIZE (32) units in z for a one-tile hop.
      // Our bound is 5 -> this is far over it, which is exactly why the check fails.
      expect(deltas).not.toBeNull()
      expect(deltas?.dz ?? 0).toBeGreaterThan(crossyRoadAcceptanceSpec().params.maxCameraDeltaPerMove)
    } finally {
      await page.close().catch(() => undefined)
    }
  }, 60_000)
})
