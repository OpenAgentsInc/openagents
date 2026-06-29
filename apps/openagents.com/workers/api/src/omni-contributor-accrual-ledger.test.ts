import { describe, expect, test } from 'vitest'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  buildOmniContributorAccrualLedger,
  OmniContributorAccrualLedgerInvariantError,
  OmniContributorAccrualLedgerValidationError,
  publicOmniContributorAccrualLedgerProjection,
  type OmniContributorAccrualShare,
} from './omni-contributor-accrual-ledger'

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

const evenShares: ReadonlyArray<OmniContributorAccrualShare> = [
  { contributorId: 'runner-1', role: 'runner', shareBasisPoints: 6000 },
  { contributorId: 'reviewer-1', role: 'reviewer', shareBasisPoints: 1000 },
  { contributorId: 'platform', role: 'platform', shareBasisPoints: 3000 },
]

describe('buildOmniContributorAccrualLedger', () => {
  test('attributes derived gross margin by basis-points share', () => {
    const ledger = buildOmniContributorAccrualLedger(baseRecord, evenShares)
    expect(ledger.distributableMarginCents).toBe(4400)
    expect(
      ledger.entries.map(entry => [
        entry.contributorId,
        entry.accruedMarginCents,
      ]),
    ).toEqual([
      ['runner-1', 2640],
      ['reviewer-1', 440],
      ['platform', 1320],
    ])
    expect(ledger.totalAccruedCents).toBe(4400)
    expect(ledger.totalShareBasisPoints).toBe(10000)
  })

  test('accruals always sum exactly to the pool despite rounding', () => {
    // 100 cents split 1/3, 1/3, 1/3 -> 34, 33, 33 by largest remainder.
    const thirdsRecord: OmniAcceptedOutcomeEconomicsRecord = {
      ...baseRecord,
      grossMarginCents: 100,
    }
    const thirds: ReadonlyArray<OmniContributorAccrualShare> = [
      { contributorId: 'a', role: 'runner', shareBasisPoints: 3334 },
      { contributorId: 'b', role: 'reviewer', shareBasisPoints: 3333 },
      { contributorId: 'c', role: 'platform', shareBasisPoints: 3333 },
    ]
    const ledger = buildOmniContributorAccrualLedger(thirdsRecord, thirds)
    expect(ledger.entries.map(entry => entry.accruedMarginCents)).toEqual([
      34, 33, 33,
    ])
    expect(ledger.totalAccruedCents).toBe(100)
  })

  test('labels accruals derived and payable/settlement unevidenced', () => {
    const ledger = buildOmniContributorAccrualLedger(baseRecord, evenShares)
    for (const entry of ledger.entries) {
      expect(entry.accrualEvidenceState).toBe('accrual_derived')
      expect(entry.payableEvidenceState).toBe('not_yet_evidenced')
      expect(entry.settlementEvidenceState).toBe('not_yet_evidenced')
      expect(entry.impliesSettlement).toBe(true)
    }
    expect(ledger.settlementEvidencedEntryCount).toBe(0)
    expect(ledger.noSettlementImplication).toBe(true)
  })

  test('a loss accrues nothing rather than negative balances', () => {
    const lossRecord: OmniAcceptedOutcomeEconomicsRecord = {
      ...baseRecord,
      grossMarginCents: -500,
    }
    const ledger = buildOmniContributorAccrualLedger(lossRecord, evenShares)
    expect(ledger.distributableMarginCents).toBe(0)
    expect(ledger.entries.every(entry => entry.accruedMarginCents === 0)).toBe(
      true,
    )
    expect(ledger.totalAccruedCents).toBe(0)
    expect(ledger.grossMarginCents).toBe(-500)
  })

  test('rejects shares that do not sum to 10000 basis points', () => {
    expect(() =>
      buildOmniContributorAccrualLedger(baseRecord, [
        { contributorId: 'runner-1', role: 'runner', shareBasisPoints: 5000 },
        { contributorId: 'platform', role: 'platform', shareBasisPoints: 4000 },
      ]),
    ).toThrow(OmniContributorAccrualLedgerValidationError)
  })

  test('rejects duplicate contributor ids', () => {
    expect(() =>
      buildOmniContributorAccrualLedger(baseRecord, [
        { contributorId: 'dup', role: 'runner', shareBasisPoints: 5000 },
        { contributorId: 'dup', role: 'platform', shareBasisPoints: 5000 },
      ]),
    ).toThrow(OmniContributorAccrualLedgerValidationError)
  })

  test('rejects an empty share set', () => {
    expect(() =>
      buildOmniContributorAccrualLedger(baseRecord, []),
    ).toThrow(OmniContributorAccrualLedgerValidationError)
  })

  test('rejects an unsafe contributor id', () => {
    expect(() =>
      buildOmniContributorAccrualLedger(baseRecord, [
        { contributorId: 'has space', role: 'runner', shareBasisPoints: 10000 },
      ]),
    ).toThrow(OmniContributorAccrualLedgerValidationError)
  })

  test('public projection keeps shares and labels but drops cents', () => {
    const projection = publicOmniContributorAccrualLedgerProjection(
      buildOmniContributorAccrualLedger(baseRecord, evenShares),
    )
    expect(projection.entries).toHaveLength(3)
    for (const entry of projection.entries) {
      expect(entry).not.toHaveProperty('accruedMarginCents')
      expect(entry.accrualEvidenceState).toBe('accrual_derived')
      expect(entry.payableEvidenceState).toBe('not_yet_evidenced')
    }
    expect(projection).not.toHaveProperty('totalAccruedCents')
    expect(projection.totalShareBasisPoints).toBe(10000)
  })

  test('is deterministic for the same inputs', () => {
    expect(
      buildOmniContributorAccrualLedger(baseRecord, evenShares),
    ).toEqual(buildOmniContributorAccrualLedger(baseRecord, evenShares))
  })

  test('exposes an invariant error type', () => {
    const error = new OmniContributorAccrualLedgerInvariantError({
      reason: 'x',
    })
    expect(error._tag).toBe('OmniContributorAccrualLedgerInvariantError')
  })
})
