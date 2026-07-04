import type {
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  MutationEnvelope,
  MutationId,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import type { Effect } from "effect"

/**
 * Local-store contract for the Khala Sync client engine (KS-5.1;
 * SPEC §6, invariants 2 + 4).
 *
 * The durable local store holds **server-confirmed state exclusively**.
 * Optimistic effects live ONLY in the in-memory overlay (KS-5.2) and are
 * NEVER written here — the store's contents must always be reconstructible
 * from the server changelog alone (SPEC §7 invariant 2, Linear's rule).
 *
 * Delivery from the server is at-least-once; apply is idempotent by
 * `(scope, version, entityType, entityId)` (SPEC §7 invariant 4). The
 * durable cursor, not the connection, is the source of truth for resume.
 */

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export type KhalaSyncClientStoreErrorReason =
  /** applyConfirmed was handed a cursor lower than the stored cursor. */
  | "cursor_regression"
  /** enqueueMutation's mutationId was not exactly last pending/acked + 1. */
  | "mutation_id_gap"
  /** An entry/identity violated a store invariant (whole batch rolled back). */
  | "constraint_violation"
  /** The underlying SQLite layer failed (I/O, closed handle, corruption). */
  | "storage_failure"

/**
 * Typed store error. `message` is public-safe by construction: it names the
 * violated invariant, never row values or post-images.
 */
export class KhalaSyncClientStoreError extends Error {
  readonly _tag = "KhalaSyncClientStoreError"
  override readonly name = "KhalaSyncClientStoreError"
  constructor(
    readonly reason: KhalaSyncClientStoreErrorReason,
    message: string,
    options?: { readonly cause?: unknown },
  ) {
    super(message, options)
  }
}

// ---------------------------------------------------------------------------
// Confirmed entities + client identity
// ---------------------------------------------------------------------------

export interface ConfirmedEntity {
  readonly entityType: string
  readonly entityId: string
  readonly postImageJson: string
  readonly version: SyncVersion
}

/**
 * Durable client identity (`meta` table): one logical client installation
 * within one client group, pinned to a data-schema version. Written once at
 * first bootstrap; a conflicting rewrite is rejected (changing identity
 * under a pending mutation queue would corrupt attribution — reset the
 * store instead).
 */
export interface ClientIdentity {
  readonly clientId: ClientId
  readonly clientGroupId: ClientGroupId
  readonly schemaVersion: SyncSchemaVersion
}

// ---------------------------------------------------------------------------
// Store interface
// ---------------------------------------------------------------------------

export interface KhalaSyncLocalStore {
  /** Durable per-scope cursor; `null` when the scope was never synced. */
  readonly cursor: (
    scope: SyncScope,
  ) => Effect.Effect<SyncVersion | null, KhalaSyncClientStoreError>
  /**
   * Apply confirmed entries + advance the cursor in ONE local transaction:
   * either every entry and the cursor land, or nothing does.
   *
   * - Idempotent: re-applying the same entries + cursor is a no-op on the
   *   end state (at-least-once delivery safety, SPEC §7 invariant 4).
   * - Entries with `version <=` the stored entity version are skipped.
   * - A `cursor` lower than the stored cursor fails with
   *   `cursor_regression` (an equal cursor is a legal redelivery).
   */
  readonly applyConfirmed: (
    scope: SyncScope,
    entries: ReadonlyArray<ChangelogEntry>,
    cursor: SyncVersion,
  ) => Effect.Effect<void, KhalaSyncClientStoreError>
  /**
   * Replace scope-local state from a bootstrap snapshot (MustRefetch path)
   * in ONE local transaction: delete the scope's rows, insert the snapshot,
   * set the cursor. The cursor may move backwards here — the snapshot is a
   * full replacement, not an incremental apply. Other scopes are untouched.
   *
   * A `SyncVersionWatermark` of 0 means "scope start" (the snapshot found a
   * scope with no committed versions, e.g. a `scope_reset` refetch): the
   * scope's rows are deleted and the durable cursor is CLEARED (back to
   * `null`/never-synced), never stored as 0.
   */
  readonly resetScope: (
    scope: SyncScope,
    entities: ReadonlyArray<ConfirmedEntity>,
    cursor: SyncVersion | SyncVersionWatermark,
  ) => Effect.Effect<void, KhalaSyncClientStoreError>
  /** Confirmed entities for a scope (optionally one entity type), ordered. */
  readonly readEntities: (
    scope: SyncScope,
    entityType?: string,
  ) => Effect.Effect<ReadonlyArray<ConfirmedEntity>, KhalaSyncClientStoreError>
  /**
   * Append to the durable FIFO pending-mutation queue (pushes survive
   * restart). `mutationId` must be exactly last pending/acked + 1 — the
   * store maintains a `last_mutation_id` counter and fails with
   * `mutation_id_gap` on any gap, duplicate, or out-of-order id.
   *
   * The queue records the mutation *intent* only; its optimistic effects
   * live in the in-memory overlay (KS-5.2), never in this store.
   */
  readonly enqueueMutation: (
    mutation: MutationEnvelope,
  ) => Effect.Effect<void, KhalaSyncClientStoreError>
  /** Pending mutations in FIFO order (ascending mutationId). */
  readonly pendingMutations: () => Effect.Effect<
    ReadonlyArray<MutationEnvelope>,
    KhalaSyncClientStoreError
  >
  /**
   * The durable `last_mutation_id` counter — the highest mutationId ever
   * enqueued or acked (acked ids stay burned); `null` before the first
   * enqueue/ack. The next enqueue must use exactly this + 1. Lets the
   * overlay (KS-5.2) assign ids without replaying the queue.
   */
  readonly lastMutationId: () => Effect.Effect<
    MutationId | null,
    KhalaSyncClientStoreError
  >
  /**
   * Drop pending mutations with `mutationId <= throughMutationId` (the
   * server ACKed them — including rejections, which also advance
   * `lastMutationId`). Acking past the local counter advances it, so the
   * next enqueue stays exactly last-acked + 1.
   */
  readonly ackMutations: (
    throughMutationId: MutationId,
  ) => Effect.Effect<void, KhalaSyncClientStoreError>
  /** Durable client identity; `null` until {@link setIdentity} ran once. */
  readonly identity: () => Effect.Effect<
    ClientIdentity | null,
    KhalaSyncClientStoreError
  >
  /**
   * Persist the client identity (idempotent for equal values; a conflicting
   * rewrite fails with `constraint_violation`).
   */
  readonly setIdentity: (
    identity: ClientIdentity,
  ) => Effect.Effect<void, KhalaSyncClientStoreError>
}
