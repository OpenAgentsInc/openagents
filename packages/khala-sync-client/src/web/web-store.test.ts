import {
  ChangelogEntry,
  EntityId,
  EntityType,
  SyncScope,
  SyncVersion,
} from "@openagentsinc/khala-sync"
import { Database } from "bun:sqlite"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { bunSqlDriver } from "../sqlite-store.js"
import { createKhalaSyncStoreCore } from "../store-core.js"
import { describeKhalaSyncStoreSemantics } from "../store-semantics.testkit.js"
import { KhalaSyncClientStoreError } from "../store.js"
import type { LockManagerLike } from "./election.js"
import {
  type KhalaSyncWasmStore,
  openKhalaSyncWasmStore,
  type StorageManagerLike,
} from "./sqlite-wasm-store.js"
import {
  createKhalaSyncStorageWorkerRuntime,
  type StorePortLike,
} from "./worker-runtime.js"
import {
  createKhalaSyncStoreWorkerServer,
  type KhalaSyncStoreWorkerServer,
} from "./worker-server.js"

/**
 * KS-5.4 acceptance in bun: the FULL store semantics suite runs through
 * the complete web pipeline — main-thread proxy → typed postMessage RPC
 * over a faked port pair (structured-clone enforced, async delivery) →
 * storage-worker runtime → RPC server → shared SQL core — with
 * `bun:sqlite` standing in for SQLite-WASM behind the driver seam.
 *
 * Browser-runtime specifics that bun cannot execute (real `opfs-sahpool`
 * init, real SharedWorker, real `navigator.locks`) are covered by the
 * focused fakes here + election.test.ts + wasm-driver.test.ts, and by
 * the manual browser verification path documented in the README.
 */

// ---------------------------------------------------------------------------
// Fake port pair: async delivery + structured clone (like real postMessage)
// ---------------------------------------------------------------------------

const createPortPair = (): {
  readonly client: StorePortLike
  readonly worker: StorePortLike
} => {
  const client: StorePortLike = {
    onmessage: null,
    postMessage: (message) => {
      const data = structuredClone(message)
      queueMicrotask(() => worker.onmessage?.({ data }))
    },
  }
  const worker: StorePortLike = {
    onmessage: null,
    postMessage: (message) => {
      const data = structuredClone(message)
      queueMicrotask(() => client.onmessage?.({ data }))
    },
  }
  return { client, worker }
}

const createServer = (): {
  readonly server: KhalaSyncStoreWorkerServer
  readonly db: Database
} => {
  const db = new Database(":memory:")
  return {
    server: createKhalaSyncStoreWorkerServer(
      createKhalaSyncStoreCore(bunSqlDriver(db)),
    ),
    db,
  }
}

interface PipelineOptions {
  readonly locks?: LockManagerLike | null
  readonly storage?: StorageManagerLike | null
  readonly initDelayMs?: number
}

const createPipelineStore = (
  options: PipelineOptions = {},
): { readonly store: KhalaSyncWasmStore; readonly cleanup: () => void } => {
  const { client, worker } = createPortPair()
  const { server, db } = createServer()
  const runtime = createKhalaSyncStorageWorkerRuntime(async () => {
    if (options.initDelayMs !== undefined) {
      await new Promise((resolve) => setTimeout(resolve, options.initDelayMs))
    }
    return server
  })
  runtime.attach(worker)
  const store = openKhalaSyncWasmStore({
    port: client,
    locks: options.locks ?? null,
    storage: options.storage ?? null,
  })
  return {
    store,
    cleanup: () => {
      Effect.runSync(store.close())
      db.close()
    },
  }
}

// ---------------------------------------------------------------------------
// The shared semantics suite, through the full RPC pipeline
// ---------------------------------------------------------------------------

describeKhalaSyncStoreSemantics(
  "web store (proxy → RPC → storage worker → shared SQL core)",
  () => {
    const { store, cleanup } = createPipelineStore()
    return { store, cleanup }
  },
)

// ---------------------------------------------------------------------------
// Focused proxy behavior
// ---------------------------------------------------------------------------

const scopeA = SyncScope.make("scope.team.alpha")

const upsertEntry = (version: number, entityId: string): ChangelogEntry =>
  new ChangelogEntry({
    scope: scopeA,
    version: SyncVersion.make(version),
    entityType: EntityType.make("task"),
    entityId: EntityId.make(entityId),
    op: "upsert",
    postImageJson: "{}",
    committedAt: "2026-07-04T00:00:00.000Z",
  })

