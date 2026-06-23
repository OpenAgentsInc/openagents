// Pricing & multiplier engine for the OpenAgents inference gateway
// (EPIC #5474, child #5478).
//
// Pure, typed, table-driven pricing. Given (model, token usage, funding kind)
// this module computes the credits/cost to charge for a completed inference
// request. It is the ONE place that turns provider token usage into a price.
//
// Boundaries (kept deliberately narrow):
//   - This module is PURE. No Effect runtime, no IO, no clock, no env. The
//     metering hook (#5477) imports `priceRequest` and decrements the live
//     credit ledger from the result; this module never moves money, never
//     touches D1 / payments-ledger, never logs.
//   - Cost numbers are CONFIG. The Vertex/Anthropic Claude per-token cost is
//     the published LIST rate, NOT our committed-use rate — that real number is
//     a TODO from the billing export (see `VERTEX_COST_IS_LIST_TODO`). The
//     Fireworks open-model costs ARE real (verified 2026-06-19). Everything is
//     a tunable constant so the table re-solves when the real cost lands.
//
// Math (from docs/inference/2026-06-19-pricing-model.md):
//   sell-rate per token = our cost per token × (1 + margin)
//   charge $            = Σ (tokens of each kind × per-kind sell-rate)
//   credits             = charge $ ÷ BASE_CREDIT_USD   (1 credit = $0.01)
//   bitcoin funding     => charge × (1 − BITCOIN_DISCOUNT)   (~5%, config)
// Cached input is billed at a fraction of the input rate; batch requests get a
// flat discount on both directions. All discounts come off COST before margin
// so margin is preserved (docs §6: "funded by real savings, not margin").
import { type InferenceUsage } from './provider-adapter'

// ----------------------------------------------------------------------------
// Config constants (tune here; everything downstream re-solves)
// ----------------------------------------------------------------------------

// Base credit unit. 1 credit = $0.01 (docs §5). We meter internally in USD;
// "credits" is the legible UI unit (credits = usd / BASE_CREDIT_USD).
export const BASE_CREDIT_USD = 0.01

// Target margin applied on top of our marginal cost to get the sell rate.
// 40% midpoint of the 30–50% target band (docs §3). Tunable per launch.
export const DEFAULT_MARGIN = 0.4

// Bitcoin/Lightning funding discount. ~5% off the final charge, funded by the
// card-processing + chargeback-reserve costs we DON'T pay on the BTC rail
// (docs §6) — cost-neutral to us, on-brand pull onto Bitcoin. Configurable;
// keep ≤ realized card-fee savings so margin is untouched.
export const BITCOIN_DISCOUNT = 0.05

// Cached-input fraction of the input rate. Fireworks prompt-cache hits bill
// ~50% of input (fireworks-provider doc); Vertex/Anthropic prompt caching is
// similar in spirit. Applied to `usage.cachedPromptTokens` (a subset of
// promptTokens) so cache hits cost the customer less, mirroring our lower cost.
export const CACHED_INPUT_FRACTION = 0.5

// Batch-inference discount. Fireworks batch = 50% of standard in BOTH
// directions (fireworks-provider doc §pricing). Applied to COST before margin.
export const BATCH_DISCOUNT = 0.5

// !! BILLING TODO (the one true unknown, docs §1 + §8) !!
// The published Cloud Billing Catalog does not expose the Vertex *partner-model*
// (Anthropic) per-token SKUs, so the Claude cost numbers below are the PUBLISHED
// VERTEX/ANTHROPIC LIST rates, not our committed-use / quota rate (which is
// lower). Confirm real per-token cost from the BigQuery billing export before
// publishing prices, then drop the real numbers into the Claude rows of
// VERTEX_CLAUDE_COST and the multiplier table re-solves. Fireworks open-model
// costs ARE real (verified 2026-06-19) and need no such caveat.
export const VERTEX_COST_IS_LIST_TODO = true as const

// ----------------------------------------------------------------------------
// Types
// ----------------------------------------------------------------------------

