import { Effect, Redacted } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  OPENROUTER_KHALA_FALLBACK_MODEL_ID,
  type OpenRouterAdapterConfig,
  type OpenRouterFetch,
  makeOpenRouterAdapter,
} from './openrouter-adapter'
import type { InferenceRequest, InferenceStreamEvent } from './provider-adapter'

const runResult = <A>(effect: Effect.Effect<A, unknown>) =>
  Effect.runPromise(Effect.result(effect))

const request = (
  overrides: Partial<InferenceRequest> = {},
): InferenceRequest => ({
  messages: [{ content: 'call the tool', role: 'user' }],
  model: 'openagents/khala',
  passthroughParams: {
    tool_choice: {
      function: { name: 'get_time' },
      type: 'function',
    },
    tools: [
      {
        function: {
          name: 'get_time',
          parameters: { type: 'object' },
        },
        type: 'function',
      },
    ],
  },
  stream: false,
  ...overrides,
})

const jsonResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    status,
  })

const sseResponse = (body: string, status = 200): Response =>
  new Response(body, {
    headers: { 'content-type': 'text/event-stream; charset=utf-8' },
    status,
  })

const completionBody = (
  overrides: Record<string, unknown> = {},
): Record<string, unknown> => ({
  choices: [
    {
      finish_reason: 'tool_calls',
      index: 0,
      message: {
        content: '',
        role: 'assistant',
        tool_calls: [
          {
            function: {
              arguments: '{"timezone":"UTC"}',
              name: 'get_time',
            },
            id: 'call_1',
            type: 'function',
          },
        ],
      },
    },
  ],
  model: 'openrouter/glm-class-fixture',
  object: 'chat.completion',
  usage: {
    completion_tokens: 4,
    prompt_tokens: 9,
    total_tokens: 13,
  },
  ...overrides,
})

const adapterConfig = (
  fetchImpl: OpenRouterFetch,
): OpenRouterAdapterConfig => ({
  apiKey: Redacted.make('test-openrouter-key'),
  baseUrl: 'https://openrouter.example.test/api/v1',
  fetchImpl,
  id: 'openrouter-khala-glm-fallback',
  upstreamModel: OPENROUTER_KHALA_FALLBACK_MODEL_ID,
})

