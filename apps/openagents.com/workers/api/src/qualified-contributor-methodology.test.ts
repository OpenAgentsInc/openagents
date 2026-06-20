// Tests for the qualified-contributor (participant/scale) methodology verifier.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing
import { describe, expect, test } from 'vitest'

import {
  QualifiedContributorReason,
  QualifiedRunReason,
  type QualifiedContributorEvidence,
  type QualifiedContributorSettlementEvidence,
  verifyQualifiedContributor,
  verifyQualifiedContributorMethodology,
} from './qualified-contributor-methodology'

const realSettlement = (
  overrides: Partial<QualifiedContributorSettlementEvidence> = {},
): QualifiedContributorSettlementEvidence => ({
  receiptRef: 'receipt.nexus.tassadar_run_settlement.worker.a',
  state: 'settled',
  providerConfirmed: true,
  realBitcoinMoved: true,
  ...overrides,
})

const qualified = (
  overrides: Partial<QualifiedContributorEvidence> = {},
): QualifiedContributorEvidence => ({
  pylonRef: 'pylon.contributor.a',
  leaseRefs: ['lease.a.1'],
  verifiedExactTraceReplayChallengeRefs: ['challenge.a.1'],
  settlementReceipts: [realSettlement()],
  ...overrides,
})

describe('verifyQualifiedContributor', () => {
  test('counts a contributor that satisfies all three prongs', () => {
    const verdict = verifyQualifiedContributor(qualified())
    expect(verdict.counts).toBe(true)
    expect(verdict.reasons).toEqual([])
    expect(verdict.countedSettlementReceiptRefs).toEqual([
      'receipt.nexus.tassadar_run_settlement.worker.a',
    ])
  })

  test('rejects a contributor with no lease (not admitted)', () => {
    const verdict = verifyQualifiedContributor(qualified({ leaseRefs: [] }))
    expect(verdict.counts).toBe(false)
    expect(verdict.reasons).toContain(
      QualifiedContributorReason.NotAdmittedNoLease,
    )
  })

  test('rejects a contributor with no replay-verified work', () => {
    const verdict = verifyQualifiedContributor(
      qualified({ verifiedExactTraceReplayChallengeRefs: [] }),
    )
    expect(verdict.counts).toBe(false)
    expect(verdict.reasons).toContain(
      QualifiedContributorReason.NoReplayVerifiedWork,
    )
  })

  test('rejects a contributor with no settlement receipt at all', () => {
    const verdict = verifyQualifiedContributor(
      qualified({ settlementReceipts: [] }),
    )
    expect(verdict.counts).toBe(false)
    expect(verdict.reasons).toContain(
      QualifiedContributorReason.NoSettlementReceipt,
    )
  })

  test('excludes simulation-only (realBitcoinMoved:false) receipts', () => {
    const verdict = verifyQualifiedContributor(
      qualified({
        settlementReceipts: [realSettlement({ realBitcoinMoved: false })],
      }),
    )
    expect(verdict.counts).toBe(false)
    expect(verdict.reasons).toContain(
      QualifiedContributorReason.SettlementSimulationOnly,
    )
    expect(verdict.countedSettlementReceiptRefs).toEqual([])
  })

  test('excludes non-settled (e.g. pending/claimed) receipts', () => {
    const verdict = verifyQualifiedContributor(
      qualified({ settlementReceipts: [realSettlement({ state: 'pending' })] }),
    )
    expect(verdict.counts).toBe(false)
    expect(verdict.reasons).toContain(
      QualifiedContributorReason.SettlementNotSettledState,
    )
  })

  test('excludes wallet-side receipts not provider-confirmed', () => {
    const verdict = verifyQualifiedContributor(
      qualified({
        settlementReceipts: [realSettlement({ providerConfirmed: false })],
      }),
    )
    expect(verdict.counts).toBe(false)
    expect(verdict.reasons).toContain(
      QualifiedContributorReason.SettlementNotProviderConfirmed,
    )
  })

  test('counts when at least one real receipt is present among excluded ones', () => {
    const verdict = verifyQualifiedContributor(
      qualified({
        settlementReceipts: [
          realSettlement({
            receiptRef: 'receipt.sim',
            realBitcoinMoved: false,
          }),
          realSettlement({ receiptRef: 'receipt.real' }),
        ],
      }),
    )
    expect(verdict.counts).toBe(true)
    expect(verdict.countedSettlementReceiptRefs).toEqual(['receipt.real'])
  })

  test('rejects an empty pylon ref', () => {
    const verdict = verifyQualifiedContributor(qualified({ pylonRef: '  ' }))
    expect(verdict.counts).toBe(false)
    expect(verdict.reasons).toContain(QualifiedContributorReason.PylonRefEmpty)
  })
})

describe('verifyQualifiedContributorMethodology', () => {
  test('conforms when the claimed count matches the rule (two real contributors)', () => {
    const result = verifyQualifiedContributorMethodology({
      claimedQualifiedContributorCount: 2,
      contributors: [
        qualified({ pylonRef: 'pylon.a' }),
        qualified({
          pylonRef: 'pylon.b',
          leaseRefs: ['lease.b.1'],
          verifiedExactTraceReplayChallengeRefs: ['challenge.b.1'],
          settlementReceipts: [realSettlement({ receiptRef: 'receipt.b' })],
        }),
      ],
    })
    expect(result.conforms).toBe(true)
    expect(result.qualifiedContributorCount).toBe(2)
    expect(result.reasons).toEqual([])
  })

  test('flags an inflated claim (registrations counted but not qualified)', () => {
    const result = verifyQualifiedContributorMethodology({
      claimedQualifiedContributorCount: 2,
      contributors: [
        qualified({ pylonRef: 'pylon.a' }),
        // Raw registration: admitted but no verified work, no settlement.
        {
          pylonRef: 'pylon.registration',
          leaseRefs: ['lease.reg'],
          verifiedExactTraceReplayChallengeRefs: [],
          settlementReceipts: [],
        },
      ],
    })
    expect(result.conforms).toBe(false)
    expect(result.qualifiedContributorCount).toBe(1)
    expect(result.reasons).toContain(QualifiedRunReason.ClaimedCountMismatch)
  })

  test('flags a double-counted contributor', () => {
    const result = verifyQualifiedContributorMethodology({
      claimedQualifiedContributorCount: 2,
      contributors: [
        qualified({ pylonRef: 'pylon.dup' }),
        qualified({ pylonRef: 'pylon.dup' }),
      ],
    })
    expect(result.conforms).toBe(false)
    expect(result.reasons).toContain(QualifiedRunReason.DuplicateContributor)
  })

  test('an empty run conforms only to a claimed count of zero', () => {
    expect(
      verifyQualifiedContributorMethodology({
        claimedQualifiedContributorCount: 0,
        contributors: [],
      }).conforms,
    ).toBe(true)
    expect(
      verifyQualifiedContributorMethodology({
        claimedQualifiedContributorCount: 1,
        contributors: [],
      }).conforms,
    ).toBe(false)
  })
})
