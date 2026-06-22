// LOCAL STREAM HARNESS for the khala-code 524 fix (issue #6035, refs #6027).
//
// Wires the REAL Fireworks adapter `streamSse` through the REAL route stream
// branch against a FAKE Fireworks SSE source, and asserts the two properties the
// prod 524 violated:
//
//   1. TERMINAL USAGE FRAME. A normal streamed completion carries its real usage
//      (the adapter opts in via `stream_options.include_usage`), so metering
//      settles receipt-first instead of erroring "missing terminal usage frame".
//   2. INCREMENTAL PASS-THROUGH. The route emits each upstream chunk AS IT
//      ARRIVES — the harness gates upstream chunk N+1 behind the route having
//      already produced chunk N to the client, which can only happen if the
//      route is NOT buffering the whole upstream stream server-side. A buffered
//      route would deadlock this harness (it would await the whole upstream
//      before emitting a byte), so a passing run is positive proof of streaming.
//
// Plus the missing-terminal-frame case: the stream still closes cleanly and the
// route settles NO metering (never an estimate).

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleChatCompletions,
  type ChatCompletionsDeps,
  type InferenceAuth,
} from './chat-completions-routes'
import {
  type FetchLike,
  makeFireworksAdapter,
} from './fireworks-adapter'
import { type MeteringContext, type MeteringHook } from './metering-hook'
import { FIREWORKS_ADAPTER_ID } from './model-router'
import { InferenceProviderRegistry } from './provider-adapter'
import { recordFromUnknown } from '../json-boundary'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

// A fetch that returns a Fireworks-shaped SSE ReadableStream. Each frame is
// gated behind a `release` promise so the harness can control WHEN the upstream
// produces the next chunk — letting us prove the route forwards chunk N before
// the upstream emits chunk N+1.
type GatedFrame = Readonly<{
  frame: unknown
  // Resolves when the harness allows this frame to be emitted upstream.
  gate: Promise<void>
}>

const gatedSseFetch = (
  frames: ReadonlyArray<GatedFrame>,
  options: Readonly<{ withDone: boolean }> = { withDone: true },
): { fetchImpl: FetchLike; calls: number } => {
  const state = { calls: 0 }
  const fetchImpl: FetchLike = () => {
    state.calls += 1
    const encoder = new TextEncoder()
    let index = 0
    const stream = new ReadableStream<Uint8Array>({
      async pull(controller) {
        if (index >= frames.length) {
          if (options.withDone) {
            controller.enqueue(encoder.encode('data: [DONE]\n\n'))
          }
          controller.close()
          return
        }
        const { frame, gate } = frames[index]!
        index += 1
        await gate
        controller.enqueue(
          encoder.encode(`data: ${JSON.stringify(frame)}\n\n`),
        )
      },
    })
    return Promise.resolve(
      new Response(stream, {
        headers: { 'content-type': 'text/event-stream' },
        status: 200,
      }),
    )
  }
  return { calls: state.calls, fetchImpl }
}

// Narrow an unknown parsed SSE frame to its first-choice content delta without
// a type assertion (lint forbids `as` narrowing). Uses the repo json-boundary
// helper to coerce records.
const deltaContentOfSse = (frame: unknown): string => {
  const record = recordFromUnknown(frame)
  const choices = record?.['choices']
  if (!Array.isArray(choices) || choices.length === 0) {
    return ''
  }
  const delta = recordFromUnknown(choices[0])?.['delta']
  const content = recordFromUnknown(delta)?.['content']
  return typeof content === 'string' ? content : ''
}

const auth: InferenceAuth = async () => ({ accountRef: 'agent:harness' })

const deps = (
  overrides: Partial<ChatCompletionsDeps>,
): ChatCompletionsDeps => ({
  authenticate: auth,
  enabled: true,
  readAvailableMsat: async () => 100_000,
  registry: new InferenceProviderRegistry(),
  ...overrides,
})

const streamRequest = (model: string): Request =>
  new Request('https://openagents.com/v1/chat/completions', {
    body: JSON.stringify({
      messages: [{ content: 'build a crossy-road game', role: 'user' }],
      model,
      stream: true,
    }),
    method: 'POST',
  })

