// Routes for live Gym / Harbor run progress (#6261, #6271, epic #6253).
//
//   - `GET /api/operator/gym/run-progress` (admin bearer): the scoped operator
//     status surface. Returns every live progress object, INCLUDING `local_only`
//     runs that are not yet authorized for web publication. Still public-safe —
//     "scoped" gates VISIBILITY of unpublished runs, not extra private fields.
//   - `POST /api/operator/gym/run-progress` (admin bearer, #6271): the push-ingest
//     boundary. A Harbor-side pusher POSTs a `GymRunProgressInput` snapshot; the
//     Worker RE-BUILDS it through `buildGymRunProgress` (which re-asserts
//     `checkGymRunProgressPublicSafety`) and upserts the public-safe object by
//     `runRef`. Anything smuggling prompts/responses/logs/trajectories/keys/
//     private endpoints is REJECTED with a typed 400 and never stored.
//   - `GET /api/public/gym/run-progress` (no auth): the public-safe projection.
//     `web_authorized` runs render their live counts; `local_only` runs degrade
//     honestly to an "awaiting authorization" marker with NO live numbers.
//
// The endpoints read the STORED ingested runs (D1) — there is no seeded fixture.
// The surface is honestly `[]` until a real run is pushed.
//
// REALTIME PUSH (#6261): the POST ingest also publishes the public-safe projected
// snapshot to the live `public-gym-run-progress` sync scope (see
// `run-progress-sync.ts`), reusing the SAME sync-room outbox + poke path the
// Khala tokens-served counter uses. The `/gym` follow-along subscribes to that
// scope over a WebSocket and updates the instant a snapshot is ingested; the GET
// projection remains as the cold-read seed plus a slow socket-down reconcile
// fallback, no longer a ~12s poll.
//
// No dispatch, spend, settlement, payout, or public-claim authority. A progress
// object is in-progress evidence only and is always `decisionGrade: false`.
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../../http/responses'
import { currentIsoTimestamp } from '../../runtime-primitives'
import { storedSnapshotStaleness } from '../../public-projection-staleness'
import {
  buildGymRunProgress,
  GymRunProgressError,
  type GymRunProgress,
  projectPublicGymRunProgress,
} from './run-progress'
import type {
  GymRunProgressSourceStore,
  GymRunProgressStore,
} from './run-progress-store'

// A live Harbor run emits at most one update per task completion, so a five-minute
// staleness bound is honest for the polled stored-snapshot projection: each
// progress object carries its own `lastUpdatedAt` capture time. The contract is
// declared in the public payload (epic #4751, public-projection-staleness).
const GYM_RUN_PROGRESS_MAX_STALENESS_SECONDS = 300

const gymRunProgressStaleness = () =>
  storedSnapshotStaleness(GYM_RUN_PROGRESS_MAX_STALENESS_SECONDS, [
    'harbor.terminal_bench.task_completed',
    'harbor.terminal_bench.run_finished',
  ])

export type GymRunProgressRouteInput = Readonly<{
  // Source of live progress objects. There is NO seeded default: the endpoints
  // return ONLY real ingested runs and `[]` when none exist yet. The live ingest
  // (Hydralisk Harbor `result.json` -> POST -> D1) feeds `store`; until a run is
  // pushed the surface is honestly empty rather than faked. Every supplied object
  // MUST already be public-safe (built via buildGymRunProgress).
  //
  // `store` is the production D1 source; `listRunProgress` is a synchronous
  // in-memory override used by tests. When both are absent the surface is `[]`.
  store?: GymRunProgressSourceStore
  listRunProgress?: () => ReadonlyArray<GymRunProgress>
  nowIso?: () => string
}>

export type GymRunProgressOperatorRouteInput = GymRunProgressRouteInput &
  Readonly<{
    requireAdminApiToken: (request: Request) => Promise<boolean>
    // The writable store for the POST ingest verb. Optional so the GET-only
    // operator surface and tests can supply a read-only source.
    store?: GymRunProgressStore
    // Fire-and-forget realtime publish (#6261). After a successful upsert, the
    // ingest publishes the public-safe projected snapshot to the live
    // `public-gym-run-progress` sync scope so the `/gym` follow-along updates the
    // instant a snapshot lands instead of waiting for the next ~12s poll. Wired
    // by the Worker to `publishGymRunProgressSnapshot`; left undefined by the
    // GET-only operator surface and tests that do not exercise the realtime path.
    // It is FAIL-SOFT: a publish failure must never break or slow the ingest, so
    // the route swallows any rejection and still returns the 201.
    publishProgress?: (progress: GymRunProgress) => Promise<void>
  }>

