// Durable inference resume-read route tests (durable-stream Rank-1, #6058).
// Proves: the route only matches durable read URLs; it is INERT (404) when the
// gateway/durable flag is off or the store is unwired; a wired store replays the
// persisted suffix with the resume headers; and it NEVER meters (it has no
// metering hook at all — it reads stored bytes only).

import {
  MemoryStreamStore,
  type StreamStore,
  handleRequest,
  streamIdFromUrl,
} from '@openagentsinc/durable-stream'
import { describe, expect, test } from 'vitest'

import {
  type DurableStreamNamespace,
  teeUpstreamToDurableDO,
} from './durable-inference-do-transport'
import {
  durableInferenceReadUrl,
  teeUpstreamToDurable,
} from './durable-inference-proxy'
import {
  routeDurableInferenceReadRequest,
  routeDurableInferenceReadRequestDO,
} from './durable-inference-read-routes'
import {
  type InferenceStreamSource,
  type InferenceUsage,
} from './provider-adapter'

const NOW = 1_700_000_000_000

const usage: InferenceUsage = {
  completionTokens: 3,
  promptTokens: 4,
  totalTokens: 7,
}

// Persist a small completion into a store under `requestId` (the producer path).
const seed = async (store: MemoryStreamStore, requestId: string) => {
  const src: InferenceStreamSource = {
    frames: (async function* () {
      yield { contentDelta: 'AAA' }
      yield { contentDelta: 'BBB' }
    })(),
    terminal: () => ({ finishReason: 'stop', servedModel: 'm', usage }),
  }
  await teeUpstreamToDurable({
    emit: () => {},
    frameForDelta: delta => `data: ${delta}\n\n`,
    nowMs: NOW,
    onEof: async () => 'data: [done]\n\n',
    requestId,
    source: src,
    store,
  })
}

const req = (path: string, method = 'GET'): Request =>
  new Request(`https://openagents.com${path}`, { method })

describe('routeDurableInferenceReadRequest', () => {
  test('returns undefined for a non-durable URL (router falls through)', () => {
    const result = routeDurableInferenceReadRequest(
      req('/v1/chat/completions'),
      { durableStream: undefined, enabled: true, nowEpochMillis: () => NOW },
    )
    expect(result).toBeUndefined()
  })

  test('INERT 404 when the gateway/durable flag is off', () => {
    const store = new MemoryStreamStore()
    const result = routeDurableInferenceReadRequest(
      req(durableInferenceReadUrl('req-1')),
      {
        durableStream: () => store,
        enabled: false,
        nowEpochMillis: () => NOW,
      },
    )
    expect(result?.status).toBe(404)
  })

  test('404 when the store factory is unwired', () => {
    const result = routeDurableInferenceReadRequest(
      req(durableInferenceReadUrl('req-1')),
      { durableStream: undefined, enabled: true, nowEpochMillis: () => NOW },
    )
    expect(result?.status).toBe(404)
  })

  test('404 for an unknown request id', () => {
    const store = new MemoryStreamStore()
    const result = routeDurableInferenceReadRequest(
      req(durableInferenceReadUrl('never')),
      {
        durableStream: () => store,
        enabled: true,
        nowEpochMillis: () => NOW,
      },
    )
    expect(result?.status).toBe(404)
  })

  test('405 on a non-GET method', () => {
    const store = new MemoryStreamStore()
    const result = routeDurableInferenceReadRequest(
      req(durableInferenceReadUrl('req-1'), 'POST'),
      {
        durableStream: () => store,
        enabled: true,
        nowEpochMillis: () => NOW,
      },
    )
    expect(result?.status).toBe(405)
  })

  test('replays the persisted suffix with resume headers and never meters', async () => {
    const store = new MemoryStreamStore()
    await seed(store, 'req-read')

    // Read from the beginning: full body + closed (EOF).
    const full = routeDurableInferenceReadRequest(
      req(durableInferenceReadUrl('req-read')),
      {
        durableStream: () => store,
        enabled: true,
        nowEpochMillis: () => NOW,
      },
    )
    expect(full?.status).toBe(200)
    expect(full?.headers.get('stream-closed')).toBe('true')
    expect(full?.headers.get('stream-next-offset')).not.toBeNull()
    const fullBody = await full!.text()
    expect(fullBody).toContain('AAA')
    expect(fullBody).toContain('BBB')

    // Resume from an offset past the first frame: the suffix excludes the seen
    // bytes (this proves resume-by-offset replays only the missing tail).
    const firstFrameBytes = new TextEncoder().encode('data: AAA\n\n').length
    const suffix = routeDurableInferenceReadRequest(
      req(`${durableInferenceReadUrl('req-read')}?offset=${firstFrameBytes}`),
      {
        durableStream: () => store,
        enabled: true,
        nowEpochMillis: () => NOW,
      },
    )
    expect(suffix?.status).toBe(200)
    const suffixBody = await suffix!.text()
    expect(suffixBody).not.toContain('AAA')
    expect(suffixBody).toContain('BBB')
  })

  test('400 on a malformed offset', async () => {
    const store = new MemoryStreamStore()
    await seed(store, 'req-bad')
    const result = routeDurableInferenceReadRequest(
      req(`${durableInferenceReadUrl('req-bad')}?offset=nope!`),
      {
        durableStream: () => store,
        enabled: true,
        nowEpochMillis: () => NOW,
      },
    )
    expect(result?.status).toBe(400)
  })
})

