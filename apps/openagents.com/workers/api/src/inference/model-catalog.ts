// Public model catalog for the OpenAgents inference gateway
// (blocker.product_promises.public_paid_model_gateway_missing).
//
// THE GAP this closes: the OpenAI-compatible gateway request surface is live
// (POST /v1/chat/completions), but there is NO public, dereferenceable surface
// that DISCLOSES which models the gateway serves, what each costs, and which are
// free-tier eligible. An OpenAI-compatible gateway is expected to answer
// `GET /v1/models`; off-the-shelf clients list models from it, and a credits
// business needs a published price for every paid model before a customer can
// fund a balance and spend it deliberately. This module is the SINGLE source of
// that public catalog, derived from the same pricing table the metering hook
// charges against — so the published price can never drift from the billed price.
//
// PURE: no D1, no clock, no network, no secrets. It exposes ONLY public-safe
// facts already implied by the pricing table (model id, supply lane, published
// sell price per 1M tokens, current free-key catalog policy, cost-basis
// provenance). It moves no money and reveals no prompts, completions, or
// credentials. The route handler (`handleModelsList`, models-routes.ts) injects
// the `created` timestamp and serves this as the OpenAI `/v1/models` payload.

import {
  BASE_CREDIT_USD,
  CACHED_INPUT_FRACTION,
  DEFAULT_MARGIN,
  MODEL_PRICING_TABLE,
  type SupplyLane,
} from './pricing'
import {
  DEFAULT_FREE_TIER_QUOTA,
  decideFreeTierLane,
} from './inference-free-tier-key'

// Human-legible provider label for each supply lane (the OpenAI `owned_by`
// field). All lanes are served THROUGH OpenAgents, so the label names the
// upstream supply lane behind the OpenAgents gateway, not a third party the
// customer contracts with directly.
const LANE_OWNED_BY: Readonly<Record<SupplyLane, string>> = {
  fireworks: 'openagents/fireworks',
  hydralisk: 'openagents/hydralisk',
  'openagents-network': 'openagents/serving-fabric',
  'vertex-anthropic': 'openagents/vertex-anthropic',
  'vertex-gemini': 'openagents/vertex-gemini',
}

// Cost-basis provenance for a published price. `verified` => the cost row is a
// real measured upstream rate (Fireworks open models, verified 2026-06-19);
// `list_placeholder` => the cost row is the published LIST rate, not our
// committed-use/quota rate (the Vertex Claude / Gemini billing TODO). Surfaced
// so the catalog never implies a price is final when its cost basis is a
// placeholder. Mirrors `ModelPricingEntry.costIsListPlaceholder`.
export type ModelCostBasis = 'verified' | 'list_placeholder'

// Published sell price for one model, per 1M tokens, in both USD and the legible
// credit unit (1 credit = $0.01). All three dimensions are derived from the
// SAME cost table the metering hook charges against, at the catalog margin.
export type PublishedModelPrice = Readonly<{
  inputUsdPerMtok: number
  cachedInputUsdPerMtok: number
  outputUsdPerMtok: number
  inputCreditsPerMtok: number
  cachedInputCreditsPerMtok: number
  outputCreditsPerMtok: number
}>

// Public free-key policy for one model. This is the self-serve per-key free
// tier from `inference-free-tier-key.ts`, not the older owner-keyed Gemini
// allowance. The `eligible` boolean is false unless the Worker has explicitly
// armed INFERENCE_FREE_TIER_ENABLED.
export type PublishedModelFreeTier = Readonly<{
  eligible: boolean
  maxRequestsPerDay: number | null
  maxTokensPerDay: number | null
  window: 'utc_day' | null
  reasonRef: string
}>

export type ModelCatalogPolicy = Readonly<{
  freeTierEnabled?: boolean
}>

// One public catalog entry for a model the gateway serves.
export type ModelCatalogEntry = Readonly<{
  // Canonical model id (the `model` a client sends to /v1/chat/completions).
  id: string
  // Supply lane the model is served from (cost provenance).
  lane: SupplyLane
  // Legible provider label (OpenAI `owned_by`).
  ownedBy: string
  // True when the public self-serve free-key tier is live for this model.
  // Backward-compatible shorthand for `freeTier.eligible`.
  freeTierEligible: boolean
  // Public self-serve free-key policy for this model.
  freeTier: PublishedModelFreeTier
  // Published per-model multiplier relative to the Sonnet baseline (1.0x).
  multiplier: number
  // Cost-basis provenance of the published price.
  costBasis: ModelCostBasis
  // Published sell price per 1M tokens.
  price: PublishedModelPrice
}>

// Round a money/credit figure to a stable number of decimal places so the
// published catalog carries no floating-point noise. Pure.
const round = (value: number, decimals: number): number => {
  const factor = 10 ** decimals
  return Math.round(value * factor) / factor
}

const FREE_TIER_DISABLED_REASON =
  'reason.inference_free_tier.disabled' as const

const disabledFreeTier = (): PublishedModelFreeTier => ({
  eligible: false,
  maxRequestsPerDay: null,
  maxTokensPerDay: null,
  reasonRef: FREE_TIER_DISABLED_REASON,
  window: null,
})

