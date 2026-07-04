import {
  BootstrapEntity,
  BootstrapResponse,
  canonicalJson,
  type ChangelogEntry,
  EntityId,
  EntityType,
  KHALA_SYNC_PROTOCOL_VERSION,
  LogPage,
  type SyncScope,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import type { SQL, TransactionSQL } from "bun"
import {
  KhalaSyncCursorBehindRetainedWindowError,
  KhalaSyncInvalidPageTokenError,
  KhalaSyncStorageError,
  storageErrorFromUnknown,
} from "./errors.js"
import { changelogEntryFromRow, type ChangelogRow } from "./outbox-writer.js"

/**
 * Khala Sync read substrate (KS-2.2; SPEC §3 bootstrap/log, §4 substrate):
 * `logPage` — offset-resumable catch-up reads over `khala_sync_changelog`,
 * and `bootstrap` — consistent snapshot pages of a scope's current entity
 * states stitched to the scope version (the stitch cursor).
 *
 * Both run on the Worker's **Hyperdrive** path: transaction-mode pooling, so
 * no session state may span requests. Each call is one self-contained
 * Postgres transaction (REPEATABLE READ, so the scope counter and the page
 * rows come from ONE snapshot); nothing is held between pages.
 *
 * ## Bootstrap snapshot design (v1)
 *
 * The current entity states are derived from the changelog itself: the
 * latest row per `(entity_type, entity_id)` with `version <=` the snapshot
 * cursor, where entities whose latest row is a tombstone (`op = 'delete'`)
 * are omitted. The snapshot cursor is `khala_sync_scopes.last_version` read
 * in the same REPEATABLE READ transaction as the first page.
 *
 * **Why reading `last_version` first is gap-free:** version allocation holds
 * the scope-counter row lock until the writing transaction commits
 * (outbox-writer, KS-2.1), so per-scope versions are commit-ordered — if we
 * observe `last_version = v`, every version `<= v` is already committed and
 * visible in our snapshot. There is no window where `v` is visible but
 * `v - 1` is not.
 *
 * **Why multi-page works without holding a transaction:** Hyperdrive cannot
 * pin one Postgres session across HTTP requests, so a classic
 * held-open-snapshot cursor is impossible. Instead page tokens are
 * self-contained: they encode `(snapshotCursor, lastEntityKey)`, and every
 * subsequent page re-derives latest-per-entity under the predicate
 * `version <= snapshotCursor AND (entity_type, entity_id) > lastEntityKey`.
 * Because committed changelog rows at versions `<= snapshotCursor` are
 * immutable (concurrent writers only ever append at higher versions), that
 * predicate returns the same rows no matter how many writes commit between
 * pages — each page is exactly what a held snapshot at `snapshotCursor`
 * would have returned. The only thing that can invalidate a token is
 * compaction advancing `retained_from_version` past the snapshot cursor;
 * that case fails closed with
 * {@link KhalaSyncCursorBehindRetainedWindowError} (client re-bootstraps).
 *
 * The final page carries `cursor = snapshotCursor` and the client stitches:
 * apply all snapshot pages, then `logPage(afterVersion = snapshotCursor)`
 * until `upToDate` (SPEC §3). Entities that changed after the snapshot are
 * re-delivered by the log with their newer post-images; apply is idempotent
 * per (scope, version, entity), so the seam is exact.
 *
 * ## Log page shape
 *
 * `logPage` bounds a page by **distinct versions**, never splitting one
 * version's rows across pages: `nextCursor` is the highest version returned,
 * and the next request reads strictly greater versions — if a page could end
 * mid-version, the rest of that version's rows would be skipped forever.
 * A page therefore contains at most `limit` versions (each version's row
 * count is bounded by the entities one transaction touched).
 */

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/** Default / max distinct versions per log page. */
export const DEFAULT_LOG_PAGE_LIMIT = 500
export const MAX_LOG_PAGE_LIMIT = 1_000

/** Default / max entities scanned per bootstrap page. */
export const DEFAULT_BOOTSTRAP_PAGE_SIZE = 500
export const MAX_BOOTSTRAP_PAGE_SIZE = 1_000

const clampPositive = (value: number | undefined, dflt: number, max: number): number => {
  if (value === undefined) return dflt
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `page limit must be a positive safe integer, got ${String(value)}`,
    )
  }
  return Math.min(value, max)
}