// Read SSE `data:` payloads from a stream reader, one decoded text chunk at a
// time, surfacing each `chat.completion.chunk` content delta as it arrives.
const readDeltas = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
  onDelta: (delta: string) => void,
): Promise<string> => {
  const decoder = new TextDecoder()
  let buffer = ''
  let full = ''
  for (;;) {
    const { done, value } = await reader.read()
    if (value !== undefined) {
      buffer += decoder.decode(value, { stream: true })
    }
    let nl = buffer.indexOf('\n')
    while (nl !== -1) {
      const line = buffer.slice(0, nl).trim()
      buffer = buffer.slice(nl + 1)
      if (line.startsWith('data:')) {
        const payload = line.slice('data:'.length).trim()
        if (payload !== '' && payload !== '[DONE]') {
          const delta = deltaContentOfSse(JSON.parse(payload))
          if (delta !== '') {
            full += delta
            onDelta(delta)
          }
        }
      }
      nl = buffer.indexOf('\n')
    }
    if (done) {
      break
    }
  }
  return full
}

describe('fireworks streamSse — local route pass-through harness', () => {
  test('forwards each upstream chunk to the client BEFORE the next upstream chunk is produced (no server-side buffering)', async () => {
    // Three gated frames: two content frames and a terminal usage frame. The
    // second/third gates stay closed until the client has SEEN the first
    // delta(s) — only possible if the route streams through incrementally.
    let release1 = (): void => {}
    let release2 = (): void => {}
    let release3 = (): void => {}
    const gate1 = new Promise<void>(r => (release1 = r))
    const gate2 = new Promise<void>(r => (release2 = r))
    const gate3 = new Promise<void>(r => (release3 = r))

    const { fetchImpl } = gatedSseFetch([
      { frame: { choices: [{ delta: { content: 'AAA' }, index: 0 }] }, gate: gate1 },
      { frame: { choices: [{ delta: { content: 'BBB' }, index: 0 }] }, gate: gate2 },
      {
        frame: {
          choices: [{ delta: {}, finish_reason: 'stop', index: 0 }],
          model: 'accounts/fireworks/models/kimi-k2p7-code',
          usage: { completion_tokens: 6, prompt_tokens: 12, total_tokens: 18 },
        },
        gate: gate3,
      },
    ])

    const adapter = makeFireworksAdapter({
      fetchImpl,
      getApiKey: () => 'fw-test',
    })
    const registry = new InferenceProviderRegistry()
    registry.register(adapter)

    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: true, receiptRef: 'rcpt-harness' }
      })

    const response = await run(
      handleChatCompletions(
        streamRequest('khala-code'),
        deps({
          lanePlan: () => [FIREWORKS_ADAPTER_ID],
          meteringHook,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    expect(response.body).not.toBeNull()
    const reader = response.body!.getReader()

    const seen: Array<string> = []
    // Open the first gate so the upstream emits 'AAA'.
    release1()
    // Drive the consumer in the background; as each delta lands, open the next
    // gate. If the route buffered, the consumer would never see 'AAA' until the
    // whole upstream completed — but the upstream is BLOCKED on gate2/gate3, so a
    // buffered route would deadlock here and the test would time out (a red).
    const full = await readDeltas(reader, delta => {
      seen.push(delta)
      if (delta === 'AAA') {
        release2()
      }
      if (delta === 'BBB') {
        release3()
      }
    })

    expect(seen).toEqual(['AAA', 'BBB'])
    expect(full).toBe('AAABBB')
    // Receipt-first metering settled from the terminal usage frame.
    expect(captured).toHaveLength(1)
    expect(captured[0]?.usage.totalTokens).toBe(18)
    expect(captured[0]?.servedModel).toBe(
      'accounts/fireworks/models/kimi-k2p7-code',
    )
    expect(captured[0]?.streamed).toBe(true)
  })

  test('the missing-terminal-frame case: stream closes cleanly, no metering (never an estimate)', async () => {
    const open = Promise.resolve()
    // Only content frames, NO terminal usage frame (the prod short-prompt symptom
    // before stream_options.include_usage).
    const { fetchImpl } = gatedSseFetch([
      { frame: { choices: [{ delta: { content: 'no-usage' }, index: 0 }] }, gate: open },
    ])

    const adapter = makeFireworksAdapter({
      fetchImpl,
      getApiKey: () => 'fw-test',
    })
    const registry = new InferenceProviderRegistry()
    registry.register(adapter)

    const captured: Array<MeteringContext> = []
    const meteringHook: MeteringHook = context =>
      Effect.sync(() => {
        captured.push(context)
        return { metered: false, receiptRef: null }
      })

    const response = await run(
      handleChatCompletions(
        streamRequest('khala-code'),
        deps({
          lanePlan: () => [FIREWORKS_ADAPTER_ID],
          meteringHook,
          registry,
        }),
      ),
    )

    expect(response.status).toBe(200)
    const text = await response.text()
    expect(text).toContain('"content":"no-usage"')
    expect(text.trimEnd().endsWith('data: [DONE]')).toBe(true)
    // Receipt-first: no terminal usage => no metering at all (never an estimate).
    expect(captured).toHaveLength(0)
  })
})
