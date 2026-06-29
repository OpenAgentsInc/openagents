import { describe, expect, it } from 'vitest'

import {
  evaluateLaneCFanoutForWorkOrder,
  laneCFanoutObjectiveRef,
  ownedCapacityStateFromPlacement,
  type LaneCFanoutBridgeInput,
} from './lane-c-fanout-bridge'

const readyInput: LaneCFanoutBridgeInput = {
  placementSource: 'none_available',
  placementAvailabilityState: 'none_available',
  privacyTier: 'public_beta',
  customerOptIn: true,
  budgetCapSats: 2000,
  quotedSats: 1,
  settlementBridgeReady: true,
  marketInventoryReady: true,
  artifactAuthorityReady: true,
  validatorPolicyReady: true,
  missionWorkOrderUnified: true,
  providerTrustTier: 'public_rung1',
}

describe('lane-c fanout bridge', () => {
  it('maps placement to owned-capacity state', () => {
    expect(ownedCapacityStateFromPlacement('requester_pylon', 'selected')).toBe(
      'available',
    )
    expect(ownedCapacityStateFromPlacement('none_available', 'none_available')).toBe(
      'dark',
    )
    expect(ownedCapacityStateFromPlacement('fallback', 'fallback')).toBe('limited')
  })

  it('is ready for market fanout when capacity is dark + opt-in + public tier + gates pass', () => {
    const result = evaluateLaneCFanoutForWorkOrder(readyInput)
    expect(result.ownedCapacityState).toBe('dark')
    expect(result.readyForMarket).toBe(true)
    expect(result.decision.lane).toBe('public_market')
    expect(result.decision.state).toBe('ready')
  })

  it('keeps owned capacity when the order placed on the owner Pylon (no fanout)', () => {
    const result = evaluateLaneCFanoutForWorkOrder({
      ...readyInput,
      placementSource: 'requester_pylon',
      placementAvailabilityState: 'selected',
    })
    expect(result.decision.lane).toBe('owned_capacity')
    expect(result.readyForMarket).toBe(false)
  })

  it('enforces the public-tier floor server-side (private never fans out)', () => {
    const result = evaluateLaneCFanoutForWorkOrder({
      ...readyInput,
      privacyTier: 'private',
    })
    expect(result.readyForMarket).toBe(false)
    expect(result.decision.reasonRefs.some(r => r.startsWith('lane_c.privacy_tier_not_public'))).toBe(true)
  })

  it('blocks without customer opt-in', () => {
    const result = evaluateLaneCFanoutForWorkOrder({ ...readyInput, customerOptIn: false })
    expect(result.readyForMarket).toBe(false)
    expect(result.decision.reasonRefs).toContain('lane_c.customer_opt_in_missing')
  })

  it('blocks when the quote exceeds the budget cap', () => {
    const result = evaluateLaneCFanoutForWorkOrder({ ...readyInput, quotedSats: 5000, budgetCapSats: 2000 })
    expect(result.readyForMarket).toBe(false)
    expect(result.decision.reasonRefs).toContain('lane_c.quote_exceeds_budget_cap')
  })

  it('builds a public-safe objective ref', () => {
    expect(laneCFanoutObjectiveRef('autopilot_work_order.abc-123')).toBe(
      'objective.public.lane_c_fanout.autopilot_work_order.abc-123',
    )
  })
})