// ---------------------------------------------------------------------------
// Scope counter row
// ---------------------------------------------------------------------------

interface ScopeRow {
  readonly last_version: string | number | bigint
  readonly retained_from_version: string | number | bigint
}

/** Like the writer's version mapping but 0 is valid (empty scope watermark). */
const toWatermark = (raw: string | number | bigint): number => {
  const value = typeof raw === "number" ? raw : Number(raw)
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `scope version watermark out of safe range: ${String(raw)}`,
    )
  }
  return value
}

/** A scope with no counter row behaves as (last_version 0, retained_from 1). */
const readScopeCounters = async (
  tx: TransactionSQL,
  scope: SyncScope,
): Promise<{ lastVersion: number; retainedFromVersion: number }> => {
  const rows: Array<ScopeRow> = await tx`
    SELECT last_version, retained_from_version
      FROM khala_sync_scopes
     WHERE scope = ${scope}
  `
  const row = rows[0]
  if (row === undefined) return { lastVersion: 0, retainedFromVersion: 1 }
  return {
    lastVersion: toWatermark(row.last_version),
    retainedFromVersion: toWatermark(row.retained_from_version),
  }
}

/**
 * Serving entries after `afterVersion` needs version `afterVersion + 1`
 * onward to still be retained: fail closed when
 * `afterVersion < retained_from_version - 1` (invariant 6 — MustRefetch,
 * never a silently partial log).
 */
const assertInsideRetainedWindow = (
  scope: SyncScope,
  afterVersion: number,
  retainedFromVersion: number,
): void => {
  if (afterVersion < retainedFromVersion - 1) {
    throw new KhalaSyncCursorBehindRetainedWindowError(
      scope,
      afterVersion,
      retainedFromVersion,
    )
  }
}

// ---------------------------------------------------------------------------
// logPage
// ---------------------------------------------------------------------------

export interface LogPageInput {
  readonly scope: SyncScope
  /** Resume-after watermark; `null` (or 0) = scope start. */
  readonly afterVersion: number | null
  /** Max distinct versions in the page; clamped to {@link MAX_LOG_PAGE_LIMIT}. */
  readonly limit?: number | undefined
}

/**
 * One ordered catch-up page of `khala_sync_changelog` for `scope`, strictly
 * after `afterVersion`, ordered by (version, entity_type, entity_id).
 *
 * - `nextCursor` = highest version returned, or `afterVersion` when empty.
 * - `upToDate` = `nextCursor === khala_sync_scopes.last_version`, with the
 *   counter read in the SAME REPEATABLE READ transaction as the page.
 * - Pages never split a version: the LIMIT bounds distinct versions, so
 *   resuming from `nextCursor` can never skip rows of a half-delivered
 *   version.
 */
