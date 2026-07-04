import {
  canonicalJson,
  CanonicalJsonError,
  type ChangelogEntry,
  decodeChangelogEntry,
  type EntityId,
  type EntityType,
  type SyncScope,
  SyncVersion,
} from "@openagentsinc/khala-sync"
import type { SQL, TransactionSQL } from "bun"
import { KhalaSyncStorageError, storageErrorFromUnknown } from "./errors.js"

/**
 * Transaction-scoped changelog writer + per-scope version allocator
 * (KS-2.1; SPEC §2.2/§2.3/§4, invariant 1).
 *
 * `withSyncTransaction` opens ONE Postgres transaction and hands the
 * callback a {@link SyncTransactionWriter}: the caller performs its own
 * business writes through `writer.sql` and records the replicated effects
 * through `writer.appendChange`, all atomically. The per-scope version is
 * allocated INSIDE that transaction under the row lock on
 * `khala_sync_scopes.last_version` (insert-on-conflict bootstrap for new
 * scopes), so per-scope versions are dense, monotonic, and commit-ordered
 * by construction — a rollback rolls the counter back with everything
 * else, and no gap can ever appear in a committed sequence.
 *
 * One transaction gets ONE version per scope: the first append (or
 * `allocateVersion` call) for a scope allocates it; every later change to
 * the same scope in the same transaction reuses it. Appending the same
 * entity twice in one transaction collapses to one row (last write wins),
 * preserving the "one row per changed entity per transaction per scope"
 * shape from SPEC §2.3.
 */

// ---------------------------------------------------------------------------
// Append input (delete ⟺ no post-image, enforced at the type level)
// ---------------------------------------------------------------------------

export interface AppendUpsert {
  readonly scope: SyncScope
  readonly entityType: EntityType
  readonly entityId: EntityId
  readonly op: "upsert"
  /**
   * Full entity post-image. Serialized with `canonicalJson` from
   * `@openagentsinc/khala-sync` — never pre-stringified JSON.
   */
  readonly postImage: unknown
  readonly mutationRef?: string
}

export interface AppendDelete {
  readonly scope: SyncScope
  readonly entityType: EntityType
  readonly entityId: EntityId
  readonly op: "delete"
  /** Tombstones carry no post-image (schema CHECK enforces the same). */
  readonly postImage?: never
  readonly mutationRef?: string
}

export type AppendChangeInput = AppendUpsert | AppendDelete

// ---------------------------------------------------------------------------
// Writer handle
// ---------------------------------------------------------------------------

export interface SyncTransactionWriter {
  /**
   * The transaction's SQL handle. Business writes issued through it commit
   * or roll back atomically with the changelog appends (invariant 5).
   */
  readonly sql: TransactionSQL
  /**
   * Allocate this transaction's version for `scope` (or return the one
   * already allocated by an earlier call in the same transaction). Takes
   * the scope-counter row lock, serializing concurrent writers per scope.
   */
  readonly allocateVersion: (scope: SyncScope) => Promise<SyncVersion>
  /**
   * Append one changed entity to `khala_sync_changelog` in this
   * transaction, allocating (or reusing) the scope version. Returns the
   * decoded entry as it will replicate.
   */
  readonly appendChange: (change: AppendChangeInput) => Promise<ChangelogEntry>
}

// ---------------------------------------------------------------------------
// Row mapping
// ---------------------------------------------------------------------------

/** Raw `khala_sync_changelog` row shape as returned by Bun SQL. */
export interface ChangelogRow {
  readonly scope: string
  readonly version: string | number | bigint
  readonly entity_type: string
  readonly entity_id: string
  readonly op: string
  readonly post_image_json: string | object | null
  readonly mutation_ref: string | null
  readonly committed_at: Date | string
}

const toVersionNumber = (raw: string | number | bigint): number => {
  const value = typeof raw === "number" ? raw : Number(raw)
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `scope version out of safe range: ${String(raw)}`,
    )
  }
  return value
}

const toCommittedAt = (raw: Date | string): string =>
  raw instanceof Date ? raw.toISOString() : new Date(raw).toISOString()

/**
 * Decode a raw `khala_sync_changelog` row into a `ChangelogEntry`.
 * `post_image_json` is re-serialized through `canonicalJson` so the entry's
 * `postImageJson` is canonical bytes regardless of jsonb normalization.
 */
