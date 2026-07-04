import type { StoreResponse } from "./protocol.js"
import type { KhalaSyncStoreWorkerServer } from "./worker-server.js"

/**
 * Port plumbing for the storage worker (KS-5.4): attaches ports
 * (SharedWorker connections or a dedicated worker's global scope) to a
 * lazily-initializing {@link KhalaSyncStoreWorkerServer}.
 *
 * Requests that arrive before async initialization (sqlite-wasm +
 * `opfs-sahpool` open) completes are answered in arrival order once it
 * does — promise-chain FIFO, no explicit queue. If initialization fails,
 * every request (current and future) is answered with a typed
 * `storage_failure` response instead of hanging the tab.
 *
 * Structural port/scope types only — this module carries no DOM lib
 * dependency so it stays testable in bun with faked ports.
 */

/** Minimal MessagePort/Worker-scope surface the runtime needs. */
export interface StorePortLike {
  postMessage: (message: unknown) => void
  onmessage: ((event: { readonly data: unknown }) => void) | null
  start?: () => void
}

export interface KhalaSyncStorageWorkerRuntime {
  /** Serve store RPC on `port`; safe to call once per connecting tab. */
  readonly attach: (port: StorePortLike) => void
  /** Resolves when the storage backend is open (rejects on init failure). */
  readonly ready: Promise<void>
}

const requestId = (data: unknown): number =>
  typeof data === "object" &&
  data !== null &&
  typeof (data as { id?: unknown }).id === "number"
    ? (data as { id: number }).id
    : -1

const initFailureResponse = (data: unknown, error: unknown): StoreResponse => ({
  id: requestId(data),
  ok: false,
  reason: "storage_failure",
  message: `khala-sync storage worker failed to initialize: ${
    error instanceof Error ? error.message : String(error)
  }`,
})

export const createKhalaSyncStorageWorkerRuntime = (
  init: () => Promise<KhalaSyncStoreWorkerServer>,
): KhalaSyncStorageWorkerRuntime => {
  // Eager: the first tab to spawn the worker pays the open cost once.
  const serverPromise = init()
  // Surface init failures through per-request responses, not an unhandled
  // rejection that kills the worker.
  const settled = serverPromise.then(
    (server) => ({ server }),
    (error: unknown) => ({ error }),
  )

  const ready = serverPromise.then(() => undefined)
  // Init failure is surfaced per-request (and to `ready` awaiters); never
  // let it escape as an unhandled rejection.
  ready.catch(() => undefined)

  return {
    attach: (port) => {
      port.onmessage = (event) => {
        void settled.then((outcome) => {
          port.postMessage(
            "server" in outcome
              ? outcome.server.handle(event.data)
              : initFailureResponse(event.data, outcome.error),
          )
        })
      }
      // MessagePorts delivered via SharedWorker onconnect must be started
      // when using addEventListener; with onmessage assignment start() is
      // implicit, but calling it is harmless and keeps fakes honest.
      port.start?.()
    },
    ready,
  }
}
