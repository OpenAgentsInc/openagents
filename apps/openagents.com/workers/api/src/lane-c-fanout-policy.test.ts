import { describe, expect, test } from 'vitest'

import {
  type LaneCFanoutRequest,
  evaluateLaneCFanout,
} from './lane-c-fanout-policy'

const readyRequest = {
  artifactAuthorityReady: true,
  budgetCapSats: 5_000,
  customerOptIn: true,
  marketInventoryReady: true,
  missionWorkOrderUnified: true,
  ownedCapacityState: 'limited',
  privacyTier: 'public_beta',
  providerTrustTier: 'public_rung1',
  quotedSats: 2_000,
  settlementBridgeReady: true,
  validatorPolicyReady: true,
} satisfies LaneCFanoutRequest

describe('Lane C fanout policy', () => {
  test('keeps work on owned capacity when an owned runner is available', () => {
    expect(
      evaluateLaneCFanout({
        ...readyRequest,
        ownedCapacityState: 'available',
      }),
    ).toEqual({
      lane: 'owned_capacity',
      reasonRefs: ['lane_c.owned_capacity_available'],
      state: 'ready',
    })
  })

  test('blocks public market fanout without explicit opt-in and prerequisite proofs', () => {
    const decision = evaluateLaneCFanout({
      ...readyRequest,
      artifactAuthorityReady: false,
      budgetCapSats: 0,
      customerOptIn: false,
      marketInventoryReady: false,
      missionWorkOrderUnified: false,
      privacyTier: 'team',
      providerTrustTier: 'unknown',
      quotedSats: 0,
      settlementBridgeReady: false,
      validatorPolicyReady: false,
    })

    expect(decision).toEqual({
      lane: 'public_market',
      reasonRefs: [
        'lane_c.artifact_authority_missing',
        'lane_c.budget_cap_missing',
        'lane_c.customer_opt_in_missing',
        'lane_c.market_inventory_missing',
        'lane_c.mission_work_order_unification_missing',
        'lane_c.privacy_tier_not_public.team',
        'lane_c.public_trust_floor_not_met.unknown',
        'lane_c.quote_required_before_market_assignment',
        'lane_c.usd_to_sats_settlement_bridge_missing',
        'lane_c.validator_policy_missing',
      ],
      state: 'blocked',
    })
  })

  test('enforces the customer budget cap before market assignment', () => {
    expect(
      evaluateLaneCFanout({
        ...readyRequest,
        quotedSats: 6_000,
      }),
    ).toMatchObject({
      reasonRefs: ['lane_c.quote_exceeds_budget_cap'],
      state: 'blocked',
    })
  })

  test('admits public-tier fanout only when all gates are present', () => {
    expect(evaluateLaneCFanout(readyRequest)).toEqual({
      lane: 'public_market',
      reasonRefs: ['lane_c.ready_for_public_market_fanout'],
      state: 'ready',
    })
  })
})
