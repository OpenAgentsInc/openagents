import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { AUTOPILOT_CONCIERGE_MODEL_ID, KHALA_MINI_MODEL_ID } from './pricing'
import {
  InferenceAdapterError,
  type InferenceRequest,
} from './provider-adapter'
import {
  DEFAULT_GEMINI_MODEL_ID,
  makeVertexGeminiAdapter,
} from './vertex-gemini-adapter'
import type { InferenceStreamEvent } from './provider-adapter'

const run = <A>(effect: Effect.Effect<A, InferenceAdapterError>): Promise<A> =>
  Effect.runPromise(effect)

const baseRequest = (
  overrides: Partial<InferenceRequest> = {},
): InferenceRequest => ({
  messages: [{ content: 'Hey Gemini!', role: 'user' }],
  model: 'gemini',
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

const geminiResponse = {
  candidates: [
    {
      content: {
        parts: [{ text: 'Hello from Gemini' }],
      },
      finishReason: 'STOP',
    },
  ],
  modelVersion: DEFAULT_GEMINI_MODEL_ID,
  usageMetadata: {
    candidatesTokenCount: 4,
    promptTokenCount: 8,
    totalTokenCount: 12,
  },
}

const fixedToken = () => Effect.succeed('test-access-token')

describe('vertex gemini adapter request mapping', () => {
  test('maps the Khala mini virtual model to the default Gemini backing model', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(adapter.complete(baseRequest({ model: KHALA_MINI_MODEL_ID })))

    expect(calls).toHaveLength(1)
    const call = calls[0]!
    expect(call.url).toBe(
      'https://aiplatform.googleapis.com/v1/projects/openagentsgemini' +
        `/locations/global/publishers/google/models/${DEFAULT_GEMINI_MODEL_ID}:generateContent`,
    )
    expect((call.init.headers as Record<string, string>).authorization).toBe(
      'Bearer test-access-token',
    )
    const body = JSON.parse(call.init.body as string) as Record<string, unknown>
    expect(body['model']).toBeUndefined()
  })

  test('maps the Autopilot Concierge virtual model to the default Gemini backing model', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(
      adapter.complete(baseRequest({ model: AUTOPILOT_CONCIERGE_MODEL_ID })),
    )

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(
      'https://aiplatform.googleapis.com/v1/projects/openagentsgemini' +
        `/locations/global/publishers/google/models/${DEFAULT_GEMINI_MODEL_ID}:generateContent`,
    )
  })

  test('maps thinking budget passthrough into Gemini generation config', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(
      adapter.complete(
        baseRequest({
          passthroughParams: { max_tokens: 4096, thinking_budget: 0 },
        }),
      ),
    )

    const body = JSON.parse(calls[0]!.init.body as string) as Record<
      string,
      unknown
    >
    expect(body['generationConfig']).toMatchObject({
      maxOutputTokens: 4096,
      thinkingConfig: { thinkingBudget: 0 },
    })
  })
})

