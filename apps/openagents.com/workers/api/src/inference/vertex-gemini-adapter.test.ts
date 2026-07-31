import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { AUTOPILOT_CONCIERGE_MODEL_ID, KHALA_MINI_MODEL_ID } from './pricing'
import {
  InferenceAdapterError,
  type InferenceRequest,
  type InferenceUsage,
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

  test('gemini 3.x maps a numeric thinking budget onto the thinkingLevel enum', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    // The default model resolves to gemini-3.6-flash (a 3.x model): a budget of
    // 0 maps to the `minimal` thinkingLevel string enum, never a numeric
    // thinkingBudget.
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
      thinkingConfig: { thinkingLevel: 'minimal' },
    })
  })

  test('gemini 3.x drops the deprecated temperature/topP/topK sampling params', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(
      adapter.complete(
        baseRequest({
          passthroughParams: {
            max_tokens: 512,
            temperature: 0.7,
            thinking_level: 'high',
            top_k: 40,
            top_p: 0.9,
          },
        }),
      ),
    )

    const generationConfig = (
      JSON.parse(calls[0]!.init.body as string) as Record<string, unknown>
    )['generationConfig'] as Record<string, unknown>
    expect(generationConfig).toEqual({
      maxOutputTokens: 512,
      thinkingConfig: { thinkingLevel: 'high' },
    })
    expect(generationConfig['temperature']).toBeUndefined()
    expect(generationConfig['topP']).toBeUndefined()
    expect(generationConfig['topK']).toBeUndefined()
  })

  test('pre-3.x Gemini keeps the legacy numeric thinkingBudget and sampling knobs', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(
      adapter.complete(
        baseRequest({
          model: 'gemini-2.5-pro',
          passthroughParams: {
            max_tokens: 4096,
            temperature: 0.2,
            thinking_budget: 256,
            top_p: 0.8,
          },
        }),
      ),
    )

    expect(
      (JSON.parse(calls[0]!.init.body as string) as Record<string, unknown>)[
        'generationConfig'
      ],
    ).toMatchObject({
      maxOutputTokens: 4096,
      temperature: 0.2,
      thinkingConfig: { thinkingBudget: 256 },
      topP: 0.8,
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
    // The Gemini enum "STOP" is normalized onto the OpenAI wire vocabulary.
    const terminal = source.terminal()
    expect(terminal.finishReason).toBe('stop')
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

  // REGRESSION (BUG 2, defect C). Gemini reports its OWN finishReason enum;
  // emitting it verbatim gives a stock OpenAI client `finish_reason: "STOP"` /
  // `"MAX_TOKENS"`, which is not in the OpenAI vocabulary.
  test('normalizes a MAX_TOKENS truncation to the OpenAI `length` reason', async () => {
    const { fetchImpl } = recordingFetch(
      sseStreamResponse([
        {
          candidates: [
            {
              content: { parts: [{ text: 'trunc' }] },
              finishReason: 'MAX_TOKENS',
            },
          ],
          usageMetadata: {
            candidatesTokenCount: 1,
            promptTokenCount: 2,
            totalTokenCount: 3,
          },
        },
      ]),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const source = await run(adapter.streamSse!(baseRequest({ stream: true })))
    const frames = await drainFrames(source.frames)

    expect(frames.map(frame => frame.finishReason)).toEqual(['length'])
    expect(source.terminal().finishReason).toBe('length')
  })
})

// REGRESSION SUITE (BUG 2, defect B). Before this, `vertex-gemini-adapter.ts`
// contained ZERO occurrences of `toolCallDeltas`: every Gemini `functionCall`
// part — and its `thoughtSignature` — was silently discarded on BOTH streaming
// paths, so a streamed tool-using turn reached the client as an empty completion
// with `finish_reason: STOP`, while the identical non-streaming request returned
// the real tool call.
//
// These fragments mirror the real Vertex wire shape: the functionCall arrives in
// its OWN SSE frame carrying name/args/id plus a SIBLING `thoughtSignature` on
// the part, and a LATER frame carries the empty text part, `finishReason`, and
// `usageMetadata`.
const toolCallFragments = [
  {
    candidates: [
      {
        content: {
          parts: [
            {
              functionCall: {
                args: { path: 'docs/roadmap.md' },
                id: 'call_vertex_1',
                name: 'read_repo_file',
              },
              thoughtSignature: 'sig-stream-abc',
            },
          ],
        },
      },
    ],
    modelVersion: DEFAULT_GEMINI_MODEL_ID,
  },
  {
    candidates: [{ content: { parts: [{ text: '' }] }, finishReason: 'STOP' }],
    modelVersion: DEFAULT_GEMINI_MODEL_ID,
    usageMetadata: {
      candidatesTokenCount: 12,
      promptTokenCount: 40,
      thoughtsTokenCount: 8,
      totalTokenCount: 60,
    },
  },
] as const

describe('vertex gemini adapter streaming tool calls (BUG 2 defect B)', () => {
  test('streamSse emits toolCallDeltas for a Gemini functionCall fragment, preserving thoughtSignature', async () => {
    const { fetchImpl } = recordingFetch(sseStreamResponse(toolCallFragments))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const source = await run(adapter.streamSse!(baseRequest({ stream: true })))
    const frames = await drainFrames(source.frames)

    const deltas = frames.flatMap(frame => frame.toolCallDeltas ?? [])
    expect(deltas).toHaveLength(1)
    expect(deltas[0]).toEqual({
      function: {
        arguments: JSON.stringify({ path: 'docs/roadmap.md' }),
        name: 'read_repo_file',
      },
      id: 'call_vertex_1',
      index: 0,
      thoughtSignature: 'sig-stream-abc',
      type: 'function',
    })

    // Gemini says "STOP" even when the turn ended in a functionCall; the wire
    // contract requires `tool_calls`.
    expect(source.terminal().finishReason).toBe('tool_calls')
    expect(source.terminal().usage?.totalTokens).toBe(60)
  })

  test('parallel tool calls across fragments keep a turn-wide ascending index', async () => {
    const { fetchImpl } = recordingFetch(
      sseStreamResponse([
        toolCallFragments[0],
        {
          candidates: [
            {
              content: {
                parts: [
                  {
                    functionCall: {
                      args: { path: 'AGENTS.md' },
                      name: 'read_repo_file',
                    },
                    thoughtSignature: 'sig-stream-def',
                  },
                  {
                    functionCall: {
                      args: { query: 'roadmap' },
                      name: 'search_repo',
                    },
                  },
                ],
              },
            },
          ],
        },
        toolCallFragments[1],
      ]),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const source = await run(adapter.streamSse!(baseRequest({ stream: true })))
    const frames = await drainFrames(source.frames)
    const deltas = frames.flatMap(frame => frame.toolCallDeltas ?? [])

    expect(deltas.map(delta => delta.index)).toEqual([0, 1, 2])
    expect(deltas.map(delta => delta.function?.name)).toEqual([
      'read_repo_file',
      'read_repo_file',
      'search_repo',
    ])
    // A call Gemini did not id gets a stable synthesized one, never a collision.
    expect(new Set(deltas.map(delta => delta.id)).size).toBe(3)
    expect(deltas[2]?.thoughtSignature).toBeUndefined()
  })

  test('the buffered stream path emits the same toolCallDeltas chunk', async () => {
    const body = toolCallFragments
      .map(fragment => `data: ${JSON.stringify(fragment)}\n\n`)
      .join('')
    const { fetchImpl } = recordingFetch(
      new Response(body, {
        headers: { 'content-type': 'text/event-stream' },
        status: 200,
      }),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const chunks = await run(adapter.stream(baseRequest({ stream: true })))
    const deltas = chunks.flatMap(chunk => chunk.toolCallDeltas ?? [])

    expect(deltas).toHaveLength(1)
    expect(deltas[0]?.function?.name).toBe('read_repo_file')
    expect(deltas[0]?.thoughtSignature).toBe('sig-stream-abc')
    expect(chunks[chunks.length - 1]?.finishReason).toBe('tool_calls')
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

  // REGRESSION (BUG 2, defect A). Omega's OpenAgents provider does not override
  // `tool_input_format`, so it sends full JSON Schema with `$defs`/`$ref`. The
  // old sanitizer stripped only `additionalProperties`/`$schema`, so Vertex
  // answered `400 Unknown name "$ref" ... Unknown name "$defs"` and the route
  // turned that into a 502 `provider_error` with no log line.
  test('inlines $defs/$ref into a Vertex-legal parameters schema', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    await run(
      adapter.complete(
        baseRequest({
          passthroughParams: {
            tools: [
              {
                function: {
                  description: 'Edit a file',
                  name: 'edit_file',
                  parameters: {
                    $defs: {
                      Mode: { enum: ['create', 'overwrite'], type: 'string' },
                    },
                    $schema: 'https://json-schema.org/draft/2020-12/schema',
                    additionalProperties: false,
                    properties: {
                      mode: { $ref: '#/$defs/Mode' },
                      path: { type: 'string' },
                    },
                    required: ['path', 'mode'],
                    type: 'object',
                  },
                },
                type: 'function',
              },
            ],
          },
        }),
      ),
    )

    const body = JSON.parse(calls[0]!.init.body as string) as Record<
      string,
      unknown
    >
    const declarations = (body['tools'] as Array<Record<string, unknown>>)[0]![
      'functionDeclarations'
    ] as Array<Record<string, unknown>>
    expect(declarations[0]!['parameters']).toEqual({
      properties: {
        mode: { enum: ['create', 'overwrite'], type: 'string' },
        path: { type: 'string' },
      },
      required: ['path', 'mode'],
      type: 'object',
    })
    // The exact keywords Vertex 400s on must not survive anywhere in the body.
    const serialized = calls[0]!.init.body as string
    expect(serialized).not.toContain('$ref')
    expect(serialized).not.toContain('$defs')
    expect(serialized).not.toContain('$schema')
    expect(serialized).not.toContain('additionalProperties')
  })

  test('normalizes a raw Gemini finish reason on the non-streaming path', async () => {
    const { fetchImpl } = recordingFetch(
      okJson({
        candidates: [
          {
            content: { parts: [{ text: 'truncated answ' }] },
            finishReason: 'MAX_TOKENS',
          },
        ],
        usageMetadata: {
          candidatesTokenCount: 4,
          promptTokenCount: 8,
          totalTokenCount: 12,
        },
      }),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    expect((await run(adapter.complete(baseRequest()))).finishReason).toBe(
      'length',
    )
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

describe('vertex gemini adapter budget truncation', () => {
  // Byte-for-byte the live Vertex reply captured on 2026-07-31 from
  // `gemini-3.6-flash` with `maxOutputTokens: 16` against a ~4400-token prompt.
  // Note what is NOT here: the candidate has no `content` key at all, because
  // `thoughtsTokenCount: 12` of the 16-token allowance went to reasoning before
  // any visible text was produced. Vertex still answered HTTP 200 — this is a
  // truncated success, not a provider fault.
  const maxTokensResponse = {
    candidates: [{ finishReason: 'MAX_TOKENS' }],
    createTime: '2026-07-31T18:54:11.482979Z',
    modelVersion: 'gemini-3.6-flash',
    responseId: 'U-9saqO9Hf-Ru-gPyJrT6AM',
    usageMetadata: {
      promptTokenCount: 4410,
      promptTokensDetails: [{ modality: 'TEXT', tokenCount: 4410 }],
      thoughtsTokenCount: 12,
      totalTokenCount: 4422,
      trafficType: 'ON_DEMAND',
    },
  }

  test('reports a content-free MAX_TOKENS candidate as an empty completion with finish reason length', async () => {
    const { calls, fetchImpl } = recordingFetch(okJson(maxTokensResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'test-project',
      tokenProvider: fixedToken,
    })

    const result = await run(
      adapter.complete(
        baseRequest({
          model: 'gemini-3.6-flash',
          passthroughParams: { max_tokens: 16 },
        }),
      ),
    )

    // `length` is the OpenAI word for MAX_TOKENS. Downstream dispatch keys off
    // it to tell "the caller's budget ran out" apart from "the lane broke", so
    // leaving the raw Gemini enum here would resurrect the 502.
    expect(result.finishReason).toBe('length')
    expect(result.content).toBe('')

    // The caller's budget is passed through verbatim: this adapter does not
    // floor it the way gemma4 does, so the tiny-budget shape stays reachable
    // and must stay handled rather than papered over.
    const body = JSON.parse(String(calls[0]?.init.body)) as {
      generationConfig: { maxOutputTokens: number }
    }
    expect(body.generationConfig.maxOutputTokens).toBe(16)
  })
})

// ---------------------------------------------------------------------------
// Reasoning-token attribution (thoughtsTokenCount -> usage.reasoningTokens).
//
// These fixtures are VERBATIM live captures taken 2026-07-31 against
// `projects/openagentsgemini/locations/global/publishers/google/models/
// gemini-3.6-flash` (`:generateContent` and `:streamGenerateContent?alt=sse`,
// both HTTP 200, thinkingLevel "medium"). They are not hand-authored guesses at
// the wire shape — the whole point of the defect they pin is that the adapter
// read a shape nobody had checked against the provider.
//
// The defect: Gemini bills thinking tokens INTO `totalTokenCount` but reports
// them ONLY in `thoughtsTokenCount`. Spend enforcement was therefore correct
// while attribution was not: 180 of the 198 billed tokens below were invisible
// as reasoning, and `token_usage_events.reasoning_tokens` stored 0 for them.
// ---------------------------------------------------------------------------

// Live capture, `:generateContent`. Trimmed only of the long `thoughtSignature`
// blob on the text part; every `usageMetadata` field is byte-for-byte upstream.
const liveGeminiThinkingResponse = {
  candidates: [
    {
      content: { parts: [{ text: '391' }], role: 'model' },
      finishReason: 'STOP',
    },
  ],
  createTime: '2026-07-31T19:21:17.631405Z',
  modelVersion: 'gemini-3.6-flash',
  responseId: 'rfVsau3EJuGprb8P7bzv6AI',
  usageMetadata: {
    candidatesTokenCount: 3,
    candidatesTokensDetails: [{ modality: 'TEXT', tokenCount: 3 }],
    promptTokenCount: 15,
    promptTokensDetails: [{ modality: 'TEXT', tokenCount: 15 }],
    thoughtsTokenCount: 180,
    totalTokenCount: 198,
    trafficType: 'ON_DEMAND',
  },
}

// Live capture, `:streamGenerateContent?alt=sse` — the FULL two-fragment body.
// Note fragment 0: `usageMetadata` is PRESENT but carries only `trafficType`
// and no token counts. That is why the streaming fold must carry prior values
// forward; reading `thoughtsTokenCount` per-fragment without carry-forward
// would zero it on any frame that is not the terminal one.
const liveGeminiThinkingSseFragments = [
  {
    candidates: [{ content: { parts: [{ text: '391' }], role: 'model' } }],
    createTime: '2026-07-31T19:21:30.146877Z',
    modelVersion: 'gemini-3.6-flash',
    responseId: 'uvVsar37CKvPu-gPvs6b-A8',
    usageMetadata: { trafficType: 'ON_DEMAND' },
  },
  {
    candidates: [
      {
        content: { parts: [{ text: '' }], role: 'model' },
        finishReason: 'STOP',
      },
    ],
    modelVersion: 'gemini-3.6-flash',
    responseId: 'uvVsar37CKvPu-gPvs6b-A8',
    usageMetadata: {
      candidatesTokenCount: 3,
      candidatesTokensDetails: [{ modality: 'TEXT', tokenCount: 3 }],
      promptTokenCount: 15,
      promptTokensDetails: [{ modality: 'TEXT', tokenCount: 15 }],
      thoughtsTokenCount: 207,
      totalTokenCount: 225,
      trafficType: 'ON_DEMAND',
    },
  },
]

describe('vertex gemini adapter reasoning-token attribution', () => {
  // Guard the fixtures themselves. `reasoning_tokens` is an INTEGER column and
  // the parse guard is `typeof value === 'number'`, so a string-encoded count
  // would be silently dropped to 0 rather than failing loudly — exactly the
  // always-false-guard failure mode. The live capture sends unquoted JSON
  // numbers; if upstream ever switches to Google's string-encoded int64 form,
  // this assertion fails first and names the reason.
  test('upstream reports every usage count as a JSON number, not a string', () => {
    const nonStreaming = liveGeminiThinkingResponse.usageMetadata
    const streaming = liveGeminiThinkingSseFragments[1]!.usageMetadata
    for (const usage of [nonStreaming, streaming]) {
      for (const field of [
        'promptTokenCount',
        'candidatesTokenCount',
        'totalTokenCount',
        'thoughtsTokenCount',
      ] as const) {
        expect(typeof (usage as Record<string, unknown>)[field]).toBe('number')
      }
    }
  })

  test('complete() maps thoughtsTokenCount onto usage.reasoningTokens', async () => {
    const { fetchImpl } = recordingFetch(okJson(liveGeminiThinkingResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const result = await run(
      adapter.complete(
        baseRequest({
          model: 'gemini-3.6-flash',
          passthroughParams: { max_tokens: 256 },
        }),
      ),
    )

    // The whole defect in one assertion: without the mapping this is
    // `undefined` and 180 billed tokens have no cause.
    expect(result.usage.reasoningTokens).toBe(180)

    // Attribution now closes: prompt + completion + reasoning accounts for the
    // provider total with nothing left in the opaque remainder.
    expect(result.usage.promptTokens).toBe(15)
    expect(result.usage.completionTokens).toBe(3)
    expect(result.usage.totalTokens).toBe(198)
    expect(
      result.usage.promptTokens +
        result.usage.completionTokens +
        (result.usage.reasoningTokens ?? 0),
    ).toBe(result.usage.totalTokens)
  })

  test('streamSse() maps thoughtsTokenCount onto the terminal usage', async () => {
    const { fetchImpl } = recordingFetch(
      sseStreamResponse(liveGeminiThinkingSseFragments),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const source = await run(
      adapter.streamSse!(
        baseRequest({
          model: 'gemini-3.6-flash',
          passthroughParams: { max_tokens: 256 },
          stream: true,
        }),
      ),
    )
    await drainFrames(source.frames)

    const terminal = source.terminal()
    expect(terminal.usage?.reasoningTokens).toBe(207)
    expect(terminal.usage?.promptTokens).toBe(15)
    expect(terminal.usage?.completionTokens).toBe(3)
    expect(terminal.usage?.totalTokens).toBe(225)
  })

  test('buffered stream() maps thoughtsTokenCount onto the terminal chunk', async () => {
    const { fetchImpl } = recordingFetch(
      sseStreamResponse(liveGeminiThinkingSseFragments),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const chunks = await run(
      adapter.stream(
        baseRequest({
          model: 'gemini-3.6-flash',
          passthroughParams: { max_tokens: 256 },
          stream: true,
        }),
      ),
    )

    const reportedUsages = chunks
      .map(chunk => chunk.usage)
      .filter((usage): usage is InferenceUsage => usage !== undefined)
    const usage = reportedUsages[reportedUsages.length - 1]
    expect(usage?.reasoningTokens).toBe(207)
    expect(usage?.totalTokens).toBe(225)
  })

  test('a mid-stream fragment without token counts does not zero the carried reasoning count', async () => {
    // Terminal fragment reports the counts, then a trailing fragment arrives
    // carrying a bare `usageMetadata` (the shape fragment 0 of the live capture
    // has). Without carry-forward the last fold wins and reasoning drops to 0.
    const { fetchImpl } = recordingFetch(
      sseStreamResponse([
        ...liveGeminiThinkingSseFragments,
        {
          candidates: [{ content: { parts: [{ text: '' }], role: 'model' } }],
          usageMetadata: { trafficType: 'ON_DEMAND' },
        },
      ]),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const source = await run(
      adapter.streamSse!(
        baseRequest({ model: 'gemini-3.6-flash', stream: true }),
      ),
    )
    await drainFrames(source.frames)

    expect(source.terminal().usage?.reasoningTokens).toBe(207)
  })

  test('reasoning is a breakdown dimension, never an addend to a reported total', async () => {
    // Gemini's own totals happen to equal prompt+candidates+thoughts, so this
    // uses a total that does NOT (tool-use tokens land in the total too). The
    // provider's total must win; adding reasoning on top would over-bill.
    const { fetchImpl } = recordingFetch(
      okJson({
        ...liveGeminiThinkingResponse,
        usageMetadata: {
          candidatesTokenCount: 3,
          promptTokenCount: 15,
          thoughtsTokenCount: 180,
          totalTokenCount: 250,
        },
      }),
    )
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const result = await run(
      adapter.complete(baseRequest({ model: 'gemini-3.6-flash' })),
    )
    expect(result.usage.totalTokens).toBe(250)
    expect(result.usage.reasoningTokens).toBe(180)
  })

  test('a model that reports no thoughts leaves reasoningTokens undefined', async () => {
    const { fetchImpl } = recordingFetch(okJson(geminiResponse))
    const adapter = makeVertexGeminiAdapter({
      fetchImpl,
      project: 'openagentsgemini',
      tokenProvider: fixedToken,
    })

    const result = await run(adapter.complete(baseRequest()))
    // Exact-only: absent, never a synthesized zero-ish estimate.
    expect(result.usage.reasoningTokens).toBeUndefined()
    expect(result.usage.totalTokens).toBe(12)
  })
})
