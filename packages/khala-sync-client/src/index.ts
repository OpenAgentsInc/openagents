/**
 * @openagentsinc/khala-sync-client — client engine for Khala Sync: local
 * store, transport, bootstrap/catch-up/live state machine, optimistic
 * mutators, and rebase.
 *
 * Spec: docs/khala-sync/SPEC.md §6. Implementation lands per the KS-5
 * workstream issues.
 *
 * Two hard client invariants (SPEC §7):
 * - Optimistic effects live ONLY in the in-memory overlay; the durable
 *   local store holds server-confirmed state exclusively.
 * - Apply is idempotent by (scope, version, entityType, entityId); the
 *   durable cursor, not the connection, is the source of truth.
 */

// ---------------------------------------------------------------------------
// Local store (KS-5.1 + KS-5.4): contracts in store.ts; ALL SQL semantics
// in the driver-agnostic store-core.ts; bun:sqlite driver in
// sqlite-store.ts (desktop). The web adapter (SQLite-WASM / opfs-sahpool
// behind a storage worker with SharedWorker single-writer election) lives
// under the `./web` subpath — and its worker entry, the only module that
// imports @sqlite.org/sqlite-wasm, under `./web/worker` — so this desktop
// entry never loads WASM.
// ---------------------------------------------------------------------------

export {
  type ClientIdentity,
  type ConfirmedEntity,
  KhalaSyncClientStoreError,
  type KhalaSyncClientStoreErrorReason,
  type KhalaSyncLocalStore,
} from "./store.js"
export {
  createKhalaSyncStoreCore,
  KHALA_SYNC_STORE_SCHEMA,
  type KhalaSyncStoreCore,
  localStoreFromCore,
  type SqlDriver,
  type SqlValue,
  toKhalaSyncStoreError,
} from "./store-core.js"
export {
  type KhalaSyncSqliteStore,
  openKhalaSyncStore,
} from "./sqlite-store.js"

// ---------------------------------------------------------------------------
// Optimistic mutators + rebase (KS-5.2): contracts + engine in overlay.ts.
// Optimistic effects live ONLY in the in-memory overlay (SPEC §7
// invariant 2); the durable store holds server-confirmed state exclusively.
// ---------------------------------------------------------------------------

export {
  type ClientMutator,
  createOverlay,
  type KhalaSyncOverlay,
  KhalaSyncOverlayError,
  type KhalaSyncOverlayErrorReason,
  type OverlayEffect,
  type OverlayEntity,
  type OverlayError,
  type OverlayReadView,
  type OverlayView,
} from "./overlay.js"

// ---------------------------------------------------------------------------
// Transport (KS-5.3): injectable seam in transport.ts; HTTP+WebSocket
// implementation against the SPEC §3 routes, khala-sync codecs at every
// boundary, bearer auth from the session config's authToken().
// ---------------------------------------------------------------------------

export {
  createHttpKhalaSyncTransport,
  type HttpTransportConfig,
  type HttpTransportDeps,
  isAccessDeniedSignal,
  isRefetchSignal,
  isRetryableTransportError,
  KHALA_SYNC_BOOTSTRAP_PATH,
  KHALA_SYNC_CONNECT_PATH,
  KHALA_SYNC_LOG_PATH,
  KHALA_SYNC_PUSH_PATH,
  type KhalaSyncTransport,
  KhalaSyncTransportError,
  type KhalaSyncTransportErrorReason,
  type LiveSocket,
  type LiveSocketHandlers,
  type WebSocketLike,
} from "./transport.js"

// ---------------------------------------------------------------------------
// Sync session (KS-5.3): per-scope state machine in session.ts
// idle → bootstrapping → catching_up → live (+ must_refetch from any state);
// reconnect resumes from the DURABLE cursor; push loop drains the pending
// queue with in-band rejection handling.
// ---------------------------------------------------------------------------

export {
  computeBackoffMs,
  createKhalaSyncSession,
  type KhalaSyncSession,
  type KhalaSyncSessionConfig,
  type KhalaSyncSessionOptions,
  type ScopeSyncState,
} from "./session.js"
