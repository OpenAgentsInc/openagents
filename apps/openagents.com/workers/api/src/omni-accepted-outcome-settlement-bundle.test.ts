import { describe, expect, test } from 'vitest'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  buildOmniAcceptedOutcomeSettlementBundle,
  OmniAcceptedOutcomeSettlementBundleInvariantError,
  publicOmniAcceptedOutcomeSettlementBundleProjection,
} from './omni-accepted-outcome-settlement-bundle'

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
  metadata: {
    contributors: {
      platformId: 'platform.openagents',
      reviewerId: 'reviewer.bob',
      runnerId: 'runner.alice',
    },
  },
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

describe('buildOmniAcceptedOutcomeSettlementBundle', () => {
  test('composes a complete eight-state machine + reconciled accrual bundle, INERT', () => {
    const bundle = buildOmniAcceptedOutcomeSettlementBundle(baseRecord)
    expect(bundle.bundleKind).toBe('accepted_outcome_settlement_bundle')
    expect(bundle.economicsId).toBe('omni_outcome_economics_1')
    expect(bundle.settlementComplete).toBe(true)
    expect(bundle.settlementMachine.transitions).toHaveLength(8)
    expect(bundle.settlementMachine.dispatchArmed).toBe(false)
    // INERT: nothing moved money, all views disclaim settlement.
    expect(
      bundle.settlementMachine.transitions.some(t => t.movedMoney),
    ).toBe(false)
    expect(bundle.settlementMachine.noSettlementImplication).toBe(true)
    expect(
      bundle.contributorAccrualBundle.grossMarginReceipt.noSettlementImplication,
    ).toBe(true)
  })

  test('reconciles margin across the machine and the accrual bundle', () => {
    const bundle = buildOmniAcceptedOutcomeSettlementBundle(baseRecord)
    const marginTransition = bundle.settlementMachine.transitions.find(
      t => t.stateId === 'margin',
    )
    expect(marginTransition?.amountCents).toBe(4400)
    expect(bundle.contributorAccrualBundle.reconciledGrossMarginCents).toBe(4400)
    expect(
      bundle.contributorAccrualBundle.grossMarginReceipt.grossMarginCents,
    ).toBe(4400)
  })

  test('reconciles pending_payout against the ledger distributable pool', () => {
    const bundle = buildOmniAcceptedOutcomeSettlementBundle(baseRecord)
    const pendingPayout = bundle.settlementMachine.transitions.find(
      t => t.stateId === 'pending_payout',
    )
    expect(pendingPayout?.amountCents).toBe(
      bundle.contributorAccrualBundle.contributorAccrualLedger
        .distributableMarginCents,
    )
  })

  test('an armed bundle records money movement and drops the disclaimer', () => {
    const bundle = buildOmniAcceptedOutcomeSettlementBundle(baseRecord, {
      dispatchArmed: true,
    })
    expect(
      bundle.settlementMachine.transitions.some(t => t.movedMoney),
    ).toBe(true)
    expect(bundle.settlementMachine.noSettlementImplication).toBe(false)
  })

  test('fails when the record names no contributors (no fabrication)', () => {
    expect(() =>
      buildOmniAcceptedOutcomeSettlementBundle({
        ...baseRecord,
        metadata: {},
      }),
    ).toThrow()
  })

  test('handles a loss: zero distributable payout, negative margin', () => {
    const lossRecord: OmniAcceptedOutcomeEconomicsRecord = {
      ...baseRecord,
      acceptedValueCents: 100,
      grossMarginCents: -500,
      totalCostCents: 600,
    }
    const bundle = buildOmniAcceptedOutcomeSettlementBundle(lossRecord)
    const margin = bundle.settlementMachine.transitions.find(
      t => t.stateId === 'margin',
    )
    const pendingPayout = bundle.settlementMachine.transitions.find(
      t => t.stateId === 'pending_payout',
    )
    expect(margin?.amountCents).toBe(-500)
    expect(pendingPayout?.amountCents).toBe(0)
    expect(
      bundle.contributorAccrualBundle.contributorAccrualLedger.totalAccruedCents,
    ).toBe(0)
  })
})

describe('OmniAcceptedOutcomeSettlementBundleInvariantError', () => {
  test('is a tagged error', () => {
    const error = new OmniAcceptedOutcomeSettlementBundleInvariantError({
      reason: 'x',
    })
    expect(error._tag).toBe('OmniAcceptedOutcomeSettlementBundleInvariantError')
  })
})

describe('publicOmniAcceptedOutcomeSettlementBundleProjection', () => {
  test('drops internal figures but keeps lifecycle + evidence labels', () => {
    const bundle = buildOmniAcceptedOutcomeSettlementBundle(baseRecord)
    const projection =
      publicOmniAcceptedOutcomeSettlementBundleProjection(bundle)
    expect(projection.settlementComplete).toBe(true)
    expect(projection.settlementMachine.transitions).toHaveLength(8)
    for (const transition of projection.settlementMachine.transitions) {
      expect(transition).not.toHaveProperty('amountCents')
      expect(transition).toHaveProperty('evidenceKind')
    }
    // accrual view keeps evidence labels, drops figures
    for (const entry of projection.contributorAccrualBundle
      .contributorAccrualLedger.entries) {
      expect(entry).not.toHaveProperty('accruedMarginCents')
      expect(entry).toHaveProperty('role')
    }
  })
})
