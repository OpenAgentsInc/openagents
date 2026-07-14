import {
  openSqliteDatabase,
  type SqliteDatabase,
} from "@openagentsinc/sqlite-runtime"
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
 * Embedded-SQLite implementation of {@link KhalaSyncLocalStore} (KS-5.1)
 * for Khala Code desktop. WAL journaling on file databases; every multi-row
 * semantic ({@link KhalaSyncLocalStore.applyConfirmed},
 * {@link KhalaSyncLocalStore.resetScope},
 * {@link KhalaSyncLocalStore.enqueueMutation}) runs in ONE SQLite
 * transaction — a failure mid-batch leaves the store byte-for-byte
 * unchanged.
 *
 * SQLite access goes through the dual-runtime seam
 * `@openagentsinc/sqlite-runtime` (BUN-1, openagents#8779): `bun:sqlite`
 * under Bun, `node:sqlite` under Node, selected at runtime. This module is
 * the pilot store on that seam — it no longer imports `bun:sqlite` at all.
 * The seam's {@link SqliteDatabase} is signature-compatible with
 * {@link SqlDriver}, so the handle feeds `createKhalaSyncStoreCore`
 * directly.
 *
 * All store SEMANTICS live in the driver-agnostic core (store-core.ts),
 * shared verbatim with the SQLite-WASM web storage worker (KS-5.4). This
 * file contributes only the seam-backed {@link SqlDriver} and the open /
 * close lifecycle.
 *
 * Invariant 2 (SPEC §7) lives at this seam: the tables hold
 * **server-confirmed state only**. The optimistic overlay (KS-5.2) is a
 * separate in-memory structure and must never write here.
 *
 * The seam is synchronous (both `bun:sqlite` and `node:sqlite` are), so the
 * Effect surface wraps direct calls with `Effect.try` — the Promise/Effect
 * split used by the khala-sync-server outbox writer is not needed here.
 */

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/**
 * Minimal structural view of a `bun:sqlite` `Database` — kept so existing
 * test harnesses can keep driving the shared store core (and the web RPC
 * pipeline) with a raw `bun:sqlite` handle standing in for SQLite-WASM,
 * WITHOUT this production module importing `bun:sqlite` itself.
 */
export interface BunSqliteDatabaseLike {
  exec(sql: string): void
  query(sql: string): {
    run(...params: Array<SqlValue>): unknown
    all(...params: Array<SqlValue>): unknown
  }
  transaction<A>(fn: () => A): () => A
}

/**
 * {@link SqlDriver} over a `bun:sqlite`-shaped database. `db.query` caches
 * prepared statements by SQL string, so per-call `query(...)` keeps
 * statement reuse without a separate cache here.
 *
 * Exported for tests that harness the shared store core (and the web RPC
 * pipeline) with `bun:sqlite` standing in for SQLite-WASM. Production opens
 * go through {@link openKhalaSyncStore} and the runtime seam instead.
 */
export const bunSqlDriver = (db: BunSqliteDatabaseLike): SqlDriver => ({
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
 * or `":memory:"`. Synchronous by nature of the embedded bindings; throws
 * {@link KhalaSyncClientStoreError} if the database cannot be opened or
 * migrated.
 */
export const openKhalaSyncStore = (path: string): KhalaSyncSqliteStore => {
  let db: SqliteDatabase | undefined
  let core: ReturnType<typeof createKhalaSyncStoreCore>
  try {
    db = openSqliteDatabase(path)
    // WAL for durable file stores (readers don't block the writer). On
    // :memory: databases SQLite reports "memory" and that is fine.
    db.exec("PRAGMA journal_mode = WAL;")
    db.exec("PRAGMA foreign_keys = ON;")
    core = createKhalaSyncStoreCore(db)
  } catch (error) {
    try {
      db?.close()
    } catch {
      // Preserve the open/migration failure as the actionable typed error.
    }
    if (error instanceof KhalaSyncClientStoreError) throw error
    throw new KhalaSyncClientStoreError(
      "storage_failure",
      "failed to open khala-sync local store",
      { cause: error },
    )
  }

  return {
    ...localStoreFromCore(core),
    close: () =>
      Effect.try({ try: () => db!.close(), catch: toKhalaSyncStoreError }),
  }
}
