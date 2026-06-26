// Routes for the recurring, published Khala external HEAD-TO-HEAD (#6308).
//
//   - `GET /api/public/khala/head-to-head` (no auth): the public-safe
//     dereferenceable quality bar. Returns the latest published snapshot — Khala
//     vs the tools/models a developer would otherwise reach for (default coding
//     model -> free/open -> paid frontier), each matchup scored on solve-rate AND
//     cost-per-accepted-outcome. If no snapshot has been published yet, it returns
//     the EMPTY shape (all matchups `awaiting_owner` with their owner-gate refs)
//     so the surface is honestly "machinery shipped, no decision-grade run yet"
//     rather than 404 or fabricated.
//   - `POST /api/operator/khala/head-to-head` (admin bearer): the publish
//     boundary. An operator (or the recurring scheduler) POSTs the decision-grade
//     `GymLeaderboardReportInput[]` from an owner-armed real sweep. The Worker
//     RE-BUILDS the head-to-head through `buildKhalaHeadToHead` (which runs the
//     shipped flat projection — decision-grade + public-safety-checked rows only)
//     and upserts the public-safe artifact by `headToHeadRef`. Anything not
//     decision-grade or not public-safe is dropped by the builder, never stored.
//
// No dispatch, spend, settlement, payout, or public-claim authority beyond the
// honest decision-grade comparison the shipped harness already produces. The bar
// is a recurring projection of owner-armed real benchmark reports.
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../../http/responses'
import { storedSnapshotStaleness } from '../../public-projection-staleness'
import { currentIsoTimestamp } from '../../runtime-primitives'
import {
  buildKhalaHeadToHead,
  KHALA_HEAD_TO_HEAD_RECURRING_CONFIG,
  type KhalaHeadToHead,
  type KhalaHeadToHeadRecurringConfig,
} from './head-to-head'
import type {
  KhalaHeadToHeadSourceStore,
  KhalaHeadToHeadStore,
} from './head-to-head-store'
import type { GymLeaderboardReportInput } from '../gym/leaderboard'
import { GymLeaderboardUnsafe } from '../gym/leaderboard'

// The head-to-head is a stored snapshot upserted by the recurring owner-armed
// publish (per Khala release / weekly / on demand). It is not live-at-read, so it
// declares a stored_snapshot staleness contract (epic #4751): the served bar is
// the latest published snapshot, refreshed on the publish transitions below. A
// wide bound is honest because the recurring cadence is deliberately slow (the
// owner-armed real sweep that re-publishes runs at most per Khala release); the
// empty bar served before the first publish is never stale by construction.
const HEAD_TO_HEAD_MAX_STALENESS_SECONDS = 1_209_600 // 14 days

const headToHeadStaleness = () =>
  storedSnapshotStaleness(HEAD_TO_HEAD_MAX_STALENESS_SECONDS, [
    'khala.head_to_head.owner_armed_sweep_published',
    'khala.head_to_head.recurring_cadence_published',
  ])

export type KhalaHeadToHeadRouteInput = Readonly<{
  // The production D1 source; absent in tests that pass an inline head-to-head.
  store?: KhalaHeadToHeadSourceStore
  // The recurring config (which bar, cadence, publish path, comparators).
  config?: KhalaHeadToHeadRecurringConfig
  nowIso?: () => string
}>

export type KhalaHeadToHeadOperatorRouteInput = KhalaHeadToHeadRouteInput &
  Readonly<{
    requireAdminApiToken: (request: Request) => Promise<boolean>
    store?: KhalaHeadToHeadStore
  }>

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const publishBadRequest = (reason: string) =>
  noStoreJsonResponse(
    { error: 'khala_head_to_head_publish_rejected', reason },
    { status: 400 },
  )

const publishUnavailable = () =>
  noStoreJsonResponse(
    {
      error: 'khala_head_to_head_publish_unavailable',
      reason: 'No writable head-to-head store is configured.',
    },
    { status: 503 },
  )

