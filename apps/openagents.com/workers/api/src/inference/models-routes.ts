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
import {
  buildModelCatalog,
  findModelCatalogEntry,
  toOpenAiModelObject,
  toOpenAiModelsResponse,
} from './model-catalog'
import {
  type SupplyLaneArming,
  filterPublicCatalog,
  filterServableCatalog,
  isModelServable,
  isPublicModelId,
  projectKhalaCatalogForArming,
} from './model-serving-policy'

export type ModelsListDeps = Readonly<{
  // Whether the gateway is enabled. The Worker passes
  // isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED); default OFF.
  enabled: boolean
  // Injected clock for the OpenAI `created` field. Defaults to the real epoch
  // seconds; tests inject a fixed value.
  nowEpochSeconds?: () => number
  // Catalog margin override (defaults to the launch margin inside the catalog).
  margin?: number
  // Provider serving policy: which supply lanes are armed (credential present).
  // When supplied, the catalog is narrowed to ONLY models the gateway can
  // actually serve, so a paid gateway never advertises an unservable model
  // (model-serving-policy.ts). When omitted, every catalog model is listed
  // (the prior behaviour — preserved for callers that do not gate on arming).
  laneArming?: SupplyLaneArming
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
    const fullCatalog = buildModelCatalog(deps.margin)
    // PROVIDER POLICY: when the worker supplies lane arming, advertise only the
    // models whose supply lane is actually servable right now; otherwise list
    // the full catalog (prior behaviour).
    const catalog =
      deps.laneArming === undefined
        ? filterPublicCatalog(fullCatalog)
        : filterServableCatalog(fullCatalog, deps.laneArming)
    return noStoreJsonResponse(toOpenAiModelsResponse(catalog, now))
  })

// Serve the OpenAI-compatible `GET /v1/models/{model}` retrieve for a single
// model. Mirrors the list route's inert posture (404 when the gateway is off,
// 405 on non-GET) and is likewise public + unauthenticated (published price +
// policy only — public-safe pre-purchase discovery). An unknown/blank model id
// returns OpenAI's standard `model_not_found` error so off-the-shelf clients
// surface it correctly. The route is expected to extract `{model}` from the
// path and pass it as `modelId`.
export const handleModelRetrieve = (
  request: Request,
  modelId: string,
  deps: ModelsListDeps,
) =>
  Effect.sync<Response>(() => {
    // INERT GATE: 404 when the gateway is flagged off, matching the list route.
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

    const catalog =
      deps.laneArming === undefined
        ? buildModelCatalog(deps.margin)
        : projectKhalaCatalogForArming(
            buildModelCatalog(deps.margin),
            deps.laneArming,
          )
    const entry =
      deps.laneArming === undefined
        ? findModelCatalogEntry(modelId, deps.margin)
        : catalog.find(model => model.id === modelId)
    // PROVIDER POLICY: when lane arming is supplied, a model whose lane is not
    // servable right now is reported `model_not_found` (same as an unknown id),
    // so the retrieve surface never confirms a model the gateway cannot serve.
    const servable =
      entry !== undefined &&
      isPublicModelId(entry.id) &&
      (deps.laneArming === undefined || isModelServable(entry, deps.laneArming))
    if (entry === undefined || !servable) {
      // OpenAI's standard 404 retrieve error shape; clients key off
      // `error.code === 'model_not_found'`.
      return noStoreJsonResponse(
        {
          error: {
            code: 'model_not_found',
            message: `The model '${modelId}' does not exist or is not served by this gateway.`,
            param: 'model',
            type: 'invalid_request_error',
          },
        },
        { status: 404 },
      )
    }

    const now = (deps.nowEpochSeconds ?? currentEpochSeconds)()
    return noStoreJsonResponse(toOpenAiModelObject(entry, now))
  })

// Path prefix for the OpenAI-compatible single-model retrieve. The list path
// (`/v1/models`) is served by the exact-route registry; this dispatcher handles
// ONLY the `/v1/models/{model}` path-param retrieve, which the exact registry
// (exact-match only) cannot route.
const MODELS_BASE = '/v1/models'

// Dispatch `GET /v1/models/{model}` (retrieve one) to `handleModelRetrieve`.
// Returns `undefined` for any non-matching path so the main router falls
// through (the `/v1/models` LIST is registered as an exact route and is
// intentionally NOT matched here). Mirrors `routeCloudCodingSessionRequest`:
// the INERT gate + method check live in the handler, so a matching path with an
// unmatched method still returns the typed 404/405 rather than a fall-through.
// Every served model id is a slash-free canonical slug (see MODEL_PRICING_TABLE),
// so a nested path is never a valid model id and falls through.
export const routeModelRetrieveRequest = (
  request: Request,
  deps: ModelsListDeps,
): Effect.Effect<Response> | undefined => {
  const pathname = new URL(request.url).pathname
  const prefix = `${MODELS_BASE}/`
  if (!pathname.startsWith(prefix)) {
    return undefined
  }
  const encodedModelId = pathname.slice(prefix.length)
  // A trailing-slash-only or real nested path is not a valid model id. Encoded
  // slashes are allowed so namespaced OpenAI-compatible ids like
  // `openagents/khala` work as `/v1/models/openagents%2Fkhala`.
  if (encodedModelId === '' || encodedModelId.includes('/')) {
    return undefined
  }
  const modelId = decodeURIComponent(encodedModelId)
  return handleModelRetrieve(request, modelId, deps)
}