// How the account funds its balance. Bitcoin gets the funding discount.
export type FundingKind = 'card' | 'bitcoin'

// Supply lane a model is served from (cost attribution / provenance). Purely
// descriptive here — routing (#5482) owns lane selection.
export type SupplyLane =
  | 'vertex-anthropic'
  | 'vertex-gemini'
  | 'fireworks'
  | 'openagents-network'

// Our marginal cost for a model, per 1M tokens, by billed dimension. Cost is
// CONFIG; sell rate = cost × (1 + margin). `cachedInput` is optional; when a
// provider does not report a distinct cached rate we fall back to
// `input × CACHED_INPUT_FRACTION`.
export type ModelCostPerMtok = Readonly<{
  // USD per 1M input (prompt) tokens.
  inputUsdPerMtok: number
  // USD per 1M output (completion) tokens.
  outputUsdPerMtok: number
  // USD per 1M cached-input tokens, when the provider publishes a distinct
  // cached rate (e.g. Fireworks). Omitted => derived from input.
  cachedInputUsdPerMtok?: number
}>

// A pricing-table entry for one model alias.
export type ModelPricingEntry = Readonly<{
  // Canonical model id used in pricing (lowercased alias key in the table).
  model: string
  // Supply lane (cost provenance).
  lane: SupplyLane
  // Marginal cost basis (config).
  cost: ModelCostPerMtok
  // Published per-model multiplier relative to the baseline (Sonnet = 1.0×).
  // This is the LEGIBLE, published number (Factory-style). It is derived from
  // the blended cost so the table is cost-proportional by default; see
  // `costProportionalMultiplier`. We store it explicitly so the published table
  // can be tuned (e.g. compress the Opus multiplier once real Vertex cost lands)
  // without changing the underlying cost basis.
  multiplier: number
  // True when the cost row is the published LIST rate, not our real cost
  // (the Vertex Claude TODO). Surfaced so callers/tests can assert provenance.
  costIsListPlaceholder: boolean
}>

// Input to a pricing computation.
export type PriceInput = Readonly<{
  // Model alias the customer is charged for (case-insensitive; matched against
  // the table). Unknown models fall back to UNKNOWN_MODEL_COST.
  model: string
  // Receipt-first usage from the provider response (never an estimate).
  usage: InferenceUsage
  // How the account funds its balance (card | bitcoin).
  fundingKind: FundingKind
  // Whether this was a batch request (Fireworks batch = −50% both directions).
  batch?: boolean
  // Margin override (defaults to DEFAULT_MARGIN). Pure knob for tests / tiers.
  margin?: number
}>

// Result of a pricing computation. All amounts are receipt-first and pure.
export type PriceResult = Readonly<{
  // The model the price was computed against (canonical table key, or the raw
  // input lowercased when it fell back to the unknown-model rate).
  model: string
  // True when the model was not in the table and the unknown-model fallback
  // rate was used.
  isUnknownModel: boolean
  // Funding kind the price reflects.
  fundingKind: FundingKind
  // Effective margin applied.
  margin: number
  // USD cost to US (our marginal cost, after cached/batch discounts, before
  // margin). Useful for margin reporting + revshare split (#5474 §6).
  costUsd: number
  // USD charge to the customer BEFORE the funding (bitcoin) discount.
  grossChargeUsd: number
  // USD charge AFTER the funding discount (what actually decrements balance).
  chargeUsd: number
  // The funding discount applied in USD (grossChargeUsd − chargeUsd).
  discountUsd: number
  // Charge expressed in credits (1 credit = $0.01). UI-facing legible unit.
  // The ledger (#5477) converts to msat; this module stays currency-pure (USD).
  credits: number
}>

// ----------------------------------------------------------------------------
// Cost basis (config) — Vertex Claude = LIST (TODO), Fireworks open = REAL
// ----------------------------------------------------------------------------

