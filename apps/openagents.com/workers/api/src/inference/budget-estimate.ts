// Pre-purchase BUDGET / affordability estimate for the OpenAgents inference
// gateway (blocker.product_promises.public_paid_model_gateway_missing).
//
// THE GAP this closes: the gateway already answers the FORWARD question — "for
// THIS model and THIS many tokens, how many credits will it cost?"
// (`estimateRequestCost`, surfaced at `POST /v1/quote`). But the question that
// actually gates a FUNDING decision runs the other way: "if I fund N credits for
// THIS model on THIS rail, how many requests of my typical shape can I run, and
// how many tokens is that?" A credits customer sizing a deliberate top-up needs
// the INVERSE of a per-request quote, and nothing computed it. This module is the
// single pure answer.
//
// It is exact-by-construction: it prices ONE representative request through the
// SAME `estimateRequestCost` the forward quote uses (which itself reuses
// `priceRequest`, the EXACT engine the metering hook charges against), then
// divides the budget by that per-request cost. So an affordability estimate can
// never disagree with the per-request quote, nor with the eventual billed charge.
//
// RECEIPT-FIRST DISCIPLINE PRESERVED: this is explicitly an ESTIMATE, never a
// charge or a grant. It moves no money, writes no ledger row, and the real charge
// is still metered from the provider's actual `usage` object. The output carries
// `isEstimate: true`. PUBLIC-SAFE: it embeds the per-request `CostEstimate`,
// which already OMITS our cost basis / margin, so no unit economics leak. PURE:
// no D1, no clock, no network, no secrets.
import { type CostEstimate, estimateRequestCost } from './cost-estimate'
import { type FundingKind } from './pricing'
import { usdCentsToMsatFloor } from './usd-msat-conversion'

// Round a credit/money figure to a stable number of decimals so the estimate
// carries no floating-point noise (mirrors cost-estimate's `round`). Pure.
const round = (value: number, decimals: number): number => {
  const factor = 10 ** decimals
  return Math.round(value * factor) / factor
}

// Clamp a budget figure to a non-negative finite number (defensive: a customer
// estimate must never imply negative affordability). Pure.
const nonNegative = (value: number): number =>
  Number.isFinite(value) && value > 0 ? value : 0

export type BudgetEstimateInput = Readonly<{
  // Model the budget is being sized against (case-insensitive; matched against
  // the same pricing table the gateway bills from).
  model: string
  // Optional internal pricing alias for virtual public models. Threaded to the
  // forward estimator so budget mode and per-request mode stay identical.
  priceModel?: string
  // The budget to size, in the legible credit unit (1 credit = $0.01 = 1 cent).
  budgetCredits: number
  // The customer's REPRESENTATIVE request shape — the per-call token mix the
  // budget will be spent on. Priced once, then divided into the budget.
  promptTokens: number
  completionTokens: number
  // Cached-input tokens per representative request (subset of promptTokens).
  cachedPromptTokens?: number
  // Funding rail the balance will be drawn from (Bitcoin applies the discount,
  // so the same budget affords MORE requests on the BTC rail).
  fundingKind: FundingKind
  // Whether the representative request is submitted as a batch (−50%).
  batch?: boolean
  // Margin override (defaults to the launch margin inside the pricing engine).
  margin?: number
}>

