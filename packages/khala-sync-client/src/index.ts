import type {
  ClientGroupId,
  ClientId,
  MutatorName,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
} from "@openagentsinc/khala-sync"
import type { Effect, Stream } from "effect"
import type { KhalaSyncClientStoreError } from "./store.js"

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
// Local store (KS-5.1): contracts in store.ts; bun:sqlite implementation in
// sqlite-store.ts (desktop). SQLite-WASM / opfs-sahpool with a SharedWorker
// single-writer on web (later lane).
// ---------------------------------------------------------------------------

export {
  type ClientIdentity,
  type ConfirmedEntity,
  KhalaSyncClientStoreError,
  type KhalaSyncClientStoreErrorReason,
  type KhalaSyncLocalStore,
} from "./store.js"
export {
  type KhalaSyncSqliteStore,
  openKhalaSyncStore,
} from "./sqlite-store.js"

// ---------------------------------------------------------------------------
// Optimistic mutators + rebase (KS-5.2)
// ---------------------------------------------------------------------------

/**
 * The client-side implementation of a named mutator: a pure function from
 * (confirmed view, args) to overlay effects. Re-executed on every rebase,
 * so it must be replay-safe and side-effect free.
 */
export interface ClientMutator<Args = unknown> {
  readonly name: MutatorName
  readonly apply: (
    args: Args,
    view: OverlayView,
  ) => ReadonlyArray<OverlayEffect>
}

export interface OverlayView {
  readonly get: (entityType: string, entityId: string) => string | undefined
}

export type OverlayEffect =
  | {
      readonly kind: "upsert"
      readonly entityType: string
      readonly entityId: string
      readonly postImageJson: string
    }
  | {
      readonly kind: "delete"
      readonly entityType: string
      readonly entityId: string
    }

// ---------------------------------------------------------------------------
// Sync session (KS-5.3): per-scope state machine
// idle → bootstrapping → catching_up → live (+ must_refetch from any state)
// ---------------------------------------------------------------------------

export type ScopeSyncState =
  | { readonly phase: "idle" }
  | { readonly phase: "bootstrapping" }
  | { readonly phase: "catching_up"; readonly cursor: SyncVersion }
  | { readonly phase: "live"; readonly cursor: SyncVersion }
  | { readonly phase: "must_refetch"; readonly reason: string }

export interface KhalaSyncSessionConfig {
  readonly baseUrl: string
  readonly clientGroupId: ClientGroupId
  readonly clientId: ClientId
  readonly schemaVersion: SyncSchemaVersion
  readonly authToken: () => string
}

export interface KhalaSyncSession {
  readonly subscribe: (scope: SyncScope) => Effect.Effect<void>
  readonly state: (scope: SyncScope) => ScopeSyncState
  /** Confirmed+overlay change notifications for UI subscription. */
  readonly changes: Stream.Stream<SyncScope>
  readonly mutate: <Args>(
    mutator: ClientMutator<Args>,
    args: Args,
  ) => Effect.Effect<void, KhalaSyncClientStoreError>
}
