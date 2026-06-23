// Onboarding incremental-stream wiring (issue #6154).
//
// Proves the two properties the buffered fake-stream violated:
//   1. PREFER streamSse. A mock adapter with `streamSse` yielding MANY frames
//      produces MANY onboarding `deltas` (token-by-token), not one buffered
//      reply. This is the fix: each upstream fragment becomes its own delta.
//   2. FALLBACK to stream. A mock adapter with NO `streamSse` still works via
//      the buffered chunk `stream` path (so existing adapters/tests don't
//      regress).

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceStreamEvent,
  type InferenceStreamSource,
} from './provider-adapter'
import { dispatchOnboardingStreamSource } from './onboarding-stream-source'

const run = <A>(effect: Effect.Effect<A, InferenceAdapterError>): Promise<A> =>
  Effect.runPromise(effect)

const request: InferenceRequest = {
  messages: [{ content: 'I run a bakery.', role: 'user' }],
  model: 'openagents/khala-mini',
  passthroughParams: {},
  stream: true,
}

const drain = async (
  deltas: AsyncIterable<string>,
): Promise<Array<string>> => {
  const collected: Array<string> = []
  for await (const delta of deltas) {
    collected.push(delta)
  }
  return collected
}

// A mock adapter whose `streamSse` yields the given content frames plus a
// terminal usage frame, mirroring the real adapter contract.
const streamSseAdapter = (
  contents: ReadonlyArray<string>,
): InferenceProviderAdapter => ({
  complete: () => Effect.die('unused'),
  id: 'mock-stream-sse',
  stream: () => Effect.die('mock streamSse adapter should not use buffered stream'),
  streamSse: () =>
    Effect.sync((): InferenceStreamSource => {
      const events: Array<InferenceStreamEvent> = [
        ...contents.map(content => ({ contentDelta: content })),
        {
          contentDelta: '',
          finishReason: 'stop',
          servedModel: 'gemini-3.5-flash',
          usage: { completionTokens: 3, promptTokens: 5, totalTokens: 8 },
        },
      ]
      return {
        frames: (async function* () {
          for (const event of events) {
            yield event
          }
        })(),
        terminal: () => ({
          finishReason: 'stop',
          servedModel: 'gemini-3.5-flash',
          usage: { completionTokens: 3, promptTokens: 5, totalTokens: 8 },
        }),
      }
    }),
})

// A mock adapter with NO `streamSse` — only the buffered chunk `stream`.
const bufferedOnlyAdapter = (
  contents: ReadonlyArray<string>,
): InferenceProviderAdapter => ({
  complete: () => Effect.die('unused'),
  id: 'mock-buffered',
  stream: () =>
    Effect.succeed([
      ...contents.map(content => ({ contentDelta: content })),
      {
        contentDelta: '',
        finishReason: 'stop',
        usage: { completionTokens: 3, promptTokens: 5, totalTokens: 8 },
      },
    ]),
})

describe('dispatchOnboardingStreamSource', () => {
  test('prefers streamSse: a multi-frame source yields MANY deltas (token-by-token)', async () => {
    const adapter = streamSseAdapter(['Great', ' — ', 'what', ' next?'])
    const source = await run(dispatchOnboardingStreamSource(adapter, request))

    const deltas = await drain(source.deltas)
    // One delta per upstream content frame; the empty terminal frame is skipped.
    expect(deltas).toEqual(['Great', ' — ', 'what', ' next?'])
    expect(deltas.length).toBeGreaterThan(1)
    // final() returns '' on the streamSse path (no content re-buffering); the
    // route falls back to its own accumulation.
    expect(source.final()).toBe('')
  })

  test('falls back to buffered stream when the adapter has no streamSse', async () => {
    const adapter = bufferedOnlyAdapter(['ok', ' done'])
    expect(adapter.streamSse).toBeUndefined()
    const source = await run(dispatchOnboardingStreamSource(adapter, request))

    const deltas = await drain(source.deltas)
    expect(deltas).toEqual(['ok', ' done'])
    // The buffered path can cheaply rejoin (content already materialized).
    expect(source.final()).toBe('ok done')
  })

  test('skips empty content deltas (terminal usage frame is not a delta)', async () => {
    const adapter = streamSseAdapter(['only'])
    const source = await run(dispatchOnboardingStreamSource(adapter, request))
    const deltas = await drain(source.deltas)
    expect(deltas).toEqual(['only'])
  })
})
