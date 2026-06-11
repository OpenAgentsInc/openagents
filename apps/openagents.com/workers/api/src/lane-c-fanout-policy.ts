export type LaneCPrivacyTier = 'private' | 'public' | 'public_beta' | 'team'
export type LaneCOwnedCapacityState = 'available' | 'dark' | 'limited'
export type LaneCTrustTier =
  | 'maintainer_granted'
  | 'public_rung0'
  | 'public_rung1'
  | 'unknown'

export type LaneCFanoutRequest = Readonly<{
  artifactAuthorityReady: boolean
  budgetCapSats: number
  customerOptIn: boolean
  marketInventoryReady: boolean
  missionWorkOrderUnified: boolean
  ownedCapacityState: LaneCOwnedCapacityState
  privacyTier: LaneCPrivacyTier
  providerTrustTier: LaneCTrustTier
  quotedSats: number
  settlementBridgeReady: boolean
  validatorPolicyReady: boolean
}>

export type LaneCFanoutDecision = Readonly<{
  lane: 'owned_capacity' | 'public_market'
  reasonRefs: ReadonlyArray<string>
  state: 'blocked' | 'ready'
}>

const publicProviderTrustTiers = new Set<LaneCTrustTier>([
  'maintainer_granted',
  'public_rung1',
])

const add = (
  reasons: Set<string>,
  condition: boolean,
  reason: string,
): void => {
  if (condition) {
    reasons.add(reason)
  }
}

export const evaluateLaneCFanout = (
  request: LaneCFanoutRequest,
): LaneCFanoutDecision => {
  if (request.ownedCapacityState === 'available') {
    return {
      lane: 'owned_capacity',
      reasonRefs: ['lane_c.owned_capacity_available'],
      state: 'ready',
    }
  }

  const reasons = new Set<string>()

  add(reasons, !request.customerOptIn, 'lane_c.customer_opt_in_missing')
  add(
    reasons,
    request.privacyTier !== 'public' && request.privacyTier !== 'public_beta',
    `lane_c.privacy_tier_not_public.${request.privacyTier}`,
  )
  add(
    reasons,
    !request.missionWorkOrderUnified,
    'lane_c.mission_work_order_unification_missing',
  )
  add(
    reasons,
    !request.settlementBridgeReady,
    'lane_c.usd_to_sats_settlement_bridge_missing',
  )
  add(reasons, !request.marketInventoryReady, 'lane_c.market_inventory_missing')
  add(
    reasons,
    !request.artifactAuthorityReady,
    'lane_c.artifact_authority_missing',
  )
  add(reasons, !request.validatorPolicyReady, 'lane_c.validator_policy_missing')
  add(reasons, request.budgetCapSats <= 0, 'lane_c.budget_cap_missing')
  add(
    reasons,
    request.quotedSats <= 0,
    'lane_c.quote_required_before_market_assignment',
  )
  add(
    reasons,
    request.budgetCapSats > 0 &&
      request.quotedSats > 0 &&
      request.quotedSats > request.budgetCapSats,
    'lane_c.quote_exceeds_budget_cap',
  )
  add(
    reasons,
    !publicProviderTrustTiers.has(request.providerTrustTier),
    `lane_c.public_trust_floor_not_met.${request.providerTrustTier}`,
  )

  if (reasons.size > 0) {
    return {
      lane: 'public_market',
      reasonRefs: [...reasons].sort(),
      state: 'blocked',
    }
  }

  return {
    lane: 'public_market',
    reasonRefs: ['lane_c.ready_for_public_market_fanout'],
    state: 'ready',
  }
}
