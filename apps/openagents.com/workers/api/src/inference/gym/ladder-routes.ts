// Routes for the recurring, published Gym benchmark LADDER (#6309).
//
//   - `GET /api/public/gym/leaderboard` (no auth): the public-safe dereferenceable
//     leaderboard. Returns the latest published ladder snapshot (Big Pickle →
//     free → paid frontier → MirrorCode public bucket) by `ladderRef`. If no
//     snapshot has been
//     published yet, it returns the EMPTY ladder shape (all rungs `awaiting_owner`
//     with their owner-gate refs) so the surface is honestly "machinery shipped,
//     no decision-grade run yet" rather than 404 or fabricated.
//   - `POST /api/operator/gym/leaderboard` (admin bearer): the publish boundary.
//     An operator (or the recurring scheduler) POSTs the decision-grade
//     `GymLeaderboardReportInput[]` from an owner-armed real sweep. The Worker
//     RE-BUILDS the ladder through `buildGymLadderLeaderboard` (which runs the
//     shipped flat projection — decision-grade + public-safety-checked rows only)
//     and upserts the public-safe ladder by `ladderRef`. Anything not
//     decision-grade or not public-safe is dropped by the builder, never stored.
//
// No dispatch, spend, settlement, payout, or public-claim authority beyond the
// honest decision-grade ranking the shipped harness already produces. The ladder
// is a recurring projection of owner-armed real benchmark reports.
import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../../http/responses'
import { currentIsoTimestamp } from '../../runtime-primitives'
import {
  projectionDataAgeSeconds,
  projectionStalenessExceeded,
  storedSnapshotStaleness,
} from '../../public-projection-staleness'
import {
  buildGymLadderLeaderboard,
  GYM_LADDER_RECURRING_CONFIG,
  type GymLadderLeaderboard,
  type GymLadderRecurringConfig,
} from './ladder'
import type { GymLeaderboardReportInput } from './leaderboard'
import { GymLeaderboardUnsafe } from './leaderboard'
import {
  buildMirrorCodeRun,
  MirrorCodeRun as MirrorCodeRunSchema,
  MirrorCodeRunError,
  type MirrorCodeRun,
} from './mirrorcode-contract'
import type {
  GymLadderSnapshot,
  GymLadderSourceStore,
  GymLadderStore,
} from './ladder-store'

// The ladder is a stored snapshot upserted by the recurring owner-armed publish
// (per model release / weekly / on demand). It is not live-at-read, so it
// declares a stored_snapshot staleness contract (epic #4751): the served ladder
// is the latest published snapshot, refreshed on the publish transitions below.
// A wide bound is honest because the recurring cadence is deliberately slow (the
// owner-armed real sweep that re-publishes runs at most per model release); the
// empty ladder served before the first publish is never stale by construction.
const GYM_LADDER_MAX_STALENESS_SECONDS = 1_209_600 // 14 days

const gymLadderStaleness = () =>
  storedSnapshotStaleness(GYM_LADDER_MAX_STALENESS_SECONDS, [
    'gym.ladder.owner_armed_sweep_published',
    'gym.ladder.recurring_cadence_published',
  ])

export type GymLadderRouteInput = Readonly<{
  // The production D1 source; absent in tests that pass an inline ladder.
  store?: GymLadderSourceStore
  // The recurring ladder config (which ladder, cadence, publish path, rungs).
  config?: GymLadderRecurringConfig
  nowIso?: () => string
}>

export type GymLadderOperatorRouteInput = GymLadderRouteInput &
  Readonly<{
    requireAdminApiToken: (request: Request) => Promise<boolean>
    store?: GymLadderStore
  }>

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const publishBadRequest = (reason: string) =>
  noStoreJsonResponse(
    { error: 'gym_ladder_publish_rejected', reason },
    { status: 400 },
  )

const publishUnavailable = () =>
  noStoreJsonResponse(
    {
      error: 'gym_ladder_publish_unavailable',
      reason: 'No writable ladder store is configured.',
    },
    { status: 503 },
  )

