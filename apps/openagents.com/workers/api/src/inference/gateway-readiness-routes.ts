// Public `GET /v1/gateway/readiness` route for the inference gateway
// (blocker.product_promises.public_paid_model_gateway_missing on
// api.hosted_gemini.v1).
//
// THE GAP this closes: `projectGatewayReadiness` (gateway-readiness.ts) already
// derives the SINGLE readiness fact ("can the paid gateway serve anything right
// now, and how degraded is its catalog?") from the SAME published catalog +
// serving policy the three live surfaces (`/v1/models`, `/v1/quote`, the
// `/v1/chat/completions` dispatch path) gate on. But it was NOT wired to a
// route: an operator (or the launch dashboard) had no dereferenceable endpoint
// to read it, so verifying gateway readiness still meant replaying each surface
// and counting by hand. This module exposes that projection as a public-safe
// route.
//
// INERT by default: gated behind the SAME `INFERENCE_GATEWAY_ENABLED` flag as
// the rest of the gateway, so when the gateway is off this route 404s exactly
// like `/v1/models`, `/v1/quote`, and `/v1/chat/completions`. No auth is
// required and the body is public-safe (servable/hidden model COUNTS + per-lane
// arming booleans + dereferenceable reason refs only — no prompts, completions,
// credentials, prices, or balances). PURE apart from the injected arming the
// Worker derives from credential PRESENCE (model-serving-policy.ts), so this
// route never reads a credential value and moves no money.

import { Effect } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { projectGatewayReadiness } from './gateway-readiness'
import type { ModelCatalogEntry } from './model-catalog'
import type { SupplyLaneArming } from './model-serving-policy'

export type GatewayReadinessDeps = Readonly<{
  // Whether the gateway is enabled. The Worker passes
  // isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED); default OFF.
  enabled: boolean
  // Provider serving policy: which supply lanes are armed (credential present).
  // The Worker passes resolveSupplyLaneArming(env); presence-only, no values.
  laneArming: SupplyLaneArming
  // Catalog override (defaults to the live published catalog inside the
  // projection). Injectable for tests.
  catalog?: ReadonlyArray<ModelCatalogEntry>
}>

// Serve `GET /v1/gateway/readiness`: the single public-safe readiness summary.
export const handleGatewayReadiness = (
  request: Request,
  deps: GatewayReadinessDeps,
) =>
  Effect.sync<Response>(() => {
    // INERT GATE: 404 when the gateway is flagged off, matching the
    // chat-completions / models / quote routes' disabled posture.
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

    const readiness =
      deps.catalog === undefined
        ? projectGatewayReadiness(deps.laneArming)
        : projectGatewayReadiness(deps.laneArming, deps.catalog)
    return noStoreJsonResponse(readiness)
  })
