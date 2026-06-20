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
import { type CostEstimate, estimateRequestCost } from './cost-estimate'

export type QuoteDeps = Readonly<{
  // Whether the gateway is enabled. The Worker passes
  // isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED); default OFF.
  enabled: boolean
  // Catalog/pricing margin override (defaults to the launch margin inside the
  // pricing engine). Tests inject a fixed value for determinism.
  margin?: number
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

    // Spread the optional knobs only when set so `exactOptionalPropertyTypes`
    // never sees an explicit `undefined`.
    const estimate: CostEstimate = estimateRequestCost({
      completionTokens: body.completionTokens,
      fundingKind: body.fundingKind ?? 'card',
      model: body.model,
      promptTokens: body.promptTokens,
      ...(body.cachedPromptTokens !== undefined
        ? { cachedPromptTokens: body.cachedPromptTokens }
        : {}),
      ...(body.batch !== undefined ? { batch: body.batch } : {}),
      ...(deps.margin !== undefined ? { margin: deps.margin } : {}),
    })

    return noStoreJsonResponse(estimate)
  })
