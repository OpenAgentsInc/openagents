// DO-fetch transport tests (durable-stream Rank-1, #6058). These prove the
// PRODUCTION wiring — the async `/v1/stream/{id}` HTTP transport to the per-request
// Durable Object — against the package's REAL `core.ts` + `http.ts` code paths
// (`TestStreamRegistry`, the same in-memory backend the conformance suite drives,
// keying one `MemoryStreamStore` per stream id exactly as the DO keys one DO per
// request id). This is the strongest oracle short of a live Workers runtime:
//
//   1. a streamed completion PERSISTS every frame to the DO log;
//   2. a CROSS-INVOCATION resume (a separate `replayFromOffsetDO` call, as a later
//      Worker invocation would do) replays the suffix from a mid-stream offset and
//      reconstructs the FULL completion — streaming-equivalence;
//   3. metering fires EXACTLY ONCE on the real upstream EOF and NEVER on a replay;
//   4. an upstream fault closes the DO stream WITHOUT metering (receipt-first);
//   5. a DO-fetch fault degrades the durable mirror but NEVER breaks the live
//      client stream and still meters once on EOF (fail-safe).

import {
  MemoryStreamStore,
  type StreamStore,
  handleRequest,
  streamIdFromUrl,
} from '@openagentsinc/durable-stream'
import { describe, expect, test } from 'vitest'

