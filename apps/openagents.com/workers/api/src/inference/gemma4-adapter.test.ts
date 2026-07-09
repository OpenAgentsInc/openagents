import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  DEFAULT_GEMMA4_MODEL_ID,
  GEMMA4_ADAPTER_ID,
  GEMMA4_DEFAULT_MIN_OUTPUT_TOKENS,
  makeGemma4Adapter,
} from './gemma4-adapter'
import {
  InferenceAdapterError,
  type InferenceRequest,
  type InferenceStreamEvent,
} from './provider-adapter'

const run = <A>(effect: Effect.Effect<A, InferenceAdapterError>): Promise<A> =>
  Effect.runPromise(effect)

const runToResult = <A>(
  effect: Effect.Effect<A, InferenceAdapterError>,
): Promise<
  { ok: true; value: A } | { ok: false; error: InferenceAdapterError }
> =>
  Effect.runPromise(
    effect.pipe(
      Effect.map(value => ({ ok: true as const, value })),
      Effect.catch(error => Effect.succeed({ error, ok: false as const })),
    ),
  )

const baseRequest = (
  overrides: Partial<InferenceRequest> = {},
): InferenceRequest => ({
  messages: [{ content: 'Hello Gemma!', role: 'user' }],
  model: 'openagents/khala',
  passthroughParams: {},
  stream: false,
  ...overrides,
})

const recordingFetch = (
  response: Response,
): {
  fetchImpl: typeof fetch
  calls: Array<{ url: string; init: RequestInit }>
} => {
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

// A Gemma 4 generateContent response: a thought scratchpad part flagged
// `thought: true` BEFORE the visible answer part, plus a thoughts token count.
const gemmaThinkingResponse = {
  candidates: [
    {
      content: {
        parts: [
          { text: 'Let me reason about this…', thought: true },
          { text: 'Hello from Gemma' },
        ],
      },
      finishReason: 'STOP',
    },
  ],
  modelVersion: DEFAULT_GEMMA4_MODEL_ID,
  usageMetadata: {
    candidatesTokenCount: 4,
    promptTokenCount: 8,
    thoughtsTokenCount: 5,
    totalTokenCount: 17,
  },
}

const armedConfig = (fetchImpl: typeof fetch) =>
  ({
    apiKey: () => Redacted.make('test-gemini-key'),
    fetchImpl: fetchImpl as never,
  }) as const

describe('gemma4 adapter — request mapping', () => {
  test('targets the Generative Language API generateContent endpoint with the key in the query string', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    await run(adapter.complete(baseRequest()))

    expect(calls).toHaveLength(1)
    const call = calls[0]!
    expect(call.url).toBe(
      'https://generativelanguage.googleapis.com/v1beta/models/' +
        `${DEFAULT_GEMMA4_MODEL_ID}:generateContent?key=test-gemini-key`,
    )
    // Body carries contents + generationConfig; the model is the path segment.
    const body = JSON.parse(call.init.body as string) as Record<string, unknown>
    expect(body['model']).toBeUndefined()
    expect(body['contents']).toBeDefined()
  })

  test('hoists system messages into systemInstruction and maps roles', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    await run(
      adapter.complete(
        baseRequest({
          messages: [
            { content: 'You are helpful.', role: 'system' },
            { content: 'Hi', role: 'user' },
            { content: 'Hello', role: 'assistant' },
          ],
        }),
      ),
    )

    const body = JSON.parse(calls[0]!.init.body as string) as Record<
      string,
      unknown
    >
    expect(body['systemInstruction']).toEqual({
      parts: [{ text: 'You are helpful.' }],
    })
    const contents = body['contents'] as Array<Record<string, unknown>>
    expect(contents.map(content => content['role'])).toEqual(['user', 'model'])
  })

  test('floors a tiny max_tokens budget to the min-output floor so the canary emits text', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    // The canary sends max_tokens=8 — a thinking model would spend all 8 on
    // thoughts and emit no visible text, so dispatch would overflow off the
    // primary. The floor keeps headroom for a visible answer.
    await run(
      adapter.complete(baseRequest({ passthroughParams: { max_tokens: 8 } })),
    )

    const body = JSON.parse(calls[0]!.init.body as string) as Record<
      string,
      unknown
    >
    expect(body['generationConfig']).toMatchObject({
      maxOutputTokens: GEMMA4_DEFAULT_MIN_OUTPUT_TOKENS,
    })
  })

  test('does not floor a max_tokens budget already above the floor', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    await run(
      adapter.complete(baseRequest({ passthroughParams: { max_tokens: 4096 } })),
    )

    const body = JSON.parse(calls[0]!.init.body as string) as Record<
      string,
      unknown
    >
    expect(body['generationConfig']).toMatchObject({ maxOutputTokens: 4096 })
  })
})