// In-process DO backend (one MemoryStreamStore per stream id via the package's
// real handleRequest) — the same backend the DO runs internally.
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
      return Promise.resolve(new Response('nope', { status: 404 }))
    }
    return handleRequest(this.storeFor(streamId), request, { streamId })
  }
}

const doNamespace = (
  registry: StreamRegistry = new StreamRegistry(),
): DurableStreamNamespace => ({
  getByName: () => ({ fetch: (r: Request) => registry.fetch(r) }),
})

const seedDO = async (namespace: DurableStreamNamespace, requestId: string) => {
  const src: InferenceStreamSource = {
    frames: (async function* () {
      yield { contentDelta: 'AAA' }
      yield { contentDelta: 'BBB' }
    })(),
    terminal: () => ({ finishReason: 'stop', servedModel: 'm', usage }),
  }
  await teeUpstreamToDurableDO({
    emit: () => {},
    frameForDelta: delta => `data: ${delta}\n\n`,
    namespace,
    onEof: async () => 'data: [done]\n\n',
    requestId,
    source: src,
  })
}

describe('routeDurableInferenceReadRequestDO (production DO-backed resume)', () => {
  test('returns undefined for a non-durable URL (router falls through)', async () => {
    const result = await routeDurableInferenceReadRequestDO(
      req('/v1/chat/completions'),
      { enabled: true, namespace: doNamespace() },
    )
    expect(result).toBeUndefined()
  })

  test('404 when the flag is off or the binding is absent', async () => {
    const offFlag = await routeDurableInferenceReadRequestDO(
      req(durableInferenceReadUrl('req-1')),
      { enabled: false, namespace: doNamespace() },
    )
    expect(offFlag?.status).toBe(404)

    const noBinding = await routeDurableInferenceReadRequestDO(
      req(durableInferenceReadUrl('req-1')),
      { enabled: true, namespace: undefined },
    )
    expect(noBinding?.status).toBe(404)
  })

  test('405 on a non-GET method', async () => {
    const result = await routeDurableInferenceReadRequestDO(
      req(durableInferenceReadUrl('req-1'), 'POST'),
      { enabled: true, namespace: doNamespace() },
    )
    expect(result?.status).toBe(405)
  })

  test('404 for an unknown request id', async () => {
    const result = await routeDurableInferenceReadRequestDO(
      req(durableInferenceReadUrl('never')),
      { enabled: true, namespace: doNamespace() },
    )
    expect(result?.status).toBe(404)
  })

  test('replays the persisted suffix from the DO with resume headers and never meters', async () => {
    const namespace = doNamespace()
    await seedDO(namespace, 'req-read')

    const full = await routeDurableInferenceReadRequestDO(
      req(`${durableInferenceReadUrl('req-read')}?offset=0`),
      { enabled: true, namespace },
    )
    expect(full?.status).toBe(200)
    expect(full?.headers.get('stream-closed')).toBe('true')
    expect(full?.headers.get('stream-next-offset')).not.toBeNull()
    const fullBody = await full!.text()
    expect(fullBody).toContain('AAA')
    expect(fullBody).toContain('BBB')

    const firstFrameBytes = new TextEncoder().encode('data: AAA\n\n').length
    const suffix = await routeDurableInferenceReadRequestDO(
      req(`${durableInferenceReadUrl('req-read')}?offset=${firstFrameBytes}`),
      { enabled: true, namespace },
    )
    expect(suffix?.status).toBe(200)
    const suffixBody = await suffix!.text()
    expect(suffixBody).not.toContain('AAA')
    expect(suffixBody).toContain('BBB')
  })

  test('400 on a malformed offset', async () => {
    const namespace = doNamespace()
    await seedDO(namespace, 'req-bad')
    const result = await routeDurableInferenceReadRequestDO(
      req(`${durableInferenceReadUrl('req-bad')}?offset=nope!`),
      { enabled: true, namespace },
    )
    expect(result?.status).toBe(400)
  })
})