// Vertex/Anthropic Claude list rates (docs §2). PLACEHOLDER COST — see
// VERTEX_COST_IS_LIST_TODO. Replace with committed-use rate from billing.
const VERTEX_CLAUDE_COST: Readonly<Record<string, ModelCostPerMtok>> = {
  opus: { inputUsdPerMtok: 15.0, outputUsdPerMtok: 75.0 },
  sonnet: { inputUsdPerMtok: 3.0, outputUsdPerMtok: 15.0 },
  haiku: { inputUsdPerMtok: 1.0, outputUsdPerMtok: 5.0 },
}

// Fireworks serverless open-model costs — REAL, verified 2026-06-19
// (docs/inference/2026-06-19-fireworks-provider.md). input / cached / output
// per 1M tokens.
const FIREWORKS_OPEN_COST: Readonly<Record<string, ModelCostPerMtok>> = {
  'gpt-oss-20b': {
    inputUsdPerMtok: 0.07,
    cachedInputUsdPerMtok: 0.035,
    outputUsdPerMtok: 0.3,
  },
  'gpt-oss-120b': {
    inputUsdPerMtok: 0.15,
    cachedInputUsdPerMtok: 0.015,
    outputUsdPerMtok: 0.6,
  },
  'deepseek-v4-flash': {
    inputUsdPerMtok: 0.14,
    cachedInputUsdPerMtok: 0.028,
    outputUsdPerMtok: 0.28,
  },
  minimax: {
    inputUsdPerMtok: 0.3,
    cachedInputUsdPerMtok: 0.06,
    outputUsdPerMtok: 1.2,
  },
  'qwen-3p7-plus': {
    inputUsdPerMtok: 0.4,
    cachedInputUsdPerMtok: 0.08,
    outputUsdPerMtok: 1.6,
  },
  'nemotron-3-ultra': {
    inputUsdPerMtok: 0.6,
    cachedInputUsdPerMtok: 0.12,
    outputUsdPerMtok: 2.4,
  },
  'kimi-k2p5': {
    inputUsdPerMtok: 0.6,
    cachedInputUsdPerMtok: 0.1,
    outputUsdPerMtok: 3.0,
  },
  'kimi-k2p6': {
    inputUsdPerMtok: 0.95,
    cachedInputUsdPerMtok: 0.16,
    outputUsdPerMtok: 4.0,
  },
  'kimi-k2p7-code': {
    inputUsdPerMtok: 0.95,
    cachedInputUsdPerMtok: 0.19,
    outputUsdPerMtok: 4.0,
  },
  'glm-5p2': {
    inputUsdPerMtok: 1.4,
    cachedInputUsdPerMtok: 0.26,
    outputUsdPerMtok: 4.4,
  },
  'glm-5p1': {
    inputUsdPerMtok: 1.4,
    cachedInputUsdPerMtok: 0.26,
    outputUsdPerMtok: 4.4,
  },
  'deepseek-v4-pro': {
    inputUsdPerMtok: 1.74,
    cachedInputUsdPerMtok: 0.145,
    outputUsdPerMtok: 3.48,
  },
}

// Vertex Gemini (Google's own model) cost basis — the Gemini 3.5 Flash lane is
// the default/free-tier model (gateway free-tier enablement §1/§3). Gemini is
// Google's first-party Vertex model (no Anthropic partner quota). These are the
// published Vertex Gemini Flash LIST rates per 1M tokens; the committed-use /
// quota rate may be lower. !! TODO-CONFIRM the real per-token Gemini Flash cost
// from the Cloud Billing export before publishing prices; this is a single
// tunable knob and the multiplier table re-solves when it lands.
export const GEMINI_FLASH_COST_IS_LIST_TODO_CONFIRM = true as const

const VERTEX_GEMINI_COST: Readonly<Record<string, ModelCostPerMtok>> = {
  // Gemini 3.5 Flash list rates (~$0.075 in / $0.30 out per Mtok range,
  // TODO-confirm). Cached input billed at ~25% of input per Vertex Gemini
  // context-cache pricing; falls back to CACHED_INPUT_FRACTION if unset.
  'gemini-3.5-flash': {
    inputUsdPerMtok: 0.075,
    cachedInputUsdPerMtok: 0.01875,
    outputUsdPerMtok: 0.3,
  },
}

