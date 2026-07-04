import {
  type ChangelogEntry,
  ClientGroupId,
  ClientId,
  MutationEnvelope,
  MutationId,
  MutatorName,
  SyncSchemaVersion,
  SyncVersion,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import {
  type ClientIdentity,
  type ConfirmedEntity,
  KhalaSyncClientStoreError,
  type KhalaSyncLocalStore,
} from "../store.js"
import {
  electWriter,
  type LockManagerLike,
  type WriterElection,
} from "./election.js"
import {
  type ChangelogEntryWire,
  type ClientIdentityWire,
  type ConfirmedEntityWire,
  isStoreResponse,
  type MutationEnvelopeWire,
  type StoreRequestBody,
  type StoreResponseValue,
} from "./protocol.js"
import type { StorePortLike } from "./worker-runtime.js"

/**
 * Main-thread web implementation of {@link KhalaSyncLocalStore} (KS-5.4):
 * a proxy over postMessage RPC to the storage worker that owns the single
 * SQLite-WASM `opfs-sahpool` connection (web/sqlite-wasm-worker.ts).
 *
 * - **Single writer**: the caller connects a SharedWorker (one instance
 *   across the origin's tabs) and passes its port; every tab additionally
 *   holds a Web Locks writer election for its lifetime (Notion pattern).
 *   v1 routes ALL operations — reads included — through that one worker:
 *   the simplest arrangement that is correct under multi-tab concurrency.
 *   Read scaling (per-tab read-only connections against the same pool) is
 *   the documented follow-up in the package README.
 * - **Durability**: `navigator.storage.persist()` is requested once, on
 *   the first write-class operation, so the origin's OPFS bucket is
 *   exempted from best-effort eviction before any state worth keeping
 *   exists.
 * - **Typed errors end-to-end**: the worker transports
 *   {@link KhalaSyncClientStoreError} reason + public-safe message over
 *   the wire; the proxy rethrows the same taxonomy, so callers cannot
 *   tell the RPC seam from the direct `bun:sqlite` store.
 *
 * All browser APIs are injected structurally (port, locks, storage), so
 * the full store semantics suite runs against this proxy in bun with a
 * fake port pair in front of the real SQL core.
 */

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/** Structural slice of `navigator.storage`. */
export interface StorageManagerLike {
  readonly persist: () => Promise<boolean>
}

export interface KhalaSyncWasmStoreOptions {
  /**
   * Port to the storage worker: `new SharedWorker(url, { type: "module" })
   * .port`, or a dedicated `Worker` (which is itself port-shaped).
   */
  readonly port: StorePortLike
  /**
   * Web Locks manager for writer election. Defaults to
   * `navigator.locks`; pass `null` to disable election (the tab then
   * reports itself writer — single-tab or test contexts).
   */
  readonly locks?: LockManagerLike | null
  /**
   * Storage manager for the first-write `persist()` request. Defaults to
   * `navigator.storage`; pass `null` to disable.
   */
  readonly storage?: StorageManagerLike | null
  /** Election lock name; one per logical database. */
  readonly lockName?: string
}

export interface KhalaSyncWasmStore extends KhalaSyncLocalStore {
  /** Resolves true when this tab wins the writer election. */
  readonly writerElected: Promise<boolean>
  /** Whether this tab currently holds the writer lock. */
  readonly isWriter: () => boolean
  /**
   * Detach this tab: reject in-flight calls, fail later calls typed, and
   * release the writer election (electing the next tab). Does NOT close
   * the worker's database — other tabs share it; the connection lives for
   * the worker's lifetime.
   */
  readonly close: () => Effect.Effect<void, KhalaSyncClientStoreError>
}

export const KHALA_SYNC_WRITER_LOCK = "khala-sync:writer"

// ---------------------------------------------------------------------------
// Wire encoding (domain → wire)
// ---------------------------------------------------------------------------

const encodeEntry = (entry: ChangelogEntry): ChangelogEntryWire => ({
  scope: entry.scope,
  version: entry.version,
  entityType: entry.entityType,
  entityId: entry.entityId,
  op: entry.op,
  ...(entry.postImageJson === undefined
    ? {}
    : { postImageJson: entry.postImageJson }),
  ...(entry.mutationRef === undefined
    ? {}
    : { mutationRef: entry.mutationRef }),
  committedAt: entry.committedAt,
})

const encodeEntity = (entity: ConfirmedEntity): ConfirmedEntityWire => ({
  entityType: entity.entityType,
  entityId: entity.entityId,
  postImageJson: entity.postImageJson,
  version: entity.version,
})

// ---------------------------------------------------------------------------
// Wire decoding (wire → domain)
// ---------------------------------------------------------------------------

const decodeEntities = (
  value: StoreResponseValue,
): ReadonlyArray<ConfirmedEntity> =>
  (value as ReadonlyArray<ConfirmedEntityWire>).map((wire) => ({
    entityType: wire.entityType,
    entityId: wire.entityId,
    postImageJson: wire.postImageJson,
    version: SyncVersion.make(wire.version),
  }))

const decodeMutations = (
  value: StoreResponseValue,
): ReadonlyArray<MutationEnvelope> =>
  (value as ReadonlyArray<MutationEnvelopeWire>).map(
    (wire) =>
      new MutationEnvelope({
        mutationId: MutationId.make(wire.mutationId),
        name: MutatorName.make(wire.name),
        argsJson: wire.argsJson,
      }),
  )

const decodeIdentity = (value: StoreResponseValue): ClientIdentity | null => {
  if (value === null || value === undefined) return null
  const wire = value as ClientIdentityWire
  return {
    clientId: ClientId.make(wire.clientId),
    clientGroupId: ClientGroupId.make(wire.clientGroupId),
    schemaVersion: SyncSchemaVersion.make(wire.schemaVersion),
  }
}

// ---------------------------------------------------------------------------
// Store proxy
// ---------------------------------------------------------------------------

const toStoreError = (error: unknown): KhalaSyncClientStoreError =>
  error instanceof KhalaSyncClientStoreError
    ? error
    : new KhalaSyncClientStoreError(
        "storage_failure",
        `khala-sync web store rpc failed: ${error instanceof Error ? error.message : String(error)}`,
        { cause: error },
      )

const defaultLocks = (): LockManagerLike | null =>
  (globalThis as { navigator?: { locks?: LockManagerLike } }).navigator
    ?.locks ?? null

const defaultStorage = (): StorageManagerLike | null => {
  const storage = (
    globalThis as { navigator?: { storage?: Partial<StorageManagerLike> } }
  ).navigator?.storage
  return typeof storage?.persist === "function"
    ? (storage as StorageManagerLike)
    : null
}

/**
 * Open the web store over an already-connected storage-worker port. The
 * worker initializes the database on its own; requests sent before it is
 * ready are answered in order once it is.
 */
export const openKhalaSyncWasmStore = (
  options: KhalaSyncWasmStoreOptions,
): KhalaSyncWasmStore => {
  const port = options.port
  const locks = options.locks === undefined ? defaultLocks() : options.locks
  const storage =
    options.storage === undefined ? defaultStorage() : options.storage

  let closed = false
  let nextId = 1
  let persistRequested = false
  const pending = new Map<
    number,
    {
      readonly resolve: (value: StoreResponseValue) => void
      readonly reject: (error: KhalaSyncClientStoreError) => void
    }
  >()

  // Writer election held for the tab's lifetime (Notion pattern). Without
  // Web Locks support there is no election to lose; report writer so v1
  // single-worker routing proceeds unchanged.
  const election: WriterElection | null =
    locks === null
      ? null
      : electWriter(locks, options.lockName ?? KHALA_SYNC_WRITER_LOCK)

  port.onmessage = (event) => {
    if (!isStoreResponse(event.data)) return
    const response = event.data
    const entry = pending.get(response.id)
    if (entry === undefined) return
    pending.delete(response.id)
    if (response.ok) {
      entry.resolve(response.value)
    } else {
      entry.reject(
        new KhalaSyncClientStoreError(response.reason, response.message),
      )
    }
  }
  port.start?.()

  const call = (
    request: StoreRequestBody,
  ): Promise<StoreResponseValue> => {
    if (closed) {
      return Promise.reject(
        new KhalaSyncClientStoreError(
          "storage_failure",
          "khala-sync web store is closed",
        ),
      )
    }
    const id = nextId++
    return new Promise<StoreResponseValue>((resolve, reject) => {
      pending.set(id, { resolve, reject })
      port.postMessage({ ...request, id })
    })
  }

  /** SPEC §6: request origin-durable storage on the FIRST write. */
  const requestPersistOnce = (): void => {
    if (persistRequested || storage === null) return
    persistRequested = true
    void storage.persist().then(
      () => undefined,
      () => undefined, // advisory: denial must not fail the write
    )
  }

  const rpc = <A>(
    request: StoreRequestBody,
    decode: (value: StoreResponseValue) => A,
  ): Effect.Effect<A, KhalaSyncClientStoreError> =>
    Effect.tryPromise({
      try: () => call(request).then(decode),
      catch: toStoreError,
    })

  const rpcWrite = (
    request: StoreRequestBody,
  ): Effect.Effect<void, KhalaSyncClientStoreError> =>
    Effect.tryPromise({
      try: () => {
        requestPersistOnce()
        return call(request).then(() => undefined)
      },
      catch: toStoreError,
    })

  return {
    cursor: (scope) =>
      rpc({ op: "cursor", scope }, (value) =>
        value === null || value === undefined
          ? null
          : SyncVersion.make(value as number),
      ),
    applyConfirmed: (scope, entries, cursor) =>
      rpcWrite({
        op: "applyConfirmed",
        scope,
        entries: entries.map(encodeEntry),
        cursor,
      }),
    resetScope: (scope, entities, cursor) =>
      rpcWrite({
        op: "resetScope",
        scope,
        entities: entities.map(encodeEntity),
        cursor,
      }),
    readEntities: (scope, entityType) =>
      rpc(
        {
          op: "readEntities",
          scope,
          ...(entityType === undefined ? {} : { entityType }),
        },
        decodeEntities,
      ),
    enqueueMutation: (mutation) =>
      rpcWrite({
        op: "enqueueMutation",
        mutation: {
          mutationId: mutation.mutationId,
          name: mutation.name,
          argsJson: mutation.argsJson,
        },
      }),
    pendingMutations: () =>
      rpc({ op: "pendingMutations" }, decodeMutations),
    lastMutationId: () =>
      rpc({ op: "lastMutationId" }, (value) =>
        value === null || value === undefined
          ? null
          : MutationId.make(value as number),
      ),
    ackMutations: (throughMutationId) =>
      rpcWrite({ op: "ackMutations", through: throughMutationId }),
    identity: () => rpc({ op: "identity" }, decodeIdentity),
    setIdentity: (identity) =>
      rpcWrite({
        op: "setIdentity",
        identity: {
          clientId: identity.clientId,
          clientGroupId: identity.clientGroupId,
          schemaVersion: identity.schemaVersion,
        },
      }),
    writerElected: election?.becameWriter ?? Promise.resolve(true),
    isWriter: () => (election === null ? !closed : election.isWriter()),
    close: () =>
      Effect.sync(() => {
        if (closed) return
        closed = true
        election?.release()
        port.onmessage = null
        const inflight = [...pending.values()]
        pending.clear()
        for (const entry of inflight) {
          entry.reject(
            new KhalaSyncClientStoreError(
              "storage_failure",
              "khala-sync web store closed with the call in flight",
            ),
          )
        }
      }),
  }
}
