import type { SyncScope } from "@openagentsinc/khala-sync"
import type { SQL, TransactionSQL } from "bun"
import { KhalaSyncStorageError, storageErrorFromUnknown } from "./errors.js"

/**
 * Changelog compaction + retained-window watermark (KS-2.3; SPEC §2.3, §4,
 * §7 invariant 6).
 *
 * Compaction advances `khala_sync_scopes.retained_from_version` (the
 * retained-window watermark) and prunes `khala_sync_changelog` rows behind
 * it — in ONE Postgres transaction per scope, under the scope-counter row
 * lock, so the watermark and the deletions are atomic and serialized
 * against concurrent writers.
 *
 * ## Watermark computation
 *
 * The new watermark is the MINIMUM of three candidates (each is a retention
 * guarantee — an entry is only compactable once it clears ALL of them):
 *
 * 1. **Entry count** — `max(1, last_version - maxRetainedEntries + 1)`:
 *    always keep the newest `maxRetainedEntries` version groups.
 * 2. **Age** — the smallest version whose `committed_at` is younger than
 *    `now - maxRetainedAgeMs` (entries younger than the age floor are never
 *    compacted; `last_version + 1` when every entry is old enough). Only
 *    applied when `maxRetainedAgeMs` is configured.
 * 3. **Capture checkpoint** — `pushed_through_version + 1` from
 *    `khala_sync_capture_checkpoints` (KS-4 capture lane): compaction MUST
 *    NOT delete changelog rows the capture worker has not pushed to the
 *    per-scope hub yet, or the hub's window would silently miss versions.
 *    Guarded by a table-existence check (`to_regclass`) because the capture
 *    lane lands concurrently: when the table does not exist yet there is no
 *    capture worker to protect and the bound is skipped; when it exists but
 *    the scope has NO checkpoint row, compaction FAILS CLOSED (treats
 *    `pushed_through_version` as 0 and holds the watermark at 1).
 *
 * The result is additionally clamped to never regress below the current
 * `retained_from_version` and never exceed `last_version + 1`, so the
 * schema CHECK (`khala_sync_scopes_retention`) can never be violated:
 * candidate 1 is `<= last_version + 1` by construction and the minimum only
 * moves down.
 *
 * ## Deletion shape (what "prune behind the watermark" means in v1)
 *
 * Bootstrap derives CURRENT entity states from the changelog itself
 * (latest row per entity — see read-service), so compaction deletes a row
 * `version < watermark` only when it is not load-bearing for that
 * derivation:
 *
 * - **superseded rows** — a newer row exists for the same entity; or
 * - **tombstones** (`op = 'delete'`) — a tombstone behind the watermark is
 *   either superseded or the entity's final state ("gone"), and bootstrap
 *   omits absent entities anyway. This is the tombstone GC from SPEC §2.3.
 *
 * A live entity's latest upsert row is therefore PRESERVED even behind the
 * watermark (the read-service doc: "Compaction must therefore always
 * preserve each live entity's latest upsert row"). Preserved rows are
 * snapshot residue: they feed bootstrap's latest-per-entity derivation but
 * are never served as log pages — `logPage` refuses any cursor behind
 * `retained_from_version - 1`, and rows at `version >= retained_from` are
 * always complete version groups (the watermark is a version boundary, so
 * no group at or above it is ever split).
 *
 * ## MustRefetch follows from the watermark
 *
 * Compaction itself never talks to clients. Advancing the watermark is what
 * makes the read path enforce invariant 6: `logPage`/`bootstrap` fail
 * closed with `KhalaSyncCursorBehindRetainedWindowError` for any cursor,
 * stitch point, or bootstrap page token behind the window (wire mapping
 * `MustRefetch(cursor_behind_retained_window)`; the hub DO's HTTP catch-up
 * returns 410 Gone with the same code), and the client re-bootstraps.
 */

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/**
 * Capture checkpoint table (KS-4 capture lane). Compaction only reads it —
 * and only when it exists (`to_regclass` guard); see the module doc.
 */
export const CAPTURE_CHECKPOINTS_TABLE = "khala_sync_capture_checkpoints"

// ---------------------------------------------------------------------------
// Options / results
// ---------------------------------------------------------------------------