// First Khala virtual model id (M0 / #6008). It is a public OpenAgents model
// alias backed by the existing Gemini Flash lane until the full Khala
// coordinator/worker fabric lands; adding it here keeps catalog, quote, and
// receipt-first metering prices derived from the same source of truth.
export const KHALA_MINI_MODEL_ID = 'openagents/khala-mini'
export const KHALA_CODE_MODEL_ID = 'openagents/khala-code'

// True when the requested id is an OpenAgents Khala virtual model. M0 ships a
// single cheap tier (khala-mini) plus the coding verifier tier (khala-code);
// the `openagents/khala-` prefix keeps future tiers (pro) recognized as Khala
// without another edit. Used by the gateway to attach the disclosure
// (`openagents`) block to the response so a Khala request is auditable (which
// concrete model/worker actually served it). Bounded id check, not an intent
// parser.
export const isKhalaModel = (model: string): boolean =>
  model.trim().toLowerCase().startsWith('openagents/khala-')

// Unknown-model fallback cost. Conservative: priced like a mid open model so an
// un-tabled model is never under-charged below plausible cost (docs edge:
// unknown models still clear a sane floor). Not a measured rate.
export const UNKNOWN_MODEL_COST: ModelCostPerMtok = {
  inputUsdPerMtok: 1.0,
  cachedInputUsdPerMtok: 0.5,
  outputUsdPerMtok: 4.0,
}

// Coding-typical input:output blend (4:1) used to derive a single legible
// multiplier from the two-dimensional cost (docs §2): blended = (4·in + 1·out)/5.
const BLEND_INPUT_WEIGHT = 4
const BLEND_OUTPUT_WEIGHT = 1

// Blended cost $/Mtok for a model (the 4:1 coding mix).
export const blendedCostPerMtok = (cost: ModelCostPerMtok): number =>
  (BLEND_INPUT_WEIGHT * cost.inputUsdPerMtok +
    BLEND_OUTPUT_WEIGHT * cost.outputUsdPerMtok) /
  (BLEND_INPUT_WEIGHT + BLEND_OUTPUT_WEIGHT)

// The baseline model whose blended cost defines multiplier = 1.0× (Sonnet).
const BASELINE_MODEL = 'sonnet'

// Cost-proportional multiplier relative to the baseline (Sonnet = 1.0×). This
// is our DEFAULT strategy (docs §4): every model clears cost+margin on its own,
// because there is no subscription to backstop a frontier subsidy.
export const costProportionalMultiplier = (cost: ModelCostPerMtok): number => {
  const baseline = blendedCostPerMtok(VERTEX_CLAUDE_COST[BASELINE_MODEL]!)
  return blendedCostPerMtok(cost) / baseline
}

// ----------------------------------------------------------------------------
// Pricing table (the published multiplier table)
// ----------------------------------------------------------------------------

const entry = (
  model: string,
  lane: SupplyLane,
  cost: ModelCostPerMtok,
  costIsListPlaceholder: boolean,
  multiplierOverride?: number,
): ModelPricingEntry => ({
  model,
  lane,
  cost,
  costIsListPlaceholder,
  // Default to the cost-proportional multiplier, rounded to 2 decimals for a
  // legible published number; allow an explicit override for tuning (e.g.
  // compressing Opus once the real Vertex cost lands).
  multiplier:
    multiplierOverride ??
    Math.round(costProportionalMultiplier(cost) * 100) / 100,
})