// The empty bar: every matchup in its `awaiting_owner` shape with owner gates
// visible. Returned by the public route when no snapshot has been published yet.
// It is the honest "machinery shipped, no decision-grade run yet" surface.
const emptyHeadToHead = (
  config: KhalaHeadToHeadRecurringConfig,
): KhalaHeadToHead => buildKhalaHeadToHead([], config)

const resolvePublished = (
  input: KhalaHeadToHeadRouteInput,
  config: KhalaHeadToHeadRecurringConfig,
): Effect.Effect<KhalaHeadToHead> => {
  if (input.store === undefined) {
    return Effect.succeed(emptyHeadToHead(config))
  }
  return input.store
    .getHeadToHead(config.headToHeadRef)
    .pipe(Effect.map(headToHead => headToHead ?? emptyHeadToHead(config)))
}

// Public-safe projection: returns the latest published head-to-head, or the
// honest empty shape when nothing decision-grade has been published yet.
export const handlePublicKhalaHeadToHeadApi = (
  request: Request,
  input: KhalaHeadToHeadRouteInput = {},
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  const config = input.config ?? KHALA_HEAD_TO_HEAD_RECURRING_CONFIG
  return resolvePublished(input, config).pipe(
    Effect.map(headToHead =>
      noStoreJsonResponse({
        schemaVersion: headToHead.schemaVersion,
        scope: 'public',
        generatedAt: (input.nowIso ?? currentIsoTimestamp)(),
        cadence: config.cadence,
        staleness: headToHeadStaleness(),
        headToHead,
      }),
    ),
  )
}

// Build the public-safe head-to-head from a posted decision-grade report-input
// set. The builder runs the shipped flat projection (which drops
// non-decision-grade and non-public-safe reports) and re-pairs Khala against each
// comparator. A malformed body or unsafe ref (GymLeaderboardUnsafe) becomes a
// typed reject -> 400, and nothing is stored. No generic thrown errors: the
// body-shape branch returns a reject directly and only the pure builder runs
// inside Effect.try.
const buildPublished = (
  raw: unknown,
  config: KhalaHeadToHeadRecurringConfig,
): Effect.Effect<
  | Readonly<{ headToHead: KhalaHeadToHead; tag: 'ok' }>
  | Readonly<{ reason: string; tag: 'reject' }>
> => {
  const body = raw as
    | { reports?: ReadonlyArray<GymLeaderboardReportInput> }
    | undefined
  const reports = body?.reports
  if (!Array.isArray(reports)) {
    return Effect.succeed({
      reason:
        'A head-to-head publish needs a `reports` array of decision-grade report inputs.',
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
    try: () => buildKhalaHeadToHead(reports, config),
  }).pipe(
    Effect.map(headToHead => ({ headToHead, tag: 'ok' as const })),
    Effect.catch(reason => Effect.succeed({ reason, tag: 'reject' as const })),
  )
}

// Operator publish boundary: rebuilds + upserts the head-to-head. Admin-bearer
// gated.
export const handleOperatorKhalaHeadToHeadApi = (
  request: Request,
  input: KhalaHeadToHeadOperatorRouteInput,
): Effect.Effect<Response> => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }
  const config = input.config ?? KHALA_HEAD_TO_HEAD_RECURRING_CONFIG
  return Effect.gen(function* () {
    const authorized = yield* Effect.promise(() =>
      input.requireAdminApiToken(request),
    )
    if (!authorized) {
      return unauthorized()
    }
    if (request.method === 'GET') {
      const headToHead = yield* resolvePublished(input, config)
      return noStoreJsonResponse({
        schemaVersion: headToHead.schemaVersion,
        scope: 'operator',
        cadence: config.cadence,
        headToHead,
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
    const result = yield* buildPublished(raw, config)
    if (result.tag === 'reject') {
      return publishBadRequest(result.reason)
    }
    const publishedAt = (input.nowIso ?? currentIsoTimestamp)()
    yield* store.upsertHeadToHead(result.headToHead, publishedAt)
    return noStoreJsonResponse(
      {
        schemaVersion: result.headToHead.schemaVersion,
        kind: 'khala_head_to_head_published',
        publishedAt,
        headToHead: result.headToHead,
      },
      { status: 201 },
    )
  })
}
