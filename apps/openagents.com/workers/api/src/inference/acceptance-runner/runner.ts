// Headless acceptance runner for the Khala crossy-road lane (EPIC #6017).
//
// THIS RUNS OUT OF THE CF WORKER. It launches a real headless chromium (Playwright),
// loads the produced single-file HTML artifact, and runs the deterministic acceptance
// suite from `acceptance-spec.ts` against the LIVE page — load → no errors → click
// PLAY → press forward N times → read exposed state. The fraction of checks passing
// is the honest `scalarReward`; `verified` is true only when ALL pass. This is the
// real verification the regex `khala-code-verifier.ts` only pretended to do.
//
// It is import-safe to load in Node/Bun but NOT in the Worker (it imports playwright).
// The route imports only `./verdict` (pure) and consumes a verdict produced here.
//
// THE STATE CONTRACT (what a crossy-road artifact must expose for execution).
// The artifact must define on `window`:
//   - `__openagentsCrossyRoadState(): {
//        player:  { x, y, z },           // world-unit position
//        camera:  { position: { x,y,z } },
//        progress: number,               // forward tiles travelled
//        worldRowsAhead?: number,        // distinct rows generated ahead of player
//        started?: boolean,              // true once PLAY started the loop
//        loopTicks?: number,             // increments while the update loop runs
//      }`
//   - `__openagentsCrossyRoadStart?()` OR a `#start-btn` / `#play` clickable element
//      that hides the start overlay and starts the loop.
//   - `__openagentsCrossyRoadRestart()` to reset to the start.
// This contract is how we EXECUTE the game deterministically; an artifact that does
// not expose it cannot be execution-verified (its checks fail honestly, never a false
// green).

import { chromium, type Browser, type Page } from 'playwright'

import type { AcceptanceSpec } from '../acceptance-spec'
import {
  type AcceptanceCheckResult,
  type AcceptanceVerdict,
  assembleAcceptanceVerdict,
} from './verdict'

// The shape we read back out of the page. `unknown`-guarded at the boundary.
type GameState = {
  player?: { x?: number; y?: number; z?: number }
  camera?: { position?: { x?: number; y?: number; z?: number } }
  progress?: number
  worldRowsAhead?: number
  started?: boolean
  loopTicks?: number
}

const num = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value) ? value : Number.NaN

const vecDelta = (
  a: { x?: number; y?: number; z?: number } | undefined,
  b: { x?: number; y?: number; z?: number } | undefined,
): number => {
  if (a === undefined || b === undefined) return Number.NaN
  const dx = num(a.x) - num(b.x)
  const dy = num(a.y) - num(b.y)
  const dz = num(a.z) - num(b.z)
  return Math.sqrt(dx * dx + dy * dy + dz * dz)
}

const monotonicNowMs = (): number => performance.now()

// Read the page's exposed state hook. Returns `undefined` if the artifact never
// exposed the contract (e.g. it crashed on load before defining it).
const readState = async (page: Page): Promise<GameState | undefined> => {
  // NOTE: the body of every `page.evaluate` callback runs IN THE BROWSER, not in the
  // Worker tsconfig's WebWorker lib. Browser globals are reached through `globalThis`
  // casts so this module typechecks without pulling the DOM lib into the Worker build.
  return page.evaluate(() => {
    const w = globalThis as unknown as {
      __openagentsCrossyRoadState?: () => unknown
    }
    if (typeof w.__openagentsCrossyRoadState !== 'function') return undefined
    try {
      return w.__openagentsCrossyRoadState() as unknown
    } catch {
      return undefined
    }
  }) as Promise<GameState | undefined>
}

