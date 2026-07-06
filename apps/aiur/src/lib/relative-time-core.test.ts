import { describe, expect, test } from 'vitest'

import { formatRelativeTimeWords } from './relative-time-core'

const NOW = Date.parse('2026-07-06T12:00:00.000Z')

describe('formatRelativeTimeWords', () => {
  test('under a minute: just now', () => {
    expect(formatRelativeTimeWords('2026-07-06T11:59:30.000Z', NOW)).toBe('just now')
  })

  test('a future timestamp (clock skew): just now, never negative', () => {
    expect(formatRelativeTimeWords('2026-07-06T12:05:00.000Z', NOW)).toBe('just now')
  })

  test('minutes ago, singular vs plural', () => {
    expect(formatRelativeTimeWords('2026-07-06T11:59:00.000Z', NOW)).toBe('1 minute ago')
    expect(formatRelativeTimeWords('2026-07-06T11:55:00.000Z', NOW)).toBe('5 minutes ago')
  })

  test('hours ago, singular vs plural', () => {
    expect(formatRelativeTimeWords('2026-07-06T11:00:00.000Z', NOW)).toBe('1 hour ago')
    expect(formatRelativeTimeWords('2026-07-06T10:00:00.000Z', NOW)).toBe('2 hours ago')
  })

  test('days ago, singular vs plural', () => {
    expect(formatRelativeTimeWords('2026-07-05T12:00:00.000Z', NOW)).toBe('1 day ago')
    expect(formatRelativeTimeWords('2026-07-03T12:00:00.000Z', NOW)).toBe('3 days ago')
  })

  test('months ago', () => {
    expect(formatRelativeTimeWords('2026-05-01T12:00:00.000Z', NOW)).toBe('2 months ago')
  })

  test('years ago', () => {
    expect(formatRelativeTimeWords('2024-07-06T12:00:00.000Z', NOW)).toBe('2 years ago')
  })

  test('an invalid/unparseable timestamp: just now, never NaN/crash', () => {
    expect(formatRelativeTimeWords('not-a-date', NOW)).toBe('just now')
  })
})
