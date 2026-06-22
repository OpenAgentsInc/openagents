import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  FIREWORKS_ADAPTER_ID,
  FIREWORKS_DEFAULT_BASE_URL,
  type FetchLike,
  type FireworksAdapterConfig,
  makeFireworksAdapter,
} from './fireworks-adapter'
import {
  InferenceAdapterError,
  type InferenceRequest,
} from './provider-adapter'
import { KHALA_CODE_MODEL_ID } from './pricing'

// --- test plumbing -------------------------------------------------------

// Effect 4 exposes failures-as-values via `Effect.result` (a `Result` with
// `_tag: "Success" | "Failure"`); there is no `Effect.either`.
const runResult = <A>(effect: Effect.Effect<A, InferenceAdapterError>) =>
  Effect.runPromise(Effect.result(effect))

// Capture the call the adapter made + return a canned Response.
type Captured = {
  url: string
  init: Parameters<FetchLike>[1]
}

const recordingFetch = (
  response: Response,
): { fetchImpl: FetchLike; calls: Array<Captured> } => {
  const calls: Array<Captured> = []
  const fetchImpl: FetchLike = (url, init) => {
    calls.push({ init, url })
    return Promise.resolve(response)
  }
  return { calls, fetchImpl }
}

const jsonResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'content-type': 'application/json' },
    status,
  })

const errorResponse = (status: number, body = 'rate limited'): Response =>
  new Response(body, { status })

const sseResponse = (frames: ReadonlyArray<unknown>): Response => {
  const text =
    frames
      .map(frame => `data: ${JSON.stringify(frame)}\n\n`)
      .join('') + 'data: [DONE]\n\n'
  return new Response(text, {
    headers: { 'content-type': 'text/event-stream' },
    status: 200,
  })
}

const baseConfig = (
  overrides: Partial<FireworksAdapterConfig> = {},
): FireworksAdapterConfig => ({
  getApiKey: () => 'fw-test-key',
  ...overrides,
})

const request = (
  overrides: Partial<InferenceRequest> = {},
): InferenceRequest => ({
  messages: [{ content: 'hello world', role: 'user' }],
  model: 'deepseek-v4-pro',
  passthroughParams: {},
  stream: false,
  ...overrides,
})

const completionBody = (
  overrides: Record<string, unknown> = {},
): Record<string, unknown> => ({
  choices: [
    {
      finish_reason: 'stop',
      index: 0,
      message: { content: 'hi there', role: 'assistant' },
    },
  ],
  model: 'accounts/fireworks/models/deepseek-v4-pro',
  object: 'chat.completion',
  usage: {
    completion_tokens: 5,
    prompt_tokens: 11,
    total_tokens: 16,
  },
  ...overrides,
})

// --- request mapping -----------------------------------------------------

