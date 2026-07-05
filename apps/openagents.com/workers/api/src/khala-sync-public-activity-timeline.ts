// Khala Sync public activity-timeline projection (KS-6.7b, #8421).
//
// Makes the public activity timeline (`GET /api/public/activity-timeline`)
// a `scope.public.activity-timeline` REBUILD-ON-CRON projection instead of a
// pure live-at-read merge on every request:
//
//   REFRESH (fail-soft, debounced, cron-triggered): `refreshActivityTimelineSnapshotBestEffort`
//   re-runs `buildPublicActivityTimelineRawSnapshot` — the EXACT SAME merge
//   function the live route already calls — against the real D1-backed
//   source stores, then upserts the whole result as ONE
//   `scope.public.activity-timeline` snapshot. Called from the Worker's
//   existing per-minute `scheduled()` cron tick (see index.ts), NOT from a
//   per-ingest write hook: the public activity timeline live-merges SEVEN
//   source stores (Pylon registrations/presence, training run/window/lease/
//   verification authority, settlement receipts, inference receipts, forum
//   topics/posts, Artanis admin ticks, Pylon capacity funnel snapshots) with
//   no single shared write-site hook a KS-6.1/KS-6.3-style event-driven
//   producer could tap — see packages/khala-sync/src/
//   activity-timeline-snapshot.ts's module doc for the full reasoning. A
//   refresh failure NEVER fails or slows the cron tick.
//
//   READER: `readActivityTimelineSnapshotCached` reads the one stored
//   snapshot through a small in-isolate cache. FAIL OPEN on any miss
//   (binding absent, Postgres unreachable, no snapshot projected yet, OR the
//   stored snapshot is older than `ACTIVITY_TIMELINE_SNAPSHOT_STALE_FAIL_OPEN_SECONDS` —
//   e.g. the cron has been broken for a while): the route falls back to the
//   existing live-at-read merge, so projection availability never regresses
//   the existing route's correctness, only its cost.
//
//   HONEST STALENESS LABEL: unlike KS-6.7's tokens-served-mix (event-driven,
//   debounced to a 30s ceiling but refreshed on every real ingest), this
//   projection's true staleness bound is the cron period itself (this
//   Worker's `scheduled()` trigger fires every 60s) — so the declared
//   `stored_snapshot` contract uses `ACTIVITY_TIMELINE_SNAPSHOT_MAX_STALENESS_SECONDS`
//   (90s, a 60s cron period + margin), not just the in-isolate read-cache
//   TTL, to avoid the mislabeling KS-6.7 itself fixed on the older tokens-
//   served routes.