export interface CompactionWindowConfig {
  /**
   * Always keep the newest N version groups per scope (entry-count bound;
   * SPEC §4 "window: max entries or age"). Positive safe integer.
   */
  readonly maxRetainedEntries: number
  /**
   * Age floor: entries younger than this are never compacted, even when
   * they fall outside the entry-count window. Omit to bound by entry count
   * (and capture checkpoints) only.
   */
  readonly maxRetainedAgeMs?: number | undefined
  /** Clock for the age bound (epoch ms). Defaults to `Date.now()`. */
  readonly now?: number | undefined
  /** Compute and report the plan without writing anything. */
  readonly dryRun?: boolean | undefined
}

export interface CompactScopeOptions extends CompactionWindowConfig {
  readonly scope: SyncScope
}

/** Which candidate produced the (attempted) watermark. */
export type CompactionBound =
  | "entry_count"
  | "age"
  | "capture_checkpoint"

export interface CompactScopeResult {
  readonly scope: SyncScope
  readonly dryRun: boolean
  readonly lastVersion: number
  readonly previousRetainedFromVersion: number
  /**
   * The watermark after this run (== previous when nothing could advance).
   * In dry-run mode: the watermark the run WOULD have set.
   */
  readonly newRetainedFromVersion: number
  /** True when the watermark advanced (or would advance, in dry-run). */
  readonly advanced: boolean
  /** The binding candidate for the computed watermark. */
  readonly boundedBy: CompactionBound
  /** Candidate breakdown (null = bound not applicable this run). */
  readonly entryCountCandidate: number
  readonly ageCandidate: number | null
  readonly captureCheckpointCandidate: number | null
  /** Rows deleted (dry-run: rows that WOULD be deleted). */
  readonly deletedRows: number
  /**
   * Rows remaining behind the new watermark: each live entity's latest
   * upsert, preserved for bootstrap's latest-per-entity derivation.
   */
  readonly preservedSnapshotRows: number
}

export interface CompactAllOptions extends CompactionWindowConfig {}

export interface CompactAllSummary {
  readonly dryRun: boolean
  /** Scopes the discovery query selected (entry-count window has moved). */
  readonly scopesExamined: number
  /** Scopes whose watermark advanced (or would, in dry-run). */
  readonly scopesAdvanced: number
  readonly totalDeletedRows: number
  readonly results: ReadonlyArray<CompactScopeResult>
  /** Per-scope failures; one scope failing never blocks the others. */
  readonly failures: ReadonlyArray<{
    readonly scope: string
    readonly messageSafe: string
  }>
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const toVersionCount = (raw: string | number | bigint, what: string): number => {
  const value = typeof raw === "number" ? raw : Number(raw)
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `${what} out of safe range: ${String(raw)}`,
    )
  }
  return value
}

const validateWindow = (config: CompactionWindowConfig): void => {
  if (
    !Number.isSafeInteger(config.maxRetainedEntries) ||
    config.maxRetainedEntries < 1
  ) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `maxRetainedEntries must be a positive safe integer, got ${String(config.maxRetainedEntries)}`,
    )
  }
  if (
    config.maxRetainedAgeMs !== undefined &&
    (!Number.isSafeInteger(config.maxRetainedAgeMs) || config.maxRetainedAgeMs < 0)
  ) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `maxRetainedAgeMs must be a non-negative safe integer, got ${String(config.maxRetainedAgeMs)}`,
    )
  }
}

const checkpointTableExists = async (tx: TransactionSQL): Promise<boolean> => {
  const rows: Array<{ table_exists: boolean }> = await tx`
    SELECT to_regclass(${CAPTURE_CHECKPOINTS_TABLE}) IS NOT NULL AS table_exists
  `
  return rows[0]?.table_exists === true
}

// ---------------------------------------------------------------------------
// compactScope
// ---------------------------------------------------------------------------

/**
 * Compact one scope: compute the new retained-window watermark (minimum of
 * the entry-count, age, and capture-checkpoint candidates — see the module
 * doc), then, in the SAME transaction, advance
 * `khala_sync_scopes.retained_from_version` and delete the compactable
 * changelog rows behind it. Never regresses the watermark and never
 * violates the retention CHECK constraint. `dryRun` computes the identical
 * plan (including would-delete row counts) without writing.
 */
