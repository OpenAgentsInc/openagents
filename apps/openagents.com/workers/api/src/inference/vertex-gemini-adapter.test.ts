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
