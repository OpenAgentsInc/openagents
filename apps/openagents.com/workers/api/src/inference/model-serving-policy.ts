// Provider serving policy for the OpenAgents inference gateway
// (blocker.product_promises.public_paid_model_gateway_missing on
// api.hosted_gemini.v1).
//
// THE GAP this closes: the public catalog (`model-catalog.ts`, served at
// `/v1/models`) publishes EVERY model in the pricing table, regardless of
// whether the gateway can actually serve that model's supply lane right now. A
// supply lane is only servable when its upstream credential/binding is
// provisioned — the Vertex lanes need `VERTEX_SA_KEY`, the Fireworks lane needs
// `FIREWORKS_API_KEY`, and the OpenAgents serving fabric is not yet live. A
// PAID gateway must not advertise (and let a credits customer fund a balance
// toward) a model it cannot serve: a request for an unarmed lane can only fail
// `model_unavailable` at dispatch time. This module is the SINGLE provider
// policy that maps which lanes are armed to which catalog models the gateway may
// publish.
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
}>

// Is an env credential present (a non-blank string)? Presence-only; the value is
// never returned or logged.
const isPresent = (value: string | undefined): boolean =>
  typeof value === 'string' && value.trim() !== ''

// Derive which supply lanes are armed from credential PRESENCE. The OpenAgents
// serving-fabric lane has no live credential surface (Pylons do not yet serve
// inference — roadmap), so it is conservatively never armed here.
export const resolveSupplyLaneArming = (
  env: SupplyLaneCredentialEnv,
): SupplyLaneArming => {
  const vertex = isPresent(env.VERTEX_SA_KEY)
  return {
    fireworks: isPresent(env.FIREWORKS_API_KEY),
    'openagents-network': false,
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
