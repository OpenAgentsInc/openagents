// Khala Sync public settled-feed projection (KS-6.4, #8414).
//
// Makes the live settled feed (openagents #5311) a
// `scope.public.settled-feed` projection (SPEC §2.1/§7 invariant 9),
// following the KS-6.1 fleet / KS-6.3 tokens-served precedent:
//
//   PRODUCER (fail-soft dual-write): every settled batch that
//   `publishSettledFeedEvents` (tassadar-settled-feed-sync.ts) already
//   writes to the legacy `public-settled-feed:tassadar` sync room is ALSO
//   best-effort projected here, into `scope.public.settled-feed`, via the
//   KHALA_SYNC_DB Hyperdrive binding. A projection failure NEVER fails or
//   slows the real settlement dispatch — every failure is a typed
//   diagnostic (same discipline as khala-sync-public-tokens-served.ts).
//
//   READER: `GET /api/public/settled-feed` serves the projection (latest
//   events + summary) behind a small in-isolate cache, so anonymous
//   homepage/stats visitors get a real, working, unauthenticated read path
//   backed by khala-sync — the legacy `/api/sync/public-settled-feed/*`
//   route stays the live-push path until the anonymous-connect gap noted in
//   #8414 is closed (see that module's doc comment).

import {
  decodeSettledFeedEventEntity,
  decodeSettledFeedSummaryEntity,
  encodeSettledFeedEventEntity,
  encodeSettledFeedSummaryEntity,
} from '@openagentsinc/khala-sync'
import {
  projectSettledFeedEventsBestEffort,
  readSettledFeedProjection,
  type SettledFeedProjectionDiagnostic,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { currentEpochMillis } from './runtime-primitives'
import type {
  PublicSettledFeedEvent,
  PublicSettledFeedSummary,
} from './tassadar-settled-feed-sync'

// ---------------------------------------------------------------------------
// Shared dependency slice
// ---------------------------------------------------------------------------

export type SettledFeedProjectionLog = (
  event: 'khala_sync_settled_feed_projection_failed',
  fields: Readonly<Record<string, string | number>>,
) => void

export type SettledFeedProjectionDeps = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /** Injectable transaction-mode-safe client factory (tests inject a fake). */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Diagnostic sink (public-safe fields only). */
  log?: SettledFeedProjectionLog | undefined
}>

const bindingConnectionString = (
  binding: KhalaSyncHyperdriveBinding | undefined,
): string | undefined =>
  binding !== undefined &&
  typeof binding.connectionString === 'string' &&
  binding.connectionString.length > 0
    ? binding.connectionString
    : undefined

