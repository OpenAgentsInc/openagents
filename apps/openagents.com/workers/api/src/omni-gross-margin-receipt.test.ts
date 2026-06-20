import { describe, expect, test } from 'vitest'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  buildOmniGrossMarginReceipt,
  OmniGrossMarginReceiptInvariantError,
  publicOmniGrossMarginReceiptProjection,
  type OmniGrossMarginReceiptStateId,
} from './omni-gross-margin-receipt'

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

const lineFor = (
  record: OmniAcceptedOutcomeEconomicsRecord,
  stateId: OmniGrossMarginReceiptStateId,
) => {
  const line = buildOmniGrossMarginReceipt(record).lines.find(
    candidate => candidate.stateId === stateId,
  )
  if (line === undefined) {
    throw new Error(`missing line ${stateId}`)
  }
  return line
}

describe('buildOmniGrossMarginReceipt', () => {
  test('names every lifecycle state exactly once', () => {
    const receipt = buildOmniGrossMarginReceipt(baseRecord)
    expect(receipt.lines.map(line => line.stateId)).toEqual([
      'buyer_authorized',
      'buyer_paid',
      'accepted_value',
      'cost_basis',
      'gross_margin',
      'pending_balance_adjustment',
      'payout_intent',
      'settlement_attempt',
      'reconciliation',
    ])
  })

  test('records accounting figures and derives gross margin', () => {
    const receipt = buildOmniGrossMarginReceipt(baseRecord)
    expect(lineFor(baseRecord, 'accepted_value')).toMatchObject({
      amountCents: 5000,
      evidenceState: 'accounting_recorded',
      impliesSettlement: false,
    })
    expect(lineFor(baseRecord, 'cost_basis')).toMatchObject({
      amountCents: 600,
      evidenceState: 'accounting_recorded',
    })
    expect(lineFor(baseRecord, 'gross_margin')).toMatchObject({
      amountCents: 4400,
      evidenceState: 'derived',
      impliesSettlement: false,
    })
    expect(receipt.grossMarginCents).toBe(4400)
  })

  test('never collapses payout/settlement/reconciliation into evidenced states', () => {
    const receipt = buildOmniGrossMarginReceipt(baseRecord)
    const settlementStates: ReadonlyArray<OmniGrossMarginReceiptStateId> = [
      'buyer_paid',
      'pending_balance_adjustment',
      'payout_intent',
      'settlement_attempt',
      'reconciliation',
    ]
    for (const stateId of settlementStates) {
      const line = lineFor(baseRecord, stateId)
      expect(line.evidenceState).toBe('not_yet_evidenced')
      expect(line.impliesSettlement).toBe(true)
      expect(line.amountCents).toBeNull()
    }
    expect(receipt.unevidencedStateIds).toEqual(settlementStates)
    expect(receipt.evidencedStateCount).toBe(4)
  })

  test('reports buyer authorization in the buyer asset', () => {
    const satsRecord: OmniAcceptedOutcomeEconomicsRecord = {
      ...baseRecord,
      buyerPriceAsset: 'sats',
      buyerPriceCents: 0,
      fundingMode: 'sats_funded',
      satsCharged: 12000,
    }
    expect(lineFor(satsRecord, 'buyer_authorized')).toMatchObject({
      amountCents: 12000,
      asset: 'sats',
      evidenceState: 'accounting_recorded',
    })
  })

  test('marks buyer authorization unevidenced for free beta', () => {
    const freeRecord: OmniAcceptedOutcomeEconomicsRecord = {
      ...baseRecord,
      buyerPriceAsset: 'none',
      buyerPriceCents: 0,
      fundingMode: 'free_beta',
    }
    expect(lineFor(freeRecord, 'buyer_authorized')).toMatchObject({
      amountCents: null,
      asset: 'none',
      evidenceState: 'not_yet_evidenced',
    })
  })

  test('public projection keeps lifecycle labels but drops internal figures', () => {
    const projection = publicOmniGrossMarginReceiptProjection(
      buildOmniGrossMarginReceipt(baseRecord),
    )
    expect(projection.lines).toHaveLength(9)
    for (const line of projection.lines) {
      expect(line).not.toHaveProperty('amountCents')
      expect(line).not.toHaveProperty('asset')
    }
    expect(projection.unevidencedStateIds).toContain('settlement_attempt')
  })

  test('is deterministic for the same record', () => {
    expect(buildOmniGrossMarginReceipt(baseRecord)).toEqual(
      buildOmniGrossMarginReceipt(baseRecord),
    )
  })

  test('exposes a settlement invariant error type', () => {
    const error = new OmniGrossMarginReceiptInvariantError({ reason: 'x' })
    expect(error._tag).toBe('OmniGrossMarginReceiptInvariantError')
  })
})
