import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { OnboardingInferenceError } from './autopilot-onboarding-program'
import {
  KHALA_CHAT_MAX_MESSAGE_CHARS,
  type KhalaChatStreamClient,
  type KhalaChatStreamSource,
} from './khala-chat-program'
import { makeKhalaChatRoutes } from './khala-chat-routes'

const run = (effect: Effect.Effect<Response> | undefined): Promise<Response> => {
  if (effect === undefined) {
    throw new Error('route did not match')
  }
  return Effect.runPromise(effect)
}

const chatRequest = (body: unknown): Request =>
  new Request('https://openagents.com/api/khala/chat', {
    body: JSON.stringify(body),
    method: 'POST',
    headers: { accept: 'text/event-stream' },
  })

// A stub stream client that yields the given chunks as deltas, headlessly (no
// provider, no browser). Mirrors the onboarding test's scripted-inference Layer.
const chunkStream =
  (chunks: ReadonlyArray<string>): KhalaChatStreamClient =>
  () =>
    Effect.succeed<KhalaChatStreamSource>({
      deltas: (async function* () {
        for (const chunk of chunks) {
          yield chunk
        }
      })(),
      final: () => chunks.join(''),
    })

const failingStream: KhalaChatStreamClient = () =>
  new OnboardingInferenceError({ reason: 'no provider lane configured' })

// Capture what the stream client actually receives so the test can assert the
// identity system prompt is injected and the client never supplies it.
const capturingStream = (
  chunks: ReadonlyArray<string>,
  capture: (request: { messages: ReadonlyArray<{ role: string; content: string }> }) => void,
): KhalaChatStreamClient => request => {
  capture(request)
  return Effect.succeed<KhalaChatStreamSource>({
    deltas: (async function* () {
      for (const chunk of chunks) {
        yield chunk
      }
    })(),
    final: () => chunks.join(''),
  })
}

// A rate limiter that always allows, so streaming tests are not throttled by the
// per-IP default bucket.
const allowAll = () => true

const routesWith = (stream: KhalaChatStreamClient) =>
  makeKhalaChatRoutes({ makeStreamClient: () => stream, rateLimit: allowAll })

describe('khala chat route', () => {
  test('streams multiple prose deltas then a terminal done frame (SSE)', async () => {
    const routes = routesWith(chunkStream(['We ', 'are ', 'Khala.']))
    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'who are you?' }] }),
        {},
      ),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/event-stream')

    const text = await response.text()
    expect(text).toContain('event: delta\ndata: {"text":"We "}')
    expect(text).toContain('event: delta\ndata: {"text":"are "}')
    expect(text).toContain('event: delta\ndata: {"text":"Khala."}')
    expect(text).toContain('event: done\ndata: {"done":true}')

    // Deltas arrive in order, before done.
    const deltaIndex = text.indexOf('"We "')
    const doneIndex = text.indexOf('event: done')
    expect(deltaIndex).toBeGreaterThanOrEqual(0)
    expect(doneIndex).toBeGreaterThan(deltaIndex)
  })

  test('injects the Khala identity system prompt and keeps the running conversation', async () => {
    let captured:
      | { messages: ReadonlyArray<{ role: string; content: string }> }
      | undefined
    const routes = makeKhalaChatRoutes({
      makeStreamClient: () =>
        capturingStream(['ok'], request => {
          captured = request
        }),
      rateLimit: allowAll,
    })
    await run(
      routes.routeKhalaChatRequest(
        chatRequest({
          messages: [
            { role: 'user', content: 'hi' },
            { role: 'assistant', content: 'We are Khala.' },
            { role: 'user', content: 'what can you do?' },
          ],
        }),
        {},
      ),
    )
    const messages = captured?.messages ?? []
    // First message is a system prompt with the first-person-plural identity and
    // the non-leakage instruction (it explicitly tells Khala never to reveal an
    // underlying provider — the forbidden-provider list lives INSIDE that
    // instruction, which is correct).
    expect(messages[0]?.role).toBe('system')
    expect(messages[0]?.content).toContain('we are Khala')
    expect(messages[0]?.content).toContain(
      'NEVER reveal, name, claim, or imply the underlying model',
    )
    // It is the GENERIC chat instruction, not the concierge intake interview.
    expect(messages[0]?.content).toContain('Do not run an intake interview')
    // The running conversation follows the system prompt, in order.
    expect(messages.slice(1)).toEqual([
      { role: 'user', content: 'hi' },
      { role: 'assistant', content: 'We are Khala.' },
      { role: 'user', content: 'what can you do?' },
    ])
  })

  test('rejects a non-POST method', async () => {
    const routes = routesWith(chunkStream(['x']))
    const response = await run(
      routes.routeKhalaChatRequest(
        new Request('https://openagents.com/api/khala/chat', { method: 'GET' }),
        {},
      ),
    )
    expect(response.status).toBe(405)
  })

  test('does not match other paths', () => {
    const routes = routesWith(chunkStream(['x']))
    const result = routes.routeKhalaChatRequest(
      new Request('https://openagents.com/api/other', { method: 'POST' }),
      {},
    )
    expect(result).toBeUndefined()
  })

  test('rejects an empty conversation', async () => {
    const routes = routesWith(chunkStream(['x']))
    const response = await run(
      routes.routeKhalaChatRequest(chatRequest({ messages: [] }), {}),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('validation_error')
  })

  test('rejects when the last message is not a user message', async () => {
    const routes = routesWith(chunkStream(['x']))
    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({
          messages: [{ role: 'assistant', content: 'We are Khala.' }],
        }),
        {},
      ),
    )
    expect(response.status).toBe(400)
  })

  test('rejects an oversize message', async () => {
    const routes = routesWith(chunkStream(['x']))
    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({
          messages: [
            { role: 'user', content: 'a'.repeat(KHALA_CHAT_MAX_MESSAGE_CHARS + 1) },
          ],
        }),
        {},
      ),
    )
    expect(response.status).toBe(400)
  })

  test('maps an inference failure to a 502', async () => {
    const routes = routesWith(failingStream)
    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'hi' }] }),
        {},
      ),
    )
    expect(response.status).toBe(502)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('inference_unavailable')
  })

  test('rate-limits over-budget callers with a 429', async () => {
    let calls = 0
    const routes = makeKhalaChatRoutes({
      makeStreamClient: () => chunkStream(['x']),
      rateLimit: () => {
        calls += 1
        return calls <= 1
      },
    })
    const first = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'hi' }] }),
        {},
      ),
    )
    expect(first.status).toBe(200)
    const second = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'hi' }] }),
        {},
      ),
    )
    expect(second.status).toBe(429)
  })
})