import type {
  ActivityTimelineSnapshotEntity,
} from '@openagentsinc/khala-sync'
import {
  projectActivityTimelineSnapshotBestEffort,
  readActivityTimelineSnapshot,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import {
  buildPublicActivityTimelineRawSnapshot,
  type PublicActivityTimelineRawSnapshot,
  type PublicActivityTimelineRawSourceInput,
} from './public-activity-timeline'
import { currentEpochMillis, currentIsoTimestamp } from './runtime-primitives'

/**
 * Explicit field-by-field adapter from the stored khala-sync entity shape
 * (`@openagentsinc/khala-sync`'s `ActivityTimelineSnapshotEntity`) to the
 * local `PublicActivityTimelineRawSnapshot` shape `paginatePublicActivityTimelineEnvelope`
 * expects. Deliberately explicit (not a bare structural cast) — same
 * "decode-then-encode is defense in depth" discipline every other KS-6.x
 * projection module boundary uses, and it keeps the two independently-
 * declared schemas (khala-sync's duplicated bounded shape vs. the feature
 * package's own schema) from silently drifting into an incompatible shape
 * without a visible compile error here.
 */
export const activityTimelineRawSnapshotFromEntity = (
  entity: ActivityTimelineSnapshotEntity,
): PublicActivityTimelineRawSnapshot => ({
  events: entity.events.map(event => ({
    ...(event.actorRef === undefined ? {} : { actorRef: event.actorRef }),
    ...(event.amountSats === undefined ? {} : { amountSats: event.amountSats }),
    blockerRefs: event.blockerRefs,
    caveatRefs: event.caveatRefs,
    cursor: event.cursor,
    eventRef: event.eventRef,
    kind: event.kind,
    ...(event.realBitcoinMoved === undefined
      ? {}
      : { realBitcoinMoved: event.realBitcoinMoved }),
    refs: event.refs,
    ...(event.runRef === undefined ? {} : { runRef: event.runRef }),
    sourceKind: event.sourceKind,
    sourceRefs: event.sourceRefs,
    ...(event.state === undefined ? {} : { state: event.state }),
    ...(event.targetRef === undefined ? {} : { targetRef: event.targetRef }),
    text: event.text,
    ts: event.ts,
    ...(event.windowRef === undefined ? {} : { windowRef: event.windowRef }),
  })),
  generatedAt: entity.generatedAt,
  sourceLag: entity.sourceLag.map(lag => ({
    blockerRefs: lag.blockerRefs,
    caveatRefs: lag.caveatRefs,
    lagSeconds: lag.lagSeconds,
    latestSourceEventAt: lag.latestSourceEventAt,
    maxStalenessSeconds: lag.maxStalenessSeconds,
    observedAt: lag.observedAt,
    sourceKind: lag.sourceKind,
    sourceRefs: lag.sourceRefs,
    status: lag.status,
  })),
})

// ---------------------------------------------------------------------------
// Shared dependency slice
// ---------------------------------------------------------------------------

/** The `rebuildsOn` label for the declared staleness contract. */
export const ACTIVITY_TIMELINE_SNAPSHOT_REBUILDS_ON = [
  'scope.public.activity-timeline',
]

export type ActivityTimelineProjectionLog = (
  event:
    | 'khala_sync_activity_timeline_refresh_failed'
    | 'khala_sync_activity_timeline_projection_failed',
  fields: Readonly<Record<string, string | number>>,
) => void

export type ActivityTimelineProjectionDeps = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /** Injectable transaction-mode-safe client factory (tests inject a fake). */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Diagnostic sink (public-safe fields only). */
  log?: ActivityTimelineProjectionLog | undefined
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
  deps: ActivityTimelineProjectionDeps,
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
// Refresh (fail-soft, debounced in-isolate; cron-triggered)
// ---------------------------------------------------------------------------

/**
 * Minimum wall-clock gap between refresh sweeps, regardless of how often the
 * caller invokes this (defensive only — the real cadence driver is the
 * Worker's per-minute `scheduled()` cron trigger, this just guards against
 * accidental concurrent/duplicate cron dispatch on the same tick).
 */
export const ACTIVITY_TIMELINE_SNAPSHOT_REFRESH_MIN_INTERVAL_MS = 45_000

let lastRefreshAttemptAtMs: number | undefined

/** Drop the in-isolate refresh debounce marker (tests only). */
export const resetActivityTimelineRefreshDebounceForTests = (): void => {
  lastRefreshAttemptAtMs = undefined
}

export type ActivityTimelineRefreshOutcome =
  | { readonly outcome: 'refreshed'; readonly eventCount: number }
  | { readonly outcome: 'skipped_debounced' }
  | { readonly outcome: 'skipped_no_binding' }
  | { readonly outcome: 'failed' }

export type ActivityTimelineRefreshDeps = ActivityTimelineProjectionDeps &
  Readonly<{
    /** The real D1-backed source stores (same shape the live route builds). */
    sources: PublicActivityTimelineRawSourceInput
    nowMs?: () => number
  }>

/**
 * Best-effort recompute-and-store the WHOLE activity-timeline snapshot.
 * NEVER throws and never fails or slows the caller (the scheduled cron
 * tick). Debounced in-isolate: a call within
 * `ACTIVITY_TIMELINE_SNAPSHOT_REFRESH_MIN_INTERVAL_MS` of the previous
 * attempt is a pure in-memory no-op (no D1/Postgres round trip at all).
 */
export const refreshActivityTimelineSnapshotBestEffort = async (
  deps: ActivityTimelineRefreshDeps,
): Promise<ActivityTimelineRefreshOutcome> => {
  const nowMs = (deps.nowMs ?? currentEpochMillis)()
  if (
    lastRefreshAttemptAtMs !== undefined &&
    nowMs - lastRefreshAttemptAtMs <
      ACTIVITY_TIMELINE_SNAPSHOT_REFRESH_MIN_INTERVAL_MS
  ) {
    return { outcome: 'skipped_debounced' }
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return { outcome: 'skipped_no_binding' }
  }
  // Set BEFORE awaiting so concurrent in-flight ticks on the same isolate
  // don't pile on additional refresh sweeps.
  lastRefreshAttemptAtMs = nowMs

  try {
    const raw = await buildPublicActivityTimelineRawSnapshot(deps.sources)
    const snapshot: ActivityTimelineSnapshotEntity = {
      events: raw.events,
      generatedAt: raw.generatedAt,
      sourceLag: raw.sourceLag,
    }
    return await withSqlClient(deps, connectionString, async client => {
      const outcome = await projectActivityTimelineSnapshotBestEffort(
        client.sql,
        snapshot,
      )
      if (!outcome.ok) {
        deps.log?.('khala_sync_activity_timeline_projection_failed', {
          messageSafe: outcome.diagnostic.messageSafe,
          reason: outcome.diagnostic.reason,
        })
        return { outcome: 'failed' }
      }
      return { eventCount: raw.events.length, outcome: 'refreshed' }
    })
  } catch {
    deps.log?.('khala_sync_activity_timeline_refresh_failed', {
      messageSafe: 'activity-timeline snapshot refresh failed',
    })
    return { outcome: 'failed' }
  }
}

// ---------------------------------------------------------------------------
// Reader (small in-isolate cache, same TTL discipline as other KS-6.x reads)
// ---------------------------------------------------------------------------

export const ACTIVITY_TIMELINE_SNAPSHOT_CACHE_TTL_MS = 2_000

/**
 * The declared `stored_snapshot` staleness bound: the Worker's `scheduled()`
 * cron trigger fires every 60s, so 90s covers one missed/slow tick with
 * margin. Unlike the in-isolate read-cache TTL, this is the REAL data-
 * freshness contract for a cron-driven projection (see module doc).
 */
export const ACTIVITY_TIMELINE_SNAPSHOT_MAX_STALENESS_SECONDS = 90

/**
 * Hard fail-open ceiling: if the stored snapshot's own `generatedAt` is
 * older than this (the cron has been broken for a while — binding removed,
 * Postgres down, a bad deploy), treat it as a miss and fall back to the live
 * merge rather than silently serving badly-stale data as current.
 */
export const ACTIVITY_TIMELINE_SNAPSHOT_HARD_STALE_SECONDS = 300

type ActivityTimelineCacheState = Readonly<{
  snapshot: ActivityTimelineSnapshotEntity
  expiresAtMs: number
}>

let cache: ActivityTimelineCacheState | undefined

/** Drop the cached snapshot (repair path + tests). */
export const invalidateActivityTimelineSnapshotCacheForTests = (): void => {
  cache = undefined
}

export type ActivityTimelineReadDeps = ActivityTimelineProjectionDeps &
  Readonly<{ nowMs?: () => number; nowIso?: () => string }>

/**
 * Read the activity-timeline snapshot through the in-isolate cache. Returns
 * `undefined` on ANY miss the caller must fail open from: binding absent,
 * Postgres unreachable, no snapshot projected yet, or the stored snapshot is
 * older than `ACTIVITY_TIMELINE_SNAPSHOT_HARD_STALE_SECONDS`. Only fresh-
 * enough successful reads are cached — misses stay live so recovery is
 * immediate once the cron catches back up.
 */
export const readActivityTimelineSnapshotCached = async (
  deps: ActivityTimelineReadDeps,
): Promise<ActivityTimelineSnapshotEntity | undefined> => {
  const nowMs = (deps.nowMs ?? currentEpochMillis)()
  if (cache !== undefined && cache.expiresAtMs > nowMs) {
    return cache.snapshot
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return undefined
  }
  try {
    const snapshot = await withSqlClient(deps, connectionString, client =>
      readActivityTimelineSnapshot(client.sql),
    )
    if (snapshot === null) {
      // Not projected yet — fail open to the live merge.
      return undefined
    }
    const nowIso = deps.nowIso ?? currentIsoTimestamp
    const generatedAtMs = Date.parse(snapshot.generatedAt)
    const ageSeconds = Number.isFinite(generatedAtMs)
      ? Math.max(0, (Date.parse(nowIso()) - generatedAtMs) / 1000)
      : Number.POSITIVE_INFINITY
    if (ageSeconds > ACTIVITY_TIMELINE_SNAPSHOT_HARD_STALE_SECONDS) {
      // Cron has been broken for a while — never silently serve data this
      // old as current; fail open to the live merge instead.
      return undefined
    }
    cache = {
      expiresAtMs: nowMs + ACTIVITY_TIMELINE_SNAPSHOT_CACHE_TTL_MS,
      snapshot,
    }
    return snapshot
  } catch {
    return undefined
  }
}
