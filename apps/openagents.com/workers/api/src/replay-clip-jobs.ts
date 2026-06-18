/**
 * Replay clip job store (EPIC #5411, issue #5432).
 *
 * Hosts clip-job create/read records inside the existing api Worker + D1. The
 * Worker stores job records and serves public-safe refs only; it NEVER renders
 * frames or runs native binaries. Rendering runs on the owned render box
 * (#5431), which claims `queued` jobs and reports a finished manifest ref.
 *
 * NEEDS-OWNER: serving the finished mp4/manifest bytes requires an
 * owner-provisioned R2 bucket + public host (see #5431). Until it exists, the
 * store records jobs and the route serves the public manifest URL the render
 * box reports; it never serves clip bytes from the Worker.
 *
 * Authority boundary: clip jobs are observation/projection records only. They
 * grant no settlement, payout, deployment, accepted-work, provider, wallet, or
 * public-claim authority.
 */
import {
  REPLAY_CLIP_CLAIM_SCOPE,
  REPLAY_CLIP_JOB_SCHEMA_VERSION,
  assertReplayClipJobRecordSafe,
  assertReplayClipJobRequestSafe,
  type ReplayClipJobRecord,
  type ReplayClipJobRequest,
  type ReplayClipJobStatus,
} from '@openagentsinc/replay-clips'

import { parseJsonUnknown, stringArrayFromUnknown } from './json-boundary'

export type ReplayClipJobStore = Readonly<{
  insert: (record: ReplayClipJobRecord) => Promise<void>
  read: (jobRef: string) => Promise<ReplayClipJobRecord | null>
  listRecent: (limit: number) => Promise<ReadonlyArray<ReplayClipJobRecord>>
}>

/** Default caveat attached to every clip job's public projection. */
export const REPLAY_CLIP_JOB_DEFAULT_CAVEAT =
  'Replay clip jobs are evidence-presentation only and grant no settlement, payout, deployment, accepted-work, provider, wallet, or public-claim authority.'

/** Default page size for the list route. */
export const REPLAY_CLIP_JOB_LIST_LIMIT = 50

/**
 * Build a freshly-queued clip-job record from a validated request. The Worker
 * only ever creates jobs in `queued`; the render box advances the lifecycle.
 */
export const queuedReplayClipJobRecord = (input: {
  jobRef: string
  request: ReplayClipJobRequest
  nowIso: string
}): ReplayClipJobRecord =>
  assertReplayClipJobRecordSafe({
    schemaVersion: REPLAY_CLIP_JOB_SCHEMA_VERSION,
    jobRef: input.jobRef,
    status: 'queued' satisfies ReplayClipJobStatus,
    claimScope: REPLAY_CLIP_CLAIM_SCOPE,
    source: input.request.source,
    render: input.request.render,
    cameraPath: input.request.cameraPath,
    sourceRefs: input.request.sourceRefs,
    caveatRefs: [REPLAY_CLIP_JOB_DEFAULT_CAVEAT],
    blockerRefs: [],
    createdAt: input.nowIso,
    updatedAt: input.nowIso,
  })

/** Decode + validate an incoming clip-job request (fails closed on unsafe input). */
export const decodeReplayClipJobRequestSafe = (
  input: unknown,
): ReplayClipJobRequest => assertReplayClipJobRequestSafe(input)

type ReplayClipJobRow = {
  job_ref: string
  status: string
  request_json: string
  source_refs_json: string
  caveat_refs_json: string
  blocker_refs_json: string
  manifest_ref: string | null
  created_at: string
  updated_at: string
}

const recordFromRow = (row: ReplayClipJobRow): ReplayClipJobRecord => {
  const request = assertReplayClipJobRequestSafe(
    parseJsonUnknown(row.request_json),
  )
  return assertReplayClipJobRecordSafe({
    schemaVersion: REPLAY_CLIP_JOB_SCHEMA_VERSION,
    jobRef: row.job_ref,
    status: row.status,
    claimScope: REPLAY_CLIP_CLAIM_SCOPE,
    source: request.source,
    render: request.render,
    cameraPath: request.cameraPath,
    sourceRefs: stringArrayFromUnknown(parseJsonUnknown(row.source_refs_json)),
    caveatRefs: stringArrayFromUnknown(parseJsonUnknown(row.caveat_refs_json)),
    blockerRefs: stringArrayFromUnknown(parseJsonUnknown(row.blocker_refs_json)),
    ...(row.manifest_ref === null ? {} : { manifestRef: row.manifest_ref }),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  })
}

export const makeD1ReplayClipJobStore = (
  db: D1Database,
): ReplayClipJobStore => ({
  insert: async record => {
    await db
      .prepare(
        `INSERT INTO replay_clip_jobs
          (job_ref, status, request_json, source_refs_json, caveat_refs_json,
           blocker_refs_json, manifest_ref, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        record.jobRef,
        record.status,
        JSON.stringify({
          schemaVersion: record.schemaVersion,
          source: record.source,
          render: record.render,
          cameraPath: record.cameraPath,
          sourceRefs: record.sourceRefs,
        }),
        JSON.stringify(record.sourceRefs),
        JSON.stringify(record.caveatRefs),
        JSON.stringify(record.blockerRefs),
        record.manifestRef ?? null,
        record.createdAt,
        record.updatedAt,
      )
      .run()
  },

  read: async jobRef => {
    const row = await db
      .prepare(`SELECT * FROM replay_clip_jobs WHERE job_ref = ? LIMIT 1`)
      .bind(jobRef)
      .first<ReplayClipJobRow>()
    return row === null ? null : recordFromRow(row)
  },

  listRecent: async limit => {
    const result = await db
      .prepare(
        `SELECT * FROM replay_clip_jobs ORDER BY updated_at DESC LIMIT ?`,
      )
      .bind(limit)
      .all<ReplayClipJobRow>()
    return (result.results ?? []).map(recordFromRow)
  },
})

/** In-memory store for tests and local development (no D1 binding required). */
export const makeInMemoryReplayClipJobStore = (
  seed: ReadonlyArray<ReplayClipJobRecord> = [],
): ReplayClipJobStore => {
  const records = new Map<string, ReplayClipJobRecord>(
    seed.map(record => [record.jobRef, record]),
  )
  return {
    insert: async record => {
      records.set(record.jobRef, record)
    },
    read: async jobRef => records.get(jobRef) ?? null,
    listRecent: async limit =>
      [...records.values()]
        .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
        .slice(0, limit),
  }
}
