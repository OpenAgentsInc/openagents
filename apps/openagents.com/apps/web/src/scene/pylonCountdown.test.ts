import { describe, expect, test } from 'vitest'

import {
  formatRemaining,
  nextPylonCountdownDeadlineMs,
  remainingToPylonCountdownDeadlineMs,
} from './pylonCountdown'

describe('formatRemaining', () => {
  test('formats a full 12-hour span', () => {
    expect(formatRemaining(12 * 60 * 60 * 1000)).toBe('12:00:00')
  })

  test('formats hours, minutes, and seconds with zero padding', () => {
    expect(formatRemaining((1 * 3600 + 2 * 60 + 3) * 1000)).toBe('01:02:03')
  })

  test('floors partial seconds', () => {
    expect(formatRemaining(1500)).toBe('00:00:01')
  })

  test('clamps negative values to zero', () => {
    expect(formatRemaining(-5000)).toBe('00:00:00')
  })
})

describe('nextPylonCountdownDeadlineMs', () => {
  test('targets 1 PM Central during daylight time', () => {
    const now = Date.parse('2026-06-15T07:00:00.000Z')

    expect(nextPylonCountdownDeadlineMs(now)).toBe(
      Date.parse('2026-06-15T18:00:00.000Z'),
    )
    expect(formatRemaining(remainingToPylonCountdownDeadlineMs(now))).toBe(
      '11:00:00',
    )
  })

  test('targets 1 PM Central during standard time', () => {
    const now = Date.parse('2026-01-15T15:00:00.000Z')

    expect(nextPylonCountdownDeadlineMs(now)).toBe(
      Date.parse('2026-01-15T19:00:00.000Z'),
    )
    expect(formatRemaining(remainingToPylonCountdownDeadlineMs(now))).toBe(
      '04:00:00',
    )
  })

  test('rolls to tomorrow after the 1 PM Central target passes', () => {
    const now = Date.parse('2026-06-15T19:30:00.000Z')

    expect(nextPylonCountdownDeadlineMs(now)).toBe(
      Date.parse('2026-06-16T18:00:00.000Z'),
    )
    expect(formatRemaining(remainingToPylonCountdownDeadlineMs(now))).toBe(
      '22:30:00',
    )
  })
})