export const compactScope = async (
  sql: SQL,
  options: CompactScopeOptions,
): Promise<CompactScopeResult> => {
  validateWindow(options)
  const dryRun = options.dryRun === true
  const now = options.now ?? Date.now()

  try {
    return await sql.begin(async (tx) => {
      // Lock the scope counter row: serializes against version allocation
      // (outbox-writer takes the same lock), so last_version is stable for
      // the whole plan+delete and the CHECK constraint is provably safe.
      const scopeRows: Array<{
        last_version: string | number | bigint
        retained_from_version: string | number | bigint
      }> = await tx`
        SELECT last_version, retained_from_version
          FROM khala_sync_scopes
         WHERE scope = ${options.scope}
           FOR UPDATE
      `
      const scopeRow = scopeRows[0]
      const lastVersion =
        scopeRow === undefined ? 0 : toVersionCount(scopeRow.last_version, "last_version")
      const retainedFrom =
        scopeRow === undefined
          ? 1
          : toVersionCount(scopeRow.retained_from_version, "retained_from_version")

      // Candidate 1: entry count. Always <= last_version + 1 (N >= 1), so
      // the minimum below can never exceed the CHECK constraint's bound.
      const entryCountCandidate = Math.max(
        1,
        lastVersion - options.maxRetainedEntries + 1,
      )

      // Candidate 2: age floor — the first version group young enough that
      // it must be kept. Whole-group granularity: MIN(version) over young
      // rows keeps the entire group containing any young row.
      let ageCandidate: number | null = null
      if (options.maxRetainedAgeMs !== undefined) {
        const cutoff = new Date(now - options.maxRetainedAgeMs)
        const rows: Array<{ min_version: string | number | bigint | null }> = await tx`
          SELECT min(version) AS min_version
            FROM khala_sync_changelog
           WHERE scope = ${options.scope}
             AND committed_at >= ${cutoff}
        `
        const minVersion = rows[0]?.min_version ?? null
        ageCandidate =
          minVersion === null
            ? lastVersion + 1
            : toVersionCount(minVersion, "age-bound min version")
      }

      // Candidate 3: capture checkpoint (table-existence-guarded; the
      // capture lane lands concurrently). Missing scope row = nothing
      // pushed yet = fail closed at watermark 1.
      let captureCheckpointCandidate: number | null = null
      if (await checkpointTableExists(tx)) {
        const rows: Array<{ pushed_through_version: string | number | bigint }> =
          await tx`
            SELECT pushed_through_version
              FROM khala_sync_capture_checkpoints
             WHERE scope = ${options.scope}
          `
        const pushed =
          rows[0] === undefined
            ? 0
            : toVersionCount(rows[0].pushed_through_version, "pushed_through_version")
        captureCheckpointCandidate = pushed + 1
      }

      // Minimum of the candidates; first minimal label wins for reporting.
      const candidates: Array<readonly [CompactionBound, number]> = [
        ["entry_count", entryCountCandidate] as const,
        ...(ageCandidate === null ? [] : [["age", ageCandidate] as const]),
        ...(captureCheckpointCandidate === null
          ? []
          : [["capture_checkpoint", captureCheckpointCandidate] as const]),
      ]
      let boundedBy: CompactionBound = "entry_count"
      let computed = Number.POSITIVE_INFINITY
      for (const [label, value] of candidates) {
        if (value < computed) {
          boundedBy = label
          computed = value
        }
      }
      // Clamps: never regress, never pass last_version + 1 (CHECK bound).
      const newFrom = Math.min(
        Math.max(computed, retainedFrom),
        lastVersion + 1,
      )
      const advanced = newFrom > retainedFrom

      const base = {
        scope: options.scope,
        dryRun,
        lastVersion,
        previousRetainedFromVersion: retainedFrom,
        boundedBy,
        entryCountCandidate,
        ageCandidate,
        captureCheckpointCandidate,
      }

      if (!advanced) {
        return {
          ...base,
          newRetainedFromVersion: retainedFrom,
          advanced: false,
          deletedRows: 0,
          preservedSnapshotRows: 0,
        }
      }

      // Compactable = behind the new watermark AND not load-bearing for
      // bootstrap's latest-per-entity derivation: superseded by a newer row
      // for the same entity, or a tombstone (tombstone GC — SPEC §2.3).
      let deletedRows: number
      if (dryRun) {
        const rows: Array<{ n: string | number | bigint }> = await tx`
          SELECT count(*) AS n
            FROM khala_sync_changelog c
           WHERE c.scope = ${options.scope}
             AND c.version < ${newFrom}
             AND (
               c.op = 'delete'
               OR EXISTS (
                 SELECT 1 FROM khala_sync_changelog n
                  WHERE n.scope = c.scope
                    AND n.entity_type = c.entity_type
                    AND n.entity_id = c.entity_id
                    AND n.version > c.version
               )
             )
        `
        deletedRows = toVersionCount(rows[0]?.n ?? 0, "dry-run delete count")
      } else {
        await tx`
          UPDATE khala_sync_scopes
             SET retained_from_version = ${newFrom},
                 updated_at = now()
           WHERE scope = ${options.scope}
        `
        const deleted: Array<{ version: string | number | bigint }> = await tx`
          DELETE FROM khala_sync_changelog c
           WHERE c.scope = ${options.scope}
             AND c.version < ${newFrom}
             AND (
               c.op = 'delete'
               OR EXISTS (
                 SELECT 1 FROM khala_sync_changelog n
                  WHERE n.scope = c.scope
                    AND n.entity_type = c.entity_type
                    AND n.entity_id = c.entity_id
                    AND n.version > c.version
               )
             )
          RETURNING c.version
        `
        deletedRows = deleted.length
      }

      // Snapshot residue behind the new watermark (live latest upserts).
      const preservedRows: Array<{ n: string | number | bigint }> = await tx`
        SELECT count(*) AS n
          FROM khala_sync_changelog
         WHERE scope = ${options.scope}
           AND version < ${newFrom}
      `
      const preservedRaw = toVersionCount(preservedRows[0]?.n ?? 0, "preserved count")
      const preservedSnapshotRows = dryRun
        ? preservedRaw - deletedRows
        : preservedRaw

      return {
        ...base,
        newRetainedFromVersion: newFrom,
        advanced: true,
        deletedRows,
        preservedSnapshotRows,
      }
    })
  } catch (error) {
    const mapped = storageErrorFromUnknown(error)
    throw mapped ?? error
  }
}

