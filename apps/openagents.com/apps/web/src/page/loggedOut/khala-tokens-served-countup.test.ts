import { describe, expect, test } from 'vitest'

import {
  KHALA_COUNTUP_MAX_DURATION_MS,
  KHALA_COUNTUP_MIN_DURATION_MS,
  easedCountUpValue,
  khalaCountUpDurationMs,
  makeKhalaCountUpAnimator,
} from './khala-tokens-served-countup'
import { parseCounterTargetValue } from './khala-tokens-served-countup-controller'

describe('khala count-up interpolation (#6324)', () => {
  test('eased value starts at `from`, ends exactly at `to`, and is monotonic up', () => {
    const from = 1_000
    const to = 1_900
    expect(easedCountUpValue(from, to, 0)).toBe(from)
    expect(easedCountUpValue(from, to, 1)).toBe(to)
    // clamps out-of-range progress
    expect(easedCountUpValue(from, to, -0.5)).toBe(from)
    expect(easedCountUpValue(from, to, 5)).toBe(to)

    let previous = from
    for (let i = 1; i <= 10; i++) {
      const value = easedCountUpValue(from, to, i / 10)
      expect(value).toBeGreaterThanOrEqual(previous)
      expect(value).toBeLessThanOrEqual(to)
      previous = value
    }
  })

  test('ease-out: more progress is made early than late (front-loaded)', () => {
    const from = 0
    const to = 1_000
    const atQuarter = easedCountUpValue(from, to, 0.25)
    // ease-out cubic at t=0.25 is ~0.578 of the way, i.e. clearly past linear 25%.
    expect(atQuarter).toBeGreaterThan(250)
  })

  test('duration is bounded: small and huge deltas both land in the band', () => {
    expect(khalaCountUpDurationMs(100, 100)).toBe(0) // no delta → no animation
    const small = khalaCountUpDurationMs(1_000, 1_010)
    expect(small).toBeGreaterThanOrEqual(KHALA_COUNTUP_MIN_DURATION_MS)
    expect(small).toBeLessThanOrEqual(KHALA_COUNTUP_MAX_DURATION_MS)
    // a 9,000,000 post-burst jump must NOT crawl for minutes — it is capped.
    const huge = khalaCountUpDurationMs(100_000_000, 109_000_000)
    expect(huge).toBe(KHALA_COUNTUP_MAX_DURATION_MS)
  })
})

// A controllable fake rAF clock so the animator can be stepped deterministically.
const makeFakeFrames = () => {
  let timeMs = 0
  const queue: Array<(timeMs: number) => void> = []
  return {
    requestFrame: (callback: (timeMs: number) => void): number => {
      queue.push(callback)
      return queue.length
    },
    cancelFrame: (_handle: number): void => {},
    advance: (deltaMs: number): void => {
      timeMs += deltaMs
      const pending = queue.splice(0, queue.length)
      pending.forEach(callback => callback(timeMs))
    },
  }
}

describe('khala count-up animator', () => {
  test('eases between updates and converges exactly to each target', () => {
    const frames = makeFakeFrames()
    const texts: Array<string> = []
    const animator = makeKhalaCountUpAnimator(
      {
        format: value => String(value),
        setText: text => texts.push(text),
        requestFrame: frames.requestFrame,
        cancelFrame: frames.cancelFrame,
        prefersReducedMotion: () => false,
      },
      1_000,
    )

    animator.animateTo(1_900)
    // Step through more than the max duration in chunks; it must end on 1900.
    for (let i = 0; i < 30; i++) {
      frames.advance(50)
    }
    expect(texts.length).toBeGreaterThan(1) // animated, not snapped
    expect(texts[texts.length - 1]).toBe('1900')

    // A second update eases from where we are up to the new target.
    texts.length = 0
    animator.animateTo(2_500)
    for (let i = 0; i < 30; i++) {
      frames.advance(50)
    }
    expect(texts[texts.length - 1]).toBe('2500')
  })

  test('snaps to the target when reduced motion is preferred (no frames)', () => {
    const frames = makeFakeFrames()
    const texts: Array<string> = []
    const animator = makeKhalaCountUpAnimator(
      {
        format: value => String(value),
        setText: text => texts.push(text),
        requestFrame: frames.requestFrame,
        cancelFrame: frames.cancelFrame,
        prefersReducedMotion: () => true,
      },
      1_000,
    )
    animator.animateTo(9_999)
    expect(texts).toEqual(['9999'])
  })

  test('snaps when no requestAnimationFrame is available (SSR/headless safe)', () => {
    const texts: Array<string> = []
    const animator = makeKhalaCountUpAnimator(
      {
        format: value => String(value),
        setText: text => texts.push(text),
        // no requestFrame → must snap
      },
      0,
    )
    animator.animateTo(42)
    expect(texts).toEqual(['42'])
  })
})

describe('counter target parsing', () => {
  test('strips thousands separators and ignores the placeholder', () => {
    expect(parseCounterTargetValue('1,234,567')).toBe(1234567)
    expect(parseCounterTargetValue('107001208')).toBe(107001208)
    expect(parseCounterTargetValue('—')).toBeNull()
    expect(parseCounterTargetValue(null)).toBeNull()
    expect(parseCounterTargetValue('')).toBeNull()
  })
})
