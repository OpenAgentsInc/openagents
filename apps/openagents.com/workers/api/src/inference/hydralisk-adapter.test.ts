import { Effect, Redacted } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type HydraliskFetch,
  type HydraliskReplicaAdapterConfig,
  makeHydraliskVllmAdapter,
  makeHydraliskVllmPoolAdapter,
} from './hydralisk-adapter'
import { HYDRALISK_ADAPTER_ID } from './model-router'
import { HYDRALISK_GPT_OSS_20B_MODEL_ID } from './pricing'
import type { InferenceRequest } from './provider-adapter'

const request = (
  overrides: Partial<InferenceRequest> = {},
): InferenceRequest => ({
  messages: [{ content: 'Say READY.', role: 'user' }],
  model: HYDRALISK_GPT_OSS_20B_MODEL_ID,
  passthroughParams: { max_tokens: 8 },
  stream: false,
  ...overrides,
})

const responseBody = {
  choices: [
    {
      finish_reason: 'stop',
      message: { content: 'READY' },
    },
  ],
  model: 'openai/gpt-oss-20b',
  usage: {
    completion_tokens: 1,
    prompt_tokens: 7,
    total_tokens: 8,
  },
}

const RETRYABLE_STATUS_CASES = [
  [429, 'rate_limited'],
  [503, 'service_overloaded'],
  [500, 'upstream_error'],
] as const

const GLM_POOL_ADAPTER_ID = 'hydralisk-vllm-glm-5p2-reap-504b'

const replicaFixture = (
  replicaId: string,
  overrides: Partial<HydraliskReplicaAdapterConfig> = {},
): HydraliskReplicaAdapterConfig => ({
  apiKey: Redacted.make(`${replicaId}-token`),
  baseUrl: `https://${replicaId}.example.test`,
  benchmarkReserved: false,
  costProfileRef:
    'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.fixture.v1',
  draining: false,
  evidenceRefs: [`receipt.hydralisk.glm.${replicaId}.fixture`],
  fetchImpl: async () => Response.json(responseBody),
  id: GLM_POOL_ADAPTER_ID,
  maxInflight: 1,
  profileRef: `profile.hydralisk.glm_52_reap_504b.${replicaId}.fixture`,
  replicaId,
  ...overrides,
})

const captureFetch =
  (inputs: Array<string>): HydraliskFetch =>
  async input => {
    inputs.push(input)
    return Response.json(responseBody)
  }

