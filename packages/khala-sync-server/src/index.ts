import type {
  BootstrapResponse,
  ClientGroupId,
  ClientId,
  LogPage,
  MutationEnvelope,
  MutationResult,
  MutatorName,
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
import type { SyncTransactionWriter } from "./outbox-writer.js"

export * from "./errors.js"
export * from "./mutation-ledger.js"
export * from "./outbox-writer.js"
export * from "./read-service.js"

/**
 * @openagentsinc/khala-sync-server — server substrate for Khala Sync:
 * Postgres schema (see ./migrations), the mutator engine, bootstrap and
 * catch-up reads, capture, and the per-scope KhalaSyncHubDO.
 *
 * Spec: docs/khala-sync/SPEC.md §§4-5. Implementation lands per the KS-2,
 * KS-3, and KS-4 workstream issues; this module currently defines the
 * service contracts those lanes fill in.
 */

// ---------------------------------------------------------------------------
// Mutator registry (KS-3)
// ---------------------------------------------------------------------------

/**
 * A named, server-authoritative mutator. `execute` runs inside ONE Postgres
 * transaction and must perform: permission check, validation, business
 * writes, changelog appends (via the transaction-scoped writer), and return
 * the per-mutation result. Rejections are values (never thrown queue
 * poison); throwing is reserved for storage failures that abort the batch.
 *
 * KS-2.1 refinement: mutators run inside `withSyncTransaction` (see
 * ./outbox-writer), so the context carries the {@link SyncTransactionWriter}
 * directly — `writer.appendChange` for changelog appends and `writer.sql`
 * for the mutator's own business writes, both bound to the ONE transaction.
 * Execution is Promise-based at this substrate seam (Bun SQL transactions
 * are Promise-scoped); Effect wrapping happens above the transaction
 * boundary in the push service.
 */
export interface MutatorContext {
  readonly userId: string
  readonly clientGroupId: ClientGroupId
  readonly clientId: ClientId
  /** Transaction-scoped changelog writer + business-write SQL handle. */
  readonly writer: SyncTransactionWriter
}

export interface MutatorDefinition<Args = unknown> {
  readonly name: MutatorName
  readonly decodeArgs: (argsJson: string) => Args
  readonly execute: (args: Args, ctx: MutatorContext) => Promise<MutationResult>
}

export interface MutatorRegistry {
  readonly get: (name: MutatorName) => MutatorDefinition | undefined
  readonly names: () => ReadonlyArray<MutatorName>
}

export const makeMutatorRegistry = (
  mutators: ReadonlyArray<MutatorDefinition>,
): MutatorRegistry => {
  const byName = new Map<string, MutatorDefinition>(
    mutators.map((m) => [String(m.name), m]),
  )
  if (byName.size !== mutators.length) {
    throw new Error("duplicate mutator name in registry")
  }
  return {
    get: (name) => byName.get(String(name)),
    names: () => mutators.map((m) => m.name),
  }
}

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

/** Scope authorization seam (KS-7). */
export interface KhalaSyncScopeAuth {
  readonly canRead: (
    userId: string,
    scope: SyncScope,
  ) => Effect.Effect<boolean, KhalaSyncStorageError>
}

// ---------------------------------------------------------------------------
// Capture (KS-4): tails khala_sync_changelog over a DIRECT Postgres
// connection (LISTEN wake + poll fallback) and pushes DeltaFrames to the
// per-scope hub. Not implemented here yet — see issue KS-4.1.
// ---------------------------------------------------------------------------

export interface KhalaSyncCaptureCheckpoint {
  readonly scope: SyncScope
  readonly pushedThroughVersion: SyncVersion
}

export const KHALA_SYNC_NOTIFY_CHANNEL = "khala_sync_changelog_append"

// ---------------------------------------------------------------------------
// Hub DO surface (KS-4): implemented as a Durable Object class inside the
// openagents.com Worker (wrangler binding KHALA_SYNC_HUB). The DO holds the
// recent log window in DO SQLite, hibernating WebSockets, and serves
// offset-resumable catch-up. See issue KS-4.2.
// ---------------------------------------------------------------------------

export const KHALA_SYNC_HUB_BINDING = "KHALA_SYNC_HUB"

/** Bounds for the hub's DO SQLite log window. */
export const HUB_WINDOW_MAX_ENTRIES = 10_000
export const HUB_WINDOW_MAX_BYTES = 64 * 1024 * 1024