describe('OpenRouter Khala fallback adapter', () => {
  it('preserves tool calls in non-streaming completions', async () => {
    const fetchImpl: OpenRouterFetch = async () => jsonResponse(completionBody())
    const adapter = makeOpenRouterAdapter(adapterConfig(fetchImpl))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Success')
    if (result._tag !== 'Success') return
    expect(result.success.toolCalls).toEqual([
      {
        function: {
          arguments: '{"timezone":"UTC"}',
          name: 'get_time',
        },
        id: 'call_1',
        type: 'function',
      },
    ])
    expect(result.success.finishReason).toBe('tool_calls')
    expect(result.success.usage.totalTokens).toBe(13)
  })

  it('maps buffered tool calls into stream deltas', async () => {
    const fetchImpl: OpenRouterFetch = async () => jsonResponse(completionBody())
    const adapter = makeOpenRouterAdapter(adapterConfig(fetchImpl))

    const result = await runResult(adapter.stream(request({ stream: true })))

    expect(result._tag).toBe('Success')
    if (result._tag !== 'Success') return
    expect(result.success[0]?.toolCallDeltas).toEqual([
      {
        function: {
          arguments: '{"timezone":"UTC"}',
          name: 'get_time',
        },
        id: 'call_1',
        index: 0,
        type: 'function',
      },
    ])
    expect(result.success.at(-1)?.finishReason).toBe('tool_calls')
    expect(result.success.at(-1)?.usage?.totalTokens).toBe(13)
  })

  it('sends the pinned OpenRouter Granite model upstream', async () => {
    let capturedBody: Record<string, unknown> | undefined
    const fetchImpl: OpenRouterFetch = async (_input, init) => {
      capturedBody = JSON.parse(init.body) as Record<string, unknown>
      return jsonResponse(completionBody())
    }
    const adapter = makeOpenRouterAdapter(adapterConfig(fetchImpl))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Success')
    expect(capturedBody?.model).toBe(OPENROUTER_KHALA_FALLBACK_MODEL_ID)
  })

  it('opens a true SSE stream with app attribution and usage opt-in', async () => {
    let capturedBody: Record<string, unknown> | undefined
    let capturedHeaders: Record<string, string> | undefined
    const fetchImpl: OpenRouterFetch = async (_input, init) => {
      capturedBody = JSON.parse(init.body) as Record<string, unknown>
      capturedHeaders = init.headers
      return sseResponse(
        [
          'data: {"model":"openrouter/granite","choices":[{"delta":{"content":"Hel"},"finish_reason":null}]}',
          'data: {"model":"openrouter/granite","choices":[{"delta":{"content":"lo"},"finish_reason":null}]}',
          'data: {"model":"openrouter/granite","choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}',
          'data: [DONE]',
          '',
        ].join('\n\n'),
      )
    }
    const adapter = makeOpenRouterAdapter(adapterConfig(fetchImpl))

    const result = await runResult(adapter.streamSse!(request({ stream: true })))

    expect(result._tag).toBe('Success')
    if (result._tag !== 'Success') return
    const frames: InferenceStreamEvent[] = []
    for await (const frame of result.success.frames) {
      frames.push(frame)
    }
    expect(frames.map(frame => frame.contentDelta)).toEqual(['Hel', 'lo', ''])
    expect(result.success.terminal()).toEqual({
      finishReason: 'stop',
      servedModel: 'openrouter/granite',
      usage: { completionTokens: 2, promptTokens: 3, totalTokens: 5 },
    })
    expect(capturedBody?.stream).toBe(true)
    expect(capturedBody?.stream_options).toEqual({ include_usage: true })
    expect(capturedHeaders?.accept).toBe('text/event-stream')
    expect(capturedHeaders?.['HTTP-Referer']).toBe('https://openagents.com')
    expect(capturedHeaders?.['X-OpenRouter-Title']).toBe('Khala Code')
  })

  it('uses a caller-supplied OpenRouter key instead of the configured fallback key', async () => {
    let capturedAuthorization: string | undefined
    const fetchImpl: OpenRouterFetch = async (_input, init) => {
      capturedAuthorization = init.headers.authorization
      return jsonResponse(completionBody())
    }
    const adapter = makeOpenRouterAdapter(adapterConfig(fetchImpl))

    const result = await runResult(
      adapter.complete(
        request({
          callerProviderKey: {
            apiKey: Redacted.make('sk-or-caller-owned'),
            provider: 'openrouter',
          },
        }),
      ),
    )

    expect(result._tag).toBe('Success')
    expect(capturedAuthorization).toBe('Bearer sk-or-caller-owned')
  })

  // 2026-07-08 incident: OpenRouter platform credits ran out, every request on
  // the lane failed 402, and because 402 was classified non-retryable the
  // dispatch chain never overflowed to Vertex Gemini / Fireworks — the whole
  // gateway surfaced 502. A platform-key 402 must be a retryable LANE failure.
  it('classifies a platform-key 402 as retryable so dispatch overflows the lane', async () => {
    const fetchImpl: OpenRouterFetch = async () =>
      jsonResponse({ error: { code: 402, message: 'Insufficient credits' } }, 402)
    const adapter = makeOpenRouterAdapter(adapterConfig(fetchImpl))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Failure')
    if (result._tag !== 'Failure') return
    const error = result.failure as {
      httpStatus?: number
      kind?: string
      retryable?: boolean
    }
    expect(error.httpStatus).toBe(402)
    expect(error.kind).toBe('quota_exhausted')
    expect(error.retryable).toBe(true)
  })

  it('classifies a platform-key 402 on the stream path as retryable', async () => {
    const fetchImpl: OpenRouterFetch = async () =>
      jsonResponse({ error: { code: 402, message: 'Insufficient credits' } }, 402)
    const adapter = makeOpenRouterAdapter(adapterConfig(fetchImpl))

    const result = await runResult(
      adapter.streamSse(request({ stream: true })),
    )

    expect(result._tag).toBe('Failure')
    if (result._tag !== 'Failure') return
    const error = result.failure as {
      httpStatus?: number
      kind?: string
      retryable?: boolean
    }
    expect(error.httpStatus).toBe(402)
    expect(error.kind).toBe('quota_exhausted')
    expect(error.retryable).toBe(true)
  })

  it('keeps a BYOK caller-key 402 non-retryable (must surface to the caller)', async () => {
    const fetchImpl: OpenRouterFetch = async () =>
      jsonResponse({ error: { code: 402, message: 'Insufficient credits' } }, 402)
    const adapter = makeOpenRouterAdapter(adapterConfig(fetchImpl))

    const result = await runResult(
      adapter.complete(
        request({
          callerProviderKey: {
            apiKey: Redacted.make('sk-or-caller-owned'),
            provider: 'openrouter',
          },
        }),
      ),
    )

    expect(result._tag).toBe('Failure')
    if (result._tag !== 'Failure') return
    const error = result.failure as {
      httpStatus?: number
      kind?: string
      retryable?: boolean
    }
    expect(error.httpStatus).toBe(402)
    expect(error.kind).toBe('request_rejected')
    expect(error.retryable).toBe(false)
  })
})
