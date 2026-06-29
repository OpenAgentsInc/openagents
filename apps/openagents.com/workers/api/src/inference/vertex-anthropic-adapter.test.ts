import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type InferenceRequest,
  InferenceAdapterError,
} from './provider-adapter'
import {
  VERTEX_ANTHROPIC_ADAPTER_ID,
  makeVertexAnthropicAdapter,
} from './vertex-anthropic-adapter'

const run = <A>(effect: Effect.Effect<A, InferenceAdapterError>): Promise<A> =>
  Effect.runPromise(effect)

// Run an effect expected to fail and return its typed error (flip success<->fail).
const runError = <A>(
  effect: Effect.Effect<A, InferenceAdapterError>,
): Promise<InferenceAdapterError> => Effect.runPromise(Effect.flip(effect))

const baseRequest = (
  overrides: Partial<InferenceRequest> = {},
): InferenceRequest => ({
  messages: [{ content: 'Hey Claude!', role: 'user' }],
  model: 'claude-opus-4-8',
  passthroughParams: {},
  stream: false,
  ...overrides,
})

// A mock fetch that records the URL + parsed body and returns a canned Response.
const recordingFetch = (
  response: Response,
): { fetchImpl: typeof fetch; calls: Array<{ url: string; init: RequestInit }> } => {
  const calls: Array<{ url: string; init: RequestInit }> = []
  const fetchImpl = (async (url: unknown, init?: RequestInit) => {
    calls.push({ init: init ?? {}, url: String(url) })
    return response
  }) as unknown as typeof fetch
  return { calls, fetchImpl }
}

const okJson = (body: unknown): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    status: 200,
  })

const messagesResponse = {
  content: [
    { text: 'Hello ', type: 'text' },
    { text: 'there', type: 'text' },
    { input: {}, name: 'noop', type: 'tool_use' },
  ],
  model: 'claude-opus-4-8',
  stop_reason: 'end_turn',
  usage: {
    cache_read_input_tokens: 4,
    input_tokens: 12,
    output_tokens: 7,
  },
}

const fixedToken = () => Effect.succeed('test-access-token')

