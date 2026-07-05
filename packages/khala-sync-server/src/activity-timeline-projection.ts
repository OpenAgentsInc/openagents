import {
  ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_ID,
  ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_TYPE,
  ACTIVITY_TIMELINE_SNAPSHOT_CHANNEL_ID,
  decodeActivityTimelineSnapshotEntity,
  encodeActivityTimelineSnapshotEntity,
  EntityId,
  EntityType,
  publicScope,
  type ActivityTimelineSnapshotEntity,
} from "@openagentsinc/khala-sync"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SqlTag, SyncSql } from "./sql.js"

/**
 * Public activity-timeline stored-snapshot projection (KS-6.7b, #8421; SPEC
 * §2.1 `scope.public.<channel>`, §7 invariant 8/9).
 *
 * See `@openagentsinc/khala-sync`'s `./activity-timeline-snapshot.js` module
 * doc for the full rationale (seven source domains, no shared write-site
 * hook, so this is a REBUILD-ON-CRON snapshot rather than an event-sourced
 * projection like settled-feed/gym-run-progress).
 *
 * UNLIKE the tokens-served public counter, this projection never invents or
 * accumulates a value itself — same discipline as settled-feed/gym-run-
 * progress/tokens-served-mix: the caller (the Worker's scheduled cron glue)
 * already computed the exact post-image via the SAME live-merge function the
 * per-request route calls, so a repeated refresh with the SAME computed
 * events is naturally idempotent — a replayed/duplicate refresh simply
 * appends another (structurally identical, or newer) changelog version.
 *
 * WRITE: `projectActivityTimelineSnapshot` upserts the ONE snapshot entity
 * (`entityId = "current"`) into `scope.public.activity-timeline` via the
 * KHALA_SYNC_DB Hyperdrive binding. FAIL-SOFT: a projection failure never
 * fails or slows the caller (the scheduled cron tick).
 *
 * READ: `readActivityTimelineSnapshot` returns the latest post-image
 * straight off `khala_sync_changelog` (no bespoke storage table — the
 * generic changelog IS the store here, same as every other KS-6.x
 * "single/few entity" projection).
 */

// ---------------------------------------------------------------------------
// Named system writer (SPEC §7 invariant 3) + scope
// ---------------------------------------------------------------------------

export const ACTIVITY_TIMELINE_PROJECTION_SYSTEM_REF =
  "system:activity_timeline_projection.public_activity_timeline.v1"

/** The activity-timeline scope: `scope.public.activity-timeline`. */
export const activityTimelinePublicScope = () =>
  publicScope(ACTIVITY_TIMELINE_SNAPSHOT_CHANNEL_ID)

// ---------------------------------------------------------------------------
// Diagnostics + fail-soft outcome shape (same shape as every other KS-6.x
// projection)
// ---------------------------------------------------------------------------

export interface ActivityTimelineProjectionDiagnostic {
  /** Coarse classification for logs/metrics; never carries row values. */
  readonly reason: "invalid_input" | "storage_failed" | "projection_failed"
  readonly messageSafe: string
}

export type ActivityTimelineProjectionOutcome =
  | { readonly ok: true }
  | {
      readonly ok: false
      readonly diagnostic: ActivityTimelineProjectionDiagnostic
    }

const diagnosticFromUnknown = (
  error: unknown,
): ActivityTimelineProjectionDiagnostic => {
  const tag = (error as { _tag?: unknown })?._tag
  if (tag === "KhalaSyncStorageError") {
    const messageSafe = (error as { messageSafe?: unknown }).messageSafe
    return {
      messageSafe:
        typeof messageSafe === "string" ? messageSafe : "storage failure",
      reason: "storage_failed",
    }
  }
  // Anything else (driver errors, decode failures) can embed raw values or
  // connection strings — never echo them.
  return {
    messageSafe: "activity-timeline snapshot projection failed",
    reason: "projection_failed",
  }
}

// ---------------------------------------------------------------------------
// Write (one upsert for the whole snapshot)
// ---------------------------------------------------------------------------

export const projectActivityTimelineSnapshot = async (
  sql: SyncSql,
  snapshot: ActivityTimelineSnapshotEntity,
): Promise<void> => {
  const validated = decodeActivityTimelineSnapshotEntity(snapshot)
  await withSyncTransaction(sql, async writer => {
    await writer.appendChange({
      entityId: EntityId.make(ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_ID),
      entityType: EntityType.make(ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_TYPE),
      mutationRef: ACTIVITY_TIMELINE_PROJECTION_SYSTEM_REF,
      op: "upsert",
      postImage: encodeActivityTimelineSnapshotEntity(validated),
      scope: activityTimelinePublicScope(),
    })
  })
}

/**
 * Decode + project one activity-timeline snapshot FAIL-SOFT: this function
 * never throws. `rawSnapshot` is the caller's plain post-image shape
 * (already produced by `buildPublicActivityTimelineRawSnapshot`, whose
 * events already passed `assertPublicActivityTimelineEventSafe` upstream);
 * it is decoded through the entity contract here as a second, structural
 * gate before anything reaches Postgres.
 */
export const projectActivityTimelineSnapshotBestEffort = async (
  sql: SyncSql,
  rawSnapshot: unknown,
): Promise<ActivityTimelineProjectionOutcome> => {
  let validated: ActivityTimelineSnapshotEntity
  try {
    validated = decodeActivityTimelineSnapshotEntity(rawSnapshot)
  } catch {
    return {
      diagnostic: {
        messageSafe:
          "activity-timeline snapshot post-image failed contract validation",
        reason: "invalid_input",
      },
      ok: false,
    }
  }
  try {
    await projectActivityTimelineSnapshot(sql, validated)
    return { ok: true }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}

// ---------------------------------------------------------------------------
// Read (latest post-image, straight off the changelog — no bespoke table)
// ---------------------------------------------------------------------------

interface ChangelogPostImageRow {
  readonly post_image_json: string | object | null
}

const parseJson = (raw: string | object | null): unknown =>
  raw === null ? null : typeof raw === "string" ? JSON.parse(raw) : raw

/**
 * Read the latest activity-timeline snapshot, or `null` when none has been
 * projected yet (pre-first-cron-tick, or after a fresh deploy before the
 * first refresh). Callers fail open to the live merge on `null`.
 */
export const readActivityTimelineSnapshot = async (
  sql: SqlTag,
): Promise<ActivityTimelineSnapshotEntity | null> => {
  const rows: Array<ChangelogPostImageRow> = await sql`
    SELECT post_image_json
      FROM khala_sync_changelog
     WHERE scope = ${activityTimelinePublicScope()}
       AND entity_type = ${ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_TYPE}
       AND entity_id = ${ACTIVITY_TIMELINE_SNAPSHOT_ENTITY_ID}
       AND op = 'upsert'
     ORDER BY version DESC
     LIMIT 1
  `
  const json = parseJson(rows[0]?.post_image_json ?? null)
  return json === null ? null : decodeActivityTimelineSnapshotEntity(json)
}
