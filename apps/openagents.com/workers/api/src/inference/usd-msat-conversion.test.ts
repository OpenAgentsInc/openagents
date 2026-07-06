import { describe, expect, test } from 'vitest'

import {
  DEFAULT_BTC_USD,
  msatToUsdCentsRound,
  usdCentsToMsatFloor,
} from './usd-msat-conversion'

describe('msatToUsdCentsRound (AIUR-2, #8500 — display-only inverse)', () => {
  test('is the mathematical inverse of usdCentsToMsatFloor at round-number amounts', () => {
    const msat = usdCentsToMsatFloor(1000, DEFAULT_BTC_USD) // $10.00
    expect(msatToUsdCentsRound(msat, DEFAULT_BTC_USD)).toBe(1000)
  })

  test('rounds to the nearest cent rather than flooring/ceiling', () => {
    // 1_000_000 msat @ $100k/BTC = $1.00 exactly.
    expect(msatToUsdCentsRound(1_000_000, DEFAULT_BTC_USD)).toBe(100)
  })

  test('non-positive or non-finite input maps to 0 cents', () => {
    expect(msatToUsdCentsRound(0)).toBe(0)
    expect(msatToUsdCentsRound(-1)).toBe(0)
    expect(msatToUsdCentsRound(Number.NaN)).toBe(0)
    expect(msatToUsdCentsRound(Number.POSITIVE_INFINITY)).toBe(0)
  })

  test('a non-finite/non-positive rate also maps to 0 cents', () => {
    expect(msatToUsdCentsRound(1_000_000, 0)).toBe(0)
    expect(msatToUsdCentsRound(1_000_000, -5)).toBe(0)
  })
})
