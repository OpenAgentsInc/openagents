import { describe, expect, test } from 'vitest'

import { assertXClaimRewardSmokeDispatchOutcome } from './x-claim-reward-smoke-dispatch-outcome'
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

describe('X claim reward smoke dispatch outcome auditor', () => {
  test('accepts a clean single-reward settlement (fresh dispatch path)', () => {
    const report = assertXClaimRewardSmokeDispatchOutcome(settledSummary())

    expect(report.ok).toBe(true)
    expect(report.blockingReasonRefs).toEqual([])
    expect(report.checks.every(check => check.ok)).toBe(true)
    expect(report.outcomeSummary).toEqual({
      failed: 0,
      pending: 0,
      polled: 0,
      requested: 1,
      settled: 1,
      skippedReasonRefs: [],
    })
  })

  test('accepts a settlement reached via pending-payment polling', () => {
    const report = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({ polled: 1, requested: 0, settled: 1 }),
    )

    expect(report.ok).toBe(true)
    expect(report.blockingReasonRefs).toEqual([])
  })

  test('blocks a run where dispatch was not enabled', () => {
    const report = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({ stats: cleanStats({ enabled: false }) }),
    )

    expect(report.ok).toBe(false)
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_dispatch_not_enabled',
    )
  })

  test('blocks a run that settled zero or more than one reward', () => {
    const none = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({ requested: 0, settled: 0 }),
    )
    const many = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({ requested: 2, settled: 2 }),
    )

    for (const report of [none, many]) {
      expect(report.ok).toBe(false)
      expect(report.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_smoke_dispatch_not_exactly_one_settled',
      )
    }
  })

  test('blocks a run with a failed reward', () => {
    const report = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({ failed: 1, requested: 1, settled: 0 }),
    )

    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_dispatch_reward_failed',
    )
  })

  test('blocks a run that left a payment pending', () => {
    const report = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({
        pending: 1,
        settled: 0,
        stats: cleanStats({ pendingPaymentCount: 1 }),
      }),
    )

    expect(report.ok).toBe(false)
    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_dispatch_payment_still_pending',
    )
  })

  test('blocks a run whose queue did not drain', () => {
    const residualRequested = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({ stats: cleanStats({ requestedDispatchCount: 1 }) }),
    )
    const residualPending = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({ stats: cleanStats({ pendingPaymentCount: 1 }) }),
    )

    for (const report of [residualRequested, residualPending]) {
      expect(report.ok).toBe(false)
      expect(report.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_smoke_dispatch_queue_not_drained',
      )
    }
  })

  test('blocks a run that skipped on liquidity or cap stops', () => {
    const report = assertXClaimRewardSmokeDispatchOutcome(
      settledSummary({
        settled: 0,
        skippedReasonRefs: [
          'reason.public.x_claim_reward_treasury_liquidity_insufficient',
        ],
      }),
    )

    expect(report.blockingReasonRefs).toContain(
      'reason.public.x_claim_reward_smoke_dispatch_run_skipped',
    )
  })

  test('outcome summary carries only aggregate counters, no payment material', () => {
    const serialized = JSON.stringify(
      assertXClaimRewardSmokeDispatchOutcome(settledSummary()).outcomeSummary,
    )

    expect(serialized).not.toContain('lnbc')
    expect(serialized).not.toContain('paymentId')
    expect(serialized).not.toContain('destination')
  })
})