// Try to start the game the way a user would: prefer a real PLAY/start click (this is
// what catches the dead-button overlay defect — a click that does nothing leaves
// `started` false), then fall back to the explicit start hook.
const startGame = async (page: Page): Promise<void> => {
  const clicked = await page.evaluate(() => {
    const doc = (globalThis as unknown as { document: unknown })
      .document as {
      querySelector: (selector: string) => { click: () => void } | null
    }
    const selectors = ['#start-btn', '#play', '#play-btn', '#start', 'button']
    for (const selector of selectors) {
      const element = doc.querySelector(selector)
      if (element !== null) {
        element.click()
        return true
      }
    }
    return false
  })
  // Always also try the explicit hook so a hook-only artifact can start, but a
  // click-driven artifact's dead button is still exercised first (above).
  await page.evaluate(() => {
    const w = globalThis as unknown as {
      __openagentsCrossyRoadStart?: () => void
    }
    if (typeof w.__openagentsCrossyRoadStart === 'function') {
      w.__openagentsCrossyRoadStart()
    }
  })
  if (!clicked) {
    // No obvious start control and no hook ran is fine; the started check will
    // measure whether the loop is actually running.
  }
}

const pressForward = async (page: Page): Promise<void> => {
  await page.keyboard.press('ArrowUp')
}

const settle = async (page: Page, ms: number): Promise<void> => {
  await page.waitForTimeout(ms)
}

// Settle until the player STOPS MOVING (the hop/move animation has completed), then
// return the final state — bounded by `maxMs` so a stuck/never-settling artifact still
// returns honestly rather than hanging. A fixed `settle(ms)` samples MID-ANIMATION when
// the artifact's hop tween is slower than the wait (e.g. a ~125ms hop sampled at 80ms
// reads ~0.2-0.7 of a tile, not a full tile), which made the single-press
// `forward_input_advances_player` check measure a partial hop and fail an otherwise
// good artifact. Polling for position stability measures the COMPLETED advance — the
// honest intent of the check ("one forward press advances ~one tile") — without
// weakening the pass threshold. Stops as soon as the player position is unchanged
// between two consecutive polls (within an epsilon), or when `maxMs` elapses.
const settleUntilStable = async (
  page: Page,
  options?: Readonly<{
    pollMs?: number
    maxMs?: number
    epsilon?: number
    stableReads?: number
  }>,
): Promise<GameState | undefined> => {
  const pollMs = options?.pollMs ?? 60
  const maxMs = options?.maxMs ?? 900
  const epsilon = options?.epsilon ?? 1e-4
  // Require TWO consecutive unchanged polls before declaring stable. The artifact's
  // render loop is requestAnimationFrame-driven and throttled in headless chromium
  // (~25fps), so a SINGLE zero-delta between two close polls can be a skipped frame
  // mid-hop, not the landed position. Requiring consecutive stable reads (with a poll
  // step coarser than a frame) waits for the hop tween to truly plateau before we
  // measure the advance. Still bounded by `maxMs` so a never-settling artifact returns
  // honestly rather than hanging.
  const stableReads = Math.max(2, options?.stableReads ?? 2)
  let previous = await readState(page)
  let stable = 0
  const deadline = monotonicNowMs() + maxMs
  for (;;) {
    await settle(page, pollMs)
    const current = await readState(page)
    const moved = vecDelta(current?.player, previous?.player)
    previous = current
    if (Number.isFinite(moved) && moved <= epsilon) {
      stable += 1
    } else {
      stable = 0
    }
    if (stable >= stableReads || monotonicNowMs() >= deadline) {
      return current
    }
  }
}

