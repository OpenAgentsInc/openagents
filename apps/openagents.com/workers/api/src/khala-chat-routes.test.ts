import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { OnboardingInferenceError } from './autopilot-onboarding-program'
import type { OnboardingStreamDelta } from './autopilot-onboarding-program'
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
  (chunks: ReadonlyArray<OnboardingStreamDelta>, metadata?: KhalaChatStreamSource['metadata']): KhalaChatStreamClient =>
  () =>
    Effect.succeed<KhalaChatStreamSource>({
      deltas: (async function* () {
        for (const chunk of chunks) {
          yield chunk
        }
      })(),
      final: () => chunks
        .map(chunk => typeof chunk === 'string' ? chunk : chunk.kind === 'content' ? chunk.text : '')
        .join(''),
      ...(metadata === undefined ? {} : { metadata }),
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
        chatRequest({ messages: [{ role: 'user', content: 'tell me what changed today' }] }),
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
    expect(response.headers.get('x-openagents-trace-ref')).toContain('trace.khala_chat.')

    // Deltas arrive in order, before done.
    const deltaIndex = text.indexOf('"We "')
    const doneIndex = text.indexOf('event: done')
    expect(deltaIndex).toBeGreaterThanOrEqual(0)
    expect(doneIndex).toBeGreaterThan(deltaIndex)
  })

  test('emits public-safe route metadata before done', async () => {
    const routes = routesWith(chunkStream(['ok'], () => ({
      requestedModel: 'khala',
      servedAdapterId: 'hydralisk',
      servedModel: 'glm-4.6',
      usage: {
        completionTokens: 1,
        promptTokens: 2,
        totalTokens: 3,
      },
    })))
    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'summarize this route' }] }),
        {},
      ),
    )
    const text = await response.text()
    expect(text).toContain('event: meta')
    expect(text).toContain('"servedAdapterId":"hydralisk"')
    expect(text).toContain('"traceRef":"trace.khala_chat.')
    expect(text.indexOf('event: meta')).toBeLessThan(text.indexOf('event: done'))
  })

  test('records served tokens from public chat metadata before done', async () => {
    const recorded: Array<{
      readonly traceRef: string
      readonly totalTokens: number | undefined
    }> = []
    const routes = makeKhalaChatRoutes({
      makeStreamClient: () =>
        chunkStream(['ok'], () => ({
          requestedModel: 'openagents/khala',
          servedAdapterId: 'hydralisk-vllm-glm-5p2-reap-504b',
          servedModel: 'openagents/glm-5.2-reap-504b',
          usage: {
            completionTokens: 1,
            promptTokens: 2,
            totalTokens: 3,
          },
        })),
      rateLimit: allowAll,
      recordServedTokens: input =>
        Effect.sync(() => {
          recorded.push({
            traceRef: input.traceRef,
            totalTokens: input.metadata.usage?.totalTokens,
          })
        }),
    })

    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'count these tokens' }] }),
        {},
      ),
    )
    const text = await response.text()
    expect(recorded).toEqual([
      {
        traceRef: expect.stringContaining('trace.khala_chat.'),
        totalTokens: 3,
      },
    ])
    expect(text.indexOf('event: meta')).toBeLessThan(text.indexOf('event: done'))
  })

  test('answers initial greeting and identity prompts without opening a provider stream', async () => {
    let openedProvider = false
    const routes = makeKhalaChatRoutes({
      makeStreamClient: () => () => {
        openedProvider = true
        return Effect.fail(new OnboardingInferenceError({ reason: 'provider should not open' }))
      },
      rateLimit: allowAll,
      recordServedTokens: () => Effect.void,
    })

    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'Who are you?' }] }),
        {},
      ),
    )

    expect(response.status).toBe(200)
    expect(openedProvider).toBe(false)
    const text = await response.text()
    expect(text).toContain('event: delta')
    expect(text).toContain('We are Khala, a collective intelligence. How can we help you?')
    expect(text).toContain('"servedAdapterId":"khala-fast-path"')
    expect(text).toContain('"servedModel":"khala-fast-greeting"')
    expect(text.indexOf('event: meta')).toBeLessThan(text.indexOf('event: done'))
  })

  test('answers Artanis questions through the read-only Blueprint signature without opening a provider stream', async () => {
    let openedProvider = false
    const routes = makeKhalaChatRoutes({
      makeStreamClient: () => () => {
        openedProvider = true
        return Effect.fail(new OnboardingInferenceError({ reason: 'provider should not open' }))
      },
      rateLimit: allowAll,
      recordServedTokens: () => Effect.void,
    })

    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'How can I talk to Artanis?' }] }),
        {},
      ),
    )

    expect(response.status).toBe(200)
    expect(openedProvider).toBe(false)
    const text = await response.text()
    expect(text).toContain('Artanis is the OpenAgents operator agent.')
    expect(text).toContain('https://openagents.com/artanis')
    expect(text).toContain('cannot command Artanis')
    expect(text).toContain('"servedAdapterId":"khala-artanis-read-only"')
    expect(text).toContain('"signatureRef":"blueprint.public.khala.artanis_interaction.read_only.v1"')
    expect(text).not.toContain('Hierarch')
    expect(text).not.toContain('Daelaam')
  })

  test('sets a larger default max_tokens budget on provider-backed public chat', async () => {
    let captured:
      | { passthroughParams: Readonly<Record<string, unknown>> }
      | undefined
    const routes = makeKhalaChatRoutes({
      makeStreamClient: () => request => {
        captured = request
        return Effect.succeed<KhalaChatStreamSource>({
          deltas: (async function* () {
            yield 'ok'
          })(),
          final: () => 'ok',
        })
      },
      rateLimit: allowAll,
    })

    await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'Write a detailed launch note.' }] }),
        {},
      ),
    )

    expect(captured?.passthroughParams.max_tokens).toBe(8192)
  })

  test('emits provider-labeled reasoning on a separate SSE event', async () => {
    const routes = routesWith(chunkStream([
      { kind: 'reasoning', text: 'provider thought' },
      'Visible answer.',
    ]))
    const response = await run(
      routes.routeKhalaChatRequest(
        chatRequest({ messages: [{ role: 'user', content: 'explain provider reasoning frames' }] }),
        {},
      ),
    )

    const text = await response.text()
    expect(text).toContain('event: reasoning\ndata: {"text":"provider thought"}')
    expect(text).toContain('event: delta\ndata: {"text":"Visible answer."}')
    expect(text.indexOf('event: reasoning')).toBeLessThan(text.indexOf('event: delta'))
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
        chatRequest({ messages: [{ role: 'user', content: 'force provider failure' }] }),
        {},
      ),
    )
    expect(response.status).toBe(502)
    expect(response.headers.get('x-openagents-trace-ref')).toContain('trace.khala_chat.')
    const body = (await response.json()) as { error: string; reason: string }
    expect(body.error).toBe('inference_unavailable')
    expect(body.reason).toBe('no provider lane configured')
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