export const logPage = async (sql: SQL, input: LogPageInput): Promise<LogPage> => {
  const after = input.afterVersion ?? 0
  if (!Number.isSafeInteger(after) || after < 0) {
    throw new KhalaSyncStorageError(
      "constraint_violation",
      `afterVersion must be a non-negative safe integer, got ${String(input.afterVersion)}`,
    )
  }
  const limit = clampPositive(input.limit, DEFAULT_LOG_PAGE_LIMIT, MAX_LOG_PAGE_LIMIT)

  try {
    return await sql.begin("isolation level repeatable read", async (tx) => {
      const { lastVersion, retainedFromVersion } = await readScopeCounters(
        tx,
        input.scope,
      )
      assertInsideRetainedWindow(input.scope, after, retainedFromVersion)

      const rows: Array<ChangelogRow> = await tx`
        SELECT scope, version, entity_type, entity_id, op,
               post_image_json, mutation_ref, committed_at
          FROM khala_sync_changelog
         WHERE scope = ${input.scope}
           AND version > ${after}
           AND version <= COALESCE(
             (SELECT max(pv.version) FROM (
                SELECT DISTINCT version
                  FROM khala_sync_changelog
                 WHERE scope = ${input.scope} AND version > ${after}
                 ORDER BY version
                 LIMIT ${limit}
              ) AS pv),
             ${after})
         ORDER BY version, entity_type, entity_id
      `
      const entries: Array<ChangelogEntry> = rows.map(changelogEntryFromRow)
      const lastEntry = entries[entries.length - 1]
      const nextCursor = lastEntry === undefined ? after : Number(lastEntry.version)
      return new LogPage({
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        scope: input.scope,
        entries,
        nextCursor: SyncVersionWatermark.make(nextCursor),
        upToDate: nextCursor === lastVersion,
      })
    })
  } catch (error) {
    const mapped = storageErrorFromUnknown(error)
    throw mapped ?? error
  }
}

// ---------------------------------------------------------------------------
// Bootstrap page tokens (self-contained — Hyperdrive holds no session state)
// ---------------------------------------------------------------------------

interface BootstrapPageToken {
  readonly v: 1
  readonly scope: string
  readonly snapshotCursor: number
  readonly lastEntityType: string
  readonly lastEntityId: string
}

const encodePageToken = (token: BootstrapPageToken): string =>
  Buffer.from(canonicalJson(token), "utf8").toString("base64url")

const decodePageToken = (scope: SyncScope, raw: string): BootstrapPageToken => {
  let parsed: unknown
  try {
    parsed = JSON.parse(Buffer.from(raw, "base64url").toString("utf8"))
  } catch {
    throw new KhalaSyncInvalidPageTokenError("bootstrap page token is not decodable")
  }
  if (typeof parsed !== "object" || parsed === null) {
    throw new KhalaSyncInvalidPageTokenError("bootstrap page token has no payload")
  }
  const t = parsed as Partial<Record<keyof BootstrapPageToken, unknown>>
  if (
    t.v !== 1 ||
    typeof t.scope !== "string" ||
    typeof t.snapshotCursor !== "number" ||
    !Number.isSafeInteger(t.snapshotCursor) ||
    t.snapshotCursor < 0 ||
    typeof t.lastEntityType !== "string" ||
    typeof t.lastEntityId !== "string"
  ) {
    throw new KhalaSyncInvalidPageTokenError("bootstrap page token is malformed")
  }
  if (t.scope !== String(scope)) {
    throw new KhalaSyncInvalidPageTokenError(
      "bootstrap page token belongs to a different scope",
    )
  }
  return {
    v: 1,
    scope: t.scope,
    snapshotCursor: t.snapshotCursor,
    lastEntityType: t.lastEntityType,
    lastEntityId: t.lastEntityId,
  }
}

// ---------------------------------------------------------------------------
// bootstrap
// ---------------------------------------------------------------------------

export interface BootstrapInput {
  readonly scope: SyncScope
  /** Max entities scanned per page; clamped to {@link MAX_BOOTSTRAP_PAGE_SIZE}. */
  readonly pageSize?: number | undefined
  /** Opaque token from the previous page's `nextPageToken`. */
  readonly pageToken?: string | undefined
}

/** Latest-row-per-entity page row (tombstoned entities still occupy a slot). */
interface SnapshotRow {
  readonly entity_type: string
  readonly entity_id: string
  readonly op: string
  readonly post_image_json: string | object | null
}

