import { describe, expect, it } from 'vitest'

import {
  assertNexusTreasuryPayoutAttemptSafe,
  assertNexusTreasuryPayoutIntentSafe,
  assertNexusTreasuryPayoutLedgerRecordSafe,
} from './nexus-treasury-payout-ledger'
import { settledSatsFromPaymentAuthorityReceipt } from './training-leaderboards'
import { buildTrainingRunRecord } from './training-run-window-authority'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import {
  TassadarRunSettlementUnsafe,
  buildTassadarRunSettlement,
} from './tassadar-run-settlement'
import type { TrainingVerificationChallengeRecord } from './training-verification'

const nowIso = '2026-06-14T10:05:00.000Z'

const activeRun = (
  overrides: Partial<TrainingRunRecord> = {},
): TrainingRunRecord => ({
  ...buildTrainingRunRecord({
    makeId: () => 'run',
    nowIso: '2026-06-14T10:00:00.000Z',
    request: {
      manifest: { artifactDigestRefs: [], blockerRefs: [], spendCapSats: 100 },
      promiseRef: 'training.decentralized_training_launch.v1',
      trainingRunRef: 'run.tassadar.executor.20260615',
    },
  }),
  state: 'active',
  ...overrides,
})

const verifiedChallenge = (
  overrides: Partial<TrainingVerificationChallengeRecord> = {},
): TrainingVerificationChallengeRecord => ({
  challengeRef: 'challenge.tassadar.5009',
  commitmentRefs: ['commitment.tassadar.5009'],
  contributionRef: 'contribution.tassadar.5009',
  createdAt: '2026-06-14T10:02:00.000Z',
  failureCodes: [],
  homeworkKind: 'admin_dispatched_homework',
  id: 'challenge_5009',
  leaseExpiresAt: null,
  leaseRef: null,
  leasedToRef: null,
  maxAttempts: 3,
  payloadJson: '{}',
  publicProjectionJson: '{}',
  rejectedAt: null,
  samplingPolicy: 'per_contribution',
  state: 'Verified',
  timedOutAt: null,
  trainingRunRef: 'run.tassadar.executor.20260615',
  updatedAt: '2026-06-14T10:03:00.000Z',
  verdictRefs: ['verdict.tassadar.5009'],
  verificationClass: 'exact_trace_replay',
  verifiedAt: '2026-06-14T10:03:00.000Z',
  windowRef: 'training.window.tassadar.executor.20260615.w1',
  ...overrides,
})

const activeLease = (
  overrides: Partial<TrainingWindowLeaseRecord> = {},
): TrainingWindowLeaseRecord => ({
  claimedAt: '2026-06-14T10:01:00.000Z',
  id: 'lease5009',
  leaseExpiresAt: '2026-06-14T12:00:00.000Z',
  leaseRef: 'lease.tassadar.5009',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.contributor.stranger',
  receiptRefs: [],
  state: 'active',
  trainingRunRef: 'run.tassadar.executor.20260615',
  windowRef: 'training.window.tassadar.executor.20260615.w1',
  ...overrides,
})

const baseRequest = {
  amountSats: 21,
  challengeRef: 'challenge.tassadar.5009',
  idempotencyRef: 'idem.tassadar.5009',
  leaseRef: 'lease.tassadar.5009',
  operatorApprovalRef: 'operator.approval.5009',
  payoutTargetApprovalRef: 'payout.target.approval.5009',
  payoutTargetRef: 'payout.target.5009',
} as const

