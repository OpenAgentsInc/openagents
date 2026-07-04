import sqlite3InitModule from "@sqlite.org/sqlite-wasm"
import { createKhalaSyncStoreCore } from "../store-core.js"
import { sqliteWasmDriver, type SqliteWasmDbLike } from "./wasm-driver.js"
import {
  createKhalaSyncStorageWorkerRuntime,
  type KhalaSyncStorageWorkerRuntime,
  type StorePortLike,
} from "./worker-runtime.js"
import { createKhalaSyncStoreWorkerServer } from "./worker-server.js"

/**
 * Storage-worker entry for the web store (KS-5.4). This is the ONLY
 * module that imports `@sqlite.org/sqlite-wasm`; it is exported solely
 * via the `./web/worker` package subpath so neither the desktop entry
 * (`.`) nor the main-thread web entry (`./web`) ever pulls the WASM
 * bundle.
 *
 * The worker owns the SINGLE database connection on the `opfs-sahpool`
 * VFS — synchronous OPFS access-handle pool, no COOP/COEP headers
 * required (unlike the `opfs` VFS, which needs SharedArrayBuffer). The
 * pool tolerates exactly one open connection per pool directory, which is
 * precisely the single-writer shape this architecture wants: run this
 * module in a SharedWorker so all of the origin's tabs share the one
 * connection.
 *
 * Usage — put this in your worker script (e.g. `khala-sync-worker.ts`,
 * bundled with `type: "module"`):
 *
 * ```ts
 * import { startKhalaSyncStorageWorker } from "@openagentsinc/khala-sync-client/web/worker"
 * startKhalaSyncStorageWorker(globalThis as never)
 * ```
 */

// ---------------------------------------------------------------------------
// Structural slices of the sqlite-wasm module (their shipped types lean on
// DOM lib; these narrow interfaces keep this package DOM-lib-free).
// ---------------------------------------------------------------------------

interface OpfsSAHPoolUtil {
  readonly OpfsSAHPoolDb: new (filename: string) => SqliteWasmDbLike
}

interface Sqlite3Static {
  readonly installOpfsSAHPoolVfs: (options?: {
    readonly directory?: string
    readonly name?: string
  }) => Promise<OpfsSAHPoolUtil>
}

type Sqlite3InitModule = (config?: {
  readonly print?: (...args: ReadonlyArray<unknown>) => void
  readonly printErr?: (...args: ReadonlyArray<unknown>) => void
}) => Promise<Sqlite3Static>

// ---------------------------------------------------------------------------
// Worker global scopes (structural; no DOM lib)
// ---------------------------------------------------------------------------

export interface SharedWorkerScopeLike {
  onconnect:
    | ((event: { readonly ports: ReadonlyArray<StorePortLike> }) => void)
    | null
}

export type DedicatedWorkerScopeLike = StorePortLike

export interface KhalaSyncStorageWorkerOptions {
  /** Database filename inside the pool VFS. */
  readonly dbFilename?: string
  /** OPFS directory holding the SAH pool (one pool = one connection). */
  readonly poolDirectory?: string
}

export const KHALA_SYNC_WEB_DB_FILENAME = "/khala-sync.sqlite3"
export const KHALA_SYNC_SAH_POOL_DIRECTORY = ".khala-sync-sahpool"

/**
 * Boot the storage worker: initialize SQLite-WASM, install the
 * `opfs-sahpool` VFS, open the single database connection, and serve the
 * typed store RPC on every connecting port. Requests arriving before the
 * async open completes are answered in order once it does; an open
 * failure answers every request with a typed `storage_failure`.
 *
 * Pass the worker's global scope (`globalThis`). SharedWorker scopes are
 * detected by `onconnect`; a dedicated Worker scope is used as the port
 * itself (single-tab fallback where SharedWorker is unavailable, e.g.
 * Chrome for Android — pair it with the Web Locks election so only the
 * elected tab's worker opens the pool).
 */
export const startKhalaSyncStorageWorker = (
  workerScope: SharedWorkerScopeLike | DedicatedWorkerScopeLike,
  options?: KhalaSyncStorageWorkerOptions,
): KhalaSyncStorageWorkerRuntime => {
  const runtime = createKhalaSyncStorageWorkerRuntime(async () => {
    const sqlite3 = await (sqlite3InitModule as unknown as Sqlite3InitModule)()
    const poolUtil = await sqlite3.installOpfsSAHPoolVfs({
      directory: options?.poolDirectory ?? KHALA_SYNC_SAH_POOL_DIRECTORY,
    })
    const db = new poolUtil.OpfsSAHPoolDb(
      options?.dbFilename ?? KHALA_SYNC_WEB_DB_FILENAME,
    )
    return createKhalaSyncStoreWorkerServer(
      createKhalaSyncStoreCore(sqliteWasmDriver(db)),
    )
  })

  if ("onconnect" in workerScope) {
    workerScope.onconnect = (event) => {
      for (const port of event.ports) runtime.attach(port)
    }
  } else {
    runtime.attach(workerScope)
  }
  return runtime
}
