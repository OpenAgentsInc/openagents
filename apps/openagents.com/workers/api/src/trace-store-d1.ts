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

/** How an upload authenticated (#6221). */
export type TraceUploadSource = 'agent' | 'user_session'

/** Public-safe demand-origin segment attached to captured traces (#6298). */
export type TraceDemandKind =
  | 'external'
  | 'internal'
  | 'own_capacity'
  | 'unlabeled'

export type TraceListFilters = Readonly<{
  demandKind?: TraceDemandKind | undefined
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
  /**
   * The public-safe ATIF trajectory projection (already tripwired). For large
   * trajectories this is the placeholder `{}` and the full JSON lives in R2 at
   * `trajectoryR2Key` (#6221); the route layer rehydrates it on read.
   */
  trajectory: unknown
  /**
   * R2 key for the full public-safe trajectory JSON when it is too large to
   * inline in a single D1 value (~1MB cap). Null when stored inline (#6221).
   */
  trajectoryR2Key: string | null
  blobRefs: ReadonlyArray<TraceBlobRef>
  idempotencyKey: string | null
  /**
   * Data market (#6221). The uploader's explicit grant to use this trace as
   * training/eval data for Khala. Defaults WITHHELD: consent is never assumed.
   */
  trainingConsent: boolean
  /** Optional public-safe license label the uploader attached. */
  license: string | null
  /**
   * SHA-256 hex digest over the canonical public-safe payload, used ONLY to
   * dedup per-owner uploads (no double reward). Not a settlement digest.
   */
  contentDigest: string | null
  /**
   * INERT revshare stub (#6221). `rewardEligible` may be set when the
   * data-market reward flag is armed, consent was granted, and the upload is
   * not a duplicate; `rewardAmountSats` stays null ("reward TBD"). Grants no
   * payout, settlement, spend, or accepted-work authority. No money moves.
   */
  rewardEligible: boolean
  rewardAmountSats: number | null
  /** Whether the upload arrived via agent bearer or a user web session. */
  uploadSource: TraceUploadSource
  /**
   * Public-safe demand-origin classification (#6298). Null means the trace was
   * created before attribution existed, or via an upload path with no resolved
   * demand metadata.
   */
  demandKind: TraceDemandKind | null
  demandSource: string | null
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
  trajectoryR2Key: string | null
  blobRefs: ReadonlyArray<TraceBlobRef>
  idempotencyKey: string | null
  trainingConsent: boolean
  license: string | null
  contentDigest: string | null
  rewardEligible: boolean
  rewardAmountSats: number | null
  uploadSource: TraceUploadSource
  demandKind?: TraceDemandKind | null | undefined
  demandSource?: string | null | undefined
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
    filters?: TraceListFilters | undefined,
  ) => Promise<ReadonlyArray<TraceRecord>>
  /**
   * Dedup lookup (#6221): the existing trace for `(ownerUserId, contentDigest)`,
   * if any. Used to reject duplicate uploads (no double reward).
   */
  findTraceByOwnerDigest: (
    ownerUserId: string,
    contentDigest: string,
  ) => Promise<TraceRecord | undefined>
  /**
   * Per-user rate limiting (#6221): how many traces the owner has stored at or
   * after `sinceIso`. Bounded; abuse control only.
   */
  countTracesForOwnerSince: (
    ownerUserId: string,
    sinceIso: string,
  ) => Promise<number>
}>

const str = (value: unknown): string => (typeof value === 'string' ? value : '')
const nullableStr = (value: unknown): string | null =>
  typeof value === 'string' ? value : null
const num = (value: unknown): number =>
  typeof value === 'number' ? value : Number(value ?? 0)
const nullableNum = (value: unknown): number | null =>
  value === null || value === undefined ? null : Number(value)
// D1 stores booleans as 0/1 integers.
const bool = (value: unknown): boolean =>
  value === 1 || value === '1' || value === true

const uploadSourceFromRow = (value: unknown): TraceUploadSource =>
  value === 'user_session' ? 'user_session' : 'agent'

