// Postgres-backed durable inference stream tests (CFG-6, #8521). These port
// the DO-transport oracle (durable-inference-do-transport.test.ts) to the new
// backend: the SAME production transport functions (`teeUpstreamToDurableDO`,
// `replayFromOffsetDO`, `seedDurableInferenceStreamDO`) drive the
// `DurableStreamNamespace` that `makeDurableInferenceStreamNamespace` builds
// over the oa-infra `DurableStreamShape` — the exact seam production wires to
// Postgres via KHALA_SYNC_DB (the Postgres backend itself is proven by the
// oa-infra conformance suite's `postgres` + `postgres.js` runs; the in-memory
// backend here passes the same suite, so it is a faithful stand-in):
//
//   1. a streamed completion PERSISTS every frame to the per-request log;
//   2. a CROSS-INVOCATION resume replays the byte suffix from a mid-stream
//      offset and reconstructs the FULL completion (streaming-equivalence,
//      byte-compatible with the DO's zero-padded byte offsets);
//   3. metering fires EXACTLY ONCE on the real upstream EOF, NEVER on replay;
//   4. an upstream fault closes the stream WITHOUT metering (receipt-first);
//   5. a backend fault degrades the durable mirror but NEVER breaks the live
//      client stream and still meters once (fail-safe);
//   6. the `/v1/stream/{id}` shim honors the DO's offset sentinels (-1/now),
//      malformed-offset 400, unknown-id 404, and created-but-empty 200;
//   7. the postgres.js session lifecycle: ended after the close POST and
//      after every GET, reopened transparently for later operations.

import { makeMemoryDurableStream } from '@openagentsinc/oa-infra/durable-stream-memory'
import { describe, expect, test } from 'vitest'

