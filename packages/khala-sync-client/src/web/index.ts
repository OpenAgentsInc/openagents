/**
 * @openagentsinc/khala-sync-client/web — main-thread web surface of the
 * Khala Sync local store (KS-5.4): SQLite-WASM on the `opfs-sahpool` VFS
 * behind a storage worker, Web Locks single-writer election, typed
 * postMessage RPC.
 *
 * IMPORTANT: this entry never imports `@sqlite.org/sqlite-wasm`. The WASM
 * bundle loads exclusively inside the storage worker, whose entry is the
 * separate `./web/worker` subpath (`startKhalaSyncStorageWorker`).
 */

export {
  electWriter,
  type LockManagerLike,
  type WriterElection,
} from "./election.js"
export {
  type ChangelogEntryWire,
  type ClientIdentityWire,
  type ConfirmedEntityWire,
  isStoreRequest,
  isStoreResponse,
  type MutationEnvelopeWire,
  type StoreRequest,
  type StoreRequestBody,
  type StoreRequestOp,
  type StoreResponse,
  type StoreResponseValue,
} from "./protocol.js"
export {
  KHALA_SYNC_WRITER_LOCK,
  type KhalaSyncWasmStore,
  type KhalaSyncWasmStoreOptions,
  openKhalaSyncWasmStore,
  type StorageManagerLike,
} from "./sqlite-wasm-store.js"
export {
  sqliteWasmDriver,
  type SqliteWasmDbLike,
  type SqliteWasmExecOptions,
} from "./wasm-driver.js"
export {
  createKhalaSyncStorageWorkerRuntime,
  type KhalaSyncStorageWorkerRuntime,
  type StorePortLike,
} from "./worker-runtime.js"
export {
  createKhalaSyncStoreWorkerServer,
  type KhalaSyncStoreWorkerServer,
  MALFORMED_REQUEST_ID,
} from "./worker-server.js"
