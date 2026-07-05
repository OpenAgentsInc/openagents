import {
  type ChangelogEntry,
  decodeSettledFeedEventEntity,
  decodeSettledFeedSummaryEntity,
  encodeSettledFeedEventEntity,
  encodeSettledFeedSummaryEntity,
  EntityId,
  EntityType,
  publicScope,
  SETTLED_FEED_CHANNEL_ID,
  SETTLED_FEED_EVENT_ENTITY_TYPE,
  SETTLED_FEED_SUMMARY_ENTITY_ID,
  SETTLED_FEED_SUMMARY_ENTITY_TYPE,
  type SettledFeedEventEntity,
  type SettledFeedSummaryEntity,
} from "@openagentsinc/khala-sync"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SqlTag, SyncSql } from "./sql.js"

/**
 * Public settled-feed scope projection (KS-6.4, #8414; SPEC §2.1
 * `scope.public.<channel>`, §7 invariant 9).
 *
 * The live settled feed (openagents #5311) used to publish exclusively
 * through the legacy `notifySyncScopes`/`SyncRoomDurableObject` spine
 * (`tassadar-settled-feed-sync.ts`). This module is the khala-sync
 * projection producer/reader for the SAME public-safe events: one settled
 * Bitcoin leg per event, plus a running summary.
 *
 * UNLIKE the tokens-served public counter (./public-counter-projection.ts),
 * this projection never invents or accumulates a value itself — the caller
 * already computed the authoritative running totals from the real
 * settlement ledger (`buildSettledFeedEvents`) before an event reaches here.
 * So there is no separate increment/idempotency-guard table: each event is
 * an UPSERT keyed by its own stable `eventRef`, and a replayed publish of
 * the SAME event is naturally idempotent (identical post-image, same
 * entity id) — same discipline the legacy outbox write already relied on
 * (`op: 'put', id: event.eventRef`).
 *
 * WRITE: `projectSettledFeedEventsBestEffort` appends N event upserts + one
 * summary upsert to `scope.public.settled-feed` in ONE Postgres transaction
 * via the KHALA_SYNC_DB Hyperdrive binding. FAIL-SOFT (same discipline as
 * the KS-6.1 fleet projection / KS-6.3 tokens-served projection): a
 * projection failure never fails the caller's real settlement dispatch.
 *
 * READ: `readSettledFeedProjection` returns the latest N event post-images
 * (recency-ordered) plus the current summary, read directly off
 * `khala_sync_changelog` (no bespoke storage table — the generic changelog
 * IS the store here, unlike the counter's dedicated
 * `khala_sync_public_counters` row).
 */

// ---------------------------------------------------------------------------
// Named system writer (SPEC §7 invariant 3) + scope
// ---------------------------------------------------------------------------

export const SETTLED_FEED_PROJECTION_SYSTEM_REF =
  "system:settled_feed_projection.tassadar_settled_feed.v1"

/** The settled-feed scope: `scope.public.settled-feed`. */
export const settledFeedPublicScope = () => publicScope(SETTLED_FEED_CHANNEL_ID)

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

export interface SettledFeedProjectionInput {
  readonly events: ReadonlyArray<SettledFeedEventEntity>
  readonly summary: SettledFeedSummaryEntity
}

/**
 * Append the settled-feed events + summary to `scope.public.settled-feed`
 * in ONE transaction (one changelog version for the whole batch). Every
 * post-image is round-tripped through the entity contract's encode so a
 * malformed caller input fails BEFORE any write (decode already ran at the
 * call site; encode here is defense in depth, same discipline as fleet's
 * `appendFleetEntityChange`).
 */
export const projectSettledFeedEvents = async (
  sql: SyncSql,
  input: SettledFeedProjectionInput,
): Promise<ReadonlyArray<ChangelogEntry>> =>
  withSyncTransaction(sql, async (writer) => {
    const scope = settledFeedPublicScope()
    const entries: Array<ChangelogEntry> = []
    for (const event of input.events) {
      // Decode-then-encode (not a bare encode): callers may pass a
      // structurally-matching plain object rather than a real class
      // instance (TypeScript's structural typing allows both), and
      // `S.encodeSync` requires an actual instance. Re-decoding first also
      // re-validates every post-image right before it is written — same
      // discipline as the public-counter projection's `counterPostImage`.
      const validated = decodeSettledFeedEventEntity(event)
      entries.push(
        await writer.appendChange({
          entityId: EntityId.make(validated.eventRef),
          entityType: EntityType.make(SETTLED_FEED_EVENT_ENTITY_TYPE),
          mutationRef: SETTLED_FEED_PROJECTION_SYSTEM_REF,
          op: "upsert",
          postImage: encodeSettledFeedEventEntity(validated),
          scope,
        }),
      )
    }
    const validatedSummary = decodeSettledFeedSummaryEntity(input.summary)
    entries.push(
      await writer.appendChange({
        entityId: EntityId.make(SETTLED_FEED_SUMMARY_ENTITY_ID),
        entityType: EntityType.make(SETTLED_FEED_SUMMARY_ENTITY_TYPE),
        mutationRef: SETTLED_FEED_PROJECTION_SYSTEM_REF,
        op: "upsert",
        postImage: encodeSettledFeedSummaryEntity(validatedSummary),
        scope,
      }),
    )
    return entries
  })

// ---------------------------------------------------------------------------
// Fail-soft producer wrapper (same discipline as fleet/public-counter)
// ---------------------------------------------------------------------------