describe('vertex anthropic adapter — complete', () => {
  test('maps the request to the Vertex rawPredict endpoint and Anthropic body', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(messagesResponse))
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      location: 'global',
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(
      adapter.complete(
        baseRequest({ passthroughParams: { max_tokens: 256, temperature: 0.5 } }),
      ),
    )

    expect(calls).toHaveLength(1)
    const call = calls[0]!
    expect(call.url).toBe(
      'https://aiplatform.googleapis.com/v1/projects/openagentsgemini' +
        '/locations/global/publishers/anthropic/models/claude-opus-4-8:rawPredict',
    )
    expect((call.init.headers as Record<string, string>).authorization).toBe(
      'Bearer test-access-token',
    )
    const body = JSON.parse(call.init.body as string) as Record<string, unknown>
    expect(body['anthropic_version']).toBe('vertex-2023-10-16')
    expect(body['model']).toBeUndefined() // model is in the path, never the body
    expect(body['max_tokens']).toBe(256)
    expect(body['temperature']).toBe(0.5)
    expect(body['stream']).toBe(false)
    expect(body['messages']).toEqual([{ content: 'Hey Claude!', role: 'user' }])
  })

  test('regional location uses the {location}-aiplatform host', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(messagesResponse))
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      location: 'us-east5',
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })
    await run(adapter.complete(baseRequest()))
    expect(calls[0]!.url).toBe(
      'https://us-east5-aiplatform.googleapis.com/v1/projects/openagentsgemini' +
        '/locations/us-east5/publishers/anthropic/models/claude-opus-4-8:rawPredict',
    )
  })

  test('applies the default max_tokens when none is supplied', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(messagesResponse))
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })
    await run(adapter.complete(baseRequest()))
    const body = JSON.parse(calls[0]!.init.body as string) as Record<string, unknown>
    expect(body['max_tokens']).toBe(1024)
  })

  test('strips a vertex/ or anthropic/ prefix from the model id', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(messagesResponse))
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })
    await run(adapter.complete(baseRequest({ model: 'vertex/claude-sonnet-4-6' })))
    expect(calls[0]!.url).toContain('/models/claude-sonnet-4-6:rawPredict')
  })

  test('extracts receipt-first usage and text from the response', async () => {
    const { fetchImpl } = recordingFetch(okJson(messagesResponse))
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })
    const result = await run(adapter.complete(baseRequest()))
    expect(result.content).toBe('Hello there')
    expect(result.finishReason).toBe('end_turn')
    expect(result.servedModel).toBe('claude-opus-4-8')
    expect(result.usage.promptTokens).toBe(12)
    expect(result.usage.completionTokens).toBe(7)
    expect(result.usage.totalTokens).toBe(19)
    expect(result.usage.cachedPromptTokens).toBe(4)
  })

  test('maps 429 to a retryable adapter error', async () => {
    const { fetchImpl } = recordingFetch(
      new Response('rate limited', { status: 429 }),
    )
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })
    const error = await runError(adapter.complete(baseRequest()))
    expect(error).toBeInstanceOf(InferenceAdapterError)
    expect(error.retryable).toBe(true)
    expect(error.adapterId).toBe(VERTEX_ANTHROPIC_ADAPTER_ID)
    expect(error.reason).toContain('HTTP 429')
  })

  test('maps a 400 to a non-retryable adapter error', async () => {
    const { fetchImpl } = recordingFetch(
      new Response('bad request', { status: 400 }),
    )
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })
    const error = await runError(adapter.complete(baseRequest()))
    expect(error.retryable).toBe(false)
  })

  test('maps a 503 to a retryable adapter error', async () => {
    const { fetchImpl } = recordingFetch(
      new Response('unavailable', { status: 503 }),
    )
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })
    const error = await runError(adapter.complete(baseRequest()))
    expect(error.retryable).toBe(true)
  })

  test('is unconfigured (non-retryable error) without a token provider', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(messagesResponse))
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: undefined,
    })
    const error = await runError(adapter.complete(baseRequest()))
    expect(error.retryable).toBe(false)
    expect(error.reason).toContain('not configured')
    // No network call when unconfigured.
    expect(calls).toHaveLength(0)
  })
})

describe('vertex anthropic adapter — stream', () => {
  const sseBody = [
    'event: message_start',
    'data: {"type":"message_start","message":{"usage":{"input_tokens":11,"cache_read_input_tokens":2,"output_tokens":0}}}',
    '',
    'event: content_block_delta',
    'data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello "}}',
    '',
    'event: content_block_delta',
    'data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"world"}}',
    '',
    'event: message_delta',
    'data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":5}}',
    '',
    'event: message_stop',
    'data: {"type":"message_stop"}',
    '',
  ].join('\n')

  test('uses streamRawPredict and parses SSE into content + terminal usage', async () => {
    const { calls, fetchImpl } = recordingFetch(
      new Response(sseBody, {
        headers: { 'content-type': 'text/event-stream' },
        status: 200,
      }),
    )
    const adapter = makeVertexAnthropicAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })
    const chunks = await run(adapter.stream(baseRequest({ stream: true })))

    expect(calls[0]!.url).toContain(':streamRawPredict')
    const body = JSON.parse(calls[0]!.init.body as string) as Record<string, unknown>
    expect(body['stream']).toBe(true)

    // First chunk carries the joined content delta; terminal chunk carries usage.
    expect(chunks[0]?.contentDelta).toBe('Hello world')
    const terminal = chunks[chunks.length - 1]!
    expect(terminal.finishReason).toBe('end_turn')
    expect(terminal.usage?.promptTokens).toBe(11)
    expect(terminal.usage?.completionTokens).toBe(5)
    expect(terminal.usage?.totalTokens).toBe(16)
    expect(terminal.usage?.cachedPromptTokens).toBe(2)
  })
})
