// Pre-purchase cost estimate for the OpenAgents inference gateway
// (blocker.product_promises.public_paid_model_gateway_missing).
//
// THE GAP this closes: the gateway now PUBLISHES per-1M-token sell prices
// (model-catalog.ts, `/v1/models`), but a credits customer who wants to "spend
// deliberately" still has to do the arithmetic by hand to answer the only
// question that matters before funding a balance: "for THIS model and THIS many
// tokens, on THIS funding rail, how many credits will it cost?" This module is
// the SINGLE pure answer to that question. It reuses `priceRequest` — the EXACT
// pricing engine the metering hook charges against — fed with the customer's
// ESTIMATED token counts, and projects the customer-facing result.
//
// RECEIPT-FIRST DISCIPLINE PRESERVED: this is explicitly an ESTIMATE, never a
// charge. It moves no money, writes no ledger row, and the real charge is still
// metered from the provider's actual `usage` object (INVARIANTS.md "Canonical
// Token Usage Ledger"). The output carries `isEstimate: true` so no caller can
// mistake a quote for a receipt.
//
// PUBLIC-SAFE: the projection deliberately OMITS our marginal cost basis and
// margin (`costUsd`) so the estimate never leaks unit economics — it exposes
// only the customer-facing charge, already implied by the published catalog
// price. PURE: no D1, no clock, no network, no secrets.
import { isFreeEligibleModel } from './inference-free-allowance'
import { type FundingKind, priceRequest } from './pricing'
import { usdToMsatCeil } from './usd-msat-conversion'

// Round a money/credit figure to a stable number of decimals so the estimate
// carries no floating-point noise (mirrors the catalog's `round`). Pure.
const round = (value: number, decimals: number): number => {
  const factor = 10 ** decimals
  return Math.round(value * factor) / factor
}

export type CostEstimateInput = Readonly<{
  // Model alias the customer intends to call (case-insensitive; matched against
  // the same pricing table the gateway bills from). Unknown models estimate at
  // the conservative unknown-model fallback rate, flagged via `isUnknownModel`.
  model: string
  // Optional internal pricing alias for virtual public models. Used when the
  // customer-facing model stays `openagents/khala` but the operator-selected
  // backing lane has a different cost row (for example Fireworks DeepSeek V4
  // Flash). The response still reports `model`, never this hidden alias.
  priceModel?: string
  // Estimated prompt (input) tokens the customer expects to send.
  promptTokens: number
  // Estimated completion (output) tokens the customer expects back.
  completionTokens: number
  // Estimated cached-input tokens (a subset of promptTokens billed at the lower
  // cached rate). Optional; defaults to 0 (no cache hit assumed).
  cachedPromptTokens?: number
  // Funding rail the balance will be drawn from. Bitcoin applies the published
  // funding discount, so the same request quotes cheaper on the BTC rail.
  fundingKind: FundingKind
  // Whether the request will be submitted as a batch (−50% both directions).
  batch?: boolean
  // Margin override (defaults to the launch margin inside the pricing engine).
  margin?: number
}>

// The customer-facing pre-purchase estimate. All amounts are what the customer
// would be charged; NONE reveal our cost basis or margin.
export type CostEstimate = Readonly<{
  // Canonical model the estimate priced against (or the lowercased input when it
  // fell back to the unknown-model rate).
  model: string
  // True when the model is not in the published catalog (conservative fallback).
  isUnknownModel: boolean
  // True when this model class is free-tier eligible (Gemini Flash today). When
  // true, a request that clears under the owner's Sybil-resistant free pool will
  // cost 0; this estimate is nonetheless the PAID price for when the pool is
  // exhausted, so a customer can plan deliberate spend conservatively.
  freeTierEligible: boolean
  // Funding rail the estimate reflects.
  fundingKind: FundingKind
  // The token counts the estimate priced (echoed back, clamped to non-negative).
  promptTokens: number
  cachedPromptTokens: number
  completionTokens: number
  // Estimated USD that would decrement the balance (AFTER any funding discount).
  estimatedChargeUsd: number
  // Same charge in the legible credit unit (1 credit = $0.01).
  estimatedCredits: number
  // Same charge in integer msat, using the EXACT ceiling conversion the metering
  // hook decrements with (`usdToMsatCeil`), so the estimate matches how the
  // msat-denominated balance is actually drawn.
  estimatedChargeMsat: number
  // USD the Bitcoin funding discount would save versus the card rail on this
  // same request (0 for card funding). Surfaces the on-brand pull onto Bitcoin.
  fundingDiscountUsd: number
  // Marker: this is an estimate, never a receipt. The real charge is metered
  // receipt-first from the provider's actual usage object.
  isEstimate: true
}>

// Clamp a token count to a non-negative integer (defensive: a customer-supplied
// estimate must never produce a negative or fractional charge). Pure.
const tokenCount = (value: number): number =>
  Number.isFinite(value) && value > 0 ? Math.floor(value) : 0

// Estimate the credit cost of a request BEFORE it is sent, from the same pricing
// engine the gateway bills with. PURE — moves no money, writes no ledger.
export const estimateRequestCost = (input: CostEstimateInput): CostEstimate => {
  const promptTokens = tokenCount(input.promptTokens)
  const completionTokens = tokenCount(input.completionTokens)
  // Cached tokens are a subset of prompt tokens; never count more than the
  // prompt, matching the metering hook's `min(cached, prompt)` split.
  const cachedPromptTokens = Math.min(
    tokenCount(input.cachedPromptTokens ?? 0),
    promptTokens,
  )

  const usage = {
    cachedPromptTokens,
    completionTokens,
    promptTokens,
    totalTokens: promptTokens + completionTokens,
  }

  // Price on the requested funding rail (this is the charge that would settle).
  // Spread the optional knobs only when set so `exactOptionalPropertyTypes`
  // never sees an explicit `undefined`.
  const priced = priceRequest({
    fundingKind: input.fundingKind,
    model: input.priceModel ?? input.model,
    usage,
    ...(input.batch !== undefined ? { batch: input.batch } : {}),
    ...(input.margin !== undefined ? { margin: input.margin } : {}),
  })

  // Always compute the card-rail price too, so we can surface the Bitcoin
  // saving regardless of which rail the customer asked about. The card price is
  // the gross charge before any funding discount.
  const cardCharge = priced.grossChargeUsd
  const fundingDiscountUsd = cardCharge - priced.chargeUsd
  const outputModel =
    input.priceModel === undefined
      ? priced.model
      : input.model.trim().toLowerCase()

  return {
    cachedPromptTokens,
    completionTokens,
    estimatedChargeMsat: usdToMsatCeil(priced.chargeUsd),
    estimatedChargeUsd: round(priced.chargeUsd, 6),
    estimatedCredits: round(priced.credits, 4),
    freeTierEligible: isFreeEligibleModel(outputModel),
    fundingDiscountUsd: round(fundingDiscountUsd, 6),
    fundingKind: input.fundingKind,
    isEstimate: true,
    isUnknownModel: priced.isUnknownModel,
    model: outputModel,
    promptTokens,
  }
}