import {
  type DurableInferenceStreamSession,
  makeDurableInferenceStreamNamespace,
} from './durable-inference-stream-backend'
import {
  type DurableStreamNamespace,
  replayFromOffsetDO,
  seedDurableInferenceStreamDO,
  teeUpstreamToDurableDO,
} from './durable-inference-do-transport'
import {
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'

// A namespace over the oa-infra in-memory DurableStream backend, with session
// open/end accounting so the connection-lifecycle contract is observable.
const memoryNamespace = (): {
  namespace: DurableStreamNamespace
  counters: { opened: number; ended: number; cleanups: number }
} => {
  const streams = makeMemoryDurableStream()
  const counters = { cleanups: 0, ended: 0, opened: 0 }
  const namespace = makeDurableInferenceStreamNamespace(async () => {
    counters.opened += 1
    const session: DurableInferenceStreamSession = {
      cleanupExpired: async () => {
        counters.cleanups += 1
      },
      end: async () => {
        counters.ended += 1
      },
      streams,
    }
    return session
  })
  return { counters, namespace }
}

// A namespace whose session can never open — models an unreachable Postgres
// (the analogue of the DO-transport test's faulting namespace).
const faultingNamespace = (): DurableStreamNamespace =>
  makeDurableInferenceStreamNamespace(async () => {
    throw new Error('postgres unreachable')
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

// Drive `teeUpstreamToDurableDO` (the UNCHANGED production producer),
// collecting emitted client frames + the metering count.
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

describe('durable inference Postgres-backend transport — persistence + cross-invocation resume', () => {
  test('a streamed completion persists every frame to the per-request log', async () => {
    const { namespace } = memoryNamespace()
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
    expect(full!.upToDate).toBe(true)
  })

  test('a mid-stream disconnect + cross-invocation resume replays the byte suffix and reconstructs the full completion', async () => {
    const { namespace } = memoryNamespace()
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
    // from the BYTE offset after that frame — the exact offset arithmetic DO
    // clients used (public offsets stay byte positions).
    const firstReadBytes = new TextEncoder().encode(emitted[0]!).length
    const resume = await replayFromOffsetDO({
      namespace,
      offset: String(firstReadBytes),
      requestId: 'req-resume',
    })
    expect(resume).toBeDefined()
    expect(resume!.body).not.toContain('"AAA"')
    expect(resume!.body).toContain('"BBB"')
    expect(resume!.body).toContain('"CCC"')
    expect(resume!.body).toContain('"done":true')
    expect(resume!.streamClosed).toBe(true)

    const reconstructed = emitted[0]! + resume!.body
    expect(reconstructed).toContain('"AAA"')
    expect(reconstructed).toContain('"BBB"')
    expect(reconstructed).toContain('"CCC"')

    // The resume cursor is the DO's zero-padded byte-position codec.
    const totalBytes = emitted.reduce(
      (sum, frame) => sum + new TextEncoder().encode(frame).length,
      0,
    )
    expect(resume!.nextOffset).toBe(String(totalBytes).padStart(18, '0'))
  })

  test('mid-stream resume works while the stream is still OPEN (before EOF)', async () => {
    const { namespace } = memoryNamespace()
    const requestId = 'req-open'

    let resolveSecond: (() => void) | undefined
    const secondGate = new Promise<void>(resolve => {
      resolveSecond = resolve
    })

    const src: InferenceStreamSource = {
      frames: (async function* () {
        yield { contentDelta: 'one' }
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

    await new Promise(resolve => setTimeout(resolve, 5))
    const openRead = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId,
    })
    expect(openRead).toBeDefined()
    expect(openRead!.body).toContain('"one"')
    expect(openRead!.streamClosed).toBe(false)
    expect(meterCount).toBe(0)

    resolveSecond?.()
    await producer
    expect(meterCount).toBe(1)
  })
})

describe('durable inference Postgres-backend transport — METERING EXACTLY ONCE', () => {
  test('metering fires exactly once on the real upstream EOF', async () => {
    const { namespace } = memoryNamespace()
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
    const { namespace } = memoryNamespace()
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

    for (let i = 0; i < 5; i++) {
      const replay = await replayFromOffsetDO({
        namespace,
        offset: '0',
        requestId: 'req-no-rebill',
      })
      expect(replay).toBeDefined()
    }
    expect(first.meterCount).toBe(1)
  })

  test('an upstream fault closes the stream WITHOUT metering (receipt-first)', async () => {
    const { namespace } = memoryNamespace()
    const { meterCount, outcome } = await drive({
      namespace,
      requestId: 'req-fault',
      src: source(
        [{ contentDelta: 'partial' }],
        { finishReason: undefined, servedModel: undefined, usage: undefined },
        { fault: true },
      ),
    })
    expect(outcome.faulted).toBe(true)
    expect(meterCount).toBe(0)

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

describe('durable inference Postgres-backend transport — FAIL-SAFE', () => {
  test('an unreachable backend degrades the durable mirror but never breaks the live stream and still meters once', async () => {
    const namespace = faultingNamespace()
    const { emitted, meterCount, outcome } = await drive({
      namespace,
      requestId: 'req-pg-down',
      src: source([{ contentDelta: 'aa' }, { contentDelta: 'bb' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
    })

    expect(outcome.faulted).toBe(false)
    expect(emitted.some(f => f.includes('"aa"'))).toBe(true)
    expect(emitted.some(f => f.includes('"bb"'))).toBe(true)
    expect(emitted.some(f => f.includes('"done":true'))).toBe(true)
    expect(meterCount).toBe(1)
  })

  test('replayFromOffsetDO returns undefined for an unknown request id (404)', async () => {
    const { namespace } = memoryNamespace()
    const replay = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'never-created',
    })
    expect(replay).toBeUndefined()
  })
})

describe('durable inference Postgres-backend shim — /v1/stream contract details', () => {
  test('seed + close persists delegation frames and seals the stream (the khala resume contract)', async () => {
    const { namespace } = memoryNamespace()
    const frame = 'data: {"delegation":true}\n\n'
    const done = 'data: [DONE]\n\n'
    const seeded = await seedDurableInferenceStreamDO({
      close: true,
      frames: [frame, done],
      namespace,
      requestId: 'req-delegation',
    })
    expect(seeded).toBe(true)

    // `khala resume <durableRequestId> --offset 0`.
    const replay = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'req-delegation',
    })
    expect(replay).toBeDefined()
    expect(replay!.body).toBe(`${frame}${done}`)
    expect(replay!.streamClosed).toBe(true)
    expect(replay!.upToDate).toBe(true)
    expect(replay!.nextOffset).toBe(
      String(new TextEncoder().encode(`${frame}${done}`).length).padStart(
        18,
        '0',
      ),
    )
  })

  test('a created-but-empty stream reads 200 empty/open (not 404)', async () => {
    const { namespace } = memoryNamespace()
    const seeded = await seedDurableInferenceStreamDO({
      frames: [],
      namespace,
      requestId: 'req-created-empty',
    })
    expect(seeded).toBe(true)
    const replay = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'req-created-empty',
    })
    expect(replay).toBeDefined()
    expect(replay!.status).toBe(200)
    expect(replay!.body).toBe('')
    expect(replay!.streamClosed).toBe(false)
    expect(replay!.upToDate).toBe(true)
    expect(replay!.nextOffset).toBe('0'.padStart(18, '0'))
  })

  test('offset sentinels: omitted and -1 read from the beginning, now reads the empty tail', async () => {
    const { namespace } = memoryNamespace()
    await seedDurableInferenceStreamDO({
      close: true,
      frames: ['data: hi\n\n'],
      namespace,
      requestId: 'req-sentinels',
    })

    const omitted = await replayFromOffsetDO({
      namespace,
      offset: undefined,
      requestId: 'req-sentinels',
    })
    expect(omitted!.body).toBe('data: hi\n\n')

    const beginning = await replayFromOffsetDO({
      namespace,
      offset: '-1',
      requestId: 'req-sentinels',
    })
    expect(beginning!.body).toBe('data: hi\n\n')

    const now = await replayFromOffsetDO({
      namespace,
      offset: 'now',
      requestId: 'req-sentinels',
    })
    expect(now!.body).toBe('')
    expect(now!.upToDate).toBe(true)
    expect(now!.streamClosed).toBe(true)
  })

  test('a malformed offset is a deterministic 400', async () => {
    const { namespace } = memoryNamespace()
    await seedDurableInferenceStreamDO({
      close: true,
      frames: ['data: hi\n\n'],
      namespace,
      requestId: 'req-bad-offset',
    })
    const replay = await replayFromOffsetDO({
      namespace,
      offset: 'not-a-number',
      requestId: 'req-bad-offset',
    })
    expect(replay).toBeDefined()
    expect(replay!.status).toBe(400)
  })

  test('an offset past the tail clamps to the tail (empty up-to-date read)', async () => {
    const { namespace } = memoryNamespace()
    const frame = 'data: tail\n\n'
    await seedDurableInferenceStreamDO({
      close: true,
      frames: [frame],
      namespace,
      requestId: 'req-past-tail',
    })
    const replay = await replayFromOffsetDO({
      namespace,
      offset: '999999',
      requestId: 'req-past-tail',
    })
    expect(replay!.body).toBe('')
    expect(replay!.upToDate).toBe(true)
    expect(replay!.streamClosed).toBe(true)
    expect(replay!.nextOffset).toBe(
      String(new TextEncoder().encode(frame).length).padStart(18, '0'),
    )
  })

  test('re-creating a COMPLETED request id is refused (PUT 409 → seed reports unavailable)', async () => {
    const { namespace } = memoryNamespace()
    await seedDurableInferenceStreamDO({
      close: true,
      frames: ['data: first\n\n'],
      namespace,
      requestId: 'req-recreate',
    })
    // The stream is sealed; a new producer PUT for the same id conflicts —
    // the transport treats that as "durable unavailable" (false), it never
    // truncates or reopens the sealed log.
    const reseeded = await seedDurableInferenceStreamDO({
      close: true,
      frames: ['data: second\n\n'],
      namespace,
      requestId: 'req-recreate',
    })
    expect(reseeded).toBe(false)
    const replay = await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'req-recreate',
    })
    expect(replay!.body).toBe('data: first\n\n')
  })
})

describe('durable inference Postgres-backend shim — session lifecycle', () => {
  test('the producer session is ended by the closing POST; a GET opens and ends a fresh one', async () => {
    const { counters, namespace } = memoryNamespace()
    await drive({
      namespace,
      requestId: 'req-lifecycle',
      src: source([{ contentDelta: 'x' }], {
        finishReason: 'stop',
        servedModel: 'served/m',
        usage,
      }),
    })
    // One session across PUT + appends, ended by the close POST.
    expect(counters.opened).toBe(1)
    expect(counters.ended).toBe(1)
    // TTL sweep ran once, at stream creation.
    expect(counters.cleanups).toBe(1)

    await replayFromOffsetDO({
      namespace,
      offset: '0',
      requestId: 'req-lifecycle',
    })
    // The GET opened a fresh session and ended it with the response.
    expect(counters.opened).toBe(2)
    expect(counters.ended).toBe(2)
  })
})
