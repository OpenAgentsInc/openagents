import type {
  BootstrapResponse,
  ClientGroupId,
  ClientId,
  LogPage,
  MutationEnvelope,
  MutationResult,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import type { Effect } from "effect"
import type {
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncInvalidPageTokenError,
  KhalaSyncStorageError,
} from "./errors.js"

export * from "./compaction.js"
export * from "./cvr-service.js"
export * from "./errors.js"
export * from "./fleet-intents.js"
export * from "./fleet-mutators.js"
export * from "./fleet-projection.js"
export * from "./mutation-ledger.js"
export * from "./outbox-writer.js"
export * from "./public-counter-projection.js"
export * from "./push-engine.js"
export * from "./read-service.js"
export * from "./scope-auth.js"
export * from "./sql.js"

/**
 * @openagentsinc/khala-sync-server — server substrate for Khala Sync:
 * Postgres schema (see ./migrations), the mutator engine (./push-engine),
 * bootstrap and catch-up reads, capture, and the per-scope KhalaSyncHubDO.
 *
 * Spec: docs/khala-sync/SPEC.md §§4-5. Implementation lands per the KS-2,
 * KS-3, and KS-4 workstream issues; this module currently defines the
 * service contracts the remaining lanes fill in. The mutator registry and
 * transactional push engine (KS-3.1) live in ./push-engine.
 */

// ---------------------------------------------------------------------------
// Substrate service contracts (KS-2)
// ---------------------------------------------------------------------------

/** Transactional push: executes a batch of mutations for one client. */
export interface KhalaSyncPushService {
  readonly push: (input: {
    readonly userId: string
    readonly clientGroupId: ClientGroupId
    readonly clientId: ClientId
    readonly schemaVersion: SyncSchemaVersion
    readonly mutations: ReadonlyArray<MutationEnvelope>
  }) => Effect.Effect<
    { readonly results: ReadonlyArray<MutationResult> },
    KhalaSyncStorageError
  >
}

/**
 * Errors the read paths can fail with: storage failures, a cursor (or
 * bootstrap stitch point) behind the retained window (maps to the wire
 * `MustRefetch(cursor_behind_retained_window)`), or an invalid bootstrap
 * page token.
 */
export type KhalaSyncReadError =
  | KhalaSyncStorageError
  | KhalaSyncCursorBehindRetainedWindowError
  | KhalaSyncInvalidPageTokenError

/**
 * Snapshot + catch-up reads (Hyperdrive path). The substrate functions are
 * `bootstrap`/`logPage` in ./read-service (Promise-based at the transaction
 * seam, like the outbox writer); this Effect-facing service wraps them above
 * the scope-auth check (KS-7).
 */
export interface KhalaSyncReadService {
  readonly bootstrap: (input: {
    readonly userId: string
    readonly scope: SyncScope
    readonly pageSize?: number | undefined
    readonly pageToken?: string | undefined
  }) => Effect.Effect<BootstrapResponse, KhalaSyncReadError>
  readonly logPage: (input: {
    readonly userId: string
    readonly scope: SyncScope
    /** Resume-after watermark; `null` = scope start (version 0). */
    readonly afterVersion: SyncVersion | SyncVersionWatermark | null
    readonly limit: number
  }) => Effect.Effect<LogPage, KhalaSyncReadError>
}

/**
 * Scope authorization seam (KS-7). The concrete resolver is
 * `resolveScopeRead` in ./scope-auth (KS-7.1, #8305): the full-taxonomy
 * read gate over injected capability callbacks, fail-closed on capability
 * failure. This Effect-facing interface remains for services that wrap it.
 */
export interface KhalaSyncScopeAuth {
  readonly canRead: (
    userId: string,
    scope: SyncScope,
  ) => Effect.Effect<boolean, KhalaSyncStorageError>
}

// ---------------------------------------------------------------------------
// Capture (KS-4.1): tails khala_sync_changelog over a DIRECT Postgres
// connection (LISTEN wake + poll fallback) and pushes ordered batches to
// the per-scope hub — implemented in ./capture and exposed ONLY through
// the Bun-side `@openagentsinc/khala-sync-server/capture` subpath
// (KHALA_SYNC_NOTIFY_CHANNEL, KhalaSyncCaptureCheckpoint, runCapturePass,
// startCaptureDaemon, …). It is a long-lived Bun daemon (`import { SQL }
// from "bun"` as a VALUE), so it must never ride the root export the
// workers-typechecked openagents.com Worker consumes — same isolation rule
// as ./hub. CLI: scripts/capture.ts.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Hub DO surface (KS-4): implemented as a Durable Object class inside the
// openagents.com Worker (wrangler binding KHALA_SYNC_HUB). The DO holds the
// recent log window in DO SQLite, hibernating WebSockets, and serves
// offset-resumable catch-up. See issue KS-4.2.
// ---------------------------------------------------------------------------

export * from "./hub.js"
