import {
  BootstrapEntity,
  canonicalJson,
  CvrDel,
  type CvrPullMode,
  CvrPullResponse,
  CvrVersion,
  EntityId,
  EntityType,
  KHALA_SYNC_PROTOCOL_VERSION,
  type SyncScope,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { KhalaSyncStorageError, storageErrorFromUnknown } from "./errors.js"
import type { SyncSql, SyncTransactionSql } from "./sql.js"

/**
 * Khala Sync CVR read-set diffing (KS-7.2, #8306; docs/khala-sync/
 * CVR_DESIGN.md; reference spec: the Replicache row-version strategy).
 *
 * A **Client View Record** is the exact row set — entity key → per-scope
 * changelog version — one client group's durable state was reconciled to,
 * taken at a snapshot cursor and stored per (client_group_id, scope,
 * cvr_version) in `khala_sync_cvrs`. `cvrPull` computes the CURRENT
 * authorized row set at a fresh snapshot and set-diffs it against the
 * client's referenced CVR:
 *
 * - **puts** — rows that are new or changed since the CVR (their latest
 *   version is greater than the CVR's recorded version, or the key is not
 *   in the CVR at all), delivered with full post-images;
 * - **dels** — rows in the CVR that are NOT in the current set anymore.
 *   Deletion, compaction of the tombstone, and permission-driven
 *   retraction all fall out of this one set difference — no tombstone
 *   retention window is needed and no full re-bootstrap happens.
 *
 * This is the SLOW/recovery path (the client's `must_refetch` recovery
 * when flagged); live DeltaFrames and `logPage` catch-up remain the
 * primary delivery path. The whole surface is additive behind the
 * `KHALA_SYNC_CVR=1` flag: unflagged deployments never call this module.
 *
 * ## Snapshot + compaction interplay
 *
 * The pull is ONE REPEATABLE READ transaction (Hyperdrive
 * transaction-mode safe: nothing spans requests): the scope counter and
 * every row come from one snapshot, and the snapshot cursor is always the
 * CURRENT `last_version` — never a client-supplied cursor — so a CVR pull
 * can never be behind the retained window. The current row set is the
 * same latest-row-per-entity derivation bootstrap uses, which compaction
 * explicitly preserves (each live entity's latest upsert row survives
 * behind the watermark — see ./compaction). Tombstones the client missed
 * because compaction pruned them are exactly the rows that fall out as
 * dels.
 *
 * ## Drift (the deviation from vanilla Replicache — CVR_DESIGN.md §5)
 *
 * In Replicache, pull is the ONLY delivery channel, so the stored CVR
 * always equals the client's state. Our clients also apply live
 * deltas/log pages, so their state can run AHEAD of their last CVR. The
 * client therefore sends `drift`: its rows whose store version is greater
 * than the referenced CVR's snapshot cursor. The diff base is the CVR
 * entries widened by the drift (max version wins). Every client row with
 * version ≤ the CVR snapshot is provably IN the CVR (it survived that
 * pull's reconciliation), so base ⊇ client set, and:
 *
 * - dels = base − current  ⊇  clientSet − current (all stale rows are
 *   retracted; extra dels for rows the client no longer has are no-ops);
 * - puts = current rows newer than their base version (rows the client
 *   already has current images for are provably at-version and skipped).
 *
 * ## Concurrency
 *
 * `cvr_version` is allocated as max+1 inside the transaction; two
 * concurrent pulls for the same (group, scope) can collide on the primary
 * key — the loser surfaces as a retryable storage error (the route maps
 * it to 503). Client groups pull serially in practice (it is the recovery
 * path), so no lock ordering is added for this.
 */

// ---------------------------------------------------------------------------
// Flag + limits
// ---------------------------------------------------------------------------

/** Env var gating the whole CVR surface. Anything but "1" = OFF. */
export const KHALA_SYNC_CVR_FLAG = "KHALA_SYNC_CVR"

export const isKhalaSyncCvrEnabled = (value: string | undefined): boolean =>
  value === "1"

/**
 * Hard cap on the current-row-set size a single CVR pull will compute and
 * store. Beyond it the pull refuses with
 * {@link KhalaSyncCvrRowSetTooLargeError} and the client falls back to the
 * paged bootstrap (which has no whole-set materialization). The cap bounds
 * pull cost at O(maxRowSet) rows scanned + one jsonb of ≤ maxRowSet keys.
 */
export const DEFAULT_CVR_MAX_ROW_SET = 50_000

/** CVR versions kept per (client_group_id, scope); older ones are pruned. */
export const CVR_RETAINED_VERSIONS = 8

/**
 * The scope's current row set is too large for the single-response CVR
 * path — the client must use the paged bootstrap instead. Not a storage
 * failure and not retryable-as-is.
 */
export class KhalaSyncCvrRowSetTooLargeError extends Error {
  readonly _tag = "KhalaSyncCvrRowSetTooLargeError"
  override readonly name = "KhalaSyncCvrRowSetTooLargeError"
  constructor(
    readonly scope: string,
    readonly maxRowSet: number,
  ) {
    super(
      `scope ${scope} has more than ${maxRowSet} current rows — ` +
        "CVR pull refused; use the paged bootstrap",
    )
  }
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

export interface CvrDriftInput {
  readonly entityType: string
  readonly entityId: string
  readonly version: number
}

export interface CvrPullInput {
  readonly scope: SyncScope
  readonly clientGroupId: string
  /** The CVR the client last reconciled to; `null` ⇒ reset-mode pull. */
  readonly cvrVersion: number | null
  /** Client rows applied after that CVR's snapshot (see module doc). */
  readonly drift?: ReadonlyArray<CvrDriftInput> | undefined
  /**
   * Row-level visibility seam (permission fanout): only rows this
   * predicate accepts are part of the "authorized row set". Rows it stops
   * accepting between pulls fall out as dels — permission-driven
   * retraction is structural. Default: every row in the scope is visible
   * (scope-level access is the route's KS-7.1 resolver, not this seam).
   */
  readonly isEntityVisible?:
    | ((entityType: string, entityId: string) => boolean)
    | undefined
  /** Defaults to {@link DEFAULT_CVR_MAX_ROW_SET}. */
  readonly maxRowSet?: number | undefined
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/**
 * CVR entry key: `<entityType>/<entityId>`. Entity types match
 * `^[a-z][a-z0-9_]*$` (no `/`), so splitting at the FIRST `/` is
 * unambiguous even when the entity id itself contains slashes.
 */
const entryKey = (entityType: string, entityId: string): string =>
  `${entityType}/${entityId}`

const splitEntryKey = (key: string): { entityType: string; entityId: string } => {
  const slash = key.indexOf("/")
  if (slash <= 0 || slash === key.length - 1) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      "stored CVR entry key is malformed",
    )
  }
  return { entityType: key.slice(0, slash), entityId: key.slice(slash + 1) }
}

const toSafeNonNegativeInt = (
  raw: string | number | bigint,
  what: string,
): number => {
  const value = typeof raw === "number" ? raw : Number(raw)
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `${what} out of safe range: ${String(raw)}`,
    )
  }
  return value
}

