import { Schema as S } from 'effect'

import type { TraceVisibility } from './atif-trace-schema'
import { parseJsonUnknown } from './json-boundary'

/**
 * D1-backed agent trace store (openagents #6208). Stores the PUBLIC-SAFE ATIF
 * trajectory projection keyed by a uuid; large blobs (video/screenshots) live in
 * R2 and are referenced here only as public-safe R2 keys.
 *
 * The store factory is plain-async (D1 prepare/bind/run/first). Effect wrapping
 * happens in the route layer (see `trace-store-routes.ts`), matching the repo's
 * store convention.
 */

export class TraceStoreError extends S.TaggedErrorClass<TraceStoreError>()(
  'TraceStoreError',
  {
    kind: S.Literals(['conflict', 'not_found', 'storage_error']),
    reason: S.String,
  },
) {}

export const traceStoreErrorFromUnknown = (error: unknown): TraceStoreError =>
  error instanceof TraceStoreError
    ? error
    : new TraceStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      })

/** A public-safe R2 reference for a video/screenshot blob. */
export type TraceBlobRef = Readonly<{
  kind: 'video' | 'screenshot' | 'image'
  r2Key: string
  contentType?: string
  caption?: string
}>

export type TraceRecord = Readonly<{
  traceUuid: string
  ownerUserId: string
  agentRef: string
  schemaVersion: string
  trajectoryId: string
  sessionId: string | null
  visibility: TraceVisibility
  stepCount: number
  /** The public-safe ATIF trajectory projection (already tripwired). */
  trajectory: unknown
  blobRefs: ReadonlyArray<TraceBlobRef>
  idempotencyKey: string | null
  createdAt: string
  updatedAt: string
}>

export type CreateTraceInput = Readonly<{
  traceUuid: string
  ownerUserId: string
  agentRef: string
  schemaVersion: string
  trajectoryId: string
  sessionId: string | null
  visibility: TraceVisibility
  stepCount: number
  trajectory: unknown
  blobRefs: ReadonlyArray<TraceBlobRef>
  idempotencyKey: string | null
  nowIso: string
}>

export type CreateTraceResult = Readonly<{
  record: TraceRecord
  /** false when an existing trace was returned for an idempotent replay. */
  created: boolean
}>

export type TraceStore = Readonly<{
  /**
   * Store a trace. If `idempotencyKey` is set and a trace already exists for
   * `(ownerUserId, idempotencyKey)`, the existing record is returned unchanged
   * with `created: false` (idempotent replay).
   */
  createTrace: (input: CreateTraceInput) => Promise<CreateTraceResult>
  readTraceByUuid: (traceUuid: string) => Promise<TraceRecord | undefined>
  listTracesForOwner: (
    ownerUserId: string,
    limit: number,
  ) => Promise<ReadonlyArray<TraceRecord>>
}>

const str = (value: unknown): string => (typeof value === 'string' ? value : '')
const nullableStr = (value: unknown): string | null =>
  typeof value === 'string' ? value : null
const num = (value: unknown): number =>
  typeof value === 'number' ? value : Number(value ?? 0)

const visibilityFromRow = (value: unknown): TraceVisibility =>
  value === 'public' || value === 'owner_only' ? value : 'unlisted'

const jsonParseOr = <A>(value: unknown, fallback: A): A => {
  if (typeof value !== 'string' || value.trim() === '') {
    return fallback
  }
  try {
    return parseJsonUnknown(value) as A
  } catch {
    return fallback
  }
}

const recordFromRow = (row: Record<string, unknown>): TraceRecord => ({
  traceUuid: str(row.trace_uuid),
  ownerUserId: str(row.owner_user_id),
  agentRef: str(row.agent_ref),
  schemaVersion: str(row.schema_version),
  trajectoryId: str(row.trajectory_id),
  sessionId: nullableStr(row.session_id),
  visibility: visibilityFromRow(row.visibility),
  stepCount: num(row.step_count),
  trajectory: jsonParseOr<unknown>(row.trajectory_json, {}),
  blobRefs: jsonParseOr<ReadonlyArray<TraceBlobRef>>(row.blob_refs_json, []),
  idempotencyKey: nullableStr(row.idempotency_key),
  createdAt: str(row.created_at),
  updatedAt: str(row.updated_at),
})

