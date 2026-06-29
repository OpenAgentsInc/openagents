import { describe, expect, test } from 'vitest'

import { TINY_TEST_CONFIG } from './fixtures'
import {
  RealLaneNotArmedError,
  makeFixtureLaneSeam,
  makeRealLaneSeam,
} from './lane-seam'
import { expandMatrix } from './matrix'

const firstCell = () =>
  expandMatrix(TINY_TEST_CONFIG).find(c => c.lane === 'fireworks')!

describe('fixture lane seam — deterministic, spend-free', () => {
  test('cannot spend', () => {
    expect(makeFixtureLaneSeam().canSpend).toBe(false)
    expect(makeFixtureLaneSeam().id).toBe('fixture')
  })

  test('same cell + same sample index → identical sample (pure)', () => {
    const seam = makeFixtureLaneSeam()
    const cell = firstCell()
    expect(seam.sample(cell, 0)).toEqual(seam.sample(cell, 0))
  })

  test('repeated samples spread deterministically (so percentiles are non-degenerate)', () => {
    const seam = makeFixtureLaneSeam()
    const cell = firstCell()
    const a = seam.sample(cell, 0)
    const b = seam.sample(cell, 1)
    const c = seam.sample(cell, 2)
    // Jitter moves wall-clock around the base; not all identical.
    const wallClocks = [a.totalWallClockMs, b.totalWallClockMs, c.totalWallClockMs]
    expect(new Set(wallClocks).size).toBeGreaterThan(1)
  })

  test('cached input tokens follow the cacheable prefix × hit fraction', () => {
    const seam = makeFixtureLaneSeam()
    const cell = firstCell()
    const sample = seam.sample(cell, 0)
    // Fireworks fixture cacheHitFraction 0.8 over a 500-token cacheable prefix.
    expect(sample.cachedInputTokens).toBe(Math.round(500 * 0.8))
  })

  test('artifact-gen cell carries an executed passed verdict (scored on outcome)', () => {
    const seam = makeFixtureLaneSeam()
    const cell = firstCell()
    const sample = seam.sample(cell, 0)
    expect(sample.verificationClass).toBe('test_passed')
    expect(sample.executedVerdict).toBe('passed')
    expect(sample.scalarReward).toBe(1)
    expect(sample.verifierTimeMs).toBeGreaterThan(0)
  })

  test('token counts mirror the sequence shape', () => {
    const seam = makeFixtureLaneSeam()
    const cell = firstCell()
    const sample = seam.sample(cell, 0)
    expect(sample.promptTokens).toBe(1000)
    expect(sample.completionTokens).toBe(100)
    expect(sample.totalTokens).toBe(1100)
  })
})

describe('real lane seam — owner/spend-gated, default OFF', () => {
  test('unarmed seam cannot spend and refuses to sample (no network ever)', () => {
    const seam = makeRealLaneSeam({ armRealSweep: false })
    expect(seam.canSpend).toBe(false)
    expect(() => seam.sample(firstCell(), 0)).toThrow(RealLaneNotArmedError)
  })

  test('armed flag WITHOUT an executor still cannot spend (refuses)', () => {
    const seam = makeRealLaneSeam({ armRealSweep: true })
    expect(seam.canSpend).toBe(false)
    expect(() => seam.sample(firstCell(), 0)).toThrow(RealLaneNotArmedError)
  })

  test('a non-true arming value is treated as OFF (no truthy-coercion bypass)', () => {
    // @ts-expect-error — exercising a defensive runtime guard against a bad flag.
    const seam = makeRealLaneSeam({ armRealSweep: 1, executor: () => firstCell() })
    expect(seam.canSpend).toBe(false)
    expect(() => seam.sample(firstCell(), 0)).toThrow(RealLaneNotArmedError)
  })

  test('only an explicit true + executor arms the seam (owner-confirmed path)', () => {
    const seam = makeRealLaneSeam({
      armRealSweep: true,
      executor: (cell, sampleIndex) => ({
        promptTokens: 10,
        completionTokens: 5,
        totalTokens: 15,
        cachedInputTokens: 0,
        ttftMs: 100 + sampleIndex,
        totalWallClockMs: 500,
        generationWallClockMs: 400,
        providerTimeMs: 480,
        gatewayOverheadMs: 20,
        verificationClass: 'none',
        executedVerdict: 'not_executed',
        scalarReward: 0,
        verifierTimeMs: 0,
        costBasisMsat: 100,
        region: cell.lane,
      }),
    })
    expect(seam.canSpend).toBe(true)
    expect(seam.sample(firstCell(), 0).promptTokens).toBe(10)
  })
})