export const changelogEntryFromRow = (row: ChangelogRow): ChangelogEntry => {
  const postImageJson =
    row.post_image_json === null
      ? undefined
      : canonicalJson(
          typeof row.post_image_json === "string"
            ? JSON.parse(row.post_image_json)
            : row.post_image_json,
        )
  return decodeChangelogEntry({
    scope: row.scope,
    version: toVersionNumber(row.version),
    entityType: row.entity_type,
    entityId: row.entity_id,
    op: row.op,
    ...(postImageJson === undefined ? {} : { postImageJson }),
    ...(row.mutation_ref === null ? {} : { mutationRef: row.mutation_ref }),
    committedAt: toCommittedAt(row.committed_at),
  })
}

// ---------------------------------------------------------------------------
// withSyncTransaction
// ---------------------------------------------------------------------------

const serializePostImage = (postImage: unknown): string => {
  if (postImage === undefined) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      "upsert changelog entries require a post-image (SPEC §2.3)",
    )
  }
  try {
    return canonicalJson(postImage)
  } catch (error) {
    if (error instanceof CanonicalJsonError) {
      throw new KhalaSyncStorageError(
        "constraint_violation",
        `post-image is not canonical-JSON representable: ${error.message}`,
        { cause: error },
      )
    }
    throw error
  }
}

/**
 * Open one Postgres transaction and run `fn` with a
 * {@link SyncTransactionWriter} bound to it. Commits when `fn` resolves;
 * rolls back (business writes, changelog rows, AND scope counters — no
 * gaps) when it rejects. SQL-layer failures are mapped to
 * {@link KhalaSyncStorageError}; the caller's own domain errors pass
 * through unchanged.
 */
export const withSyncTransaction = async <A>(
  sql: SQL,
  fn: (writer: SyncTransactionWriter) => Promise<A>,
): Promise<A> => {
  try {
    return await sql.begin(async (tx) => {
      /** scope → version allocated by THIS transaction. */
      const allocated = new Map<string, SyncVersion>()

      const allocateVersion = async (scope: SyncScope): Promise<SyncVersion> => {
        const existing = allocated.get(scope)
        if (existing !== undefined) return existing
        const rows: Array<{ last_version: string | number | bigint }> = await tx`
          INSERT INTO khala_sync_scopes (scope, last_version)
          VALUES (${scope}, 1)
          ON CONFLICT (scope) DO UPDATE SET
            last_version = khala_sync_scopes.last_version + 1,
            updated_at = now()
          RETURNING last_version
        `
        const row = rows[0]
        if (row === undefined) {
          throw new KhalaSyncStorageError(
            "unavailable",
            "scope version allocation returned no row",
          )
        }
        const version = SyncVersion.make(toVersionNumber(row.last_version))
        allocated.set(scope, version)
        return version
      }

      const appendChange = async (
        change: AppendChangeInput,
      ): Promise<ChangelogEntry> => {
        if (change.op === "delete" && change.postImage !== undefined) {
          throw new KhalaSyncStorageError(
            "constraint_violation",
            "delete changelog entries are tombstones and must not carry a post-image (SPEC §2.3)",
          )
        }
        const postImageJson =
          change.op === "upsert" ? serializePostImage(change.postImage) : null
        const mutationRef = change.mutationRef ?? null
        const version = await allocateVersion(change.scope)
        const rows: Array<{ committed_at: Date | string }> = await tx`
          INSERT INTO khala_sync_changelog
            (scope, version, entity_type, entity_id, op, post_image_json, mutation_ref)
          VALUES
            (${change.scope}, ${version}, ${change.entityType}, ${change.entityId},
             ${change.op}, ${postImageJson}::jsonb, ${mutationRef})
          ON CONFLICT (scope, version, entity_type, entity_id) DO UPDATE SET
            op = EXCLUDED.op,
            post_image_json = EXCLUDED.post_image_json,
            mutation_ref = EXCLUDED.mutation_ref,
            committed_at = EXCLUDED.committed_at
          RETURNING committed_at
        `
        const row = rows[0]
        if (row === undefined) {
          throw new KhalaSyncStorageError(
            "unavailable",
            "changelog append returned no row",
          )
        }
        return decodeChangelogEntry({
          scope: change.scope,
          version,
          entityType: change.entityType,
          entityId: change.entityId,
          op: change.op,
          ...(postImageJson === null ? {} : { postImageJson }),
          ...(mutationRef === null ? {} : { mutationRef }),
          committedAt: toCommittedAt(row.committed_at),
        })
      }

      return await fn({ sql: tx, allocateVersion, appendChange })
    })
  } catch (error) {
    const mapped = storageErrorFromUnknown(error)
    throw mapped ?? error
  }
}