// ---------------------------------------------------------------------------
// compactAll
// ---------------------------------------------------------------------------

/**
 * Compact every scope whose retained window can move. Discovery is a single
 * cheap query over `khala_sync_scopes`: the watermark can only advance when
 * the entry-count candidate has passed the current watermark (the age and
 * checkpoint bounds only ever hold it back further, so this predicate is
 * exact). Each scope compacts in its own transaction with error isolation:
 * one scope failing (its transaction rolls back watermark AND deletions
 * together) never blocks the rest.
 */
export const compactAll = async (
  sql: SQL,
  options: CompactAllOptions,
): Promise<CompactAllSummary> => {
  validateWindow(options)
  const dryRun = options.dryRun === true

  let scopes: Array<{ scope: string }>
  try {
    scopes = await sql`
      SELECT scope
        FROM khala_sync_scopes
       WHERE GREATEST(1, last_version - ${options.maxRetainedEntries} + 1)
             > retained_from_version
       ORDER BY scope
    `
  } catch (error) {
    const mapped = storageErrorFromUnknown(error)
    throw mapped ?? error
  }

  const results: Array<CompactScopeResult> = []
  const failures: Array<{ scope: string; messageSafe: string }> = []
  for (const { scope } of scopes) {
    try {
      results.push(
        await compactScope(sql, {
          ...options,
          scope: scope as SyncScope,
          dryRun,
        }),
      )
    } catch (error) {
      failures.push({
        scope,
        messageSafe:
          error instanceof KhalaSyncStorageError
            ? error.messageSafe
            : error instanceof Error
              ? error.message
              : String(error),
      })
    }
  }

  return {
    dryRun,
    scopesExamined: scopes.length,
    scopesAdvanced: results.filter((r) => r.advanced).length,
    totalDeletedRows: results.reduce((sum, r) => sum + r.deletedRows, 0),
    results,
    failures,
  }
}
