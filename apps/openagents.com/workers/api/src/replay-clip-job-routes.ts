/**
 * Public replay clip job/read API (EPIC #5411, issue #5432):
 * - `POST /api/public/replay-clips`            create a clip job (queued)
 * - `GET  /api/public/replay-clips`            list recent clip jobs
 * - `GET  /api/public/replay-clips/{jobRef}`   read one clip job
 *
 * The Worker only creates/reads job records and serves public-safe refs. It
 * does NOT render frames, run native binaries, or assume any native process.
 * Rendering is the owned render box's responsibility (#5431): it claims
 * `queued` jobs, renders with headless Chromium + ffmpeg, uploads to R2, and
 * reports a finished manifest URL. This API boundary makes that split explicit.
 *
 * Projection posture: composed from the clip-job store at read time
 * (`live_at_read`); the payload carries `generatedAt` and the shared staleness
 * contract from `public-projection-staleness.ts`. Read-only observation
 * evidence; grants no settlement, payout, deployment, accepted-work, provider,
 * wallet, or public-claim authority.
 *
 * NEEDS-OWNER: the finished clip + manifest bytes live in an owner-provisioned
 * R2 bucket (#5431). This route serves only the public manifest URL the render
 * box reports; it never streams clip bytes from the Worker. Wiring this route
 * into `workers/api/src/index.ts` is the one-line owner integration step (the
 * route is left unwired here to avoid colliding with the parallel
 * activity-timeline lane committing to main).
 */
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown } from './json-boundary'
import { liveAtReadStaleness } from './public-projection-staleness'
import {
  REPLAY_CLIP_JOB_DEFAULT_CAVEAT,
  REPLAY_CLIP_JOB_LIST_LIMIT,
  decodeReplayClipJobRequestSafe,
  makeD1ReplayClipJobStore,
  queuedReplayClipJobRecord,
  type ReplayClipJobStore,
} from './replay-clip-jobs'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'

import type { ReplayClipJobRecord } from '@openagentsinc/replay-clips'

export const PUBLIC_REPLAY_CLIP_JOBS_PROJECTION_CONTRACT =
  'projection.replay_clip_jobs.v1'

export const PUBLIC_REPLAY_CLIP_JOBS_ROUTE = '/api/public/replay-clips'

/**
 * `live_at_read`: the projection is composed from the clip-job store on every
 * request, so it can never be older than the request. It rebuilds on job
 * creation and any render-box lifecycle transition.
 */
const stalenessContract = liveAtReadStaleness([
  'replay_clip_job_created',
  'replay_clip_job_status_transition',
])

const READ_ONLY_CAVEAT = REPLAY_CLIP_JOB_DEFAULT_CAVEAT

/** Public-safe projection of a clip-job record. */
const jobProjection = (record: ReplayClipJobRecord) => ({
  blockerRefs: record.blockerRefs,
  cameraPath: record.cameraPath,
  caveatRefs: record.caveatRefs,
  claimScope: record.claimScope,
  createdAt: record.createdAt,
  jobRef: record.jobRef,
  manifestRef: record.manifestRef ?? null,
  render: record.render,
  schemaVersion: record.schemaVersion,
  source: record.source,
  sourceRefs: record.sourceRefs,
  status: record.status,
  updatedAt: record.updatedAt,
})

const envelope = (input: {
  nowIso: string
  body: Record<string, unknown>
}) => ({
  authorityCaveat: READ_ONLY_CAVEAT,
  contractVersion: PUBLIC_REPLAY_CLIP_JOBS_PROJECTION_CONTRACT,
  generatedAt: input.nowIso,
  publicSafe: true,
  staleness: stalenessContract,
  ...input.body,
})

export type PublicReplayClipJobRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  store?: ReplayClipJobStore
  nowIso?: () => string
  newJobRef?: () => string
}>

const resolveStore = (
  input: PublicReplayClipJobRouteInput,
): ReplayClipJobStore =>
  input.store ?? makeD1ReplayClipJobStore(input.OPENAGENTS_DB as D1Database)

/** Handle `POST /api/public/replay-clips` and `GET` list. */
export const handlePublicReplayClipJobsApi = (
  request: Request,
  input: PublicReplayClipJobRouteInput,
) => {
  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const store = resolveStore(input)

  if (request.method === 'GET') {
    return Effect.promise(async () => {
      const jobs = await store.listRecent(REPLAY_CLIP_JOB_LIST_LIMIT)
      return noStoreJsonResponse(
        envelope({
          nowIso,
          body: { jobs: jobs.map(jobProjection), kind: 'replay_clip_jobs' },
        }),
      )
    })
  }

  if (request.method === 'POST') {
    return Effect.promise(async () => {
      const rawBody = await request.text()
      const parsed = (() => {
        try {
          return parseJsonUnknown(rawBody)
        } catch {
          return undefined
        }
      })()
      if (parsed === undefined) {
        return noStoreJsonResponse(
          { error: 'replay_clip_job_invalid_json' },
          { status: 400 },
        )
      }

      const validated = (() => {
        try {
          return { ok: true as const, request: decodeReplayClipJobRequestSafe(parsed) }
        } catch (error) {
          return { ok: false as const, message: String(error) }
        }
      })()
      if (!validated.ok) {
        return noStoreJsonResponse(
          {
            error: 'replay_clip_job_invalid_request',
            reason: validated.message,
          },
          { status: 400 },
        )
      }

      const jobRef = `replay_clip_job.${input.newJobRef?.() ?? randomUuid()}`
      const record = queuedReplayClipJobRecord({
        jobRef,
        request: validated.request,
        nowIso,
      })
      await store.insert(record)

      return noStoreJsonResponse(
        envelope({ nowIso, body: { job: jobProjection(record) } }),
        { status: 201 },
      )
    })
  }

  return Effect.succeed(methodNotAllowed(['GET', 'POST']))
}

/** Handle `GET /api/public/replay-clips/{jobRef}`. */
export const handlePublicReplayClipJobReadApi = (
  request: Request,
  jobRef: string,
  input: PublicReplayClipJobRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso?.() ?? currentIsoTimestamp()
  const store = resolveStore(input)

  return Effect.promise(async () => {
    const record = await store.read(jobRef)
    if (record === null) {
      return noStoreJsonResponse(
        { error: 'replay_clip_job_not_found', jobRef },
        { status: 404 },
      )
    }
    return noStoreJsonResponse(
      envelope({ nowIso, body: { job: jobProjection(record) } }),
    )
  })
}