describe('gemma4 adapter — thought filtering + exact token mapping', () => {
  test('drops thought parts from visible content and maps thoughtsTokenCount to reasoningTokens', async () => {
    const { fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    const result = await run(adapter.complete(baseRequest()))

    // The `thought: true` scratchpad is filtered out of user-visible content.
    expect(result.content).toBe('Hello from Gemma')
    expect(result.content).not.toContain('reason about this')
    expect(result.finishReason).toBe('STOP')
    expect(result.servedModel).toBe(DEFAULT_GEMMA4_MODEL_ID)
    // Exact usage: reasoningTokens mapped verbatim from thoughtsTokenCount; the
    // provider total (which already includes thoughts) is trusted.
    expect(result.usage).toEqual({
      completionTokens: 4,
      promptTokens: 8,
      reasoningTokens: 5,
      totalTokens: 17,
    })
  })

  test('omits reasoningTokens when the provider reports none', async () => {
    const { fetchImpl } = recordingFetch(
      okJson({
        candidates: [
          { content: { parts: [{ text: 'plain' }] }, finishReason: 'STOP' },
        ],
        modelVersion: DEFAULT_GEMMA4_MODEL_ID,
        usageMetadata: {
          candidatesTokenCount: 1,
          promptTokenCount: 2,
          totalTokenCount: 3,
        },
      }),
    )
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    const result = await run(adapter.complete(baseRequest()))
    expect(result.usage.reasoningTokens).toBeUndefined()
    expect(result.usage.totalTokens).toBe(3)
  })
})

describe('gemma4 adapter — NO-TOOLS guard', () => {
  const tools = [
    {
      function: { name: 'read_file', parameters: { type: 'object' } },
      type: 'function',
    },
  ]

  test('refuses a request with declared tools RETRYABLY (so dispatch overflows to a tool-capable lane) without calling out', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    const outcome = await runToResult(
      adapter.complete(baseRequest({ passthroughParams: { tools } })),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.error.retryable).toBe(true)
      expect(outcome.error.kind).toBe('tool_calls_unsupported')
    }
    // The upstream Gemma endpoint was NEVER called for a tool-bearing request.
    expect(calls).toHaveLength(0)
  })

  test('refuses a request carrying prior tool-call / tool-result messages retryably', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    const outcome = await runToResult(
      adapter.complete(
        baseRequest({
          messages: [
            { content: 'read', role: 'user' },
            {
              content: 'done',
              name: 'read_file',
              role: 'tool',
              toolCallId: 'call_1',
            },
          ],
        }),
      ),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.error.retryable).toBe(true)
    }
    expect(calls).toHaveLength(0)
  })
})

describe('gemma4 adapter — failure classification', () => {
  const statusCase = async (
    status: number,
  ): Promise<{ retryable: boolean; kind: string | undefined }> => {
    const { fetchImpl } = recordingFetch(new Response('nope', { status }))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))
    const outcome = await runToResult(adapter.complete(baseRequest()))
    if (outcome.ok) {
      throw new Error('expected failure')
    }
    return { kind: outcome.error.kind, retryable: outcome.error.retryable }
  }

  test('classifies 402 as retryable quota_exhausted (lane unviable, overflow)', async () => {
    expect(await statusCase(402)).toEqual({
      kind: 'quota_exhausted',
      retryable: true,
    })
  })

  test('classifies 429 as retryable rate_limited', async () => {
    expect(await statusCase(429)).toEqual({
      kind: 'rate_limited',
      retryable: true,
    })
  })

  test('classifies 503 as retryable service_overloaded and 500 as upstream_error', async () => {
    expect(await statusCase(503)).toEqual({
      kind: 'service_overloaded',
      retryable: true,
    })
    expect(await statusCase(500)).toEqual({
      kind: 'upstream_error',
      retryable: true,
    })
  })

  test('classifies a 400 request rejection as non-retryable', async () => {
    expect(await statusCase(400)).toEqual({
      kind: 'request_rejected',
      retryable: false,
    })
  })

  test('never leaks the key-bearing URL in a surfaced error', async () => {
    const { fetchImpl } = recordingFetch(new Response('bad', { status: 400 }))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))
    const outcome = await runToResult(adapter.complete(baseRequest()))
    if (outcome.ok) {
      throw new Error('expected failure')
    }
    expect(outcome.error.reason).not.toContain('test-gemini-key')
    expect(outcome.error.reason).not.toContain('key=')
  })

  test('is inert (typed non-retryable error) with no GEMINI_API_KEY', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter({
      apiKey: () => undefined,
      fetchImpl: fetchImpl as never,
    })
    const outcome = await runToResult(adapter.complete(baseRequest()))
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.error.retryable).toBe(false)
      expect(outcome.error.adapterId).toBe(GEMMA4_ADAPTER_ID)
    }
    expect(calls).toHaveLength(0)
  })
})