import {
  type DurableStreamNamespace,
  replayFromOffsetDO,
  teeUpstreamToDurableDO,
} from './durable-inference-do-transport'
import {
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'

// A minimal in-process Durable Streams backend: one `MemoryStreamStore` per
// stream id, routed through the package's REAL `handleRequest` (`core.ts` +
// `http.ts`) — the SAME code the Cloudflare DO runs internally
// (`handleDurableStreamFetch`). So this fake exercises actual DO behavior, not a
// re-implementation.
class StreamRegistry {
  private readonly stores = new Map<string, StreamStore>()

  private storeFor(streamId: string): StreamStore {
    let s = this.stores.get(streamId)
    if (s === undefined) {
      s = new MemoryStreamStore()
      this.stores.set(streamId, s)
    }
    return s
  }

  fetch(request: Request): Promise<Response> {
    const streamId = streamIdFromUrl(request.url)
    if (streamId === null) {
      return Promise.resolve(new Response('not a stream url', { status: 404 }))
    }
    return handleRequest(this.storeFor(streamId), request, { streamId })
  }
}

// A `DurableStreamNamespace` whose `getByName(id).fetch(req)` delegates to a
// shared `StreamRegistry`.
const fakeNamespace = (
  registry: StreamRegistry = new StreamRegistry(),
): DurableStreamNamespace => ({
  getByName: (_name: string) => ({
    fetch: (request: Request) => registry.fetch(request),
  }),
})

// A `DurableStreamNamespace` whose stub `.fetch` always rejects — models a DO
// transport fault (binding present but the DO is unreachable).
const faultingNamespace = (): DurableStreamNamespace => ({
  getByName: () => ({
    fetch: () => Promise.reject(new Error('DO unreachable')),
  }),
})

const usage: InferenceUsage = {
  completionTokens: 5,
  promptTokens: 9,
  totalTokens: 14,
}

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

// Drive `teeUpstreamToDurableDO`, collecting emitted client frames + the metering
// count (the `onEof` callback is where the route meters).
const drive = async (input: {
  namespace: DurableStreamNamespace
  requestId: string
  src: InferenceStreamSource
}) => {
  const emitted: Array<string> = []
  let meterCount = 0
  const outcome = await teeUpstreamToDurableDO({
    emit: frame => emitted.push(frame),
    frameForDelta: delta => `data: {"delta":${JSON.stringify(delta)}}\n\n`,
    namespace: input.namespace,
    onEof: async (terminal, content) => {
      if (terminal.usage !== undefined) {
        meterCount += 1
      }
      return `data: {"done":true,"content":${JSON.stringify(content)}}\n\n`
    },
    requestId: input.requestId,
    source: input.src,
  })
  return { emitted, meterCount, outcome }
}

describe('durable inference DO transport — persistence + cross-invocation resume', () => {
  test('a streamed completion persists every frame to the per-request DO log', async () => {
    const namespace = fakeNamespace()
    const { outcome } = await drive({
      namespace,
      requestId: 'req-persist',
      src: source([{ contentDelta: 'Hel' }, { contentDelta: 'lo' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
    })

    expect(outcome.faulted).toBe(false)
    expect(outcome.content).toBe('Hello')

    // A SEPARATE read (a later Worker invocation) reconstructs the full log from
    // offset 0 — both deltas + the terminal frame.
    const full = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'req-persist',
    })
    expect(full).toBeDefined()
    expect(full!.body).toContain('"Hel"')
    expect(full!.body).toContain('"lo"')
    expect(full!.body).toContain('"done":true')
    expect(full!.streamClosed).toBe(true)
  })

  test('a mid-stream disconnect + cross-invocation resume replays the suffix and reconstructs the full completion (streaming-equivalence)', async () => {
    const namespace = fakeNamespace()
    const { emitted } = await drive({
      namespace,
      requestId: 'req-resume',
      src: source(
        [
          { contentDelta: 'AAA' },
          { contentDelta: 'BBB' },
          { contentDelta: 'CCC' },
        ],
        { finishReason: 'stop', servedModel: 'served/m', usage },
      ),
    })

    // SIMULATE A DISCONNECT: the client only received the first frame. Resume
    // from the offset after that frame — as a fresh Worker invocation would, with
    // only the client-held last offset.
    const firstReadBytes = new TextEncoder().encode(emitted[0]!).length
    const resume = await replayFromOffsetDO({
      namespace,
      offset: String(firstReadBytes),
      requestId: 'req-resume',
    })
    expect(resume).toBeDefined()
    // The resumed suffix carries BBB + CCC + the terminal frame, not AAA.
    expect(resume!.body).not.toContain('"AAA"')
    expect(resume!.body).toContain('"BBB"')
    expect(resume!.body).toContain('"CCC"')
    expect(resume!.body).toContain('"done":true')
    expect(resume!.streamClosed).toBe(true)

    // The full completion = the first frame the client kept + the resumed suffix.
    const reconstructed = emitted[0]! + resume!.body
    expect(reconstructed).toContain('"AAA"')
    expect(reconstructed).toContain('"BBB"')
    expect(reconstructed).toContain('"CCC"')
  })

  test('mid-stream resume works while the stream is still OPEN (before EOF)', async () => {
    // Drive the producer manually so we can read the DO BEFORE the terminal frame
    // is appended — the real "client dropped mid-generation" case.
    const registry = new StreamRegistry()
    const namespace = fakeNamespace(registry)
    const requestId = 'req-open'

    let resolveSecond: (() => void) | undefined
    const secondGate = new Promise<void>(resolve => {
      resolveSecond = resolve
    })

    const src: InferenceStreamSource = {
      frames: (async function* () {
        yield { contentDelta: 'one' }
        // Pause after the first frame so a reader can observe an OPEN stream.
        await secondGate
        yield { contentDelta: 'two' }
      })(),
      terminal: () => ({
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
    }

    let meterCount = 0
    const producer = teeUpstreamToDurableDO({
      emit: () => {},
      frameForDelta: delta => `data: {"delta":${JSON.stringify(delta)}}\n\n`,
      namespace,
      onEof: async () => {
        meterCount += 1
        return `data: {"done":true}\n\n`
      },
      requestId,
      source: src,
    })

    // Give the first frame time to persist, then read the OPEN stream.
    await new Promise(resolve => setTimeout(resolve, 5))
    const openRead = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId,
    })
    expect(openRead).toBeDefined()
    expect(openRead!.body).toContain('"one"')
    // The stream is NOT closed yet — generation is still in flight.
    expect(openRead!.streamClosed).toBe(false)
    expect(meterCount).toBe(0)

    // Let the producer finish.
    resolveSecond?.()
    await producer
    expect(meterCount).toBe(1)
  })
})

describe('durable inference DO transport — METERING EXACTLY ONCE', () => {
  test('metering fires exactly once on the real upstream EOF', async () => {
    const namespace = fakeNamespace()
    const { meterCount, outcome } = await drive({
      namespace,
      requestId: 'req-meter-once',
      src: source([{ contentDelta: 'hi' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
    })
    expect(outcome.faulted).toBe(false)
    expect(meterCount).toBe(1)
  })

  test('metering does NOT fire on replay / resume reads', async () => {
    const namespace = fakeNamespace()
    const first = await drive({
      namespace,
      requestId: 'req-no-rebill',
      src: source([{ contentDelta: 'x' }, { contentDelta: 'y' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
    })
    expect(first.meterCount).toBe(1)

    // Many reconnects / catch-up reads. `replayFromOffsetDO` has NO metering hook,
    // so none can re-bill.
    for (let i = 0; i < 5; i++) {
      const replay = await replayFromOffsetDO({
        namespace,
        offset: '0',
        requestId: 'req-no-rebill',
      })
      expect(replay).toBeDefined()
    }
    // Still exactly one settlement from the single producer drain.
    expect(first.meterCount).toBe(1)
  })

  test('an upstream fault closes the DO stream WITHOUT metering (receipt-first)', async () => {
    const namespace = fakeNamespace()
    const { meterCount, outcome } = await drive({
      namespace,
      requestId: 'req-fault',
      src: source(
        [{ contentDelta: 'partial' }],
        { finishReason: undefined, servedModel: undefined, usage: undefined },
        { fault: true },
      ),
    })
    // The upstream faulted: no terminal frame, no metering.
    expect(outcome.faulted).toBe(true)
    expect(meterCount).toBe(0)

    // The partial content is still durable + the stream is closed (a reconnect
    // sees the closed partial completion).
    const replay = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'req-fault',
    })
    expect(replay).toBeDefined()
    expect(replay!.body).toContain('"partial"')
    expect(replay!.streamClosed).toBe(true)
  })
})

describe('durable inference DO transport — FAIL-SAFE', () => {
  test('a DO-fetch fault degrades the durable mirror but never breaks the live stream and still meters once', async () => {
    const namespace = faultingNamespace()
    const { emitted, meterCount, outcome } = await drive({
      namespace,
      requestId: 'req-do-down',
      src: source([{ contentDelta: 'aa' }, { contentDelta: 'bb' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
    })

    // The client STILL received every frame (deltas + terminal), and metering
    // settled exactly once — the broken durable substrate did not break the
    // completion. Resume is simply unavailable (the DO is down).
    expect(outcome.faulted).toBe(false)
    expect(emitted.some(f => f.includes('"aa"'))).toBe(true)
    expect(emitted.some(f => f.includes('"bb"'))).toBe(true)
    expect(emitted.some(f => f.includes('"done":true'))).toBe(true)
    expect(meterCount).toBe(1)
  })

  test('replayFromOffsetDO returns undefined for an unknown request id (404)', async () => {
    const namespace = fakeNamespace()
    const replay = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'never-created',
    })
    expect(replay).toBeUndefined()
  })
})
