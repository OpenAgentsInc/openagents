import { describe, expect, test } from 'vitest'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  buildOmniContributorAccrualBundle,
  OmniContributorAccrualBundleInvariantError,
  publicOmniContributorAccrualBundleProjection,
} from './omni-contributor-accrual-bundle'

const baseRecord: OmniAcceptedOutcomeEconomicsRecord = {
  acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
  acceptedValueCents: 5000,
  archivedAt: null,
  artifactCostCents: 100,
  buyerPriceAsset: 'usd',
  buyerPriceCents: 5000,
  createdAt: '2026-06-20T00:00:00.000Z',
  creditsCharged: 0,
  fundingMode: 'credit_funded',
  grossMarginCents: 4400,
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

describe('buildOmniContributorAccrualBundle', () => {
  test('composes ledger + receipt keyed by the same accepted-outcome id', () => {
    const bundle = buildOmniContributorAccrualBundle(baseRecord, {
      runnerId: 'runner-1',
    })
    expect(bundle.bundleKind).toBe('accepted_outcome_accrual_bundle')
    expect(bundle.economicsId).toBe('omni_outcome_economics_1')
    expect(bundle.grossMarginReceipt.economicsId).toBe(bundle.economicsId)
    expect(bundle.contributorAccrualLedger.economicsId).toBe(bundle.economicsId)
  })

  test('reconciles a single gross margin across both views', () => {
    const bundle = buildOmniContributorAccrualBundle(baseRecord, {
      runnerId: 'runner-1',
    })
    expect(bundle.reconciledGrossMarginCents).toBe(4400)
    expect(bundle.grossMarginReceipt.grossMarginCents).toBe(4400)
    expect(bundle.contributorAccrualLedger.grossMarginCents).toBe(4400)
    expect(bundle.contributorAccrualLedger.distributableMarginCents).toBe(4400)
  })

  test('contributor accruals sum exactly to the reconciled gross margin', () => {
    const bundle = buildOmniContributorAccrualBundle(baseRecord, {
      runnerId: 'runner-1',
      reviewerId: 'reviewer-1',
      referrerId: 'ref-1',
    })
    const sum = bundle.contributorAccrualLedger.entries.reduce(
      (total, entry) => total + entry.accruedMarginCents,
      0,
    )
    expect(sum).toBe(bundle.reconciledGrossMarginCents)
    expect(sum).toBe(bundle.contributorAccrualLedger.distributableMarginCents)
  })

  test('a loss reconciles to zero distributable margin, never negative', () => {
    const lossRecord: OmniAcceptedOutcomeEconomicsRecord = {
      ...baseRecord,
      acceptedValueCents: 200,
      grossMarginCents: -400,
      totalCostCents: 600,
    }
    const bundle = buildOmniContributorAccrualBundle(lossRecord, {
      runnerId: 'runner-1',
    })
    expect(bundle.reconciledGrossMarginCents).toBe(-400)
    expect(bundle.contributorAccrualLedger.distributableMarginCents).toBe(0)
    expect(bundle.contributorAccrualLedger.totalAccruedCents).toBe(0)
  })

  test('keeps settlement disclaimed across both halves', () => {
    const bundle = buildOmniContributorAccrualBundle(baseRecord, {
      runnerId: 'runner-1',
    })
    expect(bundle.grossMarginReceipt.noSettlementImplication).toBe(true)
    expect(bundle.contributorAccrualLedger.noSettlementImplication).toBe(true)
    expect(
      bundle.contributorAccrualLedger.settlementEvidencedEntryCount,
    ).toBe(0)
    expect(bundle.grossMarginReceipt.unevidencedStateIds).toContain(
      'settlement_attempt',
    )
  })

  test('is deterministic for the same inputs', () => {
    const a = buildOmniContributorAccrualBundle(baseRecord, {
      runnerId: 'runner-1',
      reviewerId: 'reviewer-1',
    })
    const b = buildOmniContributorAccrualBundle(baseRecord, {
      runnerId: 'runner-1',
      reviewerId: 'reviewer-1',
    })
    expect(a).toEqual(b)
  })

  test('public projection drops monetary figures but keeps evidence labels', () => {
    const bundle = buildOmniContributorAccrualBundle(baseRecord, {
      runnerId: 'runner-1',
    })
    const projection = publicOmniContributorAccrualBundleProjection(bundle)
    const serialized = JSON.stringify(projection)
    expect(serialized).not.toContain('4400')
    expect(serialized).not.toContain('reconciledGrossMarginCents')
    expect(projection.bundleKind).toBe('accepted_outcome_accrual_bundle')
    expect(projection.grossMarginReceipt.unevidencedStateIds).toContain(
      'payout_intent',
    )
    expect(
      projection.contributorAccrualLedger.entries.every(
        entry => entry.payableEvidenceState === 'not_yet_evidenced',
      ),
    ).toBe(true)
  })

  test('exposes a tagged invariant error type', () => {
    const error = new OmniContributorAccrualBundleInvariantError({
      reason: 'test',
    })
    expect(error._tag).toBe('OmniContributorAccrualBundleInvariantError')
  })
})