describe('fireworks adapter request mapping', () => {
  test('posts OpenAI-compatible chat-completions to the Fireworks base URL', async () => {
    const { calls, fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    await runResult(adapter.complete(request()))

    expect(adapter.id).toBe(FIREWORKS_ADAPTER_ID)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(
      `${FIREWORKS_DEFAULT_BASE_URL}/chat/completions`,
    )
    expect(calls[0]?.init.method).toBe('POST')
    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.model).toBe('accounts/fireworks/models/deepseek-v4-pro')
    expect(body.messages).toEqual([
      { content: 'hello world', role: 'user' },
    ])
    expect(body.stream).toBe(false)
  })

  test('sends the bearer key in the Authorization header and never in the body', async () => {
    const { calls, fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(
      baseConfig({ fetchImpl, getApiKey: () => 'fw-secret-123' }),
    )

    await runResult(adapter.complete(request()))

    expect(calls[0]?.init.headers['authorization']).toBe('Bearer fw-secret-123')
    expect(calls[0]?.init.body).not.toContain('fw-secret-123')
  })

  test('forwards passthrough sampling params verbatim', async () => {
    const { calls, fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    await runResult(
      adapter.complete(
        request({
          passthroughParams: { max_tokens: 256, temperature: 0.7, top_p: 0.9 },
        }),
      ),
    )

    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.temperature).toBe(0.7)
    expect(body.top_p).toBe(0.9)
    expect(body.max_tokens).toBe(256)
  })

  test('passes x-session-affinity as a header, not a body field', async () => {
    const { calls, fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    await runResult(
      adapter.complete(
        request({ passthroughParams: { 'x-session-affinity': 'sess-42' } }),
      ),
    )

    expect(calls[0]?.init.headers['x-session-affinity']).toBe('sess-42')
    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body['x-session-affinity']).toBeUndefined()
  })

  test('preserves fully-qualified Fireworks model ids unchanged', async () => {
    const { calls, fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    await runResult(
      adapter.complete(
        request({ model: 'accounts/fireworks/models/glm-5p2' }),
      ),
    )

    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.model).toBe('accounts/fireworks/models/glm-5p2')
  })

  test('maps the Khala code virtual model to its Fireworks backing model', async () => {
    const { calls, fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    await runResult(adapter.complete(request({ model: KHALA_CODE_MODEL_ID })))

    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.model).toBe('accounts/fireworks/models/kimi-k2p7-code')
  })

  test('respects a custom base URL', async () => {
    const { calls, fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(
      baseConfig({ baseUrl: 'https://example.test/v1', fetchImpl }),
    )

    await runResult(adapter.complete(request()))

    expect(calls[0]?.url).toBe('https://example.test/v1/chat/completions')
  })
})

// --- usage extraction (receipt-first) ------------------------------------

describe('fireworks adapter usage extraction', () => {
  test('returns receipt-first usage from the response usage object', async () => {
    const { fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.content).toBe('hi there')
      expect(result.success.finishReason).toBe('stop')
      expect(result.success.servedModel).toBe(
        'accounts/fireworks/models/deepseek-v4-pro',
      )
      expect(result.success.usage).toEqual({
        completionTokens: 5,
        promptTokens: 11,
        totalTokens: 16,
      })
    }
  })

  test('surfaces the cached-input dimension when Fireworks reports it', async () => {
    const { fetchImpl } = recordingFetch(
      jsonResponse(
        completionBody({
          usage: {
            completion_tokens: 5,
            prompt_tokens: 100,
            prompt_tokens_details: { cached_tokens: 40 },
            total_tokens: 105,
          },
        }),
      ),
    )
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      expect(result.success.usage.cachedPromptTokens).toBe(40)
    }
  })

  test('fails typed (non-retryable) when the response omits usage', async () => {
    const noUsage = completionBody()
    delete noUsage['usage']
    const { fetchImpl } = recordingFetch(jsonResponse(noUsage))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure).toBeInstanceOf(InferenceAdapterError)
      expect(result.failure.kind).toBe('malformed_response')
      expect(result.failure.retryable).toBe(false)
    }
  })
})

// --- typed error mapping (429 / 503 / other) -----------------------------

describe('fireworks adapter typed errors', () => {
  test('429 maps to a retryable rate_limited error with the http status', async () => {
    const { fetchImpl } = recordingFetch(errorResponse(429, 'slow down'))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.adapterId).toBe(FIREWORKS_ADAPTER_ID)
      expect(result.failure.kind).toBe('rate_limited')
      expect(result.failure.retryable).toBe(true)
      expect(result.failure.httpStatus).toBe(429)
      expect(result.failure.reason).toContain('429')
    }
  })

  test('503 maps to a retryable service_overloaded error', async () => {
    const { fetchImpl } = recordingFetch(errorResponse(503, 'overloaded'))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('service_overloaded')
      expect(result.failure.retryable).toBe(true)
      expect(result.failure.httpStatus).toBe(503)
    }
  })

  test('500 maps to a retryable upstream_error', async () => {
    const { fetchImpl } = recordingFetch(errorResponse(500, 'boom'))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('upstream_error')
      expect(result.failure.retryable).toBe(true)
    }
  })

  test('400 maps to a non-retryable request_rejected error', async () => {
    const { fetchImpl } = recordingFetch(errorResponse(400, 'bad model'))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('request_rejected')
      expect(result.failure.retryable).toBe(false)
      expect(result.failure.httpStatus).toBe(400)
    }
  })

  test('a transport throw maps to a retryable transport_error', async () => {
    const fetchImpl: FetchLike = () =>
      Promise.reject(new Error('socket hang up'))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('transport_error')
      expect(result.failure.retryable).toBe(true)
      expect(result.failure.httpStatus).toBeUndefined()
    }
  })

  test('a missing key fails non-retryable without leaking key material', async () => {
    const adapter = makeFireworksAdapter(baseConfig({ getApiKey: () => undefined }))

    const result = await runResult(adapter.complete(request()))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('configuration_error')
      expect(result.failure.retryable).toBe(false)
      expect(result.failure.reason).not.toContain('Bearer')
    }
  })
})