describe('buildTassadarRunSettlement', () => {
  it('produces a provider-confirmed settlement receipt the leaderboard reads back', () => {
    const result = buildTassadarRunSettlement({
      challenge: verifiedChallenge(),
      lease: activeLease(),
      nowIso,
      request: baseRequest,
      run: activeRun(),
    })

    expect(result.amountSats).toBe(21)
    expect(result.contributorRef).toBe('pylon.contributor.stranger')
    // The payout intent foreign-keys the payout-target approval, so the build
    // must produce an approval row whose approvalRef matches the intent's
    // payoutTargetApprovalRef (otherwise the ledger insert fails the FK).
    expect(result.targetApproval.approvalRef).toBe(
      result.intent.payoutTargetApprovalRef,
    )
    expect(result.targetApproval.payoutTargetRef).toBe(result.intent.payoutTargetRef)
    expect(result.targetApproval.pylonRef).toBe('pylon.contributor.stranger')
    expect(result.targetApproval.status).toBe('active')
    expect(result.settlementReceipt.receiptKind).toBe('settlement_recorded')
    // The read contract: settledSatsFromPaymentAuthorityReceipt must see this
    // receipt as 21 settled sats (state settled + positive integer amountSats).
    expect(
      settledSatsFromPaymentAuthorityReceipt(result.settlementReceipt),
    ).toBe(21)
    expect(result.settlementReceiptRef).toBe(result.settlementReceipt.receiptRef)
    expect(result.intent.sourceKind).toBe('accepted_work')
    expect(result.reconciliationEvent.status).toBe('matched')
  })

  it('keeps the whole ledger chain free of private payment/wallet material', () => {
    const result = buildTassadarRunSettlement({
      challenge: verifiedChallenge(),
      lease: activeLease(),
      nowIso,
      request: { ...baseRequest, adapterKind: 'mdk_agent_wallet' },
      run: activeRun(),
    })

    expect(() =>
      assertNexusTreasuryPayoutIntentSafe(result.intent),
    ).not.toThrow()
    expect(() =>
      assertNexusTreasuryPayoutAttemptSafe(result.attempt),
    ).not.toThrow()
    expect(() =>
      assertNexusTreasuryPayoutLedgerRecordSafe(
        'reconciliation',
        result.reconciliationEvent,
      ),
    ).not.toThrow()
    expect(() =>
      assertNexusTreasuryPayoutLedgerRecordSafe(
        'settlement receipt',
        result.settlementReceipt,
      ),
    ).not.toThrow()
  })

  it('refuses to settle non-verified work', () => {
    expect(() =>
      buildTassadarRunSettlement({
        challenge: verifiedChallenge({ state: 'Leased' }),
        lease: activeLease(),
        nowIso,
        request: baseRequest,
        run: activeRun(),
      }),
    ).toThrowError(TassadarRunSettlementUnsafe)
  })

  it('refuses non-exact_trace_replay verification classes', () => {
    expect(() =>
      buildTassadarRunSettlement({
        challenge: verifiedChallenge({
          verificationClass: 'deterministic_recompute',
        }),
        lease: activeLease(),
        nowIso,
        request: baseRequest,
        run: activeRun(),
      }),
    ).toThrowError(TassadarRunSettlementUnsafe)
  })

  it('refuses a challenge or lease that belongs to a different run', () => {
    expect(() =>
      buildTassadarRunSettlement({
        challenge: verifiedChallenge({ trainingRunRef: 'run.other' }),
        lease: activeLease(),
        nowIso,
        request: baseRequest,
        run: activeRun(),
      }),
    ).toThrowError(TassadarRunSettlementUnsafe)
  })

  it('requires a run spend cap and rejects amounts over it', () => {
    expect(() =>
      buildTassadarRunSettlement({
        challenge: verifiedChallenge(),
        lease: activeLease(),
        nowIso,
        request: baseRequest,
        run: activeRun({ manifest: null }),
      }),
    ).toThrowError(TassadarRunSettlementUnsafe)

    expect(() =>
      buildTassadarRunSettlement({
        challenge: verifiedChallenge(),
        lease: activeLease(),
        nowIso,
        request: { ...baseRequest, amountSats: 500 },
        run: activeRun(),
      }),
    ).toThrowError(TassadarRunSettlementUnsafe)
  })

  it('refuses to settle a planned run', () => {
    expect(() =>
      buildTassadarRunSettlement({
        challenge: verifiedChallenge(),
        lease: activeLease(),
        nowIso,
        request: baseRequest,
        run: activeRun({ state: 'planned' }),
      }),
    ).toThrowError(TassadarRunSettlementUnsafe)
  })
})