/**
 * One consistent snapshot page of `scope`'s current entity states. See the
 * module doc for the full design: the first page pins the snapshot cursor
 * (`khala_sync_scopes.last_version`, read in the same REPEATABLE READ
 * transaction as the page rows); later pages are made consistent relative to
 * that cursor purely by the `version <= snapshotCursor` predicate plus
 * key-set pagination, so no transaction spans requests (Hyperdrive). The
 * final page carries `cursor = snapshotCursor` and no `nextPageToken`.
 *
 * Note: a page may return fewer than `pageSize` entities without being the
 * last page — entities whose latest state is a tombstone consume scan slots
 * but are (correctly) omitted from `entities`.
 */
export const bootstrap = async (
  sql: SQL,
  input: BootstrapInput,
): Promise<BootstrapResponse> => {
  const pageSize = clampPositive(
    input.pageSize,
    DEFAULT_BOOTSTRAP_PAGE_SIZE,
    MAX_BOOTSTRAP_PAGE_SIZE,
  )
  const token =
    input.pageToken === undefined
      ? undefined
      : decodePageToken(input.scope, input.pageToken)

  try {
    return await sql.begin("isolation level repeatable read", async (tx) => {
      const { lastVersion, retainedFromVersion } = await readScopeCounters(
        tx,
        input.scope,
      )
      const snapshotCursor = token === undefined ? lastVersion : token.snapshotCursor
      if (snapshotCursor > lastVersion) {
        // A token can never legitimately be ahead of the scope counter.
        throw new KhalaSyncInvalidPageTokenError(
          "bootstrap page token snapshot cursor is ahead of the scope",
        )
      }
      // The stitch after the last page is logPage(afterVersion =
      // snapshotCursor); fail every page early once compaction has passed
      // the snapshot (invariant 6) instead of handing out a snapshot the
      // client can no longer catch up from.
      assertInsideRetainedWindow(input.scope, snapshotCursor, retainedFromVersion)

      const rows: Array<SnapshotRow> =
        snapshotCursor === 0
          ? []
          : token === undefined
            ? await tx`
                SELECT DISTINCT ON (entity_type, entity_id)
                       entity_type, entity_id, op, post_image_json
                  FROM khala_sync_changelog
                 WHERE scope = ${input.scope}
                   AND version <= ${snapshotCursor}
                 ORDER BY entity_type, entity_id, version DESC
                 LIMIT ${pageSize}
              `
            : await tx`
                SELECT DISTINCT ON (entity_type, entity_id)
                       entity_type, entity_id, op, post_image_json
                  FROM khala_sync_changelog
                 WHERE scope = ${input.scope}
                   AND version <= ${snapshotCursor}
                   AND (entity_type, entity_id) >
                       (${token.lastEntityType}, ${token.lastEntityId})
                 ORDER BY entity_type, entity_id, version DESC
                 LIMIT ${pageSize}
              `

      const entities = rows
        .filter((row) => row.op === "upsert")
        .map((row) => {
          if (row.post_image_json === null) {
            throw new KhalaSyncStorageError(
              "constraint_violation",
              "upsert changelog row without a post-image",
            )
          }
          return new BootstrapEntity({
            entityType: EntityType.make(row.entity_type),
            entityId: EntityId.make(row.entity_id),
            // Re-serialize canonically: jsonb storage does not preserve the
            // original bytes, canonicalJson does (same rule as the writer).
            postImageJson: canonicalJson(
              typeof row.post_image_json === "string"
                ? JSON.parse(row.post_image_json)
                : row.post_image_json,
            ),
          })
        })

      const lastRow = rows[rows.length - 1]
      const hasMore = rows.length === pageSize && lastRow !== undefined
      return new BootstrapResponse({
        protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
        scope: input.scope,
        entities,
        ...(hasMore
          ? {
              nextPageToken: encodePageToken({
                v: 1,
                scope: String(input.scope),
                snapshotCursor,
                lastEntityType: lastRow.entity_type,
                lastEntityId: lastRow.entity_id,
              }),
            }
          : { cursor: SyncVersionWatermark.make(snapshotCursor) }),
      })
    })
  } catch (error) {
    const mapped = storageErrorFromUnknown(error)
    throw mapped ?? error
  }
}