// --- streaming -----------------------------------------------------------

describe('fireworks adapter streaming', () => {
  test('requests stream mode and parses SSE deltas + terminal usage', async () => {
    const { calls, fetchImpl } = recordingFetch(
      sseResponse([
        {
          choices: [{ delta: { content: 'Hel' }, index: 0 }],
        },
        {
          choices: [{ delta: { content: 'lo' }, index: 0 }],
        },
        {
          choices: [{ delta: {}, finish_reason: 'stop', index: 0 }],
          usage: {
            completion_tokens: 2,
            prompt_tokens: 7,
            total_tokens: 9,
          },
        },
      ]),
    )
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.stream(request({ stream: true })))

    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.stream).toBe(true)
    expect(calls[0]?.init.headers['accept']).toBe('text/event-stream')

    expect(result._tag).toBe('Success')
    if (result._tag === 'Success') {
      const chunks = result.success
      const content = chunks
        .map(chunk => chunk.contentDelta)
        .join('')
      expect(content).toBe('Hello')
      const terminal = chunks[chunks.length - 1]
      expect(terminal?.finishReason).toBe('stop')
      expect(terminal?.usage).toEqual({
        completionTokens: 2,
        promptTokens: 7,
        totalTokens: 9,
      })
    }
  })

  test('streaming 429 maps to a retryable rate_limited error', async () => {
    const { fetchImpl } = recordingFetch(errorResponse(429))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.stream(request({ stream: true })))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('rate_limited')
      expect(result.failure.retryable).toBe(true)
    }
  })

  test('a stream with no terminal usage fails typed (never an estimate)', async () => {
    const { fetchImpl } = recordingFetch(
      sseResponse([
        { choices: [{ delta: { content: 'partial' }, index: 0 }] },
      ]),
    )
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.stream(request({ stream: true })))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('malformed_response')
      expect(result.failure.reason).toContain('usage')
    }
  })

  // RECEIPT-FIRST STREAMING (the "missing terminal usage frame" 524 fix):
  // OpenAI-compatible providers omit the usage object from streamed responses
  // unless asked. The adapter MUST opt in via stream_options.include_usage so a
  // normal streamed completion carries its real terminal usage frame, instead of
  // failing receipt-first with "missing terminal usage frame".
  test('streaming requests opt in to a terminal usage frame via stream_options.include_usage', async () => {
    const { calls, fetchImpl } = recordingFetch(
      sseResponse([
        { choices: [{ delta: { content: 'ok' }, index: 0 }] },
        {
          choices: [{ delta: {}, finish_reason: 'stop', index: 0 }],
          usage: { completion_tokens: 1, prompt_tokens: 3, total_tokens: 4 },
        },
      ]),
    )
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    await runResult(adapter.stream(request({ stream: true })))

    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.stream).toBe(true)
    expect(body.stream_options).toEqual({ include_usage: true })
  })

  test('non-streaming requests do NOT send stream_options', async () => {
    const { calls, fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    await runResult(adapter.complete(request()))

    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.stream_options).toBeUndefined()
  })

  test('a load-bearing stream_options override beats a stray passthrough copy', async () => {
    const { calls, fetchImpl } = recordingFetch(
      sseResponse([
        {
          choices: [{ delta: {}, finish_reason: 'stop', index: 0 }],
          usage: { completion_tokens: 1, prompt_tokens: 3, total_tokens: 4 },
        },
      ]),
    )
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    await runResult(
      adapter.stream(
        request({
          passthroughParams: { stream_options: { include_usage: false } },
          stream: true,
        }),
      ),
    )

    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.stream_options).toEqual({ include_usage: true })
  })
})

// --- incremental pass-through stream (streamSse) -------------------------

