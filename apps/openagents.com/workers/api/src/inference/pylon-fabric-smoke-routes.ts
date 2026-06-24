// Operator-only smoke route for the OpenAgents/Pylon serving-fabric lane
// (#6089). This route proves the secret-backed Worker -> Pylon Psionic serve
// transport without widening public model selection: the public catalog remains
// the single `openagents/khala` id, while operators can run a fixed known-answer
// canary against the admitted `openagents-network` adapter.

import { Effect } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import {
  type InferenceProviderAdapter,
  type InferenceResult,
} from './provider-adapter'
import { KHALA_PYLON_MINI_MODEL_ID } from './pricing'

export const PYLON_FABRIC_SMOKE_ROUTE_REF =
  'route.operator.inference.pylon_fabric_smoke.v0_1'
export const PYLON_FABRIC_SMOKE_PROMPT =
  'Respond with exactly OK and nothing else.'
export const PYLON_FABRIC_SMOKE_EXPECTED_CONTENT = 'OK'

export type PylonFabricSmokeDeps = Readonly<{
  enabled: boolean
  requireOperator: () => Promise<boolean>
  adapter: InferenceProviderAdapter | undefined
  nowIso: () => string
}>

const publicSafeSuccess = (result: InferenceResult, observedAt: string) => {
  const canaryPassed =
    result.content.trim() === PYLON_FABRIC_SMOKE_EXPECTED_CONTENT
  return {
    canaryPassed,
    content: canaryPassed ? PYLON_FABRIC_SMOKE_EXPECTED_CONTENT : null,
    finishReason: result.finishReason,
    model: KHALA_PYLON_MINI_MODEL_ID,
    observedAt,
    routeRef: PYLON_FABRIC_SMOKE_ROUTE_REF,
    servedModel: result.servedModel,
    status: canaryPassed ? 'ok' : 'failed',
    usage: result.usage,
  } as const
}

export const handlePylonFabricSmoke = (
  request: Request,
  deps: PylonFabricSmokeDeps,
) =>
  Effect.gen(function* () {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }
    if (request.method !== 'POST') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }
    const authorized = yield* Effect.promise(() => deps.requireOperator())
    if (!authorized) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }
    if (deps.adapter === undefined) {
      return noStoreJsonResponse(
        {
          error: 'pylon_fabric_adapter_unavailable',
          routeRef: PYLON_FABRIC_SMOKE_ROUTE_REF,
        },
        { status: 503 },
      )
    }

    const result = yield* Effect.result(
      deps.adapter.complete({
        messages: [{ content: PYLON_FABRIC_SMOKE_PROMPT, role: 'user' }],
        model: KHALA_PYLON_MINI_MODEL_ID,
        passthroughParams: { max_tokens: 1, temperature: 0 },
        stream: false,
      }),
    )

    if (result._tag === 'Failure') {
      return noStoreJsonResponse(
        {
          adapterId: result.failure.adapterId,
          error: 'pylon_fabric_smoke_failed',
          httpStatus: result.failure.httpStatus ?? null,
          kind: result.failure.kind ?? null,
          reason: result.failure.reason,
          retryable: result.failure.retryable,
          routeRef: PYLON_FABRIC_SMOKE_ROUTE_REF,
        },
        { status: 503 },
      )
    }

    const body = publicSafeSuccess(result.success, deps.nowIso())
    return noStoreJsonResponse(body, { status: body.canaryPassed ? 200 : 502 })
  })