describe('hydralisk vLLM adapter', () => {
  it('keeps one GLM pool adapter id while dispatching to an eligible replica', async () => {
    const capturedInputs: Array<string> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('reserved', {
          baseUrl: 'https://reserved.example.test',
          benchmarkReserved: true,
          fetchImpl: captureFetch(capturedInputs),
        }),
        replicaFixture('second', {
          baseUrl: 'https://second.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(adapter.id).toBe(GLM_POOL_ADAPTER_ID)
    expect(result.content).toBe('READY')
    // Public served-model disclosure is the canonical lane id, NOT the raw vLLM
    // served-model-name that the upstream returns in the response body (#6259).
    // The fixture upstream returns `openai/gpt-oss-20b`; disclosure must be the
    // configured `openagents/glm-5.2-reap-504b`.
    expect(result.servedModel).toBe('openagents/glm-5.2-reap-504b')
    expect(result.adapterRouteMetadata).toMatchObject({
      replicaFallbackReason: null,
      replicaHealthScore: 1,
      selectedReplicaId: 'second',
      selectedReplicaRef: 'replica.hydralisk.glm_52_reap_504b.second',
    })
    expect(capturedInputs).toEqual([
      'https://second.example.test/v1/chat/completions',
    ])
  })

  it('prefers the cache-affinity replica when it is healthy and idle', async () => {
    const capturedInputs: Array<string> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      affinityOracle: affinity =>
        affinity === 'cacheaff:fixture' ? 'second' : undefined,
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('primary', { fetchImpl: captureFetch(capturedInputs) }),
        replicaFixture('second', {
          baseUrl: 'https://second.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const result = await Effect.runPromise(
      adapter.complete(
        request({
          passthroughParams: {
            max_tokens: 8,
            'x-session-affinity': 'cacheaff:fixture',
          },
        }),
      ),
    )

    expect(result.adapterRouteMetadata).toMatchObject({
      replicaFallbackReason: 'cache_affinity_hit',
      selectedReplicaId: 'second',
    })
    expect(capturedInputs).toEqual([
      'https://second.example.test/v1/chat/completions',
    ])
  })

  it('falls back to a warmed idle replica when the affinity target is busy', async () => {
    const capturedInputs: Array<string> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      affinityOracle: () => 'second',
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('primary', {
          baseUrl: 'https://primary.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
        replicaFixture('second', {
          baseUrl: 'https://second.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      routingStateOracle: replicaId =>
        replicaId === 'second'
          ? { inflightCount: 1, maxInflight: 1, warmAtEpochMs: 200 }
          : { warmAtEpochMs: 100 },
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const result = await Effect.runPromise(
      adapter.complete(
        request({
          passthroughParams: {
            max_tokens: 8,
            'x-session-affinity': 'cacheaff:fixture',
          },
        }),
      ),
    )

    expect(result.adapterRouteMetadata).toMatchObject({
      replicaFallbackReason: 'inflight_full',
      selectedReplicaId: 'primary',
    })
    expect(capturedInputs).toEqual([
      'https://primary.example.test/v1/chat/completions',
    ])
  })

  it('skips draining replicas and chooses the warmed idle candidate', async () => {
    const capturedInputs: Array<string> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('draining', {
          baseUrl: 'https://draining.example.test',
          draining: true,
          fetchImpl: captureFetch(capturedInputs),
        }),
        replicaFixture('warm', {
          baseUrl: 'https://warm.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
        replicaFixture('cold', {
          baseUrl: 'https://cold.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      routingStateOracle: replicaId =>
        replicaId === 'warm' ? { warmAtEpochMs: 500 } : undefined,
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(result.adapterRouteMetadata?.selectedReplicaId).toBe('warm')
    expect(capturedInputs).toEqual([
      'https://warm.example.test/v1/chat/completions',
    ])
  })

  it('uses heartbeat warm state to avoid cold replicas when another candidate is usable', async () => {
    const capturedInputs: Array<string> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('cold', {
          baseUrl: 'https://cold.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
        replicaFixture('unknown', {
          baseUrl: 'https://unknown.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      routingStateOracle: replicaId =>
        replicaId === 'cold' ? { warmState: 'cold' } : undefined,
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(result.adapterRouteMetadata).toMatchObject({
      replicaWarmState: 'unknown',
      selectedReplicaId: 'unknown',
    })
    expect(capturedInputs).toEqual([
      'https://unknown.example.test/v1/chat/completions',
    ])
  })

  it('keeps degraded replicas eligible but ranks them below healthy replicas', async () => {
    const capturedInputs: Array<string> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('degraded', {
          baseUrl: 'https://degraded.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
        replicaFixture('healthy', {
          baseUrl: 'https://healthy.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      routingStateOracle: replicaId =>
        replicaId === 'degraded'
          ? { health: 'degraded', warmAtEpochMs: 1_000, warmState: 'warm' }
          : { health: 'healthy', warmState: 'unknown' },
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(result.adapterRouteMetadata).toMatchObject({
      replicaHealthScore: 1,
      selectedReplicaId: 'healthy',
    })
    expect(capturedInputs).toEqual([
      'https://healthy.example.test/v1/chat/completions',
    ])
  })

  it('excludes unhealthy replicas from the GLM pool selector', async () => {
    const capturedInputs: Array<string> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('unhealthy', {
          baseUrl: 'https://unhealthy.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      routingStateOracle: () => ({ health: 'unhealthy' }),
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const outcome = await Effect.runPromise(
      Effect.result(adapter.complete(request())),
    )

    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe('glm_pool_saturated')
      expect(outcome.failure.retryable).toBe(true)
      expect(outcome.failure.adapterRouteMetadata).toMatchObject({
        replicaBusyReason: 'health_unhealthy',
      })
    }
    expect(capturedInputs).toEqual([])
  })

  it('sends concurrent singleflight requests to different idle replicas', async () => {
    const capturedInputs: Array<string> = []
    let releasePrimary: (() => void) | undefined
    let markPrimaryStarted: (() => void) | undefined
    const primaryStarted = new Promise<void>(resolve => {
      markPrimaryStarted = resolve
    })
    const releasePrimaryPromise = new Promise<void>(resolve => {
      releasePrimary = resolve
    })
    const primaryFetch: HydraliskFetch = async input => {
      capturedInputs.push(input)
      markPrimaryStarted?.()
      await releasePrimaryPromise
      return Response.json(responseBody)
    }
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('primary', {
          baseUrl: 'https://primary.example.test',
          fetchImpl: primaryFetch,
        }),
        replicaFixture('second', {
          baseUrl: 'https://second.example.test',
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const first = Effect.runPromise(adapter.complete(request()))
    await primaryStarted
    const second = Effect.runPromise(adapter.complete(request()))
    releasePrimary?.()
    const results = await Promise.all([first, second])

    expect(
      results.map(result => result.adapterRouteMetadata?.selectedReplicaId),
    ).toEqual(['primary', 'second'])
    expect(capturedInputs).toEqual([
      'https://primary.example.test/v1/chat/completions',
      'https://second.example.test/v1/chat/completions',
    ])
  })

  it('overflows immediately when every GLM replica is busy and never stacks a busy endpoint', async () => {
    const capturedInputs: Array<string> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      replicas: [
        replicaFixture('primary', {
          fetchImpl: captureFetch(capturedInputs),
        }),
        replicaFixture('second', {
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      routingStateOracle: () => ({ inflightCount: 1, maxInflight: 1 }),
      saturationPolicy: 'overflow_immediately',
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const outcome = await Effect.runPromise(
      Effect.result(adapter.complete(request())),
    )

    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe('glm_pool_saturated')
      expect(outcome.failure.httpStatus).toBe(429)
      expect(outcome.failure.retryable).toBe(true)
      expect(outcome.failure.adapterRouteMetadata).toMatchObject({
        glmSaturationPolicy: 'overflow_immediately',
        queueWaitMs: 0,
        replicaBusyReason: 'inflight_full',
      })
    }
    expect(capturedInputs).toEqual([])
  })

  it('waits a bounded queue window for non-streaming GLM work, then serves a newly idle replica', async () => {
    const capturedInputs: Array<string> = []
    let busy = true
    let now = 1_000
    const sleeps: Array<number> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      maxQueueWaitMs: 125,
      nowEpochMs: () => now,
      replicas: [
        replicaFixture('primary', {
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      routingStateOracle: () =>
        busy ? { inflightCount: 1, maxInflight: 1 } : undefined,
      saturationPolicy: 'queue_then_overflow',
      sleep: ms =>
        Effect.sync(() => {
          sleeps.push(ms)
          now += ms
          busy = false
        }),
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(result.adapterRouteMetadata).toMatchObject({
      glmSaturationPolicy: 'queue_then_overflow',
      queueWaitMs: 125,
      selectedReplicaId: 'primary',
    })
    expect(sleeps).toEqual([125])
    expect(capturedInputs).toEqual([
      'https://primary.example.test/v1/chat/completions',
    ])
  })

  it('returns stable non-retryable backpressure after the queue window for queue_then_429', async () => {
    const capturedInputs: Array<string> = []
    const sleeps: Array<number> = []
    const adapter = makeHydraliskVllmPoolAdapter({
      id: GLM_POOL_ADAPTER_ID,
      maxQueueWaitMs: 75,
      nowEpochMs: () => 0,
      replicas: [
        replicaFixture('primary', {
          fetchImpl: captureFetch(capturedInputs),
        }),
      ],
      routingStateOracle: () => ({ inflightCount: 1, maxInflight: 1 }),
      saturationPolicy: 'queue_then_429',
      sleep: ms =>
        Effect.sync(() => {
          sleeps.push(ms)
        }),
      upstreamModel: 'openagents/glm-5.2-reap-504b',
    })

    const outcome = await Effect.runPromise(
      Effect.result(adapter.complete(request())),
    )

    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe('glm_pool_saturated')
      expect(outcome.failure.httpStatus).toBe(429)
      expect(outcome.failure.retryable).toBe(false)
      expect(outcome.failure.adapterRouteMetadata).toMatchObject({
        glmSaturationPolicy: 'queue_then_429',
        queueWaitMs: 75,
        replicaBusyReason: 'inflight_full',
      })
    }
    expect(sleeps).toEqual([75])
    expect(capturedInputs).toEqual([])
  })

  it('maps the GPT-OSS model id to the Hydralisk OpenAI-compatible endpoint', async () => {
    let captured:
      | Readonly<{
          input: string
          init: RequestInit
          body: Record<string, unknown>
        }>
      | undefined

    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test/',
      fetchImpl: async (input, init) => {
        captured = {
          body: JSON.parse(String(init.body)) as Record<string, unknown>,
          init,
          input,
        }
        return Response.json(responseBody)
      },
      id: HYDRALISK_ADAPTER_ID,
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(result.content).toBe('READY')
    expect(result.servedModel).toBe('openai/gpt-oss-20b')
    expect(result.usage).toEqual({
      completionTokens: 1,
      promptTokens: 7,
      totalTokens: 8,
    })
    expect(captured?.input).toBe(
      'https://hydralisk.example.test/v1/chat/completions',
    )
    expect(captured?.body.model).toBe(HYDRALISK_GPT_OSS_20B_MODEL_ID)
    expect(captured?.body.stream).toBe(false)
    expect(
      (captured?.init.headers as Record<string, string>).authorization,
    ).toBe('Bearer hydralisk-token')
  })

  it('preserves OpenAI tool metadata in outbound messages and params', async () => {
    let captured: Record<string, unknown> | undefined

    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test/',
      fetchImpl: async (_input, init) => {
        captured = JSON.parse(String(init.body)) as Record<string, unknown>
        return Response.json(responseBody)
      },
      id: HYDRALISK_ADAPTER_ID,
    })

    await Effect.runPromise(
      adapter.complete(
        request({
          messages: [
            {
              content: '',
              role: 'assistant',
              toolCalls: [
                {
                  function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
                  id: 'call_bash',
                  type: 'function',
                },
              ],
            },
            {
              content: '/tmp/project',
              role: 'tool',
              toolCallId: 'call_bash',
            },
          ],
          passthroughParams: {
            max_tokens: 8,
            tool_choice: 'auto',
            tools: [
              {
                function: {
                  name: 'bash',
                  parameters: { type: 'object' },
                },
                type: 'function',
              },
            ],
          },
        }),
      ),
    )

    const messages = captured?.['messages'] as
      | ReadonlyArray<Record<string, unknown>>
      | undefined
    expect(captured?.['tools']).toHaveLength(1)
    expect(captured?.['tool_choice']).toBe('auto')
    expect(messages?.[0]?.['tool_calls']).toEqual([
      {
        function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
        id: 'call_bash',
        type: 'function',
      },
    ])
    expect(messages?.[1]?.['tool_call_id']).toBe('call_bash')
  })

  it('preserves non-streaming assistant tool calls from the provider response', async () => {
    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test',
      fetchImpl: async () =>
        Response.json({
          ...responseBody,
          choices: [
            {
              finish_reason: 'tool_calls',
              message: {
                content: '',
                tool_calls: [
                  {
                    function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
                    id: 'call_bash',
                    type: 'function',
                  },
                ],
              },
            },
          ],
        }),
      id: HYDRALISK_ADAPTER_ID,
    })

    const result = await Effect.runPromise(adapter.complete(request()))

    expect(result.finishReason).toBe('tool_calls')
    expect(result.toolCalls).toEqual([
      {
        function: { arguments: '{"cmd":"pwd"}', name: 'bash' },
        id: 'call_bash',
        type: 'function',
      },
    ])
  })

  it('fails closed when terminal usage is absent', async () => {
    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test',
      fetchImpl: async () =>
        Response.json({
          choices: [{ finish_reason: 'stop', message: { content: 'READY' } }],
          model: 'openai/gpt-oss-20b',
        }),
      id: HYDRALISK_ADAPTER_ID,
    })

    const outcome = await Effect.runPromise(
      Effect.result(adapter.complete(request())),
    )

    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe('malformed_response')
      expect(outcome.failure.retryable).toBe(false)
      expect(outcome.failure.reason).toBe(
        'hydralisk response missing terminal usage',
      )
    }
  })

  it.each(RETRYABLE_STATUS_CASES)(
    'classifies upstream %s as retryable %s',
    async (status, kind) => {
      const adapter = makeHydraliskVllmAdapter({
        apiKey: Redacted.make('hydralisk-token'),
        baseUrl: 'https://hydralisk.example.test',
        fetchImpl: async () => new Response('{}', { status }),
        id: HYDRALISK_ADAPTER_ID,
      })

      const outcome = await Effect.runPromise(
        Effect.result(adapter.complete(request())),
      )

      expect(outcome._tag).toBe('Failure')
      if (outcome._tag === 'Failure') {
        expect(outcome.failure.httpStatus).toBe(status)
        expect(outcome.failure.kind).toBe(kind)
        expect(outcome.failure.retryable).toBe(true)
      }
    },
  )

  it.each(RETRYABLE_STATUS_CASES)(
    'classifies streaming upstream %s as retryable %s',
    async (status, kind) => {
      const adapter = makeHydraliskVllmAdapter({
        apiKey: Redacted.make('hydralisk-token'),
        baseUrl: 'https://hydralisk.example.test',
        fetchImpl: async () => new Response('{}', { status }),
        id: HYDRALISK_ADAPTER_ID,
      })

      const outcome = await Effect.runPromise(
        Effect.result(adapter.streamSse!(request({ stream: true }))),
      )

      expect(outcome._tag).toBe('Failure')
      if (outcome._tag === 'Failure') {
        expect(outcome.failure.httpStatus).toBe(status)
        expect(outcome.failure.kind).toBe(kind)
        expect(outcome.failure.retryable).toBe(true)
      }
    },
  )

  it('parses streaming SSE deltas and terminal usage', async () => {
    let streamedBody: Record<string, unknown> | undefined
    const encoder = new TextEncoder()
    const sse = [
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{"content":"RE"},"finish_reason":null}]}',
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{"content":"ADY"},"finish_reason":null}]}',
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":1,"total_tokens":8}}',
      'data: [DONE]',
    ].join('\n\n')
    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test',
      fetchImpl: async (_input, init) => {
        streamedBody = JSON.parse(String(init.body)) as Record<string, unknown>
        return new Response(
          new ReadableStream<Uint8Array>({
            start(controller) {
              controller.enqueue(encoder.encode(sse))
              controller.close()
            },
          }),
          { headers: { 'content-type': 'text/event-stream' }, status: 200 },
        )
      },
      id: HYDRALISK_ADAPTER_ID,
    })

    const source = await Effect.runPromise(
      adapter.streamSse!(request({ stream: true })),
    )
    const deltas: Array<string> = []
    for await (const frame of source.frames) {
      if (frame.contentDelta !== '') {
        deltas.push(frame.contentDelta)
      }
    }

    expect(deltas.join('')).toBe('READY')
    expect(streamedBody?.stream).toBe(true)
    expect(streamedBody?.stream_options).toEqual({ include_usage: true })
    expect(source.terminal()).toEqual({
      finishReason: 'stop',
      servedModel: 'openai/gpt-oss-20b',
      usage: { completionTokens: 1, promptTokens: 7, totalTokens: 8 },
    })
  })

  it('preserves streamed tool_call deltas', async () => {
    const encoder = new TextEncoder()
    const sse = [
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_bash","type":"function","function":{"name":"bash"}}]},"finish_reason":null}]}',
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\\"cmd\\":\\"pwd\\"}"}}]},"finish_reason":null}]}',
      'data: {"model":"openai/gpt-oss-20b","choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":7,"completion_tokens":4,"total_tokens":11}}',
      'data: [DONE]',
    ].join('\n\n')
    const adapter = makeHydraliskVllmAdapter({
      apiKey: Redacted.make('hydralisk-token'),
      baseUrl: 'https://hydralisk.example.test',
      fetchImpl: async () =>
        new Response(
          new ReadableStream<Uint8Array>({
            start(controller) {
              controller.enqueue(encoder.encode(sse))
              controller.close()
            },
          }),
          { headers: { 'content-type': 'text/event-stream' }, status: 200 },
        ),
      id: HYDRALISK_ADAPTER_ID,
    })

    const source = await Effect.runPromise(
      adapter.streamSse!(request({ stream: true })),
    )
    const toolCallDeltas: Array<unknown> = []
    for await (const frame of source.frames) {
      if (frame.toolCallDeltas !== undefined) {
        toolCallDeltas.push(...frame.toolCallDeltas)
      }
    }

    expect(toolCallDeltas).toEqual([
      {
        function: { name: 'bash' },
        id: 'call_bash',
        index: 0,
        type: 'function',
      },
      { function: { arguments: '{"cmd":"pwd"}' }, index: 0 },
    ])
    expect(source.terminal()).toMatchObject({
      finishReason: 'tool_calls',
      usage: { completionTokens: 4, promptTokens: 7, totalTokens: 11 },
    })
  })
})
