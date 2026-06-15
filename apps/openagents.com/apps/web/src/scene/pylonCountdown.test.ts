import { describe, expect, test } from 'vitest'

import { formatRemaining } from './pylonCountdown'

describe('formatRemaining', () => {
  test('formats a full 12-hour countdown', () => {
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