interface CurrentRow {
  readonly entity_type: string
  readonly entity_id: string
  readonly op: string
  readonly version: string | number | bigint
  readonly post_image_json: string | object | null
}

interface CvrRow {
  readonly entries: string | object
  readonly snapshot_cursor: string | number | bigint
}

/** Decode a stored jsonb entries object into key → version. */
const decodeEntries = (raw: string | object): Map<string, number> => {
  const parsed: unknown = typeof raw === "string" ? JSON.parse(raw) : raw
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      "stored CVR entries are not a JSON object",
    )
  }
  const entries = new Map<string, number>()
  for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
    if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
      throw new KhalaSyncStorageError(
        "constraint_violation",
        "stored CVR entry version is not a positive safe integer",
      )
    }
    entries.set(key, value)
  }
  return entries
}

// ---------------------------------------------------------------------------
// cvrPull
// ---------------------------------------------------------------------------

/**
 * One CVR pull for `(clientGroupId, scope)` (see the module doc): computes
 * the current authorized row set at a fresh REPEATABLE READ snapshot,
 * diffs it against the referenced CVR widened by `drift`, stores the new
 * CVR, prunes old versions, and returns the puts/dels + new cvrVersion +
 * snapshot cursor. A missing/pruned/absent `cvrVersion` degrades to a
 * `reset`-mode response (puts = the complete current set; the client
 * replaces scope-local state, exactly like a bootstrap snapshot).
 */