// The empty ladder: all rungs in their `awaiting_owner` shape with owner
// gates visible. Returned by the public route when no snapshot has been published
// yet. It is the honest "machinery shipped, no decision-grade run yet" surface.
const emptyLadder = (config: GymLadderRecurringConfig): GymLadderSnapshot => ({
  ladder: buildGymLadderLeaderboard([], config),
  publishedAt: null,
})

const resolvePublishedLadder = (
  input: GymLadderRouteInput,
  config: GymLadderRecurringConfig,
): Effect.Effect<GymLadderSnapshot> => {
  if (input.store === undefined) {
    return Effect.succeed(emptyLadder(config))
  }
  return input.store
    .getLadder(config.ladderRef)
    .pipe(Effect.map(snapshot => snapshot ?? emptyLadder(config)))
}

// Public-safe projection: returns the latest published ladder, or the honest
// empty ladder shape when nothing decision-grade has been published yet.
export const handlePublicGymLeaderboardApi = (
  request: Request,
  input: GymLadderRouteInput = {},
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  const config = input.config ?? GYM_LADDER_RECURRING_CONFIG
  const nowIso = (input.nowIso ?? currentIsoTimestamp)()
  const staleness = gymLadderStaleness()
  return resolvePublishedLadder(input, config).pipe(
    Effect.map(snapshot => {
      const dataAgeSeconds = projectionDataAgeSeconds(
        snapshot.publishedAt,
        nowIso,
      )
      return noStoreJsonResponse({
        schemaVersion: snapshot.ladder.schemaVersion,
        scope: 'public',
        generatedAt: nowIso,
        cadence: config.cadence,
        publishedAt: snapshot.publishedAt,
        dataAgeSeconds,
        staleExceeded: projectionStalenessExceeded(
          staleness,
          dataAgeSeconds,
        ),
        staleness,
        ladder: snapshot.ladder,
      })
    }),
  )
}

// Build the public-safe ladder from a posted decision-grade report-input set. The
// builder runs the shipped flat projection (which drops non-decision-grade and
// non-public-safe reports) and re-groups into rungs. A malformed body or unsafe
// ref (GymLeaderboardUnsafe) becomes a typed reject -> 400, and nothing is
// stored. No generic thrown errors: the body-shape branch returns a reject
// directly and only the pure builder runs inside Effect.try.
const buildPublishedLadder = (
  raw: unknown,
  config: GymLadderRecurringConfig,
): Effect.Effect<
  | Readonly<{ ladder: GymLadderLeaderboard; tag: 'ok' }>
  | Readonly<{ reason: string; tag: 'reject' }>
