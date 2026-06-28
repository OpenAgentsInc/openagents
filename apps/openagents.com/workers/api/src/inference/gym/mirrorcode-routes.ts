// Routes for the MirrorCode-as-a-service gym demo surface (#6378, epic #6376).
//
//   - `POST /api/gym/mirrorcode/runs` (admin bearer / owner-gated): launch /
//     record a Khala MirrorCode run. The body is a public-safe MirrorCode run
//     record (the shared result contract). The Worker RE-BUILDS it through
//     buildMirrorCodeRun (which re-asserts the no-task-contents / no-canary
//     public-safety boundary) and upserts by `runId`. Anything smuggling task
//     source, test data, prompts, or canary strings is REJECTED with a typed
//     400 and never stored. Owner-scoped: no public spend, no settlement, no
//     payout — recording a run row is in-progress / measurement evidence only.
//   - `GET /api/gym/mirrorcode/runs` (no auth): the public-safe leaderboard /
//     list. Returns the stored Khala runs plus the LABELED illustrative
//     paper-reference comparators. Honestly `[]` runs until one is recorded.
//   - `GET /api/gym/mirrorcode/runs/{id}` (no auth): one run's status/result, or
//     a typed 404.
//
// The public GET is composed live from D1 at read, so it declares a
// `live_at_read` staleness contract (epic #4751). No dispatch, spend,
// settlement, payout, or public-claim authority beyond the honest run record.
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../../http/responses'
import { currentIsoTimestamp } from '../../runtime-primitives'
import { liveAtReadStaleness } from '../../public-projection-staleness'
import {
  buildMirrorCodeRun,
  KHALA_MODEL_ID,
  MIRRORCODE_BENCHMARK_LABEL,
  MIRRORCODE_GENERALIZATION_SET,
  MIRRORCODE_PAPER_REFERENCE_COMPARATORS,
  MirrorCodeRunError,
  MirrorCodeRunsSchemaVersion,
  type MirrorCodeRun,
} from './mirrorcode-contract'
import type {
  MirrorCodeRunSourceStore,
  MirrorCodeRunStore,
} from './mirrorcode-store'

const mirrorCodeStaleness = () =>
  liveAtReadStaleness([
    'gym.mirrorcode.run_recorded',
    'gym.mirrorcode.run_updated',
  ])

export type MirrorCodeRunsRouteInput = Readonly<{
  // Production D1 source. Tests may pass `listRuns`/`getRun` overrides instead.
  store?: MirrorCodeRunSourceStore
  listRuns?: () => ReadonlyArray<MirrorCodeRun>
  getRun?: (runId: string) => MirrorCodeRun | undefined
  nowIso?: () => string
}>

export type MirrorCodeRunsOperatorRouteInput = MirrorCodeRunsRouteInput &
  Readonly<{
    requireAdminApiToken: (request: Request) => Promise<boolean>
    store?: MirrorCodeRunStore
  }>

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const ingestBadRequest = (reason: string) =>
  noStoreJsonResponse(
    { error: 'mirrorcode_run_rejected', reason },
    { status: 400 },
  )

const ingestUnavailable = () =>
  noStoreJsonResponse(
    {
      error: 'mirrorcode_run_unavailable',
      reason: 'No writable MirrorCode run store is configured.',
    },
    { status: 503 },
  )

const listRuns = (
  input: MirrorCodeRunsRouteInput,
): Effect.Effect<ReadonlyArray<MirrorCodeRun>> => {
  if (input.store !== undefined) {
    return input.store.listRuns()
  }
  return Effect.succeed((input.listRuns ?? (() => []))())
}

const getRun = (
  input: MirrorCodeRunsRouteInput,
  runId: string,
): Effect.Effect<MirrorCodeRun | undefined> => {
  if (input.store !== undefined) {
    return input.store.getRun(runId)
  }
  return Effect.succeed((input.getRun ?? (() => undefined))(runId))
}

// Build a public-safe run from a posted body. The build re-validates the shape
// and RE-ASSERTS the public-safety boundary, so any payload carrying task
// contents or canary strings is rejected here and never reaches D1.
const buildIngestedRun = (
  raw: unknown,
): Effect.Effect<
  | Readonly<{ run: MirrorCodeRun; tag: 'ok' }>
  | Readonly<{ reason: string; tag: 'reject' }>
