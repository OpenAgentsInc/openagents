import {
  type WorkerBindings,
  makeD1SyncOutboxRepository,
  publicSettledFeedScope,
  PUBLIC_SETTLED_FEED_ID,
} from '@openagentsinc/sync-worker'
import { Schema as S } from 'effect'
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import type { KhalaSyncHyperdriveBinding } from './khala-sync-push-routes'
import {
  DEFAULT_SETTLED_FEED_ROUTE_LIMIT,
  readSettledFeedProjectionCached,
  SETTLED_FEED_PROJECTION_MAX_STALENESS_SECONDS,
  type SettledFeedProjectionReadDeps,
} from './khala-sync-public-settled-feed'
import {
  liveAtReadStaleness,
  PublicProjectionStalenessContract,
  rebuiltOnTransitionStaleness,
} from './public-projection-staleness'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  SETTLED_FEED_SUMMARY_COLLECTION,
  SETTLED_FEED_SUMMARY_ENTITY_ID,
  SETTLED_FEED_SYNC_COLLECTION,
  type PublicSettledFeedEvent,
  type PublicSettledFeedSummary,
} from './tassadar-settled-feed-sync'

/**
 * KS-6.4 (#8414): the live settled feed's new public, unauthenticated read
 * route. Backs the homepage/stats settled-feed surface with a real
 * anonymous-safe consumer of the `scope.public.settled-feed` khala-sync
 * projection, so the projection has genuine production evidence of
 * correctness BEFORE the legacy `notifySyncScopes` producer is retired.
 *
 * SERVING ORDER (mirrors KS-6.3's tokens-served route):
 *   1. The `scope.public.settled-feed` khala-sync projection: a small
 *      Postgres read behind an in-isolate cache
 *      (`SETTLED_FEED_PROJECTION_MAX_STALENESS_SECONDS`).
 *   2. FAIL-OPEN FALLBACK: when the binding is absent, Postgres is
 *      unreachable, or the projection is empty (no events projected yet),
 *      the route falls back to the existing legacy D1 sync-outbox
 *      snapshot for `public-settled-feed:tassadar` — the SAME store
 *      `publishSettledFeedEvents` already writes, so availability never
 *      regresses relative to today.
 *
 * The response is `no-store` (live settlement data must never be served
 * from a stale cached copy).
 */

export const PublicSettledFeedEventSchema = S.Struct({
  amountSats: S.Int,
  challengeRef: S.String,
  contributorRef: S.String,
  eventRef: S.String,
  party: S.Literals(['worker', 'validator']),
  runRef: S.String,
  settledAt: S.String,
  totalSettledCount: S.Int,
  totalSettledSats: S.Int,
  windowRef: S.NullOr(S.String),
})

export const PublicSettledFeedSummarySchema = S.Struct({
  latestEventRef: S.NullOr(S.String),
  latestSettledAt: S.NullOr(S.String),
  totalSettledCount: S.Int,
  totalSettledSats: S.Int,
  updatedAt: S.String,
})

export const PublicSettledFeedResponse = S.Struct({
  schemaVersion: S.Literal('openagents.public_settled_feed.v1'),
  events: S.Array(PublicSettledFeedEventSchema),
  summary: S.NullOr(PublicSettledFeedSummarySchema),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
})
export type PublicSettledFeedResponse = typeof PublicSettledFeedResponse.Type

type PublicSettledFeedRouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding
  /** Injectable projection-read seams (tests). */
  projectionReadDeps?: Omit<SettledFeedProjectionReadDeps, 'binding'>
  limit?: number
  nowIso?: () => string
}>

const parseLimit = (request: Request, fallback: number): number => {
  const raw = new URL(request.url).searchParams.get('limit')
  if (raw === null) return fallback
  const value = Number(raw)
  return Number.isSafeInteger(value) && value > 0
    ? Math.min(value, 200)
    : fallback
}

const legacyFallback = (
  input: PublicSettledFeedRouteInput,
  limit: number,
  respond: (
    events: ReadonlyArray<PublicSettledFeedEvent>,
    summary: PublicSettledFeedSummary | null,
    staleness: PublicProjectionStalenessContract,
  ) => Response,
): Effect.Effect<Response> =>
  Effect.promise(async () => {
    if (input.OPENAGENTS_DB === undefined) {
      return { events: [] as Array<PublicSettledFeedEvent>, summary: null }
    }
    try {
      const scope = publicSettledFeedScope(PUBLIC_SETTLED_FEED_ID)
      const store = makeD1SyncOutboxRepository(
        openAgentsDatabase({
          OPENAGENTS_DB: input.OPENAGENTS_DB,
        } as Pick<WorkerBindings, 'OPENAGENTS_DB'>),
      )
      const snapshot = await store.readSnapshot(scope)
      const eventValues = Object.values(
        snapshot.collections[SETTLED_FEED_SYNC_COLLECTION] ?? {},
      ) as Array<PublicSettledFeedEvent>
      const events = [...eventValues]
        .sort((a, b) => b.totalSettledCount - a.totalSettledCount)
        .slice(0, limit)
      const summaryValue = (
        snapshot.collections[SETTLED_FEED_SUMMARY_COLLECTION] ?? {}
      )[SETTLED_FEED_SUMMARY_ENTITY_ID] as PublicSettledFeedSummary | undefined
      return { events, summary: summaryValue ?? null }
    } catch {
      return { events: [] as Array<PublicSettledFeedEvent>, summary: null }
    }
  }).pipe(
    Effect.map(({ events, summary }) =>
      respond(events, summary, liveAtReadStaleness(['sync_changes'])),
    ),
  )

export const handlePublicSettledFeedApi = (
  request: Request,
  input: PublicSettledFeedRouteInput,
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp
  const limit = Math.min(
    parseLimit(request, input.limit ?? DEFAULT_SETTLED_FEED_ROUTE_LIMIT),
    200,
  )

  const respond = (
    events: ReadonlyArray<PublicSettledFeedEvent>,
    summary: PublicSettledFeedSummary | null,
    staleness: PublicProjectionStalenessContract,
  ) => {
    const payload: PublicSettledFeedResponse = {
      schemaVersion: 'openagents.public_settled_feed.v1',
      events,
      summary,
      generatedAt: nowIso(),
      staleness,
    }
    return noStoreJsonResponse(payload)
  }

  // Projection first (never throws; undefined ⇒ fail open to the legacy
  // D1 snapshot).
  return Effect.promise(() =>
    readSettledFeedProjectionCached({
      binding: input.KHALA_SYNC_DB,
      limit,
      ...input.projectionReadDeps,
    }),
  ).pipe(
    Effect.flatMap(projection =>
      projection === undefined || projection.events.length === 0
        ? legacyFallback(input, limit, respond)
        : Effect.succeed(
            respond(
              projection.events,
              projection.summary,
              rebuiltOnTransitionStaleness(
                SETTLED_FEED_PROJECTION_MAX_STALENESS_SECONDS,
                ['scope.public.settled-feed'],
              ),
            ),
          ),
    ),
  )
}