const freeTierForModel = (
  model: string,
  policy: ModelCatalogPolicy,
): PublishedModelFreeTier => {
  if (policy.freeTierEnabled !== true) {
    return disabledFreeTier()
  }
  const lane = decideFreeTierLane(model)
  if (!lane.freeLane) {
    return {
      eligible: false,
      maxRequestsPerDay: null,
      maxTokensPerDay: null,
      reasonRef: lane.reasonRef,
      window: null,
    }
  }
  return {
    eligible: true,
    maxRequestsPerDay: DEFAULT_FREE_TIER_QUOTA.maxRequestsPerDay,
    maxTokensPerDay: DEFAULT_FREE_TIER_QUOTA.maxTokensPerDay,
    reasonRef: lane.reasonRef,
    window: 'utc_day',
  }
}

// Build the public model catalog from the pricing table at `margin`
// (defaults to the launch margin). Deterministic + pure: the same table in
// yields the same catalog out, so the published prices re-solve automatically
// when the cost table is tuned.
export const buildModelCatalog = (
  margin: number = DEFAULT_MARGIN,
  policy: ModelCatalogPolicy = {},
): ReadonlyArray<ModelCatalogEntry> =>
  MODEL_PRICING_TABLE.map(entry => {
    const { cost } = entry
    // Sell rate = cost x (1 + margin), same math as `priceRequest`/`sellPricePerMtok`.
    const sell = (usdPerMtok: number): number => usdPerMtok * (1 + margin)
    const cachedCostUsdPerMtok =
      cost.cachedInputUsdPerMtok ?? cost.inputUsdPerMtok * CACHED_INPUT_FRACTION

    const inputUsd = sell(cost.inputUsdPerMtok)
    const cachedUsd = sell(cachedCostUsdPerMtok)
    const outputUsd = sell(cost.outputUsdPerMtok)
    const freeTier = freeTierForModel(entry.model, policy)

    return {
      costBasis: entry.costIsListPlaceholder
        ? ('list_placeholder' as const)
        : ('verified' as const),
      freeTier,
      freeTierEligible: freeTier.eligible,
      id: entry.model,
      lane: entry.lane,
      multiplier: entry.multiplier,
      ownedBy: LANE_OWNED_BY[entry.lane],
      price: {
        cachedInputCreditsPerMtok: round(cachedUsd / BASE_CREDIT_USD, 4),
        cachedInputUsdPerMtok: round(cachedUsd, 6),
        inputCreditsPerMtok: round(inputUsd / BASE_CREDIT_USD, 4),
        inputUsdPerMtok: round(inputUsd, 6),
        outputCreditsPerMtok: round(outputUsd / BASE_CREDIT_USD, 4),
        outputUsdPerMtok: round(outputUsd, 6),
      },
    }
  })

// ----------------------------------------------------------------------------
// OpenAI-compatible `/v1/models` projection
// ----------------------------------------------------------------------------

// One model object in the OpenAI `/v1/models` list. The standard fields
// (`id`, `object`, `created`, `owned_by`) make off-the-shelf clients work; the
// `oa_*` extension fields publish the gateway's price + policy. Clients ignore
// unknown fields, so this stays a drop-in OpenAI surface.
export type OpenAiModelObject = Readonly<{
  id: string
  object: 'model'
  created: number
  owned_by: string
  oa_lane: SupplyLane
  oa_free_tier_eligible: boolean
  oa_free_tier: PublishedModelFreeTier
  oa_multiplier: number
  oa_cost_basis: ModelCostBasis
  oa_price: PublishedModelPrice
}>

export type OpenAiModelsResponse = Readonly<{
  object: 'list'
  data: ReadonlyArray<OpenAiModelObject>
}>

// Project ONE catalog entry into the OpenAI `/v1/models` model object. `created`
// is the injected epoch-seconds timestamp (the route supplies the clock; this
// stays pure). The single source of the model-object shape, reused by both the
// list (`/v1/models`) and the retrieve (`/v1/models/{model}`) surfaces so they
// can never disagree on a model's published price or policy.
export const toOpenAiModelObject = (
  model: ModelCatalogEntry,
  createdEpochSeconds: number,
): OpenAiModelObject => ({
  created: createdEpochSeconds,
  id: model.id,
  oa_cost_basis: model.costBasis,
  oa_free_tier: model.freeTier,
  oa_free_tier_eligible: model.freeTierEligible,
  oa_lane: model.lane,
  oa_multiplier: model.multiplier,
  oa_price: model.price,
  object: 'model' as const,
  owned_by: model.ownedBy,
})

// Project the catalog into the OpenAI `/v1/models` response shape. `created` is
// the injected epoch-seconds timestamp (the route supplies the clock; this stays
// pure). One model object per catalog entry, in table order.
export const toOpenAiModelsResponse = (
  catalog: ReadonlyArray<ModelCatalogEntry>,
  createdEpochSeconds: number,
): OpenAiModelsResponse => ({
  data: catalog.map(model => toOpenAiModelObject(model, createdEpochSeconds)),
  object: 'list' as const,
})

// Look up a single served model by its canonical id, returning its public
// catalog entry or `undefined` when the gateway does not serve it. Backs the
// OpenAI-compatible `GET /v1/models/{model}` retrieve surface: a credits
// customer (or off-the-shelf client) verifies a model exists and reads its
// published price before funding a balance. Derived from the SAME pricing table
// the metering hook charges against, so a resolved price cannot drift from the
// billed price. Pure: an empty/blank id never matches.
export const findModelCatalogEntry = (
  modelId: string,
  margin: number = DEFAULT_MARGIN,
  policy: ModelCatalogPolicy = {},
): ModelCatalogEntry | undefined => {
  if (modelId.trim() === '') {
    return undefined
  }
  return buildModelCatalog(margin, policy).find(entry => entry.id === modelId)
}
