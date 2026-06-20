// OpenAI-compatible `GET /v1/models` route for the inference gateway
// (blocker.product_promises.public_paid_model_gateway_missing).
//
// This is the PUBLIC discovery + price surface that the paid model gateway was
// missing: a standard OpenAI `/v1/models` listing of every model the gateway
// serves, each with its published per-1M-token sell price, free-tier flag, and
// cost-basis provenance (model-catalog.ts). Off-the-shelf OpenAI clients call
// this to populate model pickers; a credits customer reads it to know what each
// model costs before funding a balance.
//
// INERT by default: gated behind the SAME `INFERENCE_GATEWAY_ENABLED` flag as
// the chat-completions route, so when the gateway is off this route 404s exactly
// like the rest of the gateway. No auth is required — the catalog is public-safe
// (published prices only, no prompts/credentials/balances) and discovery is a
// pre-purchase step, mirroring how OpenAI serves `/v1/models` cheaply. The
// handler is pure apart from the injected clock for the OpenAI `created` field.

import { Effect } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { currentEpochSeconds } from '../runtime-primitives'
import { buildModelCatalog, toOpenAiModelsResponse } from './model-catalog'

export type ModelsListDeps = Readonly<{
  // Whether the gateway is enabled. The Worker passes
  // isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED); default OFF.
  enabled: boolean
  // Injected clock for the OpenAI `created` field. Defaults to the real epoch
  // seconds; tests inject a fixed value.
  nowEpochSeconds?: () => number
  // Catalog margin override (defaults to the launch margin inside the catalog).
  margin?: number
}>

// Serve the OpenAI-compatible `/v1/models` listing for the gateway.
export const handleModelsList = (request: Request, deps: ModelsListDeps) =>
  Effect.sync<Response>(() => {
    // INERT GATE: 404 when the gateway is flagged off, matching the
    // chat-completions route's disabled posture.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'GET') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    const now = (deps.nowEpochSeconds ?? currentEpochSeconds)()
    const catalog = buildModelCatalog(deps.margin)
    return noStoreJsonResponse(toOpenAiModelsResponse(catalog, now))
  })
