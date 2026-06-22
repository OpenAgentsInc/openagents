import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  DEFAULT_GEMINI_MODEL_ID,
  makeVertexGeminiAdapter,
} from './vertex-gemini-adapter'
import {
  InferenceAdapterError,
  type InferenceRequest,
} from './provider-adapter'
import { KHALA_MINI_MODEL_ID } from './pricing'

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
})