export interface SettledFeedProjectionDiagnostic {
  /** Coarse classification for logs/metrics; never carries row values. */
  readonly reason: "invalid_input" | "storage_failed" | "projection_failed"
  readonly messageSafe: string
}

export type SettledFeedProjectionOutcome =
  | { readonly ok: true; readonly entries: ReadonlyArray<ChangelogEntry> }
  | {
      readonly ok: false
      readonly diagnostic: SettledFeedProjectionDiagnostic
    }

const diagnosticFromUnknown = (
  error: unknown,
): SettledFeedProjectionDiagnostic => {
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
    messageSafe: "settled-feed projection failed",
    reason: "projection_failed",
  }
}

/**
 * Decode + project one settled-feed batch FAIL-SOFT: this function never
 * throws. `rawEvents`/`rawSummary` are the caller's plain post-image shapes
 * (already public-safety-scanned by `assertSettledFeedPayloadPublicSafe`
 * upstream); they are decoded through the entity contracts here as a second,
 * structural gate before anything reaches Postgres.
 */
export const projectSettledFeedEventsBestEffort = async (
  sql: SyncSql,
  input: Readonly<{
    rawEvents: ReadonlyArray<unknown>
    rawSummary: unknown
  }>,
): Promise<SettledFeedProjectionOutcome> => {
  let events: ReadonlyArray<SettledFeedEventEntity>
  let summary: SettledFeedSummaryEntity
  try {
    events = input.rawEvents.map(rawEvent => decodeSettledFeedEventEntity(rawEvent))
    summary = decodeSettledFeedSummaryEntity(input.rawSummary)
  } catch {
    return {
      diagnostic: {
        messageSafe: "settled-feed post-image failed contract validation",
        reason: "invalid_input",
      },
      ok: false,
    }
  }
  if (events.length === 0) {
    return { entries: [], ok: true }
  }
  try {
    const entries = await projectSettledFeedEvents(sql, { events, summary })
    return { entries, ok: true }
  } catch (error) {
    return { diagnostic: diagnosticFromUnknown(error), ok: false }
  }
}

// ---------------------------------------------------------------------------
// Read (latest N events + current summary, straight off the changelog)
// ---------------------------------------------------------------------------

export const DEFAULT_SETTLED_FEED_READ_LIMIT = 50
export const MAX_SETTLED_FEED_READ_LIMIT = 200

export interface SettledFeedProjectionSnapshot {
  readonly events: ReadonlyArray<SettledFeedEventEntity>
  readonly summary: SettledFeedSummaryEntity | null
}

interface ChangelogPostImageRow {
  readonly post_image_json: string | object | null
}

/**
 * Read the latest `limit` settled-feed events (most-recent-first) plus the
 * current summary, straight off `khala_sync_changelog` for
 * `scope.public.settled-feed` — no bespoke storage table (see module doc).
 * Tombstoned rows never occur here (this projection only ever upserts), so
 * every matching row carries a post-image.
 */
export const readSettledFeedProjection = async (
  sql: SqlTag,
  options: Readonly<{ limit?: number }> = {},
): Promise<SettledFeedProjectionSnapshot> => {
  const limit = Math.min(
    Math.max(1, Math.trunc(options.limit ?? DEFAULT_SETTLED_FEED_READ_LIMIT)),
    MAX_SETTLED_FEED_READ_LIMIT,
  )
  const scope = settledFeedPublicScope()

  // Dedupe to the LATEST row per entity_id first (a replayed publish of the
  // same eventRef appends a new changelog row/version rather than mutating
  // the old one — the changelog is append-only), THEN order the deduped
  // latest-state rows by recency and limit. Without the inner DISTINCT ON, a
  // replayed event would show up twice (its older AND newer row) instead of
  // once at its newest version — same shape as read-service.ts's bootstrap
  // snapshot query.
  const eventRows: Array<ChangelogPostImageRow> = await sql`
    SELECT post_image_json
      FROM (
        SELECT DISTINCT ON (entity_id) entity_id, post_image_json, version
          FROM khala_sync_changelog
         WHERE scope = ${scope}
           AND entity_type = ${SETTLED_FEED_EVENT_ENTITY_TYPE}
           AND op = 'upsert'
         ORDER BY entity_id, version DESC
      ) AS latest_per_entity
     ORDER BY version DESC
     LIMIT ${limit}
  `
  const summaryRows: Array<ChangelogPostImageRow> = await sql`
    SELECT post_image_json
      FROM khala_sync_changelog
     WHERE scope = ${scope}
       AND entity_type = ${SETTLED_FEED_SUMMARY_ENTITY_TYPE}
       AND entity_id = ${SETTLED_FEED_SUMMARY_ENTITY_ID}
       AND op = 'upsert'
     ORDER BY version DESC
     LIMIT 1
  `

  const parseJson = (raw: string | object | null): unknown =>
    raw === null ? null : typeof raw === "string" ? JSON.parse(raw) : raw

  const events = eventRows
    .map((row) => parseJson(row.post_image_json))
    .filter((value): value is unknown => value !== null)
    .map(value => decodeSettledFeedEventEntity(value))

  const summaryJson = parseJson(summaryRows[0]?.post_image_json ?? null)
  const summary = summaryJson === null
    ? null
    : decodeSettledFeedSummaryEntity(summaryJson)

  return { events, summary }
}
