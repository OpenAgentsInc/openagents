import { describe, expect, test } from 'vitest'

import {
  analyzeIapPackMargin,
  APPLE_GOOGLE_SMALL_BUSINESS_STORE_CUT,
  APPLE_GOOGLE_STANDARD_STORE_CUT,
} from './iap-margin-analysis'
import { IAP_CREDIT_PACK_CATALOG } from './iap-credit-pack-catalog'

describe('analyzeIapPackMargin', () => {
  test('at the standard 30% store cut, every catalog pack runs at a LOSS if fully spent', () => {
    for (const pack of IAP_CREDIT_PACK_CATALOG) {
      const analysis = analyzeIapPackMargin({
        packAmountUsdCents: pack.amountUsdCents,
        storeCutFraction: APPLE_GOOGLE_STANDARD_STORE_CUT,
      })
      expect(analysis.profitIfFullySpentUsdCents).toBeLessThan(0)
      // The loss scales with pack size, but the RATE is constant (~-2.04% of
      // net cash) since it is a pure proportional relationship.
      expect(analysis.effectiveMarginOnNetCash).toBeCloseTo(-0.0204, 3)
    }
  })

  test('at the Small Business Program 15% cut, every catalog pack is profitable even if fully spent', () => {
    for (const pack of IAP_CREDIT_PACK_CATALOG) {
      const analysis = analyzeIapPackMargin({
        packAmountUsdCents: pack.amountUsdCents,
        storeCutFraction: APPLE_GOOGLE_SMALL_BUSINESS_STORE_CUT,
      })
      expect(analysis.profitIfFullySpentUsdCents).toBeGreaterThan(0)
      expect(analysis.effectiveMarginOnNetCash).toBeCloseTo(0.1597, 3)
    }
  })

  test('the 999-cent pack at the standard cut matches the hand-verified reference numbers', () => {
    const analysis = analyzeIapPackMargin({
      packAmountUsdCents: 999,
      storeCutFraction: APPLE_GOOGLE_STANDARD_STORE_CUT,
    })
    expect(analysis.netCashUsdCents).toBeCloseTo(699.3, 1)
    expect(analysis.computeCostUsdCents).toBeCloseTo(713.57, 1)
    expect(analysis.profitIfFullySpentUsdCents).toBeCloseTo(-14.27, 1)
  })

  test('a hypothetical higher inference margin (e.g. 60%) would clear the standard store cut', () => {
    const analysis = analyzeIapPackMargin({
      inferenceMargin: 0.6,
      packAmountUsdCents: 999,
      storeCutFraction: APPLE_GOOGLE_STANDARD_STORE_CUT,
    })
    expect(analysis.profitIfFullySpentUsdCents).toBeGreaterThan(0)
  })

  test('a zero store cut is a no-op on the store side (margin equals the raw inference margin fraction of face value)', () => {
    const analysis = analyzeIapPackMargin({ packAmountUsdCents: 1000, storeCutFraction: 0 })
    // margin/(1+margin) = 0.4/1.4
    expect(analysis.effectiveMarginOnNetCash).toBeCloseTo(0.4 / 1.4, 6)
  })
})