export const cvrPull = async (
  sql: SyncSql,
  input: CvrPullInput,
): Promise<CvrPullResponse> => {
  if (input.clientGroupId.length === 0) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      "cvrPull requires a non-empty clientGroupId",
    )
  }
  if (
    input.cvrVersion !== null &&
    (!Number.isSafeInteger(input.cvrVersion) || input.cvrVersion < 1)
  ) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `cvrVersion must be a positive safe integer, got ${String(input.cvrVersion)}`,
    )
  }
  const maxRowSet = input.maxRowSet ?? DEFAULT_CVR_MAX_ROW_SET
  if (!Number.isSafeInteger(maxRowSet) || maxRowSet < 1) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `maxRowSet must be a positive safe integer, got ${String(input.maxRowSet)}`,
    )
  }
  for (const drift of input.drift ?? []) {
    if (!Number.isSafeInteger(drift.version) || drift.version < 1) {
      throw new KhalaSyncStorageError(
        "constraint_violation",
        "drift entry version must be a positive safe integer",
      )
    }
  }
  const visible = input.isEntityVisible ?? (() => true)

  try {
    return await sql.begin(
      "isolation level repeatable read",
      async (tx: SyncTransactionSql) => {
        // Snapshot cursor = the scope's CURRENT last_version, read in the
        // same snapshot as everything below (same gap-free argument as the
        // KS-2.2 bootstrap: version allocation is commit-ordered under the
        // scope-counter row lock). Never a client cursor ⇒ never behind
        // the retained window.
        const scopeRows: Array<{ last_version: string | number | bigint }> =
          await tx`
            SELECT last_version FROM khala_sync_scopes
             WHERE scope = ${input.scope}
          `
        const snapshotCursor =
          scopeRows[0] === undefined
            ? 0
            : toSafeNonNegativeInt(scopeRows[0].last_version, "last_version")

        // Current row set: latest row per entity ≤ snapshot, tombstoned
        // entities omitted, visibility predicate applied. LIMIT bounds the
        // scan; +1 detects overflow honestly instead of silently truncating.
        const rawRows: Array<CurrentRow> =
          snapshotCursor === 0
            ? []
            : await tx`
                SELECT DISTINCT ON (entity_type, entity_id)
                       entity_type, entity_id, op, version, post_image_json
                  FROM khala_sync_changelog
                 WHERE scope = ${input.scope}
                   AND version <= ${snapshotCursor}
                 ORDER BY entity_type, entity_id, version DESC
                 LIMIT ${maxRowSet + 1}
              `
        if (rawRows.length > maxRowSet) {
          throw new KhalaSyncCvrRowSetTooLargeError(String(input.scope), maxRowSet)
        }

        interface Live {
          readonly entityType: string
          readonly entityId: string
          readonly version: number
          readonly postImageJson: string
        }
        const current = new Map<string, Live>()
        for (const row of rawRows) {
          if (row.op !== "upsert") continue
          if (!visible(row.entity_type, row.entity_id)) continue
          if (row.post_image_json === null) {
            throw new KhalaSyncStorageError(
              "constraint_violation",
              "upsert changelog row without a post-image",
            )
          }
          current.set(entryKey(row.entity_type, row.entity_id), {
            entityType: row.entity_type,
            entityId: row.entity_id,
            version: toSafeNonNegativeInt(row.version, "changelog version"),
            // Canonical bytes regardless of jsonb normalization — the same
            // rule as the writer and bootstrap, so CVR puts are byte-equal
            // to bootstrap entities.
            postImageJson: canonicalJson(
              typeof row.post_image_json === "string"
                ? JSON.parse(row.post_image_json)
                : row.post_image_json,
            ),
          })
        }

        // Referenced CVR (diff base). Missing row — pruned, never written,
        // or no cvrVersion sent — degrades to reset mode: we cannot know
        // what the client holds, so we hand it the complete set to replace
        // with (always sound; never silently under-delivers dels).
        let base: Map<string, number> | null = null
        if (input.cvrVersion !== null) {
          const cvrRows: Array<CvrRow> = await tx`
            SELECT entries, snapshot_cursor FROM khala_sync_cvrs
             WHERE client_group_id = ${input.clientGroupId}
               AND scope = ${input.scope}
               AND cvr_version = ${input.cvrVersion}
          `
          const cvrRow = cvrRows[0]
          if (cvrRow !== undefined) {
            base = decodeEntries(cvrRow.entries)
            // Widen by drift (max version wins): rows the client applied
            // via the live path after this CVR's snapshot.
            for (const drift of input.drift ?? []) {
              const key = entryKey(drift.entityType, drift.entityId)
              const existing = base.get(key)
              if (existing === undefined || drift.version > existing) {
                base.set(key, drift.version)
              }
            }
          }
        }
        const mode: CvrPullMode = base === null ? "reset" : "diff"

        // Set diff (ordered by entity key for deterministic responses).
        const sortedCurrent = [...current.values()].sort((a, b) => {
          const ka = entryKey(a.entityType, a.entityId)
          const kb = entryKey(b.entityType, b.entityId)
          return ka < kb ? -1 : ka > kb ? 1 : 0
        })
        const puts = sortedCurrent
          .filter((row) => {
            if (base === null) return true
            const baseVersion = base.get(entryKey(row.entityType, row.entityId))
            return baseVersion === undefined || row.version > baseVersion
          })
          .map(
            (row) =>
              new BootstrapEntity({
                entityType: EntityType.make(row.entityType),
                entityId: EntityId.make(row.entityId),
                postImageJson: row.postImageJson,
              }),
          )
        const dels =
          base === null
            ? []
            : [...base.keys()]
                .filter((key) => !current.has(key))
                .sort()
                .map((key) => {
                  const { entityType, entityId } = splitEntryKey(key)
                  return new CvrDel({
                    entityType: EntityType.make(entityType),
                    entityId: EntityId.make(entityId),
                  })
                })

        // Store the new CVR (the exact set the client's state now equals
        // after applying this response) and prune old versions.
        const maxRows: Array<{ v: string | number | bigint }> = await tx`
          SELECT COALESCE(MAX(cvr_version), 0) AS v FROM khala_sync_cvrs
           WHERE client_group_id = ${input.clientGroupId}
             AND scope = ${input.scope}
        `
        const newCvrVersion =
          toSafeNonNegativeInt(maxRows[0]?.v ?? 0, "cvr_version") + 1
        const entriesObject: Record<string, number> = {}
        for (const [key, row] of current) entriesObject[key] = row.version
        await tx`
          INSERT INTO khala_sync_cvrs
            (client_group_id, scope, cvr_version, snapshot_cursor, entries)
          VALUES
            (${input.clientGroupId}, ${input.scope}, ${newCvrVersion},
             ${snapshotCursor}, ${canonicalJson(entriesObject)}::jsonb)
        `
        await tx`
          DELETE FROM khala_sync_cvrs
           WHERE client_group_id = ${input.clientGroupId}
             AND scope = ${input.scope}
             AND cvr_version <= ${newCvrVersion - CVR_RETAINED_VERSIONS}
        `

        return new CvrPullResponse({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          scope: input.scope,
          mode,
          puts,
          dels,
          cvrVersion: CvrVersion.make(newCvrVersion),
          cursor: SyncVersionWatermark.make(snapshotCursor),
        })
      },
    )
  } catch (error) {
    const mapped = storageErrorFromUnknown(error)
    throw mapped ?? error
  }
}