export const makeD1TraceStore = (db: D1Database): TraceStore => {
  const readByUuid = async (
    traceUuid: string,
  ): Promise<TraceRecord | undefined> => {
    try {
      const row = await db
        .prepare(
          `SELECT trace_uuid, owner_user_id, agent_ref, schema_version,
                  trajectory_id, session_id, visibility, step_count,
                  trajectory_json, blob_refs_json, idempotency_key,
                  created_at, updated_at
             FROM agent_traces
            WHERE trace_uuid = ?1`,
        )
        .bind(traceUuid)
        .first<Record<string, unknown>>()

      return row === null ? undefined : recordFromRow(row)
    } catch (error) {
      throw traceStoreErrorFromUnknown(error)
    }
  }

  const readByIdempotency = async (
    ownerUserId: string,
    idempotencyKey: string,
  ): Promise<TraceRecord | undefined> => {
    try {
      const row = await db
        .prepare(
          `SELECT trace_uuid, owner_user_id, agent_ref, schema_version,
                  trajectory_id, session_id, visibility, step_count,
                  trajectory_json, blob_refs_json, idempotency_key,
                  created_at, updated_at
             FROM agent_traces
            WHERE owner_user_id = ?1 AND idempotency_key = ?2`,
        )
        .bind(ownerUserId, idempotencyKey)
        .first<Record<string, unknown>>()

      return row === null ? undefined : recordFromRow(row)
    } catch (error) {
      throw traceStoreErrorFromUnknown(error)
    }
  }

  return {
    createTrace: async input => {
      if (input.idempotencyKey !== null) {
        const existing = await readByIdempotency(
          input.ownerUserId,
          input.idempotencyKey,
        )
        if (existing !== undefined) {
          return { record: existing, created: false }
        }
      }

      try {
        await db
          .prepare(
            `INSERT INTO agent_traces
               (trace_uuid, owner_user_id, agent_ref, schema_version,
                trajectory_id, session_id, visibility, step_count,
                trajectory_json, blob_refs_json, idempotency_key,
                created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)`,
          )
          .bind(
            input.traceUuid,
            input.ownerUserId,
            input.agentRef,
            input.schemaVersion,
            input.trajectoryId,
            input.sessionId,
            input.visibility,
            input.stepCount,
            JSON.stringify(input.trajectory),
            JSON.stringify(input.blobRefs),
            input.idempotencyKey,
            input.nowIso,
            input.nowIso,
          )
          .run()
      } catch (error) {
        // A racing idempotent insert can lose the unique-index race; surface the
        // already-stored record rather than a conflict.
        if (input.idempotencyKey !== null) {
          const existing = await readByIdempotency(
            input.ownerUserId,
            input.idempotencyKey,
          )
          if (existing !== undefined) {
            return { record: existing, created: false }
          }
        }
        throw traceStoreErrorFromUnknown(error)
      }

      const stored = await readByUuid(input.traceUuid)
      if (stored === undefined) {
        throw new TraceStoreError({
          kind: 'storage_error',
          reason: 'Trace was inserted but could not be read back.',
        })
      }
      return { record: stored, created: true }
    },
    readTraceByUuid: readByUuid,
    listTracesForOwner: async (ownerUserId, limit) => {
      try {
        const result = await db
          .prepare(
            `SELECT trace_uuid, owner_user_id, agent_ref, schema_version,
                    trajectory_id, session_id, visibility, step_count,
                    trajectory_json, blob_refs_json, idempotency_key,
                    created_at, updated_at
               FROM agent_traces
              WHERE owner_user_id = ?1
              ORDER BY created_at DESC
              LIMIT ?2`,
          )
          .bind(ownerUserId, limit)
          .all<Record<string, unknown>>()

        return (result.results ?? []).map(recordFromRow)
      } catch (error) {
        throw traceStoreErrorFromUnknown(error)
      }
    },
  }
}
