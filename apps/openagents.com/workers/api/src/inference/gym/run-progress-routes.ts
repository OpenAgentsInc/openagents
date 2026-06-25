// Routes for live Gym / Harbor run progress (#6261, epic #6253).
//
//   - `GET /api/operator/gym/run-progress` (admin bearer): the scoped operator
//     status surface. Returns every live progress object, INCLUDING `local_only`
//     runs that are not yet authorized for web publication. Still public-safe —
//     "scoped" gates VISIBILITY of unpublished runs, not extra private fields.
//   - `GET /api/public/gym/run-progress` (no auth): the public-safe projection.
//     `web_authorized` runs render their live counts; `local_only` runs degrade
//     honestly to an "awaiting authorization" marker with NO live numbers.
//
// Polling a scoped status endpoint is the chosen path (the issue allows it): a
// Harbor job emits at most one update per task completion, so a Foldkit poll is
// the simpler robust path than wiring a sync room for a low-frequency feed.
//
// No dispatch, spend, settlement, payout, or public-claim authority. A progress
// object is in-progress evidence only and is always `decisionGrade: false`.
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../../http/responses'
import { currentIsoTimestamp } from '../../runtime-primitives'
import { storedSnapshotStaleness } from '../../public-projection-staleness'
import {
  type GymRunProgress,
  projectPublicGymRunProgress,
} from './run-progress'
import { LIVE_GYM_RUN_PROGRESS_FIXTURE } from './run-progress-fixture'

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
  // Source of live progress objects. Defaults to the seeded public-safe fixture
  // so the surface is honestly populated until the live Hydralisk Harbor poll is
  // wired. Every object MUST already be public-safe (built via buildGymRunProgress).
  listRunProgress?: () => ReadonlyArray<GymRunProgress>
  nowIso?: () => string
}>

export type GymRunProgressOperatorRouteInput = GymRunProgressRouteInput &
  Readonly<{
    requireAdminApiToken: (request: Request) => Promise<boolean>
  }>

const listProgress = (
  input: GymRunProgressRouteInput,
): ReadonlyArray<GymRunProgress> =>
  (input.listRunProgress ?? (() => LIVE_GYM_RUN_PROGRESS_FIXTURE))()

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

// Scoped operator status: full progress objects incl. local_only runs.
export const handleOperatorGymRunProgressApi = (
  request: Request,
  input: GymRunProgressOperatorRouteInput,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  return Effect.gen(function* () {
    const authorized = yield* Effect.promise(() =>
      input.requireAdminApiToken(request),
    )
    if (!authorized) {
      return unauthorized()
    }
    const runs = listProgress(input)
    return noStoreJsonResponse({
      schemaVersion: 'openagents.gym.run_progress.v1',
      scope: 'operator',
      runs,
    })
  }).pipe(Effect.catch(() => Effect.succeed(unauthorized())))
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
  const runs = listProgress(input).map(projectPublicGymRunProgress)
  return Effect.succeed(
    noStoreJsonResponse({
      schemaVersion: 'openagents.gym.run_progress.v1',
      scope: 'public',
      generatedAt: (input.nowIso ?? currentIsoTimestamp)(),
      staleness: gymRunProgressStaleness(),
      runs,
    }),
  )
}