// The canonical pricing table keyed by lowercased model alias. Multipliers are
// cost-proportional by default (Sonnet = 1.0×, Opus ≈ 5×, Haiku ≈ 0.33×),
// matching docs §4 "cost-proportional (our default, guaranteed margin)".
export const MODEL_PRICING_TABLE: ReadonlyArray<ModelPricingEntry> = [
  // Vertex/Anthropic Claude lane — cost is LIST placeholder (TODO).
  entry('opus', 'vertex-anthropic', VERTEX_CLAUDE_COST.opus!, true),
  entry('sonnet', 'vertex-anthropic', VERTEX_CLAUDE_COST.sonnet!, true),
  entry('haiku', 'vertex-anthropic', VERTEX_CLAUDE_COST.haiku!, true),
  // Vertex Gemini lane — Gemini 3.5 Flash, the default/free-tier model. Cost is
  // the LIST placeholder (TODO-confirm); the multiplier re-solves when the real
  // committed-use rate lands.
  entry(
    'gemini-3.5-flash',
    'vertex-gemini',
    VERTEX_GEMINI_COST['gemini-3.5-flash']!,
    true,
  ),
  // Khala M0 virtual model alias. Uses the same current cost basis as the
  // Gemini Flash backing lane; the richer Khala coordinator, worker fabric,
  // verification, and settlement receipt fields remain separate milestones.
  entry(
    KHALA_MINI_MODEL_ID,
    'vertex-gemini',
    VERTEX_GEMINI_COST['gemini-3.5-flash']!,
    true,
  ),
  // Khala coding verifier tier (#6010). Backed by the open/code lane today so
  // catalog, quote, route, and receipt all agree before the full Khala worker
  // fabric lands. The verifier receipt is attached by chat-completions-routes;
  // pricing stays model/usage-only.
  entry(
    KHALA_CODE_MODEL_ID,
    'fireworks',
    FIREWORKS_OPEN_COST['kimi-k2p7-code']!,
    false,
  ),
  // Fireworks open-model lane — REAL cost (verified 2026-06-19).
  entry('gpt-oss-20b', 'fireworks', FIREWORKS_OPEN_COST['gpt-oss-20b']!, false),
  entry(
    'gpt-oss-120b',
    'fireworks',
    FIREWORKS_OPEN_COST['gpt-oss-120b']!,
    false,
  ),
  entry(
    'deepseek-v4-flash',
    'fireworks',
    FIREWORKS_OPEN_COST['deepseek-v4-flash']!,
    false,
  ),
  entry('minimax', 'fireworks', FIREWORKS_OPEN_COST.minimax!, false),
  entry(
    'qwen-3p7-plus',
    'fireworks',
    FIREWORKS_OPEN_COST['qwen-3p7-plus']!,
    false,
  ),
  entry(
    'nemotron-3-ultra',
    'fireworks',
    FIREWORKS_OPEN_COST['nemotron-3-ultra']!,
    false,
  ),
  entry('kimi-k2p5', 'fireworks', FIREWORKS_OPEN_COST['kimi-k2p5']!, false),
  entry('kimi-k2p6', 'fireworks', FIREWORKS_OPEN_COST['kimi-k2p6']!, false),
  entry(
    'kimi-k2p7-code',
    'fireworks',
    FIREWORKS_OPEN_COST['kimi-k2p7-code']!,
    false,
  ),
  entry('glm-5p2', 'fireworks', FIREWORKS_OPEN_COST['glm-5p2']!, false),
  entry('glm-5p1', 'fireworks', FIREWORKS_OPEN_COST['glm-5p1']!, false),
  entry(
    'deepseek-v4-pro',
    'fireworks',
    FIREWORKS_OPEN_COST['deepseek-v4-pro']!,
    false,
  ),
]

// Index for O(1) lookup by lowercased model alias.
const MODEL_INDEX: ReadonlyMap<string, ModelPricingEntry> = new Map(
  MODEL_PRICING_TABLE.map(e => [e.model, e]),
)

// Resolve a model alias to its pricing entry (case-insensitive). Returns
// undefined when the model is not in the table (caller uses the unknown
// fallback). Substring matching is intentionally NOT done here — the gateway
// resolves provider-native aliases upstream; pricing keys on the canonical id.
export const lookupModel = (model: string): ModelPricingEntry | undefined =>
  MODEL_INDEX.get(model.trim().toLowerCase())

// ----------------------------------------------------------------------------
// Core pricing computation
// ----------------------------------------------------------------------------

