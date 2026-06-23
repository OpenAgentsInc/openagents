// Provider serving policy for the OpenAgents inference gateway
// (blocker.product_promises.public_paid_model_gateway_missing on
// api.hosted_gemini.v1).
//
// THE GAP this closes: the public catalog (`model-catalog.ts`, served at
// `/v1/models`) publishes EVERY model in the pricing table, regardless of
// whether the gateway can actually serve that model's supply lane right now. A
// supply lane is only servable when its upstream credential/binding is
// provisioned — the Vertex lanes need `VERTEX_SA_KEY`, the Fireworks lane needs
// `FIREWORKS_API_KEY`, and the OpenAgents serving fabric needs an explicit
// route-ready flag plus public-safe Pylon evidence refs. A PAID gateway must
// not advertise (and let a credits customer fund a balance toward) a model it
// cannot serve: a request for an unarmed lane can only fail `model_unavailable`
// at dispatch time. This module is the SINGLE provider policy that maps which
// lanes are armed to which catalog models the gateway may publish.
//
// PUBLIC-SAFE + NO SECRETS: the policy reads credential PRESENCE only (a boolean
// "is this env var a non-empty string"), never the secret value, so it neither
// handles nor can leak a credential. PURE: no D1, no clock, no network. It moves
// no money and changes no promise state — it only narrows what the public
// catalog advertises to what is genuinely servable.

import type { ModelCatalogEntry } from './model-catalog'
import { lookupModel } from './pricing'
import type { SupplyLane } from './pricing'

// Which supply lanes the gateway can ACTUALLY serve right now. A lane is "armed"
// only when its upstream credential/binding is provisioned.
export type SupplyLaneArming = Readonly<Record<SupplyLane, boolean>>

// Safe default: nothing servable. A gateway with no provisioned lane advertises
// no paid models rather than advertising models it cannot serve.
export const ALL_LANES_UNARMED: SupplyLaneArming = {
  fireworks: false,
  'openagents-network': false,
  'vertex-anthropic': false,
  'vertex-gemini': false,
}

// The presence-only env shape the arming is derived from. Every field is the
// SAME worker secret/flag name the corresponding adapter already reads; we only
// ever test for a non-empty value, never read the secret itself.
export type SupplyLaneCredentialEnv = Readonly<{
  // Mints the GCP token for both Vertex lanes (Claude + Gemini). See config.ts.
  VERTEX_SA_KEY?: string | undefined
  // Fireworks open-model lane key. See config.ts.
  FIREWORKS_API_KEY?: string | undefined
  // Explicit public-gateway route arming for the OpenAgents/Pylon serving
  // fabric. `ready` is the only accepted on-token; public-safe refs below carry
  // the evidence. No endpoint URL, API key, raw prompt, or private host appears
  // in this policy.
  OPENAGENTS_NETWORK_GATEWAY_ROUTE_READY?: string | undefined
  OPENAGENTS_NETWORK_GATEWAY_APPROVAL_REF?: string | undefined
  OPENAGENTS_NETWORK_SERVING_PREFLIGHT_REF?: string | undefined
  OPENAGENTS_NETWORK_SERVING_RECEIPT_REF?: string | undefined
  OPENAGENTS_NETWORK_REPLAY_CHALLENGE_REF?: string | undefined
  OPENAGENTS_NETWORK_ADMITTED_PYLON_REF?: string | undefined
  // Secret-backed transport presence for the real serving route. Presence-only:
  // the URL/token values are never returned from this policy and must not appear
  // in public readiness/catalog payloads.
  OPENAGENTS_NETWORK_FABRIC_SERVE_URL?: string | undefined
  OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN?: string | undefined
}>

