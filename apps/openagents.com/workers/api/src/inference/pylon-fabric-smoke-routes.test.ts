import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  PYLON_FABRIC_SMOKE_EXPECTED_CONTENT,
  PYLON_FABRIC_SMOKE_ROUTE_REF,
  handlePylonFabricSmoke,
} from './pylon-fabric-smoke-routes'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
} from './provider-adapter'

const result = (
  content: string = PYLON_FABRIC_SMOKE_EXPECTED_CONTENT,
): InferenceResult => ({
  content,
  finishReason: 'stop',
  servedModel: 'model.psionic.qwen35.0_8b.q8_0',
  usage: {
    completionTokens: 1,
    promptTokens: 8,
    totalTokens: 9,
  },
})

const adapter = (
  complete: InferenceProviderAdapter['complete'],
): InferenceProviderAdapter => ({
  complete,
  id: 'openagents-network',
  stream: () => Effect.succeed([]),
})

const post = () =>
  new Request('https://x/api/operator/inference/pylon-fabric/smoke', {
    method: 'POST',
  })

const run = (
  input: Readonly<{
    enabled?: boolean
    authorized?: boolean
    adapter?: InferenceProviderAdapter | undefined
    request?: Request | undefined
  }> = {},
) =>
  Effect.runPromise(
    handlePylonFabricSmoke(input.request ?? post(), {
      adapter: input.adapter,
      enabled: input.enabled ?? true,
      nowIso: () => '2026-06-24T05:00:00.000Z',
      requireOperator: async () => input.authorized ?? true,
    }),
  )

describe('handlePylonFabricSmoke', () => {
  test('is inert when the inference gateway is disabled', async () => {
    const response = await run({ enabled: false })
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({ error: 'inference_gateway_disabled' })
  })

  test('requires POST and operator authority', async () => {
    const method = await run({
      request: new Request(
        'https://x/api/operator/inference/pylon-fabric/smoke',
        { method: 'GET' },
      ),
    })
    expect(method.status).toBe(405)

    const unauthorized = await run({ authorized: false })
    expect(unauthorized.status).toBe(401)
    expect(await unauthorized.json()).toEqual({ error: 'unauthorized' })
  })

  test('fails closed when the configured fabric adapter is unavailable', async () => {
    const response = await run()
    expect(response.status).toBe(503)
    expect(await response.json()).toEqual({
      error: 'pylon_fabric_adapter_unavailable',
      routeRef: PYLON_FABRIC_SMOKE_ROUTE_REF,
    })
  })

  test('runs the fixed known-answer canary through the adapter', async () => {
    let observed: InferenceRequest | undefined
    const response = await run({
      adapter: adapter(request => {
        observed = request
        return Effect.succeed(result())
      }),
    })
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      canaryPassed: boolean
      content: string
      model: string
      routeRef: string
      servedModel: string
      status: string
    }
    expect(body).toMatchObject({
      canaryPassed: true,
      content: 'OK',
      model: 'openagents/khala-pylon-mini',
      routeRef: PYLON_FABRIC_SMOKE_ROUTE_REF,
      servedModel: 'model.psionic.qwen35.0_8b.q8_0',
      status: 'ok',
    })
    expect(observed).toMatchObject({
      model: 'openagents/khala-pylon-mini',
      passthroughParams: { max_tokens: 1, temperature: 0 },
      stream: false,
    })
    expect(observed?.messages[0]?.content).toBe(
      'Respond with exactly OK and nothing else.',
    )
  })

  test('does not echo unexpected model output', async () => {
    const response = await run({
      adapter: adapter(() => Effect.succeed(result('not ok'))),
    })
    expect(response.status).toBe(502)
    const body = (await response.json()) as {
      canaryPassed: boolean
      content: string | null
      status: string
    }
    expect(body.canaryPassed).toBe(false)
    expect(body.content).toBeNull()
    expect(body.status).toBe('failed')
  })

  test('returns typed adapter failures without endpoint or token material', async () => {
    const response = await run({
      adapter: adapter(() =>
        Effect.fail(
          new InferenceAdapterError({
            adapterId: 'openagents-network',
            httpStatus: 503,
            kind: 'service_overloaded',
            reason: 'Pylon fabric HTTP route rejected serve request with status 503',
            retryable: true,
          }),
        ),
      ),
    })
    expect(response.status).toBe(503)
    const text = await response.text()
    expect(text).toContain('pylon_fabric_smoke_failed')
    expect(text).toContain('service_overloaded')
    expect(text).not.toContain('https://')
    expect(text).not.toContain('Bearer')
    expect(text).not.toContain('sk-')
  })
})
