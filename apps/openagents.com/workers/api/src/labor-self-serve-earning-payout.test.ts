import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'
import {
  buildSelfServeLaborPayoutPlan,
  dispatchSelfServeLaborPayout,
  LABOR_SELF_SERVE_PAYOUT_PROMISE,
} from './labor-self-serve-earning-payout'

describe('LaborSelfServePayout', () => {
  test('builds a valid payout plan when balance is sufficient', () => {
    const result = buildSelfServeLaborPayoutPlan(
      { providerRef: 'agent:123', destination: 'lightning@example.com' },
      { bitcoinWithdrawableMsat: 100_000 },
      '2026-06-20T12:00:00.000Z'
    )

    expect(result.ok).toBe(true)
    if (!result.ok) return

    expect(result.plan.gate.state).toBe('ready')
    expect(result.plan.readyForMarket).toBe(true)
    expect(result.plan.payoutIntent).not.toBeNull()
    expect(result.plan.payoutIntent?.amount.amountMinorUnits).toBe(100_000)
    expect(result.plan.promiseIds[0]).toBe(LABOR_SELF_SERVE_PAYOUT_PROMISE)
    expect(result.plan.inert).toBe(true)
  })

  test('blocks payout plan when balance is insufficient', () => {
    const result = buildSelfServeLaborPayoutPlan(
      { providerRef: 'agent:123', destination: 'lightning@example.com' },
      { bitcoinWithdrawableMsat: 50_000 },
      '2026-06-20T12:00:00.000Z'
    )

    expect(result.ok).toBe(true)
    if (!result.ok) return

    expect(result.plan.gate.state).toBe('blocked')
    expect(result.plan.gate.reasonRefs).toContain('reason.labor_payout.insufficient_withdrawable_balance')
    expect(result.plan.readyForMarket).toBe(false)
    expect(result.plan.payoutIntent).toBeNull()
  })

  test('fails validation on empty inputs', () => {
    const result1 = buildSelfServeLaborPayoutPlan(
      { providerRef: '', destination: 'lightning@example.com' },
      { bitcoinWithdrawableMsat: 100_000 }
    )
    expect(result1.ok).toBe(false)

    const result2 = buildSelfServeLaborPayoutPlan(
      { providerRef: 'agent:123', destination: '' },
      { bitcoinWithdrawableMsat: 100_000 }
    )
    expect(result2.ok).toBe(false)
  })

  test('dispatch is disabled by default', async () => {
    const result = buildSelfServeLaborPayoutPlan(
      { providerRef: 'agent:123', destination: 'lightning@example.com' },
      { bitcoinWithdrawableMsat: 100_000 },
      '2026-06-20T12:00:00.000Z'
    )
    if (!result.ok) throw new Error('plan failed')

    const dispatch = await Effect.runPromise(
      dispatchSelfServeLaborPayout({ enabled: false }, { plan: result.plan })
    )

    expect(dispatch._tag).toBe('disabled')
  })

  test('dispatch authorizes payout intent when enabled and ready', async () => {
    const result = buildSelfServeLaborPayoutPlan(
      { providerRef: 'agent:123', destination: 'lightning@example.com' },
      { bitcoinWithdrawableMsat: 100_000 },
      '2026-06-20T12:00:00.000Z'
    )
    if (!result.ok) throw new Error('plan failed')

    const dispatch = await Effect.runPromise(
      dispatchSelfServeLaborPayout({ enabled: true }, { plan: result.plan })
    )

    expect(dispatch._tag).toBe('authorized')
    if (dispatch._tag === 'authorized') {
      expect(dispatch.payoutIntent.amount.amountMinorUnits).toBe(100_000)
    }
  })
})
