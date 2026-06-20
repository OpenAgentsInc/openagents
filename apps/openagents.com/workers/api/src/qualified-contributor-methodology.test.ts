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
  parseQualifiedContributorMethodologyInput,
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
    expect(verdict.countedLeaseRefs).toEqual(['lease.a.1'])
    expect(verdict.countedVerifiedWorkRefs).toEqual(['challenge.a.1'])
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

  test('flags two distinct contributors that share one real settlement receipt', () => {
    const sharedRef = 'receipt.nexus.tassadar_run_settlement.worker.shared'
    const result = verifyQualifiedContributorMethodology({
      claimedQualifiedContributorCount: 2,
      contributors: [
        qualified({
          pylonRef: 'pylon.a',
          settlementReceipts: [realSettlement({ receiptRef: sharedRef })],
        }),
        qualified({
          pylonRef: 'pylon.b',
          leaseRefs: ['lease.b.1'],
          verifiedExactTraceReplayChallengeRefs: ['challenge.b.1'],
          // Same real settlement reused: distinct pylonRefs, one Bitcoin movement.
          settlementReceipts: [realSettlement({ receiptRef: sharedRef })],
        }),
      ],
    })
    // Both pylonRefs are distinct, so the contributor count is 2, but the shared
    // receipt means it is not backed by two distinct real settlements.
    expect(result.qualifiedContributorCount).toBe(2)
    expect(result.conforms).toBe(false)
    expect(result.reasons).toContain(QualifiedRunReason.SharedSettlementReceipt)
    expect(result.reasons).not.toContain(QualifiedRunReason.DuplicateContributor)
  })

  test('flags two distinct contributors that share one window lease', () => {
    const sharedLease = 'lease.tassadar.window.shared'
    const result = verifyQualifiedContributorMethodology({
      claimedQualifiedContributorCount: 2,
      contributors: [
        qualified({
          pylonRef: 'pylon.a',
          leaseRefs: [sharedLease],
          settlementReceipts: [realSettlement({ receiptRef: 'receipt.a' })],
        }),
        qualified({
          pylonRef: 'pylon.b',
          // Same admitted lease reused: distinct pylonRefs, one admitted window.
          leaseRefs: [sharedLease],
          verifiedExactTraceReplayChallengeRefs: ['challenge.b.1'],
          settlementReceipts: [realSettlement({ receiptRef: 'receipt.b' })],
        }),
      ],
    })
    expect(result.qualifiedContributorCount).toBe(2)
    expect(result.conforms).toBe(false)
    expect(result.reasons).toContain(QualifiedRunReason.SharedLease)
    expect(result.reasons).not.toContain(QualifiedRunReason.DuplicateContributor)
  })

  test('flags two distinct contributors that share one verified work challenge', () => {
    const sharedChallenge = 'challenge.tassadar.exact_trace.shared'
    const result = verifyQualifiedContributorMethodology({
      claimedQualifiedContributorCount: 2,
      contributors: [
        qualified({
          pylonRef: 'pylon.a',
          verifiedExactTraceReplayChallengeRefs: [sharedChallenge],
          settlementReceipts: [realSettlement({ receiptRef: 'receipt.a' })],
        }),
        qualified({
          pylonRef: 'pylon.b',
          leaseRefs: ['lease.b.1'],
          // Same verified work credited twice: distinct pylonRefs, one piece of work.
          verifiedExactTraceReplayChallengeRefs: [sharedChallenge],
          settlementReceipts: [realSettlement({ receiptRef: 'receipt.b' })],
        }),
      ],
    })
    expect(result.qualifiedContributorCount).toBe(2)
    expect(result.conforms).toBe(false)
    expect(result.reasons).toContain(QualifiedRunReason.SharedVerifiedWork)
    expect(result.reasons).not.toContain(QualifiedRunReason.DuplicateContributor)
  })

  test('conforms when two contributors each have their own distinct receipt', () => {
    const result = verifyQualifiedContributorMethodology({
      claimedQualifiedContributorCount: 2,
      contributors: [
        qualified({
          pylonRef: 'pylon.a',
          settlementReceipts: [realSettlement({ receiptRef: 'receipt.a' })],
        }),
        qualified({
          pylonRef: 'pylon.b',
          leaseRefs: ['lease.b.1'],
          verifiedExactTraceReplayChallengeRefs: ['challenge.b.1'],
          settlementReceipts: [realSettlement({ receiptRef: 'receipt.b' })],
        }),
      ],
    })
    expect(result.conforms).toBe(true)
    expect(result.reasons).not.toContain(QualifiedRunReason.SharedSettlementReceipt)
    expect(result.reasons).not.toContain(QualifiedRunReason.SharedLease)
    expect(result.reasons).not.toContain(QualifiedRunReason.SharedVerifiedWork)
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

describe('parseQualifiedContributorMethodologyInput', () => {
  // A structurally sound, public-safe document an auditor could load from JSON.
  const validDocument = (): unknown => ({
    claimedQualifiedContributorCount: 2,
    contributors: [
      {
        pylonRef: 'pylon.a',
        leaseRefs: ['lease.a.1'],
        verifiedExactTraceReplayChallengeRefs: ['challenge.a.1'],
        settlementReceipts: [
          {
            receiptRef: 'receipt.a',
            state: 'settled',
            providerConfirmed: true,
            realBitcoinMoved: true,
          },
        ],
      },
      {
        pylonRef: 'pylon.b',
        leaseRefs: ['lease.b.1'],
        verifiedExactTraceReplayChallengeRefs: ['challenge.b.1'],
        settlementReceipts: [
          {
            receiptRef: 'receipt.b',
            state: 'settled',
            providerConfirmed: true,
            realBitcoinMoved: true,
          },
        ],
      },
    ],
  })

  test('parses a sound document and the result feeds the verifier', () => {
    const parsed = parseQualifiedContributorMethodologyInput(validDocument())
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return
    const result = verifyQualifiedContributorMethodology(parsed.value)
    expect(result.conforms).toBe(true)
    expect(result.qualifiedContributorCount).toBe(2)
  })

  test('round-trips through JSON (the real auditor path)', () => {
    const parsed = parseQualifiedContributorMethodologyInput(
      JSON.parse(JSON.stringify(validDocument())),
    )
    expect(parsed.ok).toBe(true)
  })

  test('rejects a non-object document', () => {
    const parsed = parseQualifiedContributorMethodologyInput([])
    expect(parsed.ok).toBe(false)
    if (parsed.ok) return
    expect(parsed.errors).toContain('not-an-object:$')
  })

  test('rejects a leak-prone extra key at the document level', () => {
    const doc = validDocument() as Record<string, unknown>
    doc.walletBalanceSats = 1005
    const parsed = parseQualifiedContributorMethodologyInput(doc)
    expect(parsed.ok).toBe(false)
    if (parsed.ok) return
    expect(parsed.errors).toContain('unexpected-key:$.walletBalanceSats')
  })

  test('rejects a leak-prone extra key on a settlement receipt', () => {
    const parsed = parseQualifiedContributorMethodologyInput({
      claimedQualifiedContributorCount: 1,
      contributors: [
        {
          pylonRef: 'pylon.a',
          leaseRefs: ['lease.a.1'],
          verifiedExactTraceReplayChallengeRefs: ['challenge.a.1'],
          settlementReceipts: [
            {
              receiptRef: 'receipt.a',
              state: 'settled',
              providerConfirmed: true,
              realBitcoinMoved: true,
              rawSparkAddress: 'sp1qredacted',
            },
          ],
        },
      ],
    })
    expect(parsed.ok).toBe(false)
    if (parsed.ok) return
    expect(parsed.errors).toContain(
      'unexpected-key:$.contributors[0].settlementReceipts[0].rawSparkAddress',
    )
  })

  test('rejects a non-integer claimed count', () => {
    const doc = validDocument() as Record<string, unknown>
    doc.claimedQualifiedContributorCount = 1.5
    const parsed = parseQualifiedContributorMethodologyInput(doc)
    expect(parsed.ok).toBe(false)
    if (parsed.ok) return
    expect(parsed.errors).toContain(
      'not-a-non-negative-integer:$.claimedQualifiedContributorCount',
    )
  })

  test('rejects a mistyped settlement state with a path-qualified error', () => {
    const parsed = parseQualifiedContributorMethodologyInput({
      claimedQualifiedContributorCount: 2,
      contributors: [
        {
          pylonRef: 'pylon.a',
          leaseRefs: ['lease.a.1'],
          verifiedExactTraceReplayChallengeRefs: ['challenge.a.1'],
          settlementReceipts: [
            {
              receiptRef: 'receipt.a',
              state: 'settled',
              providerConfirmed: true,
              realBitcoinMoved: true,
            },
          ],
        },
        {
          pylonRef: 'pylon.b',
          leaseRefs: ['lease.b.1'],
          verifiedExactTraceReplayChallengeRefs: ['challenge.b.1'],
          settlementReceipts: [
            {
              receiptRef: 'receipt.b',
              state: 7,
              providerConfirmed: true,
              realBitcoinMoved: true,
            },
          ],
        },
      ],
    })
    expect(parsed.ok).toBe(false)
    if (parsed.ok) return
    expect(parsed.errors).toContain(
      'not-a-string:$.contributors[1].settlementReceipts[0].state',
    )
  })

  test('rejects contributors that is not an array', () => {
    const doc = validDocument() as Record<string, unknown>
    doc.contributors = { 0: 'nope' }
    const parsed = parseQualifiedContributorMethodologyInput(doc)
    expect(parsed.ok).toBe(false)
    if (parsed.ok) return
    expect(parsed.errors).toContain('not-an-array:$.contributors')
  })

  test('rejects a non-string ref inside leaseRefs with its index', () => {
    const parsed = parseQualifiedContributorMethodologyInput({
      claimedQualifiedContributorCount: 1,
      contributors: [
        {
          pylonRef: 'pylon.a',
          leaseRefs: ['lease.ok', 42],
          verifiedExactTraceReplayChallengeRefs: ['challenge.a.1'],
          settlementReceipts: [
            {
              receiptRef: 'receipt.a',
              state: 'settled',
              providerConfirmed: true,
              realBitcoinMoved: true,
            },
          ],
        },
      ],
    })
    expect(parsed.ok).toBe(false)
    if (parsed.ok) return
    expect(parsed.errors).toContain(
      'not-a-string:$.contributors[0].leaseRefs[1]',
    )
  })

  test('an empty contributors array parses (verifier then judges the count)', () => {
    const parsed = parseQualifiedContributorMethodologyInput({
      claimedQualifiedContributorCount: 0,
      contributors: [],
    })
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) return
    expect(
      verifyQualifiedContributorMethodology(parsed.value).conforms,
    ).toBe(true)
  })
})
