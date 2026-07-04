import { Database } from "bun:sqlite"
import { Effect } from "effect"
import {
  createKhalaSyncStoreCore,
  localStoreFromCore,
  type SqlDriver,
  type SqlValue,
  toKhalaSyncStoreError,
} from "./store-core.js"
import {
  KhalaSyncClientStoreError,
  type KhalaSyncLocalStore,
} from "./store.js"

/**
 * `bun:sqlite` implementation of {@link KhalaSyncLocalStore} (KS-5.1) for
 * Khala Code desktop. WAL journaling on file databases; every multi-row
 * semantic ({@link KhalaSyncLocalStore.applyConfirmed},
 * {@link KhalaSyncLocalStore.resetScope},
 * {@link KhalaSyncLocalStore.enqueueMutation}) runs in ONE SQLite
 * transaction — a failure mid-batch leaves the store byte-for-byte
 * unchanged.
 *
 * All store SEMANTICS live in the driver-agnostic core (store-core.ts),
 * shared verbatim with the SQLite-WASM web storage worker (KS-5.4). This
 * file contributes only the `bun:sqlite` {@link SqlDriver} and the open /
 * close lifecycle.
 *
 * Invariant 2 (SPEC §7) lives at this seam: the tables hold
 * **server-confirmed state only**. The optimistic overlay (KS-5.2) is a
 * separate in-memory structure and must never write here.
 *
 * `bun:sqlite` is synchronous, so the Effect surface wraps direct calls
 * with `Effect.try` — the Promise/Effect split used by the
 * khala-sync-server outbox writer is not needed at this seam.
 */

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/**
 * {@link SqlDriver} over a `bun:sqlite` {@link Database}. `db.query`
 * caches prepared statements by SQL string, so per-call `query(...)` keeps
 * statement reuse without a separate cache here.
 *
 * Exported for tests that harness the shared store core (and the web RPC
 * pipeline) with `bun:sqlite` standing in for SQLite-WASM.
 */
export const bunSqlDriver = (db: Database): SqlDriver => ({
  exec: (sql) => db.exec(sql),
  run: (sql, params = []) => {
    db.query(sql).run(...(params as Array<SqlValue>))
  },
  all: <Row>(sql: string, params: ReadonlyArray<SqlValue> = []) =>
    db.query(sql).all(...(params as Array<SqlValue>)) as ReadonlyArray<Row>,
  transaction: (fn) => db.transaction(fn)(),
})

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export interface KhalaSyncSqliteStore extends KhalaSyncLocalStore {
  /** Close the underlying database handle; later calls fail typed. */
  readonly close: () => Effect.Effect<void, KhalaSyncClientStoreError>
}

/**
 * Open (or create) a Khala Sync local store at `path` — a filesystem path
 * or `":memory:"`. Synchronous by nature of `bun:sqlite`; throws
 * {@link KhalaSyncClientStoreError} if the database cannot be opened or
 * migrated.
 */
export const openKhalaSyncStore = (path: string): KhalaSyncSqliteStore => {
  let db: Database
  let core: ReturnType<typeof createKhalaSyncStoreCore>
  try {
    db = new Database(path, { create: true })
    // WAL for durable file stores (readers don't block the writer). On
    // :memory: databases SQLite reports "memory" and that is fine.
    db.exec("PRAGMA journal_mode = WAL;")
    db.exec("PRAGMA foreign_keys = ON;")
    core = createKhalaSyncStoreCore(bunSqlDriver(db))
  } catch (error) {
    throw new KhalaSyncClientStoreError(
      "storage_failure",
      "failed to open khala-sync local store",
      { cause: error },
    )
  }

  return {
    ...localStoreFromCore(core),
    close: () =>
      Effect.try({ try: () => db.close(), catch: toKhalaSyncStoreError }),
  }
}