// Run the full crossy-road acceptance suite against a live page. PURE w.r.t. the spec
// + page (no global state); returns one result per spec check.
const runCrossyRoadChecks = async (
  page: Page,
  spec: AcceptanceSpec,
  consoleErrors: ReadonlyArray<string>,
  pageErrors: ReadonlyArray<string>,
): Promise<ReadonlyArray<AcceptanceCheckResult>> => {
  const results: AcceptanceCheckResult[] = []
  const { params } = spec

  // 1. loads_without_errors — captured during navigation.
  const loadOk = consoleErrors.length === 0 && pageErrors.length === 0
  results.push({
    detail: loadOk
      ? 'No console or page errors during load.'
      : `Console errors: ${consoleErrors.length}; page errors: ${pageErrors.length}. First: ${(pageErrors[0] ?? consoleErrors[0] ?? '').slice(0, 200)}`,
    id: 'loads_without_errors',
    passed: loadOk,
  })

  // 2. play_starts_game — click PLAY/start, then verify the loop actually runs and
  //    the game reports started. This catches the dead-button overlay (click does
  //    nothing -> started stays false / loop never ticks).
  const before = await readState(page)
  await startGame(page)
  await settle(page, 250)
  const afterStart = await readState(page)
  const ticksBefore = num(before?.loopTicks)
  const ticksAfter = num(afterStart?.loopTicks)
  const loopRunning =
    (Number.isFinite(ticksBefore) &&
      Number.isFinite(ticksAfter) &&
      ticksAfter > ticksBefore) ||
    afterStart?.started === true
  results.push({
    detail: loopRunning
      ? `Game started (started=${String(afterStart?.started)}, loopTicks ${ticksBefore}->${ticksAfter}).`
      : `PLAY did not start the game (started=${String(afterStart?.started)}, loopTicks ${ticksBefore}->${ticksAfter}). Dead button / overlay?`,
    id: 'play_starts_game',
    passed: loopRunning,
  })

  // 3. forward_input_advances_player — one forward press advances ~one tile. Settle
  //    UNTIL the hop animation lands (not a fixed wait) so we measure the COMPLETED
  //    advance, not a mid-hop fraction; a fixed wait shorter than the artifact's hop
  //    duration under-measures the tile and fails a good artifact.
  const preMove = await readState(page)
  await pressForward(page)
  const postMove = await settleUntilStable(page)
  const advance = vecDelta(postMove?.player, preMove?.player)
  const advancedOneTile =
    Number.isFinite(advance) &&
    advance >= params.expectedForwardAdvance * 0.5 &&
    advance <= params.expectedForwardAdvance * 2
  results.push({
    detail: advancedOneTile
      ? `Forward input advanced player by ~${advance.toFixed(2)} (expected ~${params.expectedForwardAdvance}).`
      : `Forward input advanced player by ${advance.toFixed(2)}; expected ~${params.expectedForwardAdvance}.`,
    id: 'forward_input_advances_player',
    passed: advancedOneTile,
  })

  // 4 & 5. Drive N forward moves; track max camera delta per move AND world rows
  //        ahead. This catches both the 100x camera bug and the blue-sky bug.
  let prevState = await readState(page)
  let maxCameraDelta = 0
  for (let move = 0; move < params.forwardMoves; move += 1) {
    await pressForward(page)
    // Snapshot AFTER the hop lands so each per-move camera delta is measured between
    // settled positions (consistent with the single-press check), not mid-tween.
    const next = await settleUntilStable(page)
    const camDelta = vecDelta(next?.camera?.position, prevState?.camera?.position)
    if (Number.isFinite(camDelta)) {
      maxCameraDelta = Math.max(maxCameraDelta, camDelta)
    }
    prevState = next
  }
  const finalState = await readState(page)

  const cameraBounded =
    Number.isFinite(maxCameraDelta) &&
    maxCameraDelta > 0 &&
    maxCameraDelta <= params.maxCameraDeltaPerMove
  results.push({
    detail: cameraBounded
      ? `Max camera delta per move ${maxCameraDelta.toFixed(2)} <= bound ${params.maxCameraDeltaPerMove}.`
      : `Max camera delta per move ${maxCameraDelta.toFixed(2)} exceeds bound ${params.maxCameraDeltaPerMove} (100x camera bug?).`,
    id: 'camera_follow_delta_bounded',
    passed: cameraBounded,
  })

  const rowsAhead = num(finalState?.worldRowsAhead)
  const worldGenerating =
    Number.isFinite(rowsAhead) && rowsAhead >= params.minWorldRowsAhead
  results.push({
    detail: worldGenerating
      ? `World has ${rowsAhead} rows ahead after ${params.forwardMoves} moves (>= ${params.minWorldRowsAhead}).`
      : `World has ${Number.isFinite(rowsAhead) ? rowsAhead : 'unknown'} rows ahead after ${params.forwardMoves} moves; need >= ${params.minWorldRowsAhead} (blue sky?).`,
    id: 'world_keeps_generating_ahead',
    passed: worldGenerating,
  })

  // 6. restart_resets_state — restart returns player + progress to start.
  await page.evaluate(() => {
    const w = globalThis as unknown as {
      __openagentsCrossyRoadRestart?: () => void
    }
    if (typeof w.__openagentsCrossyRoadRestart === 'function') {
      w.__openagentsCrossyRoadRestart()
    }
  })
  await settle(page, 80)
  const afterRestart = await readState(page)
  const playerBackToStart =
    Number.isFinite(num(afterRestart?.player?.x)) &&
    Math.abs(num(afterRestart?.player?.x)) < 0.001 &&
    Math.abs(num(afterRestart?.player?.z)) < 0.001
  const progressReset = num(afterRestart?.progress) === 0
  const restartOk = playerBackToStart && progressReset
  results.push({
    detail: restartOk
      ? 'Restart reset player to origin and progress to 0.'
      : `Restart did not fully reset (player x=${num(afterRestart?.player?.x)}, z=${num(afterRestart?.player?.z)}, progress=${num(afterRestart?.progress)}).`,
    id: 'restart_resets_state',
    passed: restartOk,
  })

  return results
}