export type OpenAgentsNetworkGatewayArming = Readonly<{
  armed: boolean
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

// Is an env credential present (a non-blank string)? Presence-only; the value is
// never returned or logged.
const isPresent = (value: string | undefined): boolean =>
  typeof value === 'string' && value.trim() !== ''

const GATEWAY_ROUTE_READY_ON_TOKEN = 'ready'

const PUBLIC_SAFE_REF = /^[a-z0-9][a-z0-9._:-]{1,199}$/iu

const isPublicSafeRef = (value: string | undefined): value is string => {
  if (typeof value !== 'string' || value.trim() === '') {
    return false
  }
  const trimmed = value.trim()
  return (
    trimmed === value &&
    PUBLIC_SAFE_REF.test(trimmed) &&
    !trimmed.includes('://') &&
    !trimmed.toLowerCase().startsWith('sk-')
  )
}

export const resolveOpenAgentsNetworkGatewayArming = (
  env: SupplyLaneCredentialEnv,
): OpenAgentsNetworkGatewayArming => {
  const evidence: Array<[string, string | undefined]> = [
    [
      'blocker.openagents_network_gateway.approval_ref_missing',
      env.OPENAGENTS_NETWORK_GATEWAY_APPROVAL_REF,
    ],
    [
      'blocker.openagents_network_gateway.serving_preflight_ref_missing',
      env.OPENAGENTS_NETWORK_SERVING_PREFLIGHT_REF,
    ],
    [
      'blocker.openagents_network_gateway.serving_receipt_ref_missing',
      env.OPENAGENTS_NETWORK_SERVING_RECEIPT_REF,
    ],
    [
      'blocker.openagents_network_gateway.replay_challenge_ref_missing',
      env.OPENAGENTS_NETWORK_REPLAY_CHALLENGE_REF,
    ],
    [
      'blocker.openagents_network_gateway.admitted_pylon_ref_missing',
      env.OPENAGENTS_NETWORK_ADMITTED_PYLON_REF,
    ],
  ]

  const blockerRefs: Array<string> = []
  if (
    env.OPENAGENTS_NETWORK_GATEWAY_ROUTE_READY?.trim() !==
    GATEWAY_ROUTE_READY_ON_TOKEN
  ) {
    blockerRefs.push('blocker.openagents_network_gateway.route_not_ready')
  }
  if (!isPresent(env.OPENAGENTS_NETWORK_FABRIC_SERVE_URL)) {
    blockerRefs.push('blocker.openagents_network_gateway.transport_url_missing')
  }
  if (!isPresent(env.OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN)) {
    blockerRefs.push('blocker.openagents_network_gateway.transport_bearer_missing')
  }

  const evidenceRefs: Array<string> = []
  for (const [blockerRef, value] of evidence) {
    if (isPublicSafeRef(value)) {
      evidenceRefs.push(value)
    } else {
      blockerRefs.push(blockerRef)
    }
  }

  return {
    armed: blockerRefs.length === 0,
    blockerRefs,
    evidenceRefs,
  }
}

// Derive which supply lanes are armed from credential PRESENCE. The OpenAgents
// serving-fabric lane is stricter than a credential presence check: it only arms
// when a deploy supplies route-ready plus public-safe evidence refs for an
// admitted Pylon, serving preflight, serving receipt, replay challenge, and
// owner approval.
export const resolveSupplyLaneArming = (
  env: SupplyLaneCredentialEnv,
): SupplyLaneArming => {
  const vertex = isPresent(env.VERTEX_SA_KEY)
  const openAgentsNetwork = resolveOpenAgentsNetworkGatewayArming(env)
  return {
    fireworks: isPresent(env.FIREWORKS_API_KEY),
    'openagents-network': openAgentsNetwork.armed,
    'vertex-anthropic': vertex,
    'vertex-gemini': vertex,
  }
}

// Is a single lane armed?
export const isLaneArmed = (
  arming: SupplyLaneArming,
  lane: SupplyLane,
): boolean => arming[lane]

// Is a single catalog model servable under the given arming (its lane is armed)?
export const isModelServable = (
  entry: ModelCatalogEntry,
  arming: SupplyLaneArming,
): boolean => isLaneArmed(arming, entry.lane)

// Servability for a model the customer NAMES by id (vs a catalog entry already
// in hand). Resolves the id against the SAME pricing table the gateway bills
// from (`lookupModel`, case-insensitive), so a named quote cannot disagree with
// the catalog on which lane a model belongs to. Returns:
//   - true       : the model is in the pricing table AND its lane is armed
//                  (servable right now)
//   - false      : the model is in the pricing table but its lane is NOT armed
//                  (a quote would fund a balance toward a model that can only
//                  fail `model_unavailable` at dispatch — the gap to gate)
//   - undefined  : the model id is unknown to the pricing table. The gateway
//                  intentionally prices unknown ids at a conservative fallback
//                  rate (cost-estimate.ts), so an unknown id is NOT gated here;
//                  the caller keeps its existing unknown-model behaviour.
export const resolveNamedModelServability = (
  modelId: string,
  arming: SupplyLaneArming,
): boolean | undefined => {
  const entry = lookupModel(modelId)
  if (entry === undefined) {
    return undefined
  }
  return isLaneArmed(arming, entry.lane)
}

// Narrow a published catalog to only the models the gateway can actually serve
// right now. Order is preserved. With every lane armed this is the identity
// filter (no model is dropped); with no lane armed it is empty.
export const filterServableCatalog = (
  catalog: ReadonlyArray<ModelCatalogEntry>,
  arming: SupplyLaneArming,
): ReadonlyArray<ModelCatalogEntry> =>
  catalog.filter(entry => isModelServable(entry, arming))
