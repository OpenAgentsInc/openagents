/**
 * CFG-9 (#8524): Durable Object namespace shims for the Cloud Run monolith.
 *
 * - `makeUnavailableDurableObjectNamespace`: id/stub construction stays
 *   synchronous (matching the DO API), and every call ON the stub rejects
 *   with the typed `BindingUnavailableError` → 503 pattern. Used for the DO
 *   classes that have no owned replacement yet (SYNC_ROOM, the MDK
 *   containers, AGENT_DEFINITION_SCHEDULER, EVENT_LEDGER_OWNER).
 *
 * - `makeInMemoryDurableStreamNamespace`: an in-process implementation of
 *   the `@openagentsinc/durable-stream` `/v1/stream/{id}` wire contract
 *   backing `INFERENCE_DURABLE_STREAM`. Runs the package's OWN sync core +
 *   HTTP adapter over per-stream `MemoryStreamStore`s, so resume offsets,
 *   producer idempotency, and EOF semantics are byte-identical to the DO.
 *   BRIDGE HONESTY: durability is per-instance-lifetime (an instance
 *   restart forgets streams) — the documented fail-safe is that a client
 *   simply cannot resume and metering is untouched (it settles on upstream
 *   EOF, never on replay). CFG-6's Postgres DurableStream replaces this.
 */

import {
  MemoryStreamStore,
  handleRequest,
} from '@openagentsinc/durable-stream'

import { BindingUnavailableError } from './binding-unavailable'

const unavailableStub = (binding: string): DurableObjectStub =>
  new Proxy({} as DurableObjectStub, {
    get(_target, property) {
      if (typeof property !== 'string' || property === 'then') {
        return undefined
      }
      if (property === 'id' || property === 'name') return undefined
      return (..._args: ReadonlyArray<unknown>) =>
        Promise.reject(
          new BindingUnavailableError({
            binding,
            operation: `stub.${property}`,
          }),
        )
    },
  })

export const makeUnavailableDurableObjectNamespace = (
  binding: string,
): DurableObjectNamespace => {
  const namespace = {
    get: (_id: unknown) => unavailableStub(binding),
    getByName: (_name: string) => unavailableStub(binding),
    idFromName: (name: string) =>
      ({ name, toString: () => `${binding}:${name}` }) as unknown,
    idFromString: (id: string) =>
      ({ toString: () => id }) as unknown,
    jurisdiction: () => namespace,
    newUniqueId: () =>
      ({ toString: () => `${binding}:${crypto.randomUUID()}` }) as unknown,
  }
  return namespace as unknown as DurableObjectNamespace
}

const MAX_RESIDENT_STREAMS = 512

type StreamEntry = Readonly<{ store: MemoryStreamStore; touchedAtMs: number }>

export type InMemoryDurableStreamNamespace = Readonly<{
  getByName: (name: string) => { fetch: (request: Request) => Promise<Response> }
  /** Resident stream count (tests/ops). */
  size: () => number
}>

export const makeInMemoryDurableStreamNamespace = (
  nowMs: () => number = () => Date.now(),
): InMemoryDurableStreamNamespace => {
  const streams = new Map<string, StreamEntry>()

  const evictIfNeeded = (): void => {
    const now = nowMs()
    for (const [name, entry] of streams) {
      const meta = entry.store.getMeta()
      if (
        meta !== null &&
        meta.expiresAtMs !== null &&
        meta.expiresAtMs <= now
      ) {
        streams.delete(name)
      }
    }
    if (streams.size <= MAX_RESIDENT_STREAMS) return
    const byAge = [...streams.entries()].sort(
      (a, b) => a[1].touchedAtMs - b[1].touchedAtMs,
    )
    for (const [name] of byAge.slice(0, streams.size - MAX_RESIDENT_STREAMS)) {
      streams.delete(name)
    }
  }

  const storeFor = (name: string): MemoryStreamStore => {
    const existing = streams.get(name)
    const store = existing?.store ?? new MemoryStreamStore()
    streams.set(name, { store, touchedAtMs: nowMs() })
    if (existing === undefined) evictIfNeeded()
    return store
  }

  return {
    getByName: (name: string) => ({
      fetch: async (request: Request): Promise<Response> =>
        handleRequest(storeFor(name), request, {
          nowMs: nowMs(),
          streamId: name,
        }),
    }),
    size: () => streams.size,
  }
}