// Build an SSE Response over a ReadableStream that emits each frame in its own
// chunk, so the test exercises the adapter's incremental parsing (partial lines
// across reads) rather than a single buffered blob.
const chunkedSseResponse = (
  frames: ReadonlyArray<unknown>,
): Response => {
  const encoder = new TextEncoder()
  const lines = [
    ...frames.map(frame => `data: ${JSON.stringify(frame)}\n\n`),
    'data: [DONE]\n\n',
  ]
  let index = 0
  const stream = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (index >= lines.length) {
        controller.close()
        return
      }
      // Emit each line split across two chunks to prove partial-line buffering.
      const line = lines[index]!
      const mid = Math.floor(line.length / 2)
      controller.enqueue(encoder.encode(line.slice(0, mid)))
      controller.enqueue(encoder.encode(line.slice(mid)))
      index += 1
    },
  })
  return new Response(stream, {
    headers: { 'content-type': 'text/event-stream' },
    status: 200,
  })
}

const drainSource = async (
  adapter: ReturnType<typeof makeFireworksAdapter>,
  req: InferenceRequest,
) => {
  const sourceResult = await runResult(adapter.streamSse!(req))
  if (sourceResult._tag !== 'Success') {
    return { failure: sourceResult, ok: false as const }
  }
  const source = sourceResult.success
  const deltas: Array<string> = []
  for await (const event of source.frames) {
    if (event.contentDelta !== '') {
      deltas.push(event.contentDelta)
    }
  }
  return { ok: true as const, source, deltas }
}

describe('fireworks adapter incremental pass-through stream', () => {
  test('exposes streamSse', () => {
    const { fetchImpl } = recordingFetch(jsonResponse(completionBody()))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))
    expect(typeof adapter.streamSse).toBe('function')
  })

  test('yields content deltas incrementally and captures the terminal usage frame', async () => {
    const { calls, fetchImpl } = recordingFetch(
      chunkedSseResponse([
        { choices: [{ delta: { content: 'Hel' }, index: 0 }] },
        { choices: [{ delta: { content: 'lo' }, index: 0 }] },
        {
          choices: [{ delta: {}, finish_reason: 'stop', index: 0 }],
          model: 'accounts/fireworks/models/kimi-k2p7-code',
          usage: { completion_tokens: 2, prompt_tokens: 7, total_tokens: 9 },
        },
      ]),
    )
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const drained = await drainSource(adapter, request({ stream: true }))
    expect(drained.ok).toBe(true)
    if (!drained.ok) {
      return
    }
    expect(drained.deltas).toEqual(['Hel', 'lo'])
    expect(calls[0]?.init.headers['accept']).toBe('text/event-stream')
    const body = JSON.parse(calls[0]?.init.body ?? '{}')
    expect(body.stream_options).toEqual({ include_usage: true })

    const terminal = drained.source.terminal()
    expect(terminal.finishReason).toBe('stop')
    expect(terminal.usage).toEqual({
      completionTokens: 2,
      promptTokens: 7,
      totalTokens: 9,
    })
    expect(terminal.servedModel).toBe('accounts/fireworks/models/kimi-k2p7-code')
  })

  // The missing-terminal-frame case for the pass-through path: the source still
  // drains (the client got partial content), and terminal usage is undefined so
  // the ROUTE settles no metering (receipt-first — never an estimate). The
  // adapter does not throw; metering policy lives at the route.
  test('a stream with no terminal usage drains with undefined terminal usage', async () => {
    const { fetchImpl } = recordingFetch(
      chunkedSseResponse([
        { choices: [{ delta: { content: 'partial' }, index: 0 }] },
      ]),
    )
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const drained = await drainSource(adapter, request({ stream: true }))
    expect(drained.ok).toBe(true)
    if (!drained.ok) {
      return
    }
    expect(drained.deltas).toEqual(['partial'])
    expect(drained.source.terminal().usage).toBeUndefined()
  })

  test('streamSse connect-time 429 maps to a retryable rate_limited error', async () => {
    const { fetchImpl } = recordingFetch(errorResponse(429))
    const adapter = makeFireworksAdapter(baseConfig({ fetchImpl }))

    const result = await runResult(adapter.streamSse!(request({ stream: true })))

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(result.failure.kind).toBe('rate_limited')
      expect(result.failure.retryable).toBe(true)
    }
  })
})