> =>
  Effect.try({
    catch: error =>
      error instanceof MirrorCodeRunError
        ? error.message
        : error instanceof Error
          ? error.message
          : String(error),
    try: () => buildMirrorCodeRun(raw),
  }).pipe(
    Effect.map(run => ({ run, tag: 'ok' as const })),
    Effect.catch(reason => Effect.succeed({ reason, tag: 'reject' as const })),
  )

const publicEnvelope = (
  input: MirrorCodeRunsRouteInput,
  runs: ReadonlyArray<MirrorCodeRun>,
) => ({
  schemaVersion: MirrorCodeRunsSchemaVersion,
  scope: 'public' as const,
  generatedAt: (input.nowIso ?? currentIsoTimestamp)(),
  staleness: mirrorCodeStaleness(),
  model: KHALA_MODEL_ID,
  benchmark: MIRRORCODE_BENCHMARK_LABEL,
  generalizationSet: MIRRORCODE_GENERALIZATION_SET,
  runs,
  comparators: MIRRORCODE_PAPER_REFERENCE_COMPARATORS,
})

// Owner-gated launch / ingest: rebuilds + upserts a run. Admin-bearer gated.
const handleOperatorIngestRun = (
  request: Request,
  input: MirrorCodeRunsOperatorRouteInput,
): Effect.Effect<Response> => {
  const store = input.store
  if (store === undefined) {
    return Effect.succeed(ingestUnavailable())
  }
  return Effect.tryPromise({
    catch: error => (error instanceof Error ? error.message : String(error)),
    try: () => request.json(),
  }).pipe(
    Effect.flatMap(raw => buildIngestedRun(raw)),
    Effect.flatMap(result =>
      result.tag === 'reject'
        ? Effect.succeed(ingestBadRequest(result.reason))
        : store
            .upsertRun(result.run, (input.nowIso ?? currentIsoTimestamp)())
            .pipe(
              Effect.map(() =>
                noStoreJsonResponse(
                  {
                    schemaVersion: MirrorCodeRunsSchemaVersion,
                    kind: 'mirrorcode_run_recorded',
                    run: result.run,
                  },
                  { status: 201 },
                ),
              ),
            ),
    ),
    Effect.catch(reason => Effect.succeed(ingestBadRequest(reason))),
  )
}

// `/api/gym/mirrorcode/runs`: public GET list/leaderboard + owner-gated POST.
export const handleMirrorCodeRunsApi = (
  request: Request,
  input: MirrorCodeRunsOperatorRouteInput,
): Effect.Effect<Response> => {
  if (request.method === 'POST') {
    return Effect.gen(function* () {
      const authorized = yield* Effect.promise(() =>
        input.requireAdminApiToken(request),
      )
      if (!authorized) {
        return unauthorized()
      }
      return yield* handleOperatorIngestRun(request, input)
    })
  }
  if (request.method === 'GET') {
    return listRuns(input).pipe(
      Effect.map(runs => noStoreJsonResponse(publicEnvelope(input, runs))),
    )
  }
  return Effect.succeed(methodNotAllowed(['GET', 'POST']))
}

// `/api/gym/mirrorcode/runs/{id}`: one run's public-safe status/result.
export const handleMirrorCodeRunByIdApi = (
  request: Request,
  runId: string,
  input: MirrorCodeRunsRouteInput,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  return getRun(input, runId).pipe(
    Effect.map(run =>
      run === undefined
        ? noStoreJsonResponse(
            { error: 'mirrorcode_run_not_found', runId },
            { status: 404 },
          )
        : noStoreJsonResponse({
            schemaVersion: MirrorCodeRunsSchemaVersion,
            scope: 'public',
            generatedAt: (input.nowIso ?? currentIsoTimestamp)(),
            staleness: mirrorCodeStaleness(),
            run,
          }),
    ),
  )
}

// Match `/api/gym/mirrorcode/runs/{id}` and extract the id. Returns undefined
// for the base path (handled by the exact route) or any non-matching path, so
// the worker route cascade falls through cleanly.
export const matchMirrorCodeRunByIdRequest = (
  request: Request,
): string | undefined => {
  const pathname = new URL(request.url).pathname
  const match = /^\/api\/gym\/mirrorcode\/runs\/([^/]+)$/.exec(pathname)
  if (match === null) {
    return undefined
  }
  try {
    const decoded = decodeURIComponent(match[1]!)
    return decoded.length > 0 ? decoded : undefined
  } catch {
    return undefined
  }
}