// Build a streamGenerateContent(?alt=sse)-shaped ReadableStream from a list of
// GenerateContentResponse fragments, each on its own `data:` line.
const sseStreamResponse = (fragments: ReadonlyArray<unknown>): Response => {
  const encoder = new TextEncoder()
  let index = 0
  const stream = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (index >= fragments.length) {
        controller.close()
        return
      }
      const fragment = fragments[index]
      index += 1
      controller.enqueue(encoder.encode(`data: ${JSON.stringify(fragment)}\n\n`))
    },
  })
  return new Response(stream, {
    headers: { 'content-type': 'text/event-stream' },
    status: 200,
  })
}

const drainFrames = async (
  frames: AsyncIterable<InferenceStreamEvent>,
): Promise<Array<InferenceStreamEvent>> => {
  const collected: Array<InferenceStreamEvent> = []
  for await (const frame of frames) {
    collected.push(frame)
  }
  return collected
}

describe('gemma4 adapter — streamSse incremental pass-through', () => {
  test('parses multi-fragment SSE into per-fragment events, routing thoughts to reasoningDelta', async () => {
    const { calls, fetchImpl } = recordingFetch(
      sseStreamResponse([
        { candidates: [{ content: { parts: [{ text: 'think', thought: true }] } }] },
        { candidates: [{ content: { parts: [{ text: 'Hello' }] } }] },
        {
          candidates: [
            { content: { parts: [{ text: ', world' }] }, finishReason: 'STOP' },
          ],
          modelVersion: DEFAULT_GEMMA4_MODEL_ID,
          usageMetadata: {
            candidatesTokenCount: 3,
            promptTokenCount: 5,
            thoughtsTokenCount: 2,
            totalTokenCount: 10,
          },
        },
      ]),
    )
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    expect(adapter.streamSse).toBeDefined()
    const source = await run(adapter.streamSse!(baseRequest({ stream: true })))
    const frames = await drainFrames(source.frames)

    // Visible content deltas exclude the thought fragment.
    const contentDeltas = frames
      .map(frame => frame.contentDelta)
      .filter(delta => delta !== '')
    expect(contentDeltas).toEqual(['Hello', ', world'])
    // The thought fragment surfaced on the separate reasoning channel only.
    const reasoning = frames
      .map(frame => frame.reasoningDelta)
      .filter((delta): delta is string => delta !== undefined && delta !== '')
    expect(reasoning).toEqual(['think'])

    // Endpoint + receipt-first terminal state.
    expect(calls[0]?.url).toContain(':streamGenerateContent?key=test-gemini-key')
    expect(calls[0]?.url).toContain('&alt=sse')
    const terminal = source.terminal()
    expect(terminal.finishReason).toBe('STOP')
    expect(terminal.servedModel).toBe(DEFAULT_GEMMA4_MODEL_ID)
    expect(terminal.usage).toEqual({
      completionTokens: 3,
      promptTokens: 5,
      reasoningTokens: 2,
      totalTokens: 10,
    })
  })

  test('a non-2xx stream open surfaces a typed retryable adapter error before any frame', async () => {
    const { fetchImpl } = recordingFetch(new Response('overloaded', { status: 503 }))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    const outcome = await runToResult(
      adapter.streamSse!(baseRequest({ stream: true })),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.error.retryable).toBe(true)
      expect(outcome.error.kind).toBe('service_overloaded')
    }
  })

  test('the buffered stream path yields a content chunk plus a terminal receipt chunk', async () => {
    const { fetchImpl } = recordingFetch(okJson(gemmaThinkingResponse))
    const adapter = makeGemma4Adapter(armedConfig(fetchImpl))

    const chunks = await run(adapter.stream(baseRequest({ stream: true })))
    expect(chunks[0]?.contentDelta).toBe('Hello from Gemma')
    const terminal = chunks.at(-1)!
    expect(terminal.finishReason).toBe('STOP')
    expect(terminal.usage?.reasoningTokens).toBe(5)
    expect(terminal.servedModel).toBe(DEFAULT_GEMMA4_MODEL_ID)
  })
})