// Per-token sell rate (USD) for one dimension: cost × (1 + margin).
const sellPerToken = (usdPerMtok: number, margin: number): number =>
  (usdPerMtok / 1_000_000) * (1 + margin)

// Clamp to a non-negative finite number (defensive: usage counts and rates must
// be non-negative; a NaN/negative override should not produce a negative charge).
const nonNeg = (n: number): number => (Number.isFinite(n) && n > 0 ? n : 0)

// Compute the charge for a completed request. PURE.
//
// charge = Σ over {uncached-input, cached-input, output} of:
//            tokens × per-token sell-rate × (batch ? BATCH_DISCOUNT : 1)
// then bitcoin funding applies a flat BITCOIN_DISCOUNT on the gross charge.
//
// Token split:
//   cachedTokens     = min(usage.cachedPromptTokens ?? 0, promptTokens)
//   uncachedTokens   = promptTokens − cachedTokens
//   completionTokens = usage.completionTokens
export const priceRequest = (input: PriceInput): PriceResult => {
  const margin = nonNeg(input.margin ?? DEFAULT_MARGIN)
  const batchFactor = input.batch ? BATCH_DISCOUNT : 1

  const matched = lookupModel(input.model)
  const isUnknownModel = matched === undefined
  const cost = matched?.cost ?? UNKNOWN_MODEL_COST
  const canonicalModel = matched?.model ?? input.model.trim().toLowerCase()

  const promptTokens = nonNeg(input.usage.promptTokens)
  const completionTokens = nonNeg(input.usage.completionTokens)
  const cachedTokens = Math.min(
    nonNeg(input.usage.cachedPromptTokens ?? 0),
    promptTokens,
  )
  const uncachedTokens = promptTokens - cachedTokens

  // Cached-input cost per Mtok: provider's published cached rate when present,
  // else input × CACHED_INPUT_FRACTION.
  const cachedInputUsdPerMtok =
    cost.cachedInputUsdPerMtok ?? cost.inputUsdPerMtok * CACHED_INPUT_FRACTION

  // --- our marginal COST (before margin), after cached + batch discounts ---
  const costUsd =
    ((uncachedTokens * cost.inputUsdPerMtok) / 1_000_000 +
      (cachedTokens * cachedInputUsdPerMtok) / 1_000_000 +
      (completionTokens * cost.outputUsdPerMtok) / 1_000_000) *
    batchFactor

  // --- customer SELL charge (cost × (1 + margin)), same token split ---
  const grossChargeUsd =
    (uncachedTokens * sellPerToken(cost.inputUsdPerMtok, margin) +
      cachedTokens * sellPerToken(cachedInputUsdPerMtok, margin) +
      completionTokens * sellPerToken(cost.outputUsdPerMtok, margin)) *
    batchFactor

  // --- bitcoin funding discount (off the top, funded by real card savings) ---
  const fundingFactor =
    input.fundingKind === 'bitcoin' ? 1 - BITCOIN_DISCOUNT : 1
  const chargeUsd = grossChargeUsd * fundingFactor
  const discountUsd = grossChargeUsd - chargeUsd

  return {
    model: canonicalModel,
    isUnknownModel,
    fundingKind: input.fundingKind,
    margin,
    costUsd,
    grossChargeUsd,
    chargeUsd,
    discountUsd,
    credits: chargeUsd / BASE_CREDIT_USD,
  }
}

