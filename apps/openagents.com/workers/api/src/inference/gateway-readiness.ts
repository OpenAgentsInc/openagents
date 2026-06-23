// Gateway readiness projection for the OpenAgents inference gateway
// (blocker.product_promises.public_paid_model_gateway_missing on
// api.hosted_gemini.v1).
//
// THE GAP this closes: the provider serving policy (model-serving-policy.ts)
// already gates all three public gateway surfaces (`/v1/models`, `/v1/quote`,
// and the `/v1/chat/completions` dispatch path) to lanes whose upstream
// credential is provisioned. But that gating is applied surface-by-surface;
// there is NO single, dereferenceable FACT that answers the launch question
// "can the paid gateway actually serve anything right now, and how degraded is
// its catalog?" Without it an operator (or the launch dashboard) cannot verify
// gateway readiness without replaying each surface and counting by hand. This
// module is the SINGLE readiness summary, derived from the SAME catalog +
// arming the surfaces use, so it can never disagree with what the gateway will
// actually serve.
//
// PUBLIC-SAFE + NO SECRETS: arming arrives as presence-only booleans (the
// serving policy already reduced credentials to "is this lane provisioned");
// this module never sees a credential value. PURE: no D1, no clock, no network.
// It moves no money, changes no promise state, and reveals no prompts,
// completions, or credentials -- it only summarizes which PUBLIC catalog models
// are servable under the current arming.

import { buildModelCatalog, type ModelCatalogEntry } from './model-catalog'
import {
  isLaneArmed,
  isModelServable,
  type SupplyLaneArming,
} from './model-serving-policy'
import type { SupplyLane } from './pricing'

// The order lanes are reported in (stable, deterministic output).
const LANE_ORDER: ReadonlyArray<SupplyLane> = [
  'vertex-gemini',
  'vertex-anthropic',
  'fireworks',
  'hydralisk',
  'openagents-network',
]

// Overall gateway readiness, derived from how many published catalog models are
// servable right now:
//   - 'unavailable' : ZERO models are servable (no lane armed, or no armed lane
//                     carries a published model) -- the paid gateway cannot
//                     serve any request.
//   - 'degraded'    : SOME models are servable but at least one published model
//                     is hidden because its lane is unarmed.
//   - 'ready'       : EVERY published model is servable (no model is hidden).
// An empty catalog reports 'unavailable' (there is nothing to serve).
export type GatewayReadinessStatus = 'unavailable' | 'degraded' | 'ready'

// Per-lane readiness counts.
export type GatewayLaneReadiness = Readonly<{
  lane: SupplyLane
  armed: boolean
  // Published catalog models on this lane that are servable right now.
  servableModelCount: number
  // Published catalog models on this lane hidden because the lane is unarmed.
  hiddenModelCount: number
}>

// The single readiness summary for the paid gateway.
export type GatewayReadiness = Readonly<{
  status: GatewayReadinessStatus
  // Total published catalog models considered.
  totalModelCount: number
  // Models servable right now across all armed lanes.
  servableModelCount: number
  // Models hidden because their lane is unarmed.
  hiddenModelCount: number
  // Per-lane breakdown, in stable LANE_ORDER.
  lanes: ReadonlyArray<GatewayLaneReadiness>
  // Public-safe reason refs explaining the status (dereferenceable, no secrets).
  reasonRefs: ReadonlyArray<string>
}>

const STATUS_REASON_REF: Readonly<Record<GatewayReadinessStatus, string>> = {
  degraded: 'gateway.readiness.degraded.some_lanes_unarmed',
  ready: 'gateway.readiness.ready.all_models_servable',
  unavailable: 'gateway.readiness.unavailable.no_servable_models',
}

const resolveStatus = (
  totalModelCount: number,
  servableModelCount: number,
  hiddenModelCount: number,
): GatewayReadinessStatus => {
  if (servableModelCount === 0) {
    return 'unavailable'
  }
  if (hiddenModelCount > 0 || totalModelCount === 0) {
    return 'degraded'
  }
  return 'ready'
}

// Project the published catalog + current lane arming into a single readiness
// summary. The catalog defaults to the live published catalog (`buildModelCatalog`)
// but is injectable for tests. Output is deterministic (lanes in LANE_ORDER).
export const projectGatewayReadiness = (
  arming: SupplyLaneArming,
  catalog: ReadonlyArray<ModelCatalogEntry> = buildModelCatalog(),
): GatewayReadiness => {
  const lanes = LANE_ORDER.map((lane): GatewayLaneReadiness => {
    const armed = isLaneArmed(arming, lane)
    const onLane = catalog.filter(entry => entry.lane === lane)
    const servable = onLane.filter(entry => isModelServable(entry, arming))
    return {
      armed,
      hiddenModelCount: onLane.length - servable.length,
      lane,
      servableModelCount: servable.length,
    }
  })

  const totalModelCount = catalog.length
  const servableModelCount = catalog.filter(entry =>
    isModelServable(entry, arming),
  ).length
  const hiddenModelCount = totalModelCount - servableModelCount
  const status = resolveStatus(
    totalModelCount,
    servableModelCount,
    hiddenModelCount,
  )

  const reasonRefs = [
    STATUS_REASON_REF[status],
    ...lanes
      .filter(lane => !lane.armed && lane.hiddenModelCount > 0)
      .map(lane => `gateway.readiness.lane_unarmed.${lane.lane}`),
  ]

  return {
    hiddenModelCount,
    lanes,
    reasonRefs,
    servableModelCount,
    status,
    totalModelCount,
  }
}