const demandKindFromRow = (value: unknown): TraceDemandKind | null => {
  if (
    value === 'external' ||
    value === 'internal' ||
    value === 'own_capacity' ||
    value === 'unlabeled'
  ) {
    return value
  }
  return null
}

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
  trajectoryR2Key: nullableStr(row.trajectory_r2_key),
  blobRefs: jsonParseOr<ReadonlyArray<TraceBlobRef>>(row.blob_refs_json, []),
  idempotencyKey: nullableStr(row.idempotency_key),
  trainingConsent: bool(row.training_consent),
  license: nullableStr(row.license),
  contentDigest: nullableStr(row.content_digest),
  rewardEligible: bool(row.reward_eligible),
  rewardAmountSats: nullableNum(row.reward_amount_sats),
  uploadSource: uploadSourceFromRow(row.upload_source),
  demandKind: demandKindFromRow(row.demand_kind),
  demandSource: nullableStr(row.demand_source),
  createdAt: str(row.created_at),
  updatedAt: str(row.updated_at),
})

// Shared public-safe column projection. Extended by #6221 with the data-market
// consent/license/digest/reward/upload-source columns (migration 0229) and the
// large-trajectory R2 pointer (migration 0230). Demand-origin columns (#6298)
// are selected only after their migration is present so code-before-migration
// deploys continue to read existing traces.
const TRACE_BASE_COLUMNS = `trace_uuid, owner_user_id, agent_ref, schema_version,
                  trajectory_id, session_id, visibility, step_count,
                  trajectory_json, trajectory_r2_key, blob_refs_json,
                  idempotency_key,
                  training_consent, license, content_digest,
                  reward_eligible, reward_amount_sats, upload_source`

const TRACE_COLUMNS_WITH_DEMAND = `${TRACE_BASE_COLUMNS},
                  demand_kind, demand_source,
                  created_at, updated_at`

const TRACE_COLUMNS_WITHOUT_DEMAND = `${TRACE_BASE_COLUMNS},
                  NULL AS demand_kind, NULL AS demand_source,
                  created_at, updated_at`

const traceColumns = (includeDemandAttribution: boolean): string =>
  includeDemandAttribution
    ? TRACE_COLUMNS_WITH_DEMAND
    : TRACE_COLUMNS_WITHOUT_DEMAND