> => {
  const body = raw as
    | {
        mirrorCodeRuns?: ReadonlyArray<unknown>
        reports?: ReadonlyArray<GymLeaderboardReportInput>
      }
    | undefined
  const reports = body?.reports
  if (!Array.isArray(reports)) {
    return Effect.succeed({
      reason:
        'A ladder publish needs a `reports` array of decision-grade report inputs.',
      tag: 'reject' as const,
    })
  }
  const rebuildMirrorCodeRun = (rawRun: unknown): MirrorCodeRun => {
    try {
      return buildMirrorCodeRun(rawRun)
    } catch (rawInputError) {
      try {
        const storedRun = S.decodeUnknownSync(MirrorCodeRunSchema)(rawRun)
        return buildMirrorCodeRun({
          runId: storedRun.runId,
          model: storedRun.model,
          taskId: storedRun.taskId,
          bucket: storedRun.bucket,
          language: storedRun.language,
          status: storedRun.status,
          passRate: storedRun.passRate,
          tokens: { total: storedRun.tokensTotal },
          exactTokenUsageEventRefs: storedRun.exactTokenUsageEventRefs,
          startedAt: storedRun.startedAt,
          finishedAt: storedRun.finishedAt,
          summary: storedRun.summary,
          grade: storedRun.grade,
        })
      } catch {
        throw rawInputError
      }
    }
  }
  const mirrorCodeRunsResult = (body?.mirrorCodeRuns ?? []).reduce<
    | Readonly<{ runs: ReadonlyArray<MirrorCodeRun>; tag: 'ok' }>
    | Readonly<{ reason: string; tag: 'reject' }>
  >(
    (result, rawRun) => {
      if (result.tag === 'reject') {
        return result
      }
      try {
        return {
          runs: [...result.runs, rebuildMirrorCodeRun(rawRun)],
          tag: 'ok',
        }
      } catch (error) {
        return {
          reason:
            error instanceof MirrorCodeRunError
              ? error.message
              : error instanceof Error
                ? error.message
                : String(error),
          tag: 'reject',
        }
      }
    },
    { runs: [], tag: 'ok' },
  )
  if (mirrorCodeRunsResult.tag === 'reject') {
    return Effect.succeed({
      reason: mirrorCodeRunsResult.reason,
      tag: 'reject' as const,
    })
  }
  return Effect.try({
    catch: error =>
      error instanceof GymLeaderboardUnsafe
        ? error.reason
        : error instanceof Error
          ? error.message
          : String(error),
    try: () =>
      buildGymLadderLeaderboard(reports, config, mirrorCodeRunsResult.runs),
  }).pipe(
    Effect.map(ladder => ({ ladder, tag: 'ok' as const })),
    Effect.catch(reason => Effect.succeed({ reason, tag: 'reject' as const })),
  )
}

// Operator publish boundary: rebuilds + upserts the ladder. Admin-bearer gated.
export const handleOperatorGymLeaderboardApi = (
  request: Request,
  input: GymLadderOperatorRouteInput,
): Effect.Effect<Response> => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }
  const config = input.config ?? GYM_LADDER_RECURRING_CONFIG
  return Effect.gen(function* () {
    const authorized = yield* Effect.promise(() =>
      input.requireAdminApiToken(request),
    )
    if (!authorized) {
      return unauthorized()
    }
    if (request.method === 'GET') {
      const nowIso = (input.nowIso ?? currentIsoTimestamp)()
      const staleness = gymLadderStaleness()
      const snapshot = yield* resolvePublishedLadder(input, config)
      const dataAgeSeconds = projectionDataAgeSeconds(
        snapshot.publishedAt,
        nowIso,
      )
      return noStoreJsonResponse({
        schemaVersion: snapshot.ladder.schemaVersion,
        scope: 'operator',
        cadence: config.cadence,
        generatedAt: nowIso,
        publishedAt: snapshot.publishedAt,
        dataAgeSeconds,
        staleExceeded: projectionStalenessExceeded(
          staleness,
          dataAgeSeconds,
        ),
        staleness,
        ladder: snapshot.ladder,
      })
    }
    const store = input.store
    if (store === undefined) {
      return publishUnavailable()
    }
    const raw = yield* Effect.tryPromise({
      catch: error => (error instanceof Error ? error.message : String(error)),
      try: () => request.json(),
    }).pipe(Effect.catch(reason => Effect.succeed({ __parseError: reason })))
    if (
      typeof raw === 'object' &&
      raw !== null &&
      '__parseError' in raw &&
      typeof (raw as { __parseError: unknown }).__parseError === 'string'
    ) {
      return publishBadRequest((raw as { __parseError: string }).__parseError)
    }
    const result = yield* buildPublishedLadder(raw, config)
    if (result.tag === 'reject') {
      return publishBadRequest(result.reason)
    }
    const publishedAt = (input.nowIso ?? currentIsoTimestamp)()
    yield* store.upsertLadder(result.ladder, publishedAt)
    return noStoreJsonResponse(
      {
        schemaVersion: result.ladder.schemaVersion,
        kind: 'gym_ladder_published',
        publishedAt,
        ladder: result.ladder,
      },
      { status: 201 },
    )
  })
}