export type AcceptanceRunOptions = Readonly<{
  // Inject a browser (test reuse). When absent the runner launches its own chromium.
  browser?: Browser | undefined
  // Per-step settle budget is internal; this caps total navigation wait.
  navTimeoutMs?: number | undefined
}>

// Run the acceptance suite against a single-file HTML artifact string. Launches (or
// reuses) headless chromium, loads the artifact via `setContent` (no server needed
// for a self-contained file), captures console/page errors, and runs the suite.
// ALWAYS returns a verdict (never throws into the caller): a crash on load shows up as
// `loads_without_errors: false` plus failing downstream checks — an honest red, never
// a false green.
export const runAcceptanceSuite = async (
  input: Readonly<{ artifactHtml: string; spec: AcceptanceSpec }>,
  options?: AcceptanceRunOptions,
): Promise<AcceptanceVerdict> => {
  const ownBrowser = options?.browser === undefined
  const browser =
    options?.browser ?? (await chromium.launch({ headless: true }))
  const consoleErrors: string[] = []
  const pageErrors: string[] = []
  let page: Page | undefined
  try {
    page = await browser.newPage({ viewport: { height: 720, width: 1280 } })
    page.on('console', message => {
      if (message.type() === 'error') consoleErrors.push(message.text())
    })
    page.on('pageerror', error => {
      pageErrors.push(error.message)
    })
    await page.setContent(input.artifactHtml, {
      timeout: options?.navTimeoutMs ?? 15_000,
      waitUntil: 'load',
    })
    // Let any deferred construction / first frame run so a load-time crash surfaces.
    await page.waitForTimeout(300)
    const checks = await runCrossyRoadChecks(
      page,
      input.spec,
      consoleErrors,
      pageErrors,
    )
    return assembleAcceptanceVerdict({
      checks,
      consoleErrors,
      pageErrors,
      spec: input.spec,
    })
  } catch (error) {
    // A runner-level failure (e.g. navigation timeout) is reported as an all-fail
    // verdict with the error captured — never a silent pass.
    const message = error instanceof Error ? error.message : String(error)
    pageErrors.push(`runner_error: ${message}`)
    const checks: ReadonlyArray<AcceptanceCheckResult> = input.spec.checks.map(
      id => ({
        detail: `Runner aborted before this check could execute: ${message.slice(0, 160)}`,
        id,
        passed: false,
      }),
    )
    return assembleAcceptanceVerdict({
      checks,
      consoleErrors,
      pageErrors,
      spec: input.spec,
    })
  } finally {
    if (page !== undefined) await page.close().catch(() => undefined)
    if (ownBrowser) await browser.close().catch(() => undefined)
  }
}
