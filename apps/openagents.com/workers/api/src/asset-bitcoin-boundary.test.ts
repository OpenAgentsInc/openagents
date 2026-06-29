import { describe, expect, test } from 'vitest'

import {
  ASSET_BOUNDARY_CREDIT_REVENUE_NO_BITCOIN_REF,
  ASSET_BOUNDARY_FREE_NO_WITHDRAWABLE_BITCOIN_REF,
  assetBoundaryAllows,
  validateAssetBoundary,
} from './asset-bitcoin-boundary'

describe('credit<->Bitcoin asset boundary (RL-3 #5460)', () => {
  test('Bitcoin revenue -> Bitcoin share is allowed', () => {
    expect(
      validateAssetBoundary({
        contributorAsset: 'bitcoin',
        movement: 'payout',
        revenueAsset: 'bitcoin',
      }),
    ).toBeNull()
  })

  test('credit revenue -> credit share is allowed', () => {
    expect(
      validateAssetBoundary({
        contributorAsset: 'credit',
        movement: 'revshare',
        revenueAsset: 'credit',
      }),
    ).toBeNull()
  })

  test('USD revenue -> credit share is allowed', () => {
    expect(
      assetBoundaryAllows({
        contributorAsset: 'credit',
        movement: 'purchase',
        revenueAsset: 'usd',
      }),
    ).toBe(true)
  })

  test('credit revenue -> Bitcoin share is REFUSED (no credit-funded Bitcoin liability)', () => {
    const violation = validateAssetBoundary({
      contributorAsset: 'bitcoin',
      movement: 'payout',
      revenueAsset: 'credit',
    })

    expect(violation).not.toBeNull()
    expect(violation?.reasonRef).toBe(
      ASSET_BOUNDARY_CREDIT_REVENUE_NO_BITCOIN_REF,
    )
  })

  test('USD revenue -> Bitcoin share is REFUSED', () => {
    const violation = validateAssetBoundary({
      contributorAsset: 'bitcoin',
      movement: 'payout',
      revenueAsset: 'usd',
    })

    expect(violation?.reasonRef).toBe(
      ASSET_BOUNDARY_CREDIT_REVENUE_NO_BITCOIN_REF,
    )
  })

  test('free/promo revenue -> Bitcoin share is REFUSED (no withdrawable Bitcoin from free spend)', () => {
    const violation = validateAssetBoundary({
      contributorAsset: 'bitcoin',
      movement: 'payout',
      revenueAsset: 'free',
    })

    expect(violation?.reasonRef).toBe(
      ASSET_BOUNDARY_FREE_NO_WITHDRAWABLE_BITCOIN_REF,
    )
  })

  test('free/promo revenue -> credit share is allowed (no Bitcoin crossing)', () => {
    expect(
      assetBoundaryAllows({
        contributorAsset: 'credit',
        movement: 'spend',
        revenueAsset: 'free',
      }),
    ).toBe(true)
  })

  test('the violation carries only public-safe, secret-free fields', () => {
    const violation = validateAssetBoundary({
      contributorAsset: 'bitcoin',
      movement: 'payout',
      revenueAsset: 'credit',
    })
    const serialized = JSON.stringify(violation)
    expect(serialized).not.toMatch(/sk-/)
    expect(serialized).not.toMatch(/lnbc/i)
    expect(serialized).not.toMatch(/mnemonic/i)
  })
})
