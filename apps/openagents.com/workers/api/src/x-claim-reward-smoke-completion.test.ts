import { describe, expect, test } from 'vitest'

import type { XClaimRewardRecord } from './agent-owner-claim-routes'
import { assertXClaimRewardSmokeCompletion } from './x-claim-reward-smoke-completion'
import type {
  XClaimRewardTreasuryDispatchStats,
  XClaimRewardTreasuryDispatchSummary,
} from './x-claim-reward-treasury-dispatcher'

const cleanStats = (
  overrides: Partial<XClaimRewardTreasuryDispatchStats> = {},
): XClaimRewardTreasuryDispatchStats => ({
  dailySatsCap: 5000,
  enabled: true,
  liquidityBufferSats: 11,
  pendingPaymentCount: 0,
  perRunRewardCap: 1,
  requestedDispatchCount: 0,
  todayReservedSats: 1000,
  ...overrides,
})

const settledSummary = (
  overrides: Partial<XClaimRewardTreasuryDispatchSummary> = {},
): XClaimRewardTreasuryDispatchSummary => ({
  failed: 0,
  pending: 0,
  polled: 0,
  requested: 1,
  settled: 1,
  skippedReasonRefs: [],
  stats: cleanStats(),
  ...overrides,
})

const settledReward = (
  overrides: Partial<XClaimRewardRecord> = {},
): XClaimRewardRecord => ({
  agentUserId: 'user_agent_1',
  amountSats: 1000,
  challengeId: 'x_challenge_1',
  claimId: 'agent_claim_1',
  createdAt: '2026-06-10T10:00:00.000Z',
  evidenceRefs: [
    'receipt.public.x_claim.1',
    'settlement_evidence.public.mdk_treasury.x_claim_reward_1',
    'receipt.public.x_claim_reward.settled_x_claim_reward_1',
  ],
  id: 'x_claim_reward_1',
  ownerUserId: 'user_owner_1',
  receiptRef: 'x_claim_reward_receipt_x_claim_reward_1',
  state: 'settled',
  stateReasonRef: null,
  treasuryPaymentId: 'payment_secret_1',
  updatedAt: '2026-06-10T12:00:00.000Z',
  xAccountRef: 'x_account.public.owner_1',
  ...overrides,
})

describe('X claim reward smoke completion gate', () => {
  test('emits the transition request when run and row both pass', () => {
    const report = assertXClaimRewardSmokeCompletion({
      reward: settledReward(),
      summary: settledSummary(),
    })

    expect(report.ready).toBe(true)
    expect(report.blockingReasonRefs).toEqual([])
    expect(report.dispatchOutcome.ok).toBe(true)
    expect(report.transitionProposal.ready).toBe(true)
    expect(report.transitionRequest).toEqual({
      evidenceRefs: [
        'x_claim_reward_receipt_x_claim_reward_1',
        'settlement_evidence.public.mdk_treasury.x_claim_reward_1',
        'receipt.public.x_claim_reward.settled_x_claim_reward_1',
      ],
      promiseId: 'agents.x_claim_reward.v1',
      toState: 'green',
    })
  })

  test('withholds the transition when the dispatch run was not clean', () => {
    const report = assertXClaimRewardSmokeCompletion({
      reward: settledReward(),
      summary: settledSummary({ settled: 2 }),
    })

    expect(report.ready).toBe(false)
    expect(report.transitionRequest).toBeNull()
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_completion_dispatch_run_not_clean',
    )
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_dispatch_not_exactly_one_settled',
    )
    // The row itself is clean, so its proposal alone would have been ready.
    expect(report.transitionProposal.ready).toBe(true)
  })

  test('withholds the transition when the run left a payment pending', () => {
    const report = assertXClaimRewardSmokeCompletion({
      reward: settledReward(),
      summary: settledSummary({
        pending: 1,
        stats: cleanStats({ pendingPaymentCount: 1 }),
      }),
    })

    expect(report.ready).toBe(false)
    expect(report.transitionRequest).toBeNull()
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_dispatch_payment_still_pending',
    )
  })

  test('withholds the transition when the settled row is not ready', () => {
    const report = assertXClaimRewardSmokeCompletion({
      reward: settledReward({ state: 'dispatched' }),
      summary: settledSummary(),
    })

    expect(report.ready).toBe(false)
    expect(report.transitionRequest).toBeNull()
    expect(report.dispatchOutcome.ok).toBe(true)
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_completion_settled_row_not_ready',
    )
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_unexpected_state',
    )
  })

  test('aggregates blocking reasons when both gates fail', () => {
    const report = assertXClaimRewardSmokeCompletion({
      reward: settledReward({ state: 'failed' }),
      summary: settledSummary({ failed: 1, settled: 0 }),
    })

    expect(report.ready).toBe(false)
    expect(report.transitionRequest).toBeNull()
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_completion_dispatch_run_not_clean',
    )
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_completion_settled_row_not_ready',
    )
  })

  test('leaks no payment material in the serialized report', () => {
    const report = assertXClaimRewardSmokeCompletion({
      reward: settledReward(),
      summary: settledSummary(),
    })

    const serialized = JSON.stringify(report)
    expect(serialized).not.toContain('payment_secret_1')
    expect(serialized).not.toMatch(/ln(bc|tb|bcrt|sb)[0-9]/i)
    expect(serialized).not.toMatch(/lno1[a-z0-9]{20,}/i)
    expect(serialized).not.toMatch(/\b[0-9a-f]{64}\b/i)
  })
})