export const makeD1TraceStore = (db: D1Database): TraceStore => {
  let demandAttributionColumnsAvailable: boolean | undefined

  const hasDemandAttributionColumns = async (): Promise<boolean> => {
    if (demandAttributionColumnsAvailable !== undefined) {
      return demandAttributionColumnsAvailable
    }
    const result = await db
      .prepare('PRAGMA table_info(agent_traces)')
      .all<Record<string, unknown>>()
    const names = new Set((result.results ?? []).map(row => str(row.name)))
    demandAttributionColumnsAvailable =
      names.has('demand_kind') && names.has('demand_source')
    return demandAttributionColumnsAvailable
  }

  const selectTraceColumns = async (): Promise<string> =>
    traceColumns(await hasDemandAttributionColumns())

  const readByUuid = async (
    traceUuid: string,
  ): Promise<TraceRecord | undefined> => {
    try {
      const columns = await selectTraceColumns()
      const row = await db
        .prepare(
          `SELECT ${columns}
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
      const columns = await selectTraceColumns()
      const row = await db
        .prepare(
          `SELECT ${columns}
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
        const demandColumnsAvailable = await hasDemandAttributionColumns()
        const statement = demandColumnsAvailable
          ? db
              .prepare(
                `INSERT INTO agent_traces
                   (trace_uuid, owner_user_id, agent_ref, schema_version,
                    trajectory_id, session_id, visibility, step_count,
                    trajectory_json, trajectory_r2_key, blob_refs_json,
                    idempotency_key,
                    training_consent, license, content_digest,
                    reward_eligible, reward_amount_sats, upload_source,
                    demand_kind, demand_source,
                    created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                         ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)`,
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
                input.trajectoryR2Key,
                JSON.stringify(input.blobRefs),
                input.idempotencyKey,
                input.trainingConsent ? 1 : 0,
                input.license,
                input.contentDigest,
                input.rewardEligible ? 1 : 0,
                input.rewardAmountSats,
                input.uploadSource,
                input.demandKind ?? null,
                input.demandSource ?? null,
                input.nowIso,
                input.nowIso,
              )
          : db
              .prepare(
                `INSERT INTO agent_traces
                   (trace_uuid, owner_user_id, agent_ref, schema_version,
                    trajectory_id, session_id, visibility, step_count,
                    trajectory_json, trajectory_r2_key, blob_refs_json,
                    idempotency_key,
                    training_consent, license, content_digest,
                    reward_eligible, reward_amount_sats, upload_source,
                    created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                         ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)`,
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
                input.trajectoryR2Key,
                JSON.stringify(input.blobRefs),
                input.idempotencyKey,
                input.trainingConsent ? 1 : 0,
                input.license,
                input.contentDigest,
                input.rewardEligible ? 1 : 0,
                input.rewardAmountSats,
                input.uploadSource,
                input.nowIso,
                input.nowIso,
              )
        await statement.run()
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
    listTracesForOwner: async (ownerUserId, limit, filters) => {
      try {
        const demandKind = filters?.demandKind
        const demandColumnsAvailable = await hasDemandAttributionColumns()
        if (demandKind !== undefined && !demandColumnsAvailable) {
          return []
        }
        const columns = traceColumns(demandColumnsAvailable)
        const result =
          demandKind === undefined
            ? await db
                .prepare(
                  `SELECT ${columns}
                     FROM agent_traces
                    WHERE owner_user_id = ?1
                    ORDER BY created_at DESC
                    LIMIT ?2`,
                )
                .bind(ownerUserId, limit)
                .all<Record<string, unknown>>()
            : await db
                .prepare(
                  `SELECT ${columns}
                     FROM agent_traces
                    WHERE owner_user_id = ?1 AND demand_kind = ?2
                    ORDER BY created_at DESC
                    LIMIT ?3`,
                )
                .bind(ownerUserId, demandKind, limit)
                .all<Record<string, unknown>>()

        return (result.results ?? []).map(recordFromRow)
      } catch (error) {
        throw traceStoreErrorFromUnknown(error)
      }
    },
    findTraceByOwnerDigest: async (ownerUserId, contentDigest) => {
      try {
        const columns = await selectTraceColumns()
        const row = await db
          .prepare(
            `SELECT ${columns}
               FROM agent_traces
              WHERE owner_user_id = ?1 AND content_digest = ?2`,
          )
          .bind(ownerUserId, contentDigest)
          .first<Record<string, unknown>>()

        return row === null ? undefined : recordFromRow(row)
      } catch (error) {
        throw traceStoreErrorFromUnknown(error)
      }
    },
    countTracesForOwnerSince: async (ownerUserId, sinceIso) => {
      try {
        const row = await db
          .prepare(
            `SELECT COUNT(*) AS n
               FROM agent_traces
              WHERE owner_user_id = ?1 AND created_at >= ?2`,
          )
          .bind(ownerUserId, sinceIso)
          .first<Record<string, unknown>>()

        return row === null ? 0 : num(row.n)
      } catch (error) {
        throw traceStoreErrorFromUnknown(error)
      }
    },
  }
}

/**
 * R2-backed store for large public-safe trajectory JSON (#6221). A real agent
 * session can be a few MB of redacted ATIF, which exceeds D1's ~1MB-per-value
 * limit, so the trajectory JSON goes to R2 and only a pointer lives in D1.
 *
 * R2 holds ONLY the same public-safe, already-tripwired projection D1 would
 * have stored — never raw prompts, logs, provider payloads, secrets, or PII.
 */
export type TraceTrajectoryBlobStore = Readonly<{
  /** Persist the public-safe trajectory JSON for a trace; returns the R2 key. */
  putTrajectory: (traceUuid: string, trajectoryJson: string) => Promise<string>
  /** Read back the trajectory JSON for a stored R2 key (null if missing). */
  getTrajectory: (r2Key: string) => Promise<string | null>
}>

/** The canonical R2 key for a trace's offloaded trajectory JSON. */
export const traceTrajectoryR2Key = (traceUuid: string): string =>
  `traces/${traceUuid}/trajectory.json`

export const makeR2TraceTrajectoryBlobStore = (
  bucket: R2Bucket,
): TraceTrajectoryBlobStore => ({
  putTrajectory: async (traceUuid, trajectoryJson) => {
    const key = traceTrajectoryR2Key(traceUuid)
    try {
      await bucket.put(key, trajectoryJson, {
        httpMetadata: { contentType: 'application/json; charset=utf-8' },
      })
    } catch (error) {
      throw traceStoreErrorFromUnknown(error)
    }
    return key
  },
  getTrajectory: async r2Key => {
    try {
      const object = await bucket.get(r2Key)
      return object === null ? null : await object.text()
    } catch (error) {
      throw traceStoreErrorFromUnknown(error)
    }
  },
})

/**
 * R2-backed store for a trace's playable MEDIA blobs (#6223): the recording
 * (`session.mp4`/`session.webm`) and screenshots referenced from the trace's
 * `blobRefs[]`. The bytes live under `trace-blobs/{uuid}/{r2Key}` so the
 * visibility-gated serve route (`GET /api/traces/{uuid}/blob/{r2Key}`) can
 * stream them. This makes `/trace/{uuid}` self-hosted — it never depends on a
 * GitHub attachment.
 *
 * PUBLIC-SAFE by construction: a blob here is the video/screenshots of a PUBLIC
 * QA session — no secrets. The trajectory TEXT is separately tripwired on
 * ingest; media bytes carry no token/PII surface to scan.
 */
export type TraceMediaBlobObject = Readonly<{
  /** The streamable body. */
  body: ReadableStream
  /** Byte size of the object, when known. */
  size: number
  /** The stored content type (R2 httpMetadata), when present. */
  contentType: string | undefined
  /** The R2 entity tag, when present (used for caching/conditional reads). */
  httpEtag: string | undefined
}>

export type TraceMediaBlobStore = Readonly<{
  /**
   * Persist one media blob for a trace; returns the full R2 key it was stored
   * at. `r2Key` is the public-safe RELATIVE artifact path from the trace's
   * blobRef (e.g. `session.mp4`, `shots/00-login.png`).
   */
  putBlob: (
    traceUuid: string,
    r2Key: string,
    bytes: ArrayBuffer | Uint8Array,
    contentType: string | undefined,
  ) => Promise<string>
  /** Read back one media blob (null if missing). */
  getBlob: (
    traceUuid: string,
    r2Key: string,
  ) => Promise<TraceMediaBlobObject | null>
}>

/** The canonical R2 key for one of a trace's media blobs. */
export const traceMediaBlobR2Key = (traceUuid: string, r2Key: string): string =>
  // Strip any leading slashes / `..` traversal from the relative artifact path;
  // the trace uuid namespaces the object so different traces never collide.
  `trace-blobs/${traceUuid}/${r2Key.replace(/^\/+/, '').replace(/\.\.(?=\/|$)/g, '')}`

export const makeR2TraceMediaBlobStore = (
  bucket: R2Bucket,
): TraceMediaBlobStore => ({
  putBlob: async (traceUuid, r2Key, bytes, contentType) => {
    const key = traceMediaBlobR2Key(traceUuid, r2Key)
    try {
      await bucket.put(key, bytes, {
        httpMetadata: contentType === undefined ? {} : { contentType },
      })
    } catch (error) {
      throw traceStoreErrorFromUnknown(error)
    }
    return key
  },
  getBlob: async (traceUuid, r2Key) => {
    const key = traceMediaBlobR2Key(traceUuid, r2Key)
    try {
      const object = await bucket.get(key)
      if (object === null) {
        return null
      }
      return {
        body: object.body,
        size: object.size,
        contentType: object.httpMetadata?.contentType,
        httpEtag: object.httpEtag,
      }
    } catch (error) {
      throw traceStoreErrorFromUnknown(error)
    }
  },
})