// Build a Vertex Gemini streamGenerateContent(?alt=sse)-shaped ReadableStream
// from a list of GenerateContentResponse fragments, each on its own `data:`
// line. Mirrors how Vertex emits SSE so the adapter's incremental reader sees
// many fragments rather than one buffered body.
const sseStreamResponse = (
  fragments: ReadonlyArray<unknown>,
): Response => {
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
      controller.enqueue(
        encoder.encode(`data: ${JSON.stringify(fragment)}\n\n`),
      )
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

describe('vertex gemini adapter streamSse — incremental pass-through', () => {
  test('parses a multi-fragment Gemini SSE body into multiple events (one per fragment)', async () => {
    const { fetchImpl } = recordingFetch(
      sseStreamResponse([
        { candidates: [{ content: { parts: [{ text: 'Hello' }] } }] },
        { candidates: [{ content: { parts: [{ text: ', ' }] } }] },
        {
          candidates: [
            {
              content: { parts: [{ text: 'world' }] },
              finishReason: 'STOP',
            },
          ],
          modelVersion: DEFAULT_GEMINI_MODEL_ID,
          usageMetadata: {
            candidatesTokenCount: 3,
            promptTokenCount: 5,
            totalTokenCount: 8,
          },
        },
      ]),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    expect(adapter.streamSse).toBeDefined()
    const source = await run(adapter.streamSse!(baseRequest({ stream: true })))
    const frames = await drainFrames(source.frames)

    // MANY events, not one buffered chunk: one per upstream Gemini fragment.
    const contentFrames = frames.filter(frame => frame.contentDelta !== '')
    expect(contentFrames.map(frame => frame.contentDelta)).toEqual([
      'Hello',
      ', ',
      'world',
    ])

    // Receipt-first terminal state from the final fragment's cumulative usage.
    const terminal = source.terminal()
    expect(terminal.finishReason).toBe('STOP')
    expect(terminal.servedModel).toBe(DEFAULT_GEMINI_MODEL_ID)
    expect(terminal.usage?.promptTokens).toBe(5)
    expect(terminal.usage?.completionTokens).toBe(3)
    expect(terminal.usage?.totalTokens).toBe(8)
  })

  test('hits the streamGenerateContent?alt=sse endpoint', async () => {
    const { calls, fetchImpl } = recordingFetch(
      sseStreamResponse([
        { candidates: [{ content: { parts: [{ text: 'hi' }] } }] },
      ]),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const source = await run(adapter.streamSse!(baseRequest({ stream: true })))
    await drainFrames(source.frames)

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toContain(':streamGenerateContent?alt=sse')
  })

  test('a non-2xx stream open surfaces a typed retryable adapter error before any frame', async () => {
    const { fetchImpl } = recordingFetch(
      new Response('quota exceeded', { status: 429 }),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const result = await Effect.runPromise(
      adapter.streamSse!(baseRequest({ stream: true })).pipe(
        Effect.map(() => 'ok' as const),
        Effect.catch(error =>
          Effect.succeed({ reason: error.reason, retryable: error.retryable }),
        ),
      ),
    )
    expect(result).not.toBe('ok')
    expect(result).toMatchObject({ retryable: true })
  })
})

describe('vertex gemini adapter function calling (#6364)', () => {
  const tools = [
    {
      function: {
        description: 'Read a public repo file',
        name: 'read_repo_file',
        parameters: {
          additionalProperties: false,
          properties: { path: { type: 'string' } },
          required: ['path'],
          type: 'object',
        },
      },
      type: 'function',
    },
  ]

  test('forwards OpenAI tools as Gemini functionDeclarations (additionalProperties stripped)', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(
      adapter.complete(
        baseRequest({ passthroughParams: { tool_choice: 'auto', tools } }),
      ),
    )

    const body = JSON.parse(calls[0]!.init.body as string) as Record<
      string,
      unknown
    >
    const geminiTools = body['tools'] as Array<Record<string, unknown>>
    const declarations = geminiTools[0]!['functionDeclarations'] as Array<
      Record<string, unknown>
    >
    expect(declarations[0]!['name']).toBe('read_repo_file')
    const params = declarations[0]!['parameters'] as Record<string, unknown>
    expect(params['additionalProperties']).toBeUndefined()
    expect(params['properties']).toBeDefined()
    expect(body['toolConfig']).toEqual({
      functionCallingConfig: { mode: 'AUTO' },
    })
  })

  test('parses a Gemini functionCall response into OpenAI-compatible toolCalls', async () => {
    const { fetchImpl } = recordingFetch(
      okJson({
        candidates: [
          {
            content: {
              parts: [
                {
                  functionCall: {
                    args: { path: 'docs/roadmap.md' },
                    name: 'read_repo_file',
                  },
                  thoughtSignature: 'sig-abc',
                },
              ],
            },
            finishReason: 'STOP',
          },
        ],
        modelVersion: DEFAULT_GEMINI_MODEL_ID,
        usageMetadata: {
          candidatesTokenCount: 4,
          promptTokenCount: 10,
          totalTokenCount: 14,
        },
      }),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const result = await run(
      adapter.complete(
        baseRequest({ passthroughParams: { tools } }),
      ),
    )

    expect(result.finishReason).toBe('tool_calls')
    expect(result.toolCalls).toHaveLength(1)
    expect(result.toolCalls?.[0]?.function.name).toBe('read_repo_file')
    expect(result.toolCalls?.[0]?.function.arguments).toBe(
      JSON.stringify({ path: 'docs/roadmap.md' }),
    )
    expect(result.toolCalls?.[0]?.type).toBe('function')
    // Gemini 3 thoughtSignature is captured so it can be replayed next turn.
    expect(result.toolCalls?.[0]?.thoughtSignature).toBe('sig-abc')
  })

  test('round-trips a tool result back as a Gemini functionResponse', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(
      adapter.complete(
        baseRequest({
          messages: [
            { content: 'read the roadmap', role: 'user' },
            {
              content: '',
              role: 'assistant',
              toolCalls: [
                {
                  function: {
                    arguments: JSON.stringify({ path: 'docs/roadmap.md' }),
                    name: 'read_repo_file',
                  },
                  id: 'call_1',
                  thoughtSignature: 'sig-abc',
                  type: 'function',
                },
              ],
            },
            {
              content: 'First priority: the #6316 serving track.',
              name: 'read_repo_file',
              role: 'tool',
              toolCallId: 'call_1',
            },
          ],
          passthroughParams: { tools },
        }),
      ),
    )

    const body = JSON.parse(calls[0]!.init.body as string) as Record<
      string,
      unknown
    >
    const contents = body['contents'] as Array<Record<string, unknown>>
    // The assistant tool call became a model functionCall part.
    const modelTurn = contents.find(content => content['role'] === 'model')!
    const modelParts = modelTurn['parts'] as Array<Record<string, unknown>>
    expect(modelParts[0]!['functionCall']).toMatchObject({
      name: 'read_repo_file',
    })
    // The Gemini 3 thoughtSignature is replayed on the part.
    expect(modelParts[0]!['thoughtSignature']).toBe('sig-abc')
    // The tool result became a user functionResponse part.
    const responseTurn = contents.find(content => {
      const parts = content['parts'] as Array<Record<string, unknown>>
      return parts.some(part => 'functionResponse' in part)
    })!
    const responseParts = responseTurn['parts'] as Array<Record<string, unknown>>
    expect(responseParts[0]!['functionResponse']).toMatchObject({
      name: 'read_repo_file',
      response: { content: 'First priority: the #6316 serving track.' },
    })
  })
})
