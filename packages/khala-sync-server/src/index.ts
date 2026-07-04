import type {
  BootstrapResponse,
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  LogPage,
  MutationEnvelope,
  MutationResult,
  MutatorName,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
} from "@openagentsinc/khala-sync"
import type { Effect } from "effect"

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
// Errors
// ---------------------------------------------------------------------------

export type KhalaSyncStorageErrorReason =
  | "connection_failed"
  | "transaction_conflict"
  | "constraint_violation"
  | "unavailable"

export class KhalaSyncStorageError extends Error {
  readonly _tag = "KhalaSyncStorageError"
  constructor(
    readonly reason: KhalaSyncStorageErrorReason,
    readonly messageSafe: string,
  ) {
    super(messageSafe)
  }
}

// ---------------------------------------------------------------------------
// Mutator registry (KS-3)
// ---------------------------------------------------------------------------

/**
 * A named, server-authoritative mutator. `execute` runs inside ONE Postgres
 * transaction and must perform: permission check, validation, business
 * writes, changelog appends (via the transaction-scoped writer), and return
 * the per-mutation result. Rejections are values (never thrown queue
 * poison); throwing is reserved for storage failures that abort the batch.
 */
export interface MutatorContext {
  readonly userId: string
  readonly clientGroupId: ClientGroupId
  readonly clientId: ClientId
  /** Append one changed entity to the changelog inside this transaction. */
  readonly appendChange: (
    change: Omit<ChangelogEntry, "version" | "committedAt" | "mutationRef">,
  ) => Effect.Effect<void, KhalaSyncStorageError>
  /** Run a parameterized SQL statement inside this transaction. */
  readonly sql: (
    statement: string,
    params: ReadonlyArray<unknown>,
  ) => Effect.Effect<ReadonlyArray<Record<string, unknown>>, KhalaSyncStorageError>
}

export interface MutatorDefinition<Args = unknown> {
  readonly name: MutatorName
  readonly decodeArgs: (argsJson: string) => Args
  readonly execute: (
    args: Args,
    ctx: MutatorContext,
  ) => Effect.Effect<MutationResult, KhalaSyncStorageError>
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

/** Snapshot + catch-up reads (Hyperdrive path). */
export interface KhalaSyncReadService {
  readonly bootstrap: (input: {
    readonly userId: string
    readonly scope: SyncScope
    readonly pageToken?: string
  }) => Effect.Effect<BootstrapResponse, KhalaSyncStorageError>
  readonly logPage: (input: {
    readonly userId: string
    readonly scope: SyncScope
    readonly afterVersion: SyncVersion | null
    readonly limit: number
  }) => Effect.Effect<LogPage, KhalaSyncStorageError>
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