// The customer-facing affordability estimate. All amounts are customer-facing;
// none reveal our cost basis or margin (the embedded per-request estimate omits
// them).
export type BudgetEstimate = Readonly<{
  // Echoed budget in each unit a customer might think in.
  budgetCredits: number
  budgetUsd: number
  // The budget as spendable msat, using the SAME floor conversion the USD->msat
  // bridge grants with (`usdCentsToMsatFloor`), so this matches the balance the
  // budget would actually fund — never overstating spendable balance.
  budgetMsat: number
  fundingKind: FundingKind
  // The representative per-request estimate the budget was divided by (the exact
  // forward quote for one such request). Reused, so the two surfaces agree.
  perRequest: CostEstimate
  // How many representative requests the budget affords (floored — only whole
  // requests are billable). 0 when the budget cannot cover a single request.
  affordableRequests: number
  // True when the representative request prices to ZERO credits (e.g. an
  // all-empty token shape), so the budget affords an UNBOUNDED count. When true,
  // `affordableRequests` is 0 (a finite count is undefined) and the caller should
  // treat capacity as unlimited for this (degenerate) shape.
  affordableRequestsUnbounded: boolean
  // Total tokens across all affordable requests (observability / planning).
  totalPromptTokens: number
  totalCachedPromptTokens: number
  totalCompletionTokens: number
  totalTokens: number
  // Credits the affordable requests would actually consume, and what is left
  // over (too small to buy one more request). spentCredits + leftoverCredits
  // reconciles to budgetCredits.
  spentCredits: number
  leftoverCredits: number
  // Marker: this is an estimate, never a receipt/grant.
  isEstimate: true
}>

// Estimate how many representative requests a credit budget affords for a model,
// BEFORE funding a balance, from the same pricing engine the gateway bills with.
// PURE — moves no money, writes no ledger.
export const estimateBudgetCapacity = (
  input: BudgetEstimateInput,
): BudgetEstimate => {
  const budgetCredits = nonNegative(input.budgetCredits)

  // Price ONE representative request via the forward estimator (single source of
  // per-request truth). Spread optional knobs only when set so
  // `exactOptionalPropertyTypes` never sees an explicit `undefined`.
  const perRequest = estimateRequestCost({
    completionTokens: input.completionTokens,
    fundingKind: input.fundingKind,
    model: input.model,
    ...(input.priceModel !== undefined ? { priceModel: input.priceModel } : {}),
    promptTokens: input.promptTokens,
    ...(input.cachedPromptTokens !== undefined
      ? { cachedPromptTokens: input.cachedPromptTokens }
      : {}),
    ...(input.batch !== undefined ? { batch: input.batch } : {}),
    ...(input.margin !== undefined ? { margin: input.margin } : {}),
  })

  // 1 credit = $0.01 = 1 cent, so the credit budget maps to cents 1:1 for the
  // grant-floor conversion that mirrors how funding produces spendable msat.
  const budgetMsat = usdCentsToMsatFloor(budgetCredits)
  const budgetUsd = round(budgetCredits * 0.01, 6)

  const perRequestCredits = perRequest.estimatedCredits

  // Degenerate shape (zero-cost representative request): capacity is unbounded;
  // we cannot return a finite count, so flag it and report 0 affordable with the
  // whole budget left over.
  if (perRequestCredits <= 0) {
    return {
      affordableRequests: 0,
      affordableRequestsUnbounded: true,
      budgetCredits: round(budgetCredits, 4),
      budgetMsat,
      budgetUsd,
      fundingKind: input.fundingKind,
      isEstimate: true,
      leftoverCredits: round(budgetCredits, 4),
      perRequest,
      spentCredits: 0,
      totalCachedPromptTokens: 0,
      totalCompletionTokens: 0,
      totalPromptTokens: 0,
      totalTokens: 0,
    }
  }

  // Only whole requests are billable, so floor the quotient (with a tiny dust
  // tolerance so a budget of exactly N×cost is not pushed to N−1 by binary
  // representation error).
  const affordableRequests = Math.floor(
    budgetCredits / perRequestCredits + 1e-9,
  )
  const spentCredits = round(affordableRequests * perRequestCredits, 4)

  return {
    affordableRequests,
    affordableRequestsUnbounded: false,
    budgetCredits: round(budgetCredits, 4),
    budgetMsat,
    budgetUsd,
    fundingKind: input.fundingKind,
    isEstimate: true,
    leftoverCredits: round(budgetCredits - spentCredits, 4),
    perRequest,
    spentCredits,
    totalCachedPromptTokens: affordableRequests * perRequest.cachedPromptTokens,
    totalCompletionTokens: affordableRequests * perRequest.completionTokens,
    totalPromptTokens: affordableRequests * perRequest.promptTokens,
    totalTokens:
      affordableRequests *
      (perRequest.promptTokens + perRequest.completionTokens),
  }
}
