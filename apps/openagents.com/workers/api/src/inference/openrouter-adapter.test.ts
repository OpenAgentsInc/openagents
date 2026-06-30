import { Effect, Redacted } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  OPENROUTER_KHALA_FALLBACK_MODEL_ID,
  type OpenRouterAdapterConfig,
  type OpenRouterFetch,
  makeOpenRouterAdapter,
} from './openrouter-adapter'
import type { InferenceRequest } from './provider-adapter'

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
})