const withSqlClient = async <A>(
  deps: SettledFeedProjectionDeps,
  connectionString: string,
  fn: (client: KhalaSyncPushSqlClient) => Promise<A>,
): Promise<A> => {
  const makeSqlClient = deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  let client: KhalaSyncPushSqlClient | undefined
  try {
    client = await makeSqlClient(connectionString)
    return await fn(client)
  } finally {
    if (client !== undefined) {
      try {
        await client.end()
      } catch {
        // best-effort teardown (same discipline as the khala-sync routes).
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Producer (fail-soft, one call per settled batch)
// ---------------------------------------------------------------------------

export type SettledFeedBatchProjectionOutcome =
  | { readonly outcome: 'projected' }
  | { readonly outcome: 'skipped_no_events' }
  | { readonly outcome: 'skipped_no_binding' }
  | {
      readonly outcome: 'failed'
      readonly diagnostic: SettledFeedProjectionDiagnostic
    }

/**
 * Best-effort project one settled batch (the SAME public-safe events +
 * summary the legacy producer already writes). NEVER throws and never
 * fails or slows the caller's real settlement dispatch; failures land in
 * the injected log as a typed public-safe diagnostic.
 */
export const projectSettledFeedBatchBestEffort = async (
  deps: SettledFeedProjectionDeps,
  batch: Readonly<{
    events: ReadonlyArray<PublicSettledFeedEvent>
    summary: PublicSettledFeedSummary
  }>,
): Promise<SettledFeedBatchProjectionOutcome> => {
  if (batch.events.length === 0) {
    return { outcome: 'skipped_no_events' }
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return { outcome: 'skipped_no_binding' }
  }
  try {
    const result = await withSqlClient(deps, connectionString, client =>
      projectSettledFeedEventsBestEffort(client.sql, {
        rawEvents: batch.events,
        rawSummary: batch.summary,
      }),
    )
    if (result.ok) {
      return { outcome: 'projected' }
    }
    deps.log?.('khala_sync_settled_feed_projection_failed', {
      messageSafe: result.diagnostic.messageSafe,
      reason: result.diagnostic.reason,
    })
    return { diagnostic: result.diagnostic, outcome: 'failed' }
  } catch {
    // Client construction/teardown failures: still fail-soft. Never echo
    // driver errors (they can embed the DSN).
    const diagnostic: SettledFeedProjectionDiagnostic = {
      messageSafe: 'settled-feed projection client failed',
      reason: 'projection_failed',
    }
    deps.log?.('khala_sync_settled_feed_projection_failed', {
      messageSafe: diagnostic.messageSafe,
      reason: diagnostic.reason,
    })
    return { diagnostic, outcome: 'failed' }
  }
}

// ---------------------------------------------------------------------------
// Reader (latest events + summary, behind a small in-isolate cache)
// ---------------------------------------------------------------------------

/**
 * The in-isolate cache TTL for the projection read — same 2s bound as the
 * tokens-served projection's `rebuilt_on_transition` staleness contract.
 */
export const SETTLED_FEED_PROJECTION_CACHE_TTL_MS = 2_000
export const SETTLED_FEED_PROJECTION_MAX_STALENESS_SECONDS =
  SETTLED_FEED_PROJECTION_CACHE_TTL_MS / 1000

export const DEFAULT_SETTLED_FEED_ROUTE_LIMIT = 20

export type SettledFeedProjectionReadSnapshot = Readonly<{
  events: ReadonlyArray<PublicSettledFeedEvent>
  summary: PublicSettledFeedSummary | null
}>

/** Injectable projection-read seam for route tests. */
export type ReadSettledFeedProjectionFn = typeof readSettledFeedProjection

type ProjectionCacheState = Readonly<{
  snapshot: SettledFeedProjectionReadSnapshot
  expiresAtMs: number
}>

let projectionCache: ProjectionCacheState | undefined

/** Drop the cached snapshot (tests only). */
export const invalidateSettledFeedProjectionCacheForTests = (): void => {
  projectionCache = undefined
}

export type SettledFeedProjectionReadDeps = SettledFeedProjectionDeps &
  Readonly<{
    readProjection?: ReadSettledFeedProjectionFn | undefined
    nowMs?: (() => number) | undefined
    limit?: number | undefined
  }>

/**
 * Read the settled-feed projection through the in-isolate cache. Returns
 * `undefined` on ANY miss the caller must fail open from: binding absent or
 * Postgres unreachable. Only successful reads are cached — failures stay
 * live so recovery is immediate.
 */
export const readSettledFeedProjectionCached = async (
  deps: SettledFeedProjectionReadDeps,
): Promise<SettledFeedProjectionReadSnapshot | undefined> => {
  const nowMs = (deps.nowMs ?? currentEpochMillis)()
  if (projectionCache !== undefined && projectionCache.expiresAtMs > nowMs) {
    return projectionCache.snapshot
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return undefined
  }
  try {
    const raw = await withSqlClient(deps, connectionString, client =>
      (deps.readProjection ?? readSettledFeedProjection)(client.sql, {
        limit: deps.limit ?? DEFAULT_SETTLED_FEED_ROUTE_LIMIT,
      }),
    )
    // Decode-then-encode (not a bare encode): a `readProjection` seam may
    // hand back a structurally-matching plain object rather than a real
    // class instance (TypeScript's structural typing allows both, and test
    // fakes commonly do), and `S.encodeSync` requires an actual instance.
    // Re-decoding first also re-validates every post-image, same discipline
    // as the packages-layer projection writer.
    const snapshot: SettledFeedProjectionReadSnapshot = {
      events: raw.events.map(event =>
        encodeSettledFeedEventEntity(decodeSettledFeedEventEntity(event)),
      ),
      summary:
        raw.summary === null
          ? null
          : encodeSettledFeedSummaryEntity(
              decodeSettledFeedSummaryEntity(raw.summary),
            ),
    }
    projectionCache = {
      expiresAtMs: nowMs + SETTLED_FEED_PROJECTION_CACHE_TTL_MS,
      snapshot,
    }
    return snapshot
  } catch {
    return undefined
  }
}
