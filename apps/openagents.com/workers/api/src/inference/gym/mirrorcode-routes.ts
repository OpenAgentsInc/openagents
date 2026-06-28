// Routes for the MirrorCode-as-a-service gym demo surface (#6378, epic #6376).
//
//   - `GET /api/public/gym/mirrorcode/runs` (no auth): the public-safe
//     leaderboard / list. Returns the stored Khala runs plus the LABELED
//     illustrative paper-reference comparators. Honestly `[]` runs until one is
//     recorded.
//   - `GET /api/public/gym/mirrorcode/runs/{id}` (no auth): one run's
//     status/result, or a typed 404.
//   - `GET /api/public/gym/mirrorcode/token-burn` (no auth): automated
//     public-safe token-burn reporter over the stored run rows. It aggregates
//     exact token-row refs where present and keeps unproven totals separate.
//   - `POST /api/gym/mirrorcode/runs` (admin bearer / owner-gated): launch /
//     record a Khala MirrorCode run. The body is a public-safe MirrorCode run
//     record (the shared result contract). The Worker RE-BUILDS it through
//     buildMirrorCodeRun (which re-asserts the no-task-contents / no-canary
//     public-safety boundary) and upserts by `runId`. Anything smuggling task
//     source, test data, prompts, or canary strings is REJECTED with a typed
//     400 and never stored. Owner-scoped: no public spend, no settlement, no
//     payout — recording a run row is in-progress / measurement evidence only.
//   - Legacy `/api/gym/mirrorcode/*` GET paths remain as compatibility aliases.
//
// The public GET is composed live from D1 at read, so it declares a
// `live_at_read` staleness contract (epic #4751). No dispatch, spend,
// settlement, payout, or public-claim authority beyond the honest run record.
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../../http/responses'
import { currentIsoTimestamp } from '../../runtime-primitives'
import { liveAtReadStaleness } from '../../public-projection-staleness'
import {
  buildMirrorCodeLaunchRun,
  buildMirrorCodeRun,
  buildMirrorCodeTokenBurnReport,
  KHALA_MODEL_ID,
  MIRRORCODE_BENCHMARK_LABEL,
  MIRRORCODE_PAPER_REFERENCE_COMPARATORS,
  MirrorCodeTokenBurnReportSchemaVersion,
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

const MAX_ROUTE_RUN_ID_LENGTH = 128
const routeRunIdPattern = /^[a-zA-Z0-9._:-]+$/

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

const isLaunchRequest = (raw: unknown): boolean =>
  typeof raw === 'object' &&
  raw !== null &&
  'kind' in raw &&
  raw.kind === 'launch'

// Build a public-safe run from a posted body. Launch intents create an honest
// queued row; result records update that row later. Both paths re-assert the
// no-task-contents / no-canary public-safety boundary before storage.
const buildPostedRun = (
  raw: unknown,
  nowIso: string,
): Effect.Effect<
  | Readonly<{
      kind: 'mirrorcode_run_launched' | 'mirrorcode_run_recorded'
      run: MirrorCodeRun
      tag: 'ok'
    }>
  | Readonly<{ reason: string; tag: 'reject' }>
> =>
  Effect.try({
    catch: error =>
      error instanceof MirrorCodeRunError
        ? error.message
        : error instanceof Error
          ? error.message
          : String(error),
    try: () =>
      isLaunchRequest(raw)
        ? {
            kind: 'mirrorcode_run_launched' as const,
            run: buildMirrorCodeLaunchRun(raw, nowIso),
          }
        : {
            kind: 'mirrorcode_run_recorded' as const,
            run: buildMirrorCodeRun(raw),
          },
  }).pipe(
    Effect.map(result => ({ ...result, tag: 'ok' as const })),
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
  runs,
  comparators: MIRRORCODE_PAPER_REFERENCE_COMPARATORS,
})

const tokenBurnEnvelope = (
  input: MirrorCodeRunsRouteInput,
  runs: ReadonlyArray<MirrorCodeRun>,
) => ({
  schemaVersion: MirrorCodeTokenBurnReportSchemaVersion,
  scope: 'public' as const,
  generatedAt: (input.nowIso ?? currentIsoTimestamp)(),
  staleness: mirrorCodeStaleness(),
  report: buildMirrorCodeTokenBurnReport(runs),
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
    Effect.flatMap(raw =>
      Effect.gen(function* () {
        const postedAt = (input.nowIso ?? currentIsoTimestamp)()
        const result = yield* buildPostedRun(raw, postedAt)
        if (result.tag === 'reject') {
          return ingestBadRequest(result.reason)
        }
        yield* store.upsertRun(result.run, postedAt)
        return noStoreJsonResponse(
          {
            schemaVersion: MirrorCodeRunsSchemaVersion,
            kind: result.kind,
            run: result.run,
          },
          {
            status: result.kind === 'mirrorcode_run_launched' ? 202 : 201,
          },
        )
      }),
    ),
    Effect.catch(reason => Effect.succeed(ingestBadRequest(reason))),
  )
}

const isTokenBurnPath = (pathname: string): boolean =>
  pathname === '/api/gym/mirrorcode/token-burn' ||
  pathname === '/api/public/gym/mirrorcode/token-burn'

const isOwnerWritableRunsPath = (pathname: string): boolean =>
  pathname === '/api/gym/mirrorcode/runs'

// `/api/public/gym/mirrorcode/runs`: public GET list/leaderboard.
// `/api/gym/mirrorcode/runs`: compatibility GET + owner-gated POST.
export const handleMirrorCodeRunsApi = (
  request: Request,
  input: MirrorCodeRunsOperatorRouteInput,
): Effect.Effect<Response> => {
  const pathname = new URL(request.url).pathname
  if (isTokenBurnPath(pathname)) {
    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }
    return listRuns(input).pipe(
      Effect.map(runs => noStoreJsonResponse(tokenBurnEnvelope(input, runs))),
    )
  }
  if (request.method === 'POST' && isOwnerWritableRunsPath(pathname)) {
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
  return Effect.succeed(
    isOwnerWritableRunsPath(pathname)
      ? methodNotAllowed(['GET', 'POST'])
      : methodNotAllowed(['GET']),
  )
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

// Match `/api/public/gym/mirrorcode/runs/{id}` or legacy
// `/api/gym/mirrorcode/runs/{id}` and extract the id. Returns undefined for the
// base path (handled by the exact route) or any non-matching path, so the worker
// route cascade falls through cleanly.
export const matchMirrorCodeRunByIdRequest = (
  request: Request,
): string | undefined => {
  const pathname = new URL(request.url).pathname
  const match =
    /^\/api\/(?:public\/)?gym\/mirrorcode\/runs\/([^/]+)$/.exec(pathname)
  if (match === null) {
    return undefined
  }
  try {
    const decoded = decodeURIComponent(match[1]!)
    return decoded.length > 0 &&
      decoded.length <= MAX_ROUTE_RUN_ID_LENGTH &&
      routeRunIdPattern.test(decoded)
      ? decoded
      : undefined
  } catch {
    return undefined
  }
}
