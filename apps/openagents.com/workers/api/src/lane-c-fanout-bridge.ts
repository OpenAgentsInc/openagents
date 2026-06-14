// Lane C fanout bridge (#4783): turns a real Autopilot product work order into a
// public-market labor work request when owned capacity is dark, the customer has
// opted in, and the public trust-tier floor is met — enforcing the floor
// server-side. Pure logic over the checked-in `evaluateLaneCFanout` gate; the
// route layer applies the decision and creates the linked market work request.

import {
  type LaneCFanoutDecision,
  type LaneCOwnedCapacityState,
  type LaneCPrivacyTier,
  evaluateLaneCFanout,
} from './lane-c-fanout-policy'

export type LaneCFanoutBridgeInput = Readonly<{
  // From the product work order's placement decision.
  placementSource: string // 'requester_pylon' | 'fallback' | 'none_available' | ...
  placementAvailabilityState: string // 'selected' | 'fallback' | 'none_available' | 'retry_later'
  privacyTier: string // placementPolicy.privacyTier
  // Customer choices for this fanout.
  customerOptIn: boolean
  budgetCapSats: number
  quotedSats: number
  // System readiness (P4 #4780 is built/closed -> settlement bridge ready).
  settlementBridgeReady: boolean
  marketInventoryReady: boolean
  artifactAuthorityReady: boolean
  validatorPolicyReady: boolean
  missionWorkOrderUnified: boolean
  providerTrustTier:
    | 'maintainer_granted'
    | 'public_rung0'
    | 'public_rung1'
    | 'unknown'
}>

// Maps the product order's placement state to the Lane C owned-capacity state.
// Owned capacity is "available" only when the order was actually placed on the
// owner's Pylon (requester_pylon selected). A fallback to SHC or a
// none-available placement means owned capacity is dark/limited and the order is
// a Lane C candidate.
export const ownedCapacityStateFromPlacement = (
  placementSource: string,
  placementAvailabilityState: string,
): LaneCOwnedCapacityState => {
  if (
    placementSource === 'requester_pylon' &&
    placementAvailabilityState === 'selected'
  ) {
    return 'available'
  }
  if (placementAvailabilityState === 'none_available') {
    return 'dark'
  }
  // SHC/fallback placement: owned (first-party) Pylon capacity is dark; SHC is a
  // first-party lane but not the owner's own node, so for Lane C purposes the
  // owner's owned capacity is limited and the order may burst to the market.
  return 'limited'
}

// The public trust-tier floor: only `public`/`public_beta` privacy tiers may
// leave the first-party lanes for the open market. Enforced here (server-side).
const normalizePrivacyTier = (tier: string): LaneCPrivacyTier =>
  tier === 'public' || tier === 'public_beta' || tier === 'team' || tier === 'private'
    ? (tier as LaneCPrivacyTier)
    : 'private'

export type LaneCFanoutBridgeResult = Readonly<{
  decision: LaneCFanoutDecision
  ownedCapacityState: LaneCOwnedCapacityState
  // True only when the gate says ready for public-market fanout.
  readyForMarket: boolean
}>

export const evaluateLaneCFanoutForWorkOrder = (
  input: LaneCFanoutBridgeInput,
): LaneCFanoutBridgeResult => {
  const ownedCapacityState = ownedCapacityStateFromPlacement(
    input.placementSource,
    input.placementAvailabilityState,
  )
  const decision = evaluateLaneCFanout({
    artifactAuthorityReady: input.artifactAuthorityReady,
    budgetCapSats: input.budgetCapSats,
    customerOptIn: input.customerOptIn,
    marketInventoryReady: input.marketInventoryReady,
    missionWorkOrderUnified: input.missionWorkOrderUnified,
    ownedCapacityState,
    privacyTier: normalizePrivacyTier(input.privacyTier),
    providerTrustTier: input.providerTrustTier,
    quotedSats: input.quotedSats,
    settlementBridgeReady: input.settlementBridgeReady,
    validatorPolicyReady: input.validatorPolicyReady,
  })
  return {
    decision,
    ownedCapacityState,
    readyForMarket:
      decision.lane === 'public_market' && decision.state === 'ready',
  }
}

// The public-safe objective ref for a market job fanned out from a product order.
export const laneCFanoutObjectiveRef = (workOrderRef: string): string =>
  `objective.public.lane_c_fanout.${workOrderRef.replace(/[^a-z0-9._-]+/giu, '_')}`
