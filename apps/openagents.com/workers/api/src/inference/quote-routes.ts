// OpenAgents-native `POST /v1/quote` pre-purchase cost-quote route for the
// inference gateway (blocker.product_promises.public_paid_model_gateway_missing).
//
// THE GAP this closes: the gateway already PUBLISHES per-1M-token sell prices
// (`GET /v1/models`) and a pure estimator (`estimateRequestCost`, cost-estimate.ts)
// can answer the only question that gates a funding decision — "for THIS model
// and THIS many tokens, on THIS funding rail, how many credits will it cost?" —
// but nothing exposed that estimator over HTTP, so a credits customer who wants
// to spend deliberately still had to compute it by hand. This route is the thin,
// callable surface over the existing estimator: a customer POSTs an intended
// model + token estimate + funding rail and gets back the exact credit/USD/msat
// charge the metering hook would settle, BEFORE funding a balance.
//
// INERT by default: gated behind the SAME `INFERENCE_GATEWAY_ENABLED` flag as
// `/v1/chat/completions` and `/v1/models`, so it 404s when the gateway is off.
// PUBLIC + unauthenticated: like `/v1/models`, a quote is public-safe
// pre-purchase discovery — it moves no money, writes no ledger row, reads only
// the published catalog price, and the estimator already OMITS our cost basis /
// margin (cost-estimate.ts), so no unit economics leak. RECEIPT-FIRST PRESERVED:
// the response carries `isEstimate: true`; the real charge is still metered
// receipt-first from the provider's actual `usage` object. PURE apart from the
// estimator it delegates to.

import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { type BudgetEstimate, estimateBudgetCapacity } from './budget-estimate'
import { type CostEstimate, estimateRequestCost } from './cost-estimate'
import {
  isPublicModelId,
  resolveNamedModelServability,
  type SupplyLaneArming,
} from './model-serving-policy'
import { normalizeKhalaModelId } from './pricing'

export type QuoteDeps = Readonly<{
  // Whether the gateway is enabled. The Worker passes
  // isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED); default OFF.
  enabled: boolean
  // Catalog/pricing margin override (defaults to the launch margin inside the
  // pricing engine). Tests inject a fixed value for determinism.
  margin?: number
  // Provider serving policy: which supply lanes are armed (credential present).
  // When supplied, a quote for a KNOWN model whose supply lane is NOT armed is
  // refused `model_unavailable` instead of returning a fundable price — so the
  // quote surface can never let a credits customer fund a balance toward a model
  // the gateway cannot serve (mirrors how `/v1/models` filters its catalog;
  // model-serving-policy.ts). When omitted, every model is quotable (the prior
  // behaviour — preserved for callers that do not gate on arming). An UNKNOWN
  // model id is never gated here: the estimator prices it at the conservative
  // fallback rate exactly as before.
  laneArming?: SupplyLaneArming
}>

// QUOTE REQUEST SCHEMA ----------------------------------------------------
// Only the load-bearing fields are decoded. `fundingKind` defaults to the
// conservative card rail (no Bitcoin discount) when omitted; `cachedPromptTokens`
// and `batch` default inside the estimator. Token counts are accepted loosely
// (the estimator clamps negative/fractional/NaN inputs to non-negative ints), so
// a well-formed-but-sloppy estimate still yields a safe, non-negative quote.
const QuoteRequestBody = S.Struct({
  model: S.String,
  promptTokens: S.Number,
  completionTokens: S.Number,
  cachedPromptTokens: S.optionalKey(S.Number),
  fundingKind: S.optionalKey(S.Literals(['card', 'bitcoin'])),
  batch: S.optionalKey(S.Boolean),
  // BUDGET (affordability) MODE — purely additive. When present, the same
  // request shape is priced once and divided into this credit budget, so the
  // response answers the INVERSE question ("how many such requests does N
  // credits buy?") via `estimateBudgetCapacity`. When omitted, the route returns
  // the per-request `CostEstimate` exactly as before (backward compatible).
  budgetCredits: S.optionalKey(S.Number),
})

const decodeBody = (value: unknown): typeof QuoteRequestBody.Type | undefined => {
  try {
    return S.decodeUnknownSync(QuoteRequestBody)(value)
  } catch {
    return undefined
  }
}

// Serve a pre-purchase cost quote. Delegates to the SAME estimator the price
// page / future clients reuse, which itself reuses `priceRequest` — the EXACT
// pricing engine the metering hook charges against — so a quote cannot drift
// from the eventual billed charge.
export const handleQuote = (request: Request, deps: QuoteDeps) =>
  Effect.gen(function* () {
    // INERT GATE: 404 when the gateway is flagged off, matching the
    // chat-completions / models routes' disabled posture.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'POST') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    const rawBody = yield* Effect.promise(async () => {
      try {
        return (await request.json()) as unknown
      } catch {
        return undefined
      }
    })
    if (rawBody === undefined) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const body = decodeBody(rawBody)
    if (body === undefined) {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }
    const quotedModel = normalizeKhalaModelId(body.model)

    if (!isPublicModelId(quotedModel)) {
      return noStoreJsonResponse(
        { error: 'model_unavailable', model: quotedModel },
        { status: 404 },
      )
    }

    // PROVIDER POLICY GATE: when lane arming is supplied, refuse a quote for a
    // KNOWN model whose supply lane is not armed right now — such a request can
    // only fail `model_unavailable` at dispatch, so quoting a fundable price for
    // it would invite funding a balance toward an unservable model. An unknown
    // id (servability `undefined`) falls through to the conservative fallback
    // quote unchanged. Omitting `laneArming` keeps every model quotable.
    if (
      deps.laneArming !== undefined &&
      resolveNamedModelServability(quotedModel, deps.laneArming) === false
    ) {
      return noStoreJsonResponse(
        { error: 'model_unavailable', model: quotedModel },
        { status: 404 },
      )
    }

    // Shared knobs for both modes. Spread the optionals only when set so
    // `exactOptionalPropertyTypes` never sees an explicit `undefined`.
    const shared = {
      completionTokens: body.completionTokens,
      fundingKind: body.fundingKind ?? 'card',
      model: quotedModel,
      promptTokens: body.promptTokens,
      ...(body.cachedPromptTokens !== undefined
        ? { cachedPromptTokens: body.cachedPromptTokens }
        : {}),
      ...(body.batch !== undefined ? { batch: body.batch } : {}),
      ...(deps.margin !== undefined ? { margin: deps.margin } : {}),
    } as const

    // BUDGET MODE: when a credit budget is supplied, answer the inverse
    // affordability question. The response embeds the same per-request
    // `CostEstimate` under `perRequest`, so a token-mode client still gets it.
    if (body.budgetCredits !== undefined) {
      const budget: BudgetEstimate = estimateBudgetCapacity({
        ...shared,
        budgetCredits: body.budgetCredits,
      })
      return noStoreJsonResponse(budget)
    }

    // TOKEN MODE (default, unchanged): the per-request cost quote.
    const estimate: CostEstimate = estimateRequestCost(shared)

    return noStoreJsonResponse(estimate)
  })
