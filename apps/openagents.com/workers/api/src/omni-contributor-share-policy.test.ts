import { describe, expect, test } from 'vitest'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import { buildOmniContributorAccrualLedger } from './omni-contributor-accrual-ledger'
import {
  OmniContributorSharePolicyError,
  resolveOmniContributorShares,
} from './omni-contributor-share-policy'

const totalBasisPoints = (
  shares: ReadonlyArray<{ shareBasisPoints: number }>,
): number => shares.reduce((sum, share) => sum + share.shareBasisPoints, 0)

describe('resolveOmniContributorShares', () => {
  test('runner + default platform split sums to 10000', () => {
    const shares = resolveOmniContributorShares({ runnerId: 'runner-1' })
    expect(shares).toEqual([
      { contributorId: 'runner-1', role: 'runner', shareBasisPoints: 8000 },
      { contributorId: 'platform', role: 'platform', shareBasisPoints: 2000 },
    ])
    expect(totalBasisPoints(shares)).toBe(10000)
  })

  test('all roles present split exactly by canonical weights', () => {
    const shares = resolveOmniContributorShares({
      runnerId: 'runner-1',
      reviewerId: 'reviewer-1',
      originatorId: 'origin-1',
      referrerId: 'ref-1',
      platformId: 'platform',
    })
    expect(shares).toEqual([
      { contributorId: 'runner-1', role: 'runner', shareBasisPoints: 6000 },
      { contributorId: 'reviewer-1', role: 'reviewer', shareBasisPoints: 1000 },
      { contributorId: 'origin-1', role: 'originator', shareBasisPoints: 1000 },
      { contributorId: 'ref-1', role: 'referrer', shareBasisPoints: 500 },
      { contributorId: 'platform', role: 'platform', shareBasisPoints: 1500 },
    ])
    expect(totalBasisPoints(shares)).toBe(10000)
  })

  test('partial roles renormalize to exactly 10000 by largest remainder', () => {
    const shares = resolveOmniContributorShares({
      runnerId: 'runner-1',
      reviewerId: 'reviewer-1',
    })
    expect(shares).toEqual([
      { contributorId: 'runner-1', role: 'runner', shareBasisPoints: 7059 },
      { contributorId: 'reviewer-1', role: 'reviewer', shareBasisPoints: 1176 },
      { contributorId: 'platform', role: 'platform', shareBasisPoints: 1765 },
    ])
    expect(totalBasisPoints(shares)).toBe(10000)
  })

  test('emits roles in canonical order regardless of input shape', () => {
    const shares = resolveOmniContributorShares({
      runnerId: 'runner-1',
      referrerId: 'ref-1',
      originatorId: 'origin-1',
    })
    expect(shares.map(share => share.role)).toEqual([
      'runner',
      'originator',
      'referrer',
      'platform',
    ])
    expect(totalBasisPoints(shares)).toBe(10000)
  })

  test('resolved shares feed the accrual ledger without tripping its invariant', () => {
    const record: OmniAcceptedOutcomeEconomicsRecord = {
      acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
      acceptedValueCents: 5000,
      archivedAt: null,
      artifactCostCents: 100,
      buyerPriceAsset: 'usd',
      buyerPriceCents: 5000,
      createdAt: '2026-06-20T00:00:00.000Z',
      creditsCharged: 0,
      fundingMode: 'credit_funded',
      grossMarginCents: 4401,
      id: 'omni_outcome_economics_1',
      idempotencyKey: 'idem-1',
      internalCaveatRef: null,
      metadata: {},
      noSettlementImplication: true,
      providerCostCents: 300,
      publicCaveatRef: 'caveat.no_settlement',
      retryCostCents: 0,
      reviewCostCents: 100,
      reviewMinutes: 5,
      runnerCostCents: 100,
      satsCharged: 0,
      totalCostCents: 600,
      updatedAt: '2026-06-20T00:00:00.000Z',
      workKind: 'coding',
      workroomId: 'omni_workroom_coding_1',
    }
    const shares = resolveOmniContributorShares({
      runnerId: 'runner-1',
      reviewerId: 'reviewer-1',
    })
    const ledger = buildOmniContributorAccrualLedger(record, shares)
    expect(ledger.totalShareBasisPoints).toBe(10000)
    expect(ledger.totalAccruedCents).toBe(4401)
  })

  test('rejects an unsafe runner id', () => {
    expect(() =>
      resolveOmniContributorShares({ runnerId: 'has space' }),
    ).toThrow(OmniContributorSharePolicyError)
  })

  test('rejects a party id reused across roles', () => {
    expect(() =>
      resolveOmniContributorShares({
        runnerId: 'same',
        reviewerId: 'same',
      }),
    ).toThrow(OmniContributorSharePolicyError)
  })

  test('rejects a runner colliding with the default platform id', () => {
    expect(() =>
      resolveOmniContributorShares({ runnerId: 'platform' }),
    ).toThrow(OmniContributorSharePolicyError)
  })

  test('is deterministic for the same input', () => {
    const input = { runnerId: 'runner-1', referrerId: 'ref-1' }
    expect(resolveOmniContributorShares(input)).toEqual(
      resolveOmniContributorShares(input),
    )
  })

  test('exposes a policy error type', () => {
    const error = new OmniContributorSharePolicyError({ reason: 'x' })
    expect(error._tag).toBe('OmniContributorSharePolicyError')
  })
})