describe("openKhalaSyncWasmStore", () => {
  test("requests navigator.storage.persist() exactly once, on the first write", async () => {
    let persistCalls = 0
    const storage: StorageManagerLike = {
      persist: () => {
        persistCalls += 1
        return Promise.resolve(true)
      },
    }
    const { store, cleanup } = createPipelineStore({ storage })
    try {
      // reads never request persistence
      await Effect.runPromise(store.cursor(scopeA))
      await Effect.runPromise(store.readEntities(scopeA))
      expect(persistCalls).toBe(0)

      await Effect.runPromise(
        store.applyConfirmed(scopeA, [upsertEntry(1, "t1")], SyncVersion.make(1)),
      )
      expect(persistCalls).toBe(1)

      // later writes do not re-request
      await Effect.runPromise(
        store.applyConfirmed(scopeA, [upsertEntry(2, "t2")], SyncVersion.make(2)),
      )
      expect(persistCalls).toBe(1)
    } finally {
      cleanup()
    }
  })

  test("a denied persist() request does not fail the write", async () => {
    const storage: StorageManagerLike = {
      persist: () => Promise.reject(new Error("denied")),
    }
    const { store, cleanup } = createPipelineStore({ storage })
    try {
      await Effect.runPromise(
        store.applyConfirmed(scopeA, [upsertEntry(1, "t1")], SyncVersion.make(1)),
      )
      expect(
        Number(await Effect.runPromise(store.cursor(scopeA))),
      ).toBe(1)
    } finally {
      cleanup()
    }
  })

  test("requests sent before the worker finishes opening are answered in order", async () => {
    const { store, cleanup } = createPipelineStore({ initDelayMs: 15 })
    try {
      const results = await Promise.all([
        Effect.runPromise(
          store.applyConfirmed(scopeA, [upsertEntry(1, "t1")], SyncVersion.make(1)),
        ),
        Effect.runPromise(store.cursor(scopeA)),
      ])
      expect(Number(results[1])).toBe(1)
    } finally {
      cleanup()
    }
  })

  test("a failing worker initialization answers every call with a typed storage error", async () => {
    const { client, worker } = createPortPair()
    const runtime = createKhalaSyncStorageWorkerRuntime(() =>
      Promise.reject(new Error("OPFS unavailable")),
    )
    runtime.attach(worker)
    const store = openKhalaSyncWasmStore({
      port: client,
      locks: null,
      storage: null,
    })
    const error = await Effect.runPromise(Effect.flip(store.cursor(scopeA)))
    expect(error).toBeInstanceOf(KhalaSyncClientStoreError)
    expect(error.reason).toBe("storage_failure")
    expect(error.message).toContain("failed to initialize")
  })

  test("close() detaches this tab: later + in-flight calls fail typed, election released", async () => {
    // fake locks: grant immediately, remember release
    let lockReleased = false
    const locks: LockManagerLike = {
      // Promise.resolve flattens the returned hold promise, so this .then
      // fires exactly when the election releases the lock.
      request: (_name, _options, callback) =>
        Promise.resolve(callback({})).then(() => {
          lockReleased = true
        }),
    }
    const { store, cleanup } = createPipelineStore({ locks })
    try {
      expect(await store.writerElected).toBe(true)
      expect(store.isWriter()).toBe(true)

      // fire a call and close before its response can arrive
      const inflight = Effect.runPromise(
        Effect.flip(store.cursor(scopeA)),
      )
      await Effect.runPromise(store.close())

      expect((await inflight).reason).toBe("storage_failure")
      expect(store.isWriter()).toBe(false)
      await new Promise((resolve) => setTimeout(resolve, 0))
      expect(lockReleased).toBe(true)

      const late = await Effect.runPromise(Effect.flip(store.cursor(scopeA)))
      expect(late.reason).toBe("storage_failure")
      expect(late.message).toContain("closed")
    } finally {
      cleanup()
    }
  })

  test("two tabs on one lock: exactly one writer; close hands the election over", async () => {
    // shared fake lock manager across two proxies
    const queue: Array<() => void> = []
    let held = false
    const locks: LockManagerLike = {
      request: (_name, _options, callback) =>
        new Promise((resolve) => {
          const grant = () => {
            held = true
            // flattened: fires when the hold promise (the lock) releases
            void Promise.resolve(callback({})).then(() => {
              held = false
              resolve(undefined)
              queue.shift()?.()
            })
          }
          if (held) queue.push(grant)
          else grant()
        }),
    }

    const tabA = createPipelineStore({ locks })
    const tabB = createPipelineStore({ locks })
    try {
      expect(await tabA.store.writerElected).toBe(true)
      expect(tabA.store.isWriter()).toBe(true)
      expect(tabB.store.isWriter()).toBe(false)

      await Effect.runPromise(tabA.store.close())
      expect(await tabB.store.writerElected).toBe(true)
      expect(tabB.store.isWriter()).toBe(true)
    } finally {
      tabA.cleanup()
      tabB.cleanup()
    }
  })

  test("typed error reasons survive the RPC boundary (cursor_regression)", async () => {
    const { store, cleanup } = createPipelineStore()
    try {
      await Effect.runPromise(
        store.applyConfirmed(scopeA, [upsertEntry(5, "t1")], SyncVersion.make(5)),
      )
      const error = await Effect.runPromise(
        Effect.flip(
          store.applyConfirmed(scopeA, [], SyncVersion.make(3)),
        ),
      )
      expect(error).toBeInstanceOf(KhalaSyncClientStoreError)
      expect(error._tag).toBe("KhalaSyncClientStoreError")
      expect(error.reason).toBe("cursor_regression")
    } finally {
      cleanup()
    }
  })
})