// ----------------------------------------------------------------------------
// Accepted-outcome pricing (Khala M3 / #6011, EPIC #6017)
// ----------------------------------------------------------------------------
//
// `khala-code` is paid TWICE over: per-token (the metered charge above) AND per
// ACCEPTED OUTCOME — a flat price the customer is charged (and the worker +
// validator are settled against) when a verified, EXECUTED accepted outcome is
// produced (a crossy-road build that actually passed the headless acceptance
// suite). Per-token covers the compute; the accepted-outcome price covers the
// VERIFIED RESULT — the thing the customer actually wanted. Pure config, like
// every other number here; the settlement path derives the worker/validator
// shares from it (`khala-accepted-outcome-settlement.ts`).
//
// Denominated in msat so it slots straight into the Bitcoin/Spark settlement
// path (1 sat = 1000 msat). Kept deliberately TINY + treasury-bounded for the
// guinea-pig test wave: a few sats per accepted outcome, well under the gate's
// per-payout cap. Tune here; the catalog + settlement re-solve.
export type AcceptedOutcomePrice = Readonly<{
  // The model the accepted-outcome price applies to (canonical id).
  model: string
  // The flat price per accepted (verified, executed) outcome, in integer msat.
  priceMsat: number
  // The fraction of the accepted-outcome price that goes to the serving WORKER
  // (the rest goes to the VALIDATOR). The validator earns a smaller, published
  // share for the independent verification. Worker + validator shares sum to 1.
  workerShare: number
}>

// The published accepted-outcome price for khala-code. 5 sats total (5_000 msat)
// split 60/40 worker/validator: the worker that produced the accepted artifact
// earns the larger share; the validator earns a real, published cut for the
// independent headless verification that made the outcome trustworthy. Both
// well under the owner gate's per-payout cap. TINY + treasury-bounded by design.
export const KHALA_CODE_ACCEPTED_OUTCOME_PRICE: AcceptedOutcomePrice = {
  model: KHALA_CODE_MODEL_ID,
  priceMsat: 5_000,
  workerShare: 0.6,
}

// The accepted-outcome price table, keyed by lowercased model id. Only models
// with a verified accepted-outcome rubric have an entry; everything else has no
// accepted-outcome price (per-token only).
const ACCEPTED_OUTCOME_PRICE_INDEX: ReadonlyMap<string, AcceptedOutcomePrice> =
  new Map([
    [KHALA_CODE_MODEL_ID.toLowerCase(), KHALA_CODE_ACCEPTED_OUTCOME_PRICE],
  ])

// Resolve a model's accepted-outcome price (case-insensitive). Returns undefined
// when the model has no accepted-outcome lane (per-token pricing only). PURE.
export const lookupAcceptedOutcomePrice = (
  model: string,
): AcceptedOutcomePrice | undefined =>
  ACCEPTED_OUTCOME_PRICE_INDEX.get(model.trim().toLowerCase())

// The integer-msat worker + validator split of an accepted-outcome price. PURE +
// deterministic: the worker share floors and the validator share takes the exact
// remainder, so worker + validator sum EXACTLY to `priceMsat` (no dust lost or
// minted). A non-positive price yields a zero split.
export const acceptedOutcomeSettlementShares = (
  price: AcceptedOutcomePrice,
): Readonly<{ workerMsat: number; validatorMsat: number }> => {
  const total =
    Number.isFinite(price.priceMsat) && price.priceMsat > 0
      ? Math.floor(price.priceMsat)
      : 0
  if (total <= 0) {
    return { validatorMsat: 0, workerMsat: 0 }
  }
  const share =
    Number.isFinite(price.workerShare) && price.workerShare > 0
      ? Math.min(price.workerShare, 1)
      : 0
  const workerMsat = Math.floor(total * share)
  // Validator takes the EXACT remainder so the split conserves the price.
  const validatorMsat = total - workerMsat
  return { validatorMsat, workerMsat }
}

// Convenience: published sell price $/Mtok for a model + dimension, for the
// pricing table / public price page. Pure, derived from cost × (1 + margin).
export const sellPricePerMtok = (
  model: string,
  dimension: 'input' | 'cached' | 'output',
  margin: number = DEFAULT_MARGIN,
): number | undefined => {
  const matched = lookupModel(model)
  if (matched === undefined) return undefined
  const cost = matched.cost
  const usdPerMtok =
    dimension === 'input'
      ? cost.inputUsdPerMtok
      : dimension === 'output'
        ? cost.outputUsdPerMtok
        : (cost.cachedInputUsdPerMtok ??
          cost.inputUsdPerMtok * CACHED_INPUT_FRACTION)
  return usdPerMtok * (1 + margin)
}
