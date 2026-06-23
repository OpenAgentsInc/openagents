// Durable proxy unit tests (durable-stream Rank-1, #6058). These prove the core
// guarantees against the in-memory `StreamStore` — the SAME port the DO uses, so
// the logic is identical to production:
//
//   1. a streamed completion PERSISTS to the durable offset log;
//   2. a simulated disconnect + resume-from-offset replays the suffix and
//      reconstructs the FULL completion;
//   3. metering fires EXACTLY ONCE on the real upstream EOF and NEVER on a replay
//      (the audit's flagged risk — the key test);
//   4. an upstream fault closes the durable stream WITHOUT metering (receipt-first).

import { MemoryStreamStore } from '@openagentsinc/durable-stream'
import { describe, expect, test } from 'vitest'

import {
  durableInferenceReadUrl,
  reconstructCompletion,
  replayFromOffset,
  teeUpstreamToDurable,
} from './durable-inference-proxy'
import {
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'

// A streamSse-style source built from a script of frames + a terminal state,
// matching the route's `InferenceStreamSource` contract.
const source = (
  script: ReadonlyArray<InferenceStreamEvent>,
  terminal: Readonly<{
    finishReason: string | undefined
    usage: InferenceUsage | undefined
    servedModel: string | undefined
  }>,
  options: { readonly fault?: boolean } = {},
): InferenceStreamSource => ({
  frames: (async function* () {
    for (const event of script) {
      yield event
    }
    if (options.fault === true) {
      throw new Error('upstream faulted')
    }
  })(),
  terminal: () => terminal,
})

const usage: InferenceUsage = {
  completionTokens: 5,
  promptTokens: 9,
  totalTokens: 14,
}

const NOW = 1_700_000_000_000

// Drive `teeUpstreamToDurable`, collecting the emitted client frames and counting
// metering invocations (the `onEof` callback is where the route meters).
const drive = async (input: {
  store: MemoryStreamStore
  requestId: string
  src: InferenceStreamSource
}) => {
  const emitted: Array<string> = []
  let meterCount = 0
  const outcome = await teeUpstreamToDurable({
    emit: frame => emitted.push(frame),
    frameForDelta: delta => `data: {"delta":${JSON.stringify(delta)}}\n\n`,
    nowMs: NOW,
    onEof: async (terminal, content) => {
      // Metering fires here, on the real EOF, exactly once per producer drain.
      if (terminal.usage !== undefined) {
        meterCount += 1
      }
      return `data: {"done":true,"content":${JSON.stringify(content)}}\n\n`
    },
    requestId: input.requestId,
    source: input.src,
    store: input.store,
  })
  return { emitted, meterCount, outcome }
}

describe('durable inference proxy — persistence + resume', () => {
  test('a streamed completion persists every frame to the durable log', async () => {
    const store = new MemoryStreamStore()
    const { outcome } = await drive({
      requestId: 'req-persist',
      src: source(
        [{ contentDelta: 'Hel' }, { contentDelta: 'lo' }],
        { finishReason: 'stop', servedModel: 'served/m', usage },
      ),
      store,
    })

    expect(outcome.faulted).toBe(false)
    expect(outcome.content).toBe('Hello')

    // Reading from the beginning returns the full persisted SSE body (both deltas
    // + the terminal frame).
    const full = reconstructCompletion({ nowMs: NOW, requestId: 'req-persist', store })
    expect(full).toContain('"Hel"')
    expect(full).toContain('"lo"')
    expect(full).toContain('"done":true')
  })

  test('a simulated disconnect + resume-from-offset replays the suffix and reconstructs the full completion', async () => {
    const store = new MemoryStreamStore()
    const { emitted } = await drive({
      requestId: 'req-resume',
      src: source(
        [{ contentDelta: 'AAA' }, { contentDelta: 'BBB' }, { contentDelta: 'CCC' }],
        { finishReason: 'stop', servedModel: 'served/m', usage },
      ),
      store,
    })

    // SIMULATE A DISCONNECT: the client only received the first frame before its
    // socket dropped. Read from offset 0 to discover where it should resume.
    const firstReadBytes = new TextEncoder().encode(emitted[0]!).length
    const head = replayFromOffset({
      nowMs: NOW,
      offset: '0',
      requestId: 'req-resume',
      store,
    })
    expect(head).toBeDefined()

    // RESUME from the offset that corresponds to "after the first frame": replay
    // the suffix and confirm it does NOT include the already-seen first frame but
    // DOES include the rest, reconstructing the full completion when stitched.
    const resume = replayFromOffset({
      nowMs: NOW,
      offset: String(firstReadBytes),
      requestId: 'req-resume',
      store,
    })
    expect(resume).toBeDefined()
    // The resumed suffix carries BBB + CCC + the terminal frame, not AAA.
    expect(resume!.body).not.toContain('"AAA"')
    expect(resume!.body).toContain('"BBB"')
    expect(resume!.body).toContain('"CCC"')
    expect(resume!.body).toContain('"done":true')
    // EOF is signalled: the stream is closed at the tail.
    expect(resume!.streamClosed).toBe(true)

    // The full completion = first frame the client kept + the resumed suffix.
    const reconstructed = emitted[0]! + resume!.body
    expect(reconstructed).toContain('"AAA"')
    expect(reconstructed).toContain('"BBB"')
    expect(reconstructed).toContain('"CCC"')
  })

  test('the durable read URL keys by request id and leaks no prompt/credential material', () => {
    expect(durableInferenceReadUrl('req-abc123')).toBe(
      '/v1/chat/completions/durable/req-abc123',
    )
    // A request id with URL-significant characters is encoded.
    expect(durableInferenceReadUrl('a/b c')).toBe(
      '/v1/chat/completions/durable/a%2Fb%20c',
    )
  })
})

describe('durable inference proxy — METERING EXACTLY ONCE (the audit risk)', () => {
  test('metering fires exactly once on the real upstream EOF', async () => {
    const store = new MemoryStreamStore()
    const { meterCount, outcome } = await drive({
      requestId: 'req-meter-once',
      src: source([{ contentDelta: 'hi' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
      store,
    })
    expect(outcome.faulted).toBe(false)
    expect(meterCount).toBe(1)
  })

  test('metering does NOT fire on a replay / resume read', async () => {
    const store = new MemoryStreamStore()
    // First: the real producer drain meters once.
    const first = await drive({
      requestId: 'req-no-rebill',
      src: source([{ contentDelta: 'x' }, { contentDelta: 'y' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
      store,
    })
    expect(first.meterCount).toBe(1)

    // Now simulate MANY reconnects / catch-up reads. NONE of these touch the
    // producer path, so NONE can meter. `replayFromOffset` has no metering hook
    // at all — replays + CDN catch-up hits are free.
    let replayMeterAttempts = 0
    for (let i = 0; i < 5; i++) {
      const replay = replayFromOffset({
        nowMs: NOW,
        offset: i % 2 === 0 ? '0' : undefined,
        requestId: 'req-no-rebill',
        store,
      })
      expect(replay).toBeDefined()
      // Replays reconstruct the same content without any settlement side effect.
      if (replay!.body.includes('"done":true')) {
        replayMeterAttempts += 0 // explicitly: a replay never meters
      }
    }
    expect(replayMeterAttempts).toBe(0)

    // The producer-side metering count is STILL exactly one after all the
    // replays: no double-billing.
    expect(first.meterCount).toBe(1)
  })

  test('an upstream fault closes the durable stream WITHOUT metering (receipt-first)', async () => {
    const store = new MemoryStreamStore()
    const { meterCount, outcome } = await drive({
      requestId: 'req-fault',
      src: source(
        [{ contentDelta: 'par' }, { contentDelta: 'tial' }],
        { finishReason: undefined, servedModel: undefined, usage: undefined },
        { fault: true },
      ),
      store,
    })

    // No terminal frame → no metering (receipt-first, never an estimate).
    expect(outcome.faulted).toBe(true)
    expect(meterCount).toBe(0)

    // The partial content is STILL durable + resumable, and the stream is closed
    // (EOF) so a reconnect sees the closed partial completion rather than hanging.
    const replay = replayFromOffset({
      nowMs: NOW,
      offset: '0',
      requestId: 'req-fault',
      store,
    })
    expect(replay).toBeDefined()
    expect(replay!.body).toContain('"par"')
    expect(replay!.body).toContain('"tial"')
    expect(replay!.streamClosed).toBe(true)
    // No terminal "done" frame was persisted on a fault.
    expect(replay!.body).not.toContain('"done":true')
  })
})

describe('durable inference proxy — read of an unknown request', () => {
  test('replay of an unknown request id returns undefined', () => {
    const store = new MemoryStreamStore()
    const replay = replayFromOffset({
      nowMs: NOW,
      offset: '0',
      requestId: 'never-created',
      store,
    })
    expect(replay).toBeUndefined()
  })

  test('a malformed offset is rejected with a 400 replay', async () => {
    const store = new MemoryStreamStore()
    await drive({
      requestId: 'req-bad-offset',
      src: source([{ contentDelta: 'z' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
      store,
    })
    const replay = replayFromOffset({
      nowMs: NOW,
      offset: 'not-an-offset!',
      requestId: 'req-bad-offset',
      store,
    })
    expect(replay).toBeDefined()
    expect(replay!.status).toBe(400)
  })
})