const listProgress = (
  input: GymRunProgressRouteInput,
): Effect.Effect<ReadonlyArray<GymRunProgress>> => {
  if (input.store !== undefined) {
    return input.store.listRunProgress()
  }
  return Effect.succeed((input.listRunProgress ?? (() => []))())
}

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const ingestBadRequest = (reason: string) =>
  noStoreJsonResponse(
    { error: 'gym_run_progress_ingest_rejected', reason },
    { status: 400 },
  )

const ingestUnavailable = () =>
  noStoreJsonResponse(
    {
      error: 'gym_run_progress_ingest_unavailable',
      reason: 'No writable run-progress store is configured.',
    },
    { status: 503 },
  )

// Build a public-safe progress object from a pushed snapshot. The build
// re-validates counts and RE-ASSERTS the public-safety boundary, so any payload
// carrying prompts/responses/logs/trajectories/keys/private endpoints is rejected
// here and never reaches D1. Returns the built object or a typed reject reason.
const buildIngestedProgress = (
  raw: unknown,
): Effect.Effect<
  | Readonly<{ progress: GymRunProgress; tag: 'ok' }>
  | Readonly<{ reason: string; tag: 'reject' }>
> =>
  Effect.try({
    catch: error =>
      error instanceof GymRunProgressError
        ? error.message
        : error instanceof Error
          ? error.message
          : String(error),
    try: () => buildGymRunProgress(raw),
  }).pipe(
    Effect.map(progress => ({ progress, tag: 'ok' as const })),
    Effect.catch(reason => Effect.succeed({ reason, tag: 'reject' as const })),
  )

const handleOperatorListRunProgress = (
  input: GymRunProgressOperatorRouteInput,
): Effect.Effect<Response> =>
  listProgress(input).pipe(
    Effect.map(runs =>
      noStoreJsonResponse({
        schemaVersion: 'openagents.gym.run_progress.v1',
        scope: 'operator',
        runs,
      }),
    ),
  )

const handleOperatorIngestRunProgress = (
  request: Request,
  input: GymRunProgressOperatorRouteInput,
): Effect.Effect<Response> => {
  const store = input.store
  if (store === undefined) {
    return Effect.succeed(ingestUnavailable())
  }
  return Effect.tryPromise({
    catch: error => (error instanceof Error ? error.message : String(error)),
    try: () => request.json(),
  }).pipe(
    Effect.flatMap(raw => buildIngestedProgress(raw)),
    Effect.flatMap(result =>
      result.tag === 'reject'
        ? Effect.succeed(ingestBadRequest(result.reason))
        : store.upsertRunProgress(result.progress).pipe(
            // Fire-and-forget realtime publish AFTER the upsert lands. Fail-soft:
            // a publish error must never break or slow the ingest, so any
            // rejection is swallowed and the 201 is still returned.
            Effect.tap(() =>
              input.publishProgress === undefined
                ? Effect.void
                : Effect.promise(() =>
                    input.publishProgress!(result.progress).catch(() => {}),
                  ),
            ),
            Effect.map(() =>
              noStoreJsonResponse(
                {
                  schemaVersion: 'openagents.gym.run_progress.v1',
                  kind: 'gym_run_progress_ingested',
                  run: result.progress,
                },
                { status: 201 },
              ),
            ),
          ),
    ),
    Effect.catch(reason => Effect.succeed(ingestBadRequest(reason))),
  )
}

// Scoped operator surface (#6261, #6271):
//   GET  returns full progress objects incl. local_only runs.
//   POST ingests a pushed snapshot through the public-safety boundary.
export const handleOperatorGymRunProgressApi = (
  request: Request,
  input: GymRunProgressOperatorRouteInput,
): Effect.Effect<Response> => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }
  return Effect.gen(function* () {
    const authorized = yield* Effect.promise(() =>
      input.requireAdminApiToken(request),
    )
    if (!authorized) {
      return unauthorized()
    }
    if (request.method === 'POST') {
      return yield* handleOperatorIngestRunProgress(request, input)
    }
    return yield* handleOperatorListRunProgress(input)
  })
}

// Public-safe projection: web_authorized runs render live; local_only runs
// degrade honestly to an awaiting-authorization marker.
export const handlePublicGymRunProgressApi = (
  request: Request,
  input: GymRunProgressRouteInput = {},
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  return listProgress(input).pipe(
    Effect.map(runs =>
      noStoreJsonResponse({
        schemaVersion: 'openagents.gym.run_progress.v1',
        scope: 'public',
        generatedAt: (input.nowIso ?? currentIsoTimestamp)(),
        staleness: gymRunProgressStaleness(),
        runs: runs.map(projectPublicGymRunProgress),
      }),
    ),
  )
}
