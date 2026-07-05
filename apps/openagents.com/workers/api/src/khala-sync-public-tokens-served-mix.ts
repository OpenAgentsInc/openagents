// Khala Sync public tokens-served aggregates projection (KS-6.7, #8417).
//
// Makes the public tokens-served model-mix, demand-mix, channel-mix, and
// per-day history reads `scope.public.tokens-served-aggregates` projections
// (SPEC §2.1/§7 invariant 8), replacing the mislabeled/repeated live-at-read
// query each request currently issues against the KS-8.2 (#8308) rollup
// twins with a small, honestly-staleness-labeled stored snapshot:
//
//   REFRESH (fail-soft, debounced): `refreshTokensServedAggregatesBestEffort`
//   recomputes all four bounded windows (`today`/`7d`/`30d`/`all`) for all
//   three mixes plus the history series (fixed default timezone/bucket)
//   using the SAME ledger reads the live routes already call
//   (`readPublicTokensServedModelMix`/`...DemandMix`/`...ChannelMix`/
//   `...History`), then upserts each shaped result as a
//   `scope.public.tokens-served-aggregates` snapshot. A refresh failure
//   NEVER fails or slows the caller (the ingest observer that triggers it).
//   Debounced in-isolate (no extra round trip): at most one refresh sweep
//   per `TOKENS_SERVED_AGGREGATES_REFRESH_MIN_INTERVAL_MS`, regardless of
//   ingest volume.
//
//   READER: each `readTokensServed*SnapshotCached` reads one snapshot
//   through a small in-isolate cache. FAIL-OPEN: when the binding is
//   absent, Postgres is unreachable, or the requested window/timezone has
//   not been projected yet, the route falls back to the existing
//   live-at-read ledger call — projection availability never regresses the
//   existing routes.
//
//   RECONCILE (invariant 8): because every snapshot's post-image is
//   EXACTLY the ledger's own shaped output for that window (no separate
//   shaping logic here), the reconcile check is simply "does the stored
//   snapshot match a fresh ledger read for the same window" — see this
//   module's test file and packages/khala-sync-server's own local-Postgres
//   integration tests for the parity evidence.

import type {
  TokensServedChannelMixSnapshotEntity,
  TokensServedDemandMixSnapshotEntity,
  TokensServedHistorySnapshotEntity,
  TokensServedModelMixSnapshotEntity,
  TokensServedWindow,
} from '@openagentsinc/khala-sync'
import {
  projectTokensServedChannelMixSnapshotBestEffort,
  projectTokensServedDemandMixSnapshotBestEffort,
  projectTokensServedHistorySnapshotBestEffort,
  projectTokensServedModelMixSnapshotBestEffort,
  readTokensServedChannelMixSnapshot,
  readTokensServedDemandMixSnapshot,
  readTokensServedHistorySnapshot,
  readTokensServedModelMixSnapshot,
} from '@openagentsinc/khala-sync-server'
import { Effect } from 'effect'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { currentEpochMillis, currentIsoTimestamp } from './runtime-primitives'
import {
  PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'

// ---------------------------------------------------------------------------
// Shared dependency slice
// ---------------------------------------------------------------------------

export const TOKENS_SERVED_AGGREGATES_WINDOWS: ReadonlyArray<TokensServedWindow> =
  ['today', '7d', '30d', 'all']

/** History is currently only refreshed/served for this default timezone. */
export const TOKENS_SERVED_AGGREGATES_HISTORY_TIMEZONE =
  PUBLIC_KHALA_TOKENS_SERVED_TIMEZONE

export type TokensServedAggregatesLog = (
  event:
    | 'khala_sync_tokens_served_aggregates_refresh_failed'
    | 'khala_sync_tokens_served_aggregates_projection_failed',
  fields: Readonly<Record<string, string | number>>,
) => void

export type TokensServedAggregatesDeps = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: KhalaSyncHyperdriveBinding | undefined
  /** Injectable transaction-mode-safe client factory (tests inject a fake). */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  /** Diagnostic sink (public-safe fields only). */
  log?: TokensServedAggregatesLog | undefined
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
  deps: TokensServedAggregatesDeps,
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
// Refresh (fail-soft, debounced in-isolate; ONE call recomputes everything)
// ---------------------------------------------------------------------------

/** Minimum wall-clock gap between refresh sweeps, regardless of ingest volume. */
export const TOKENS_SERVED_AGGREGATES_REFRESH_MIN_INTERVAL_MS = 30_000

let lastAggregatesRefreshAttemptAtMs: number | undefined

/** Drop the in-isolate refresh debounce marker (tests only). */
export const resetTokensServedAggregatesRefreshDebounceForTests = (): void => {
  lastAggregatesRefreshAttemptAtMs = undefined
}

export type TokensServedAggregatesRefreshOutcome =
  | { readonly outcome: 'refreshed' }
  | { readonly outcome: 'skipped_debounced' }
  | { readonly outcome: 'skipped_no_binding' }
  | { readonly outcome: 'failed'; readonly failedSteps: number }

export type TokensServedAggregatesRefreshDeps = TokensServedAggregatesDeps &
  Readonly<{
    ledger: Pick<
      TokenUsageLedgerShape,
      | 'readPublicTokensServedModelMix'
      | 'readPublicTokensServedDemandMix'
      | 'readPublicTokensServedChannelMix'
      | 'readPublicTokensServedHistory'
    >
    nowIso?: () => string
    nowMs?: () => number
  }>

/**
 * Best-effort recompute-and-store ALL bounded windows for all four snapshot
 * kinds. NEVER throws and never fails or slows the caller (the ingest
 * observer that triggers this). Debounced in-isolate: a call within
 * `TOKENS_SERVED_AGGREGATES_REFRESH_MIN_INTERVAL_MS` of the previous attempt
 * is a pure in-memory no-op (no Postgres round trip at all), so ingest
 * volume never drives Postgres read/write cost beyond this bound.
 */
export const refreshTokensServedAggregatesBestEffort = async (
  deps: TokensServedAggregatesRefreshDeps,
): Promise<TokensServedAggregatesRefreshOutcome> => {
  const nowMs = (deps.nowMs ?? currentEpochMillis)()
  if (
    lastAggregatesRefreshAttemptAtMs !== undefined &&
    nowMs - lastAggregatesRefreshAttemptAtMs <
      TOKENS_SERVED_AGGREGATES_REFRESH_MIN_INTERVAL_MS
  ) {
    return { outcome: 'skipped_debounced' }
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return { outcome: 'skipped_no_binding' }
  }
  // Set BEFORE awaiting so concurrent in-flight requests on the same
  // isolate don't pile on additional refresh sweeps.
  lastAggregatesRefreshAttemptAtMs = nowMs
  const nowIso = deps.nowIso ?? currentIsoTimestamp

  try {
    return await withSqlClient(deps, connectionString, async client => {
      let failedSteps = 0

      const runEffect = async <A, E>(
        effect: Effect.Effect<A, E>,
      ): Promise<A | undefined> => {
        try {
          return await Effect.runPromise(effect)
        } catch {
          failedSteps += 1
          return undefined
        }
      }

      for (const window of TOKENS_SERVED_AGGREGATES_WINDOWS) {
        const [modelMix, demandMix, channelMix] = await Promise.all([
          runEffect(deps.ledger.readPublicTokensServedModelMix({ window })),
          runEffect(deps.ledger.readPublicTokensServedDemandMix({ window })),
          runEffect(deps.ledger.readPublicTokensServedChannelMix({ window })),
        ])

        if (modelMix !== undefined) {
          const snapshot: TokensServedModelMixSnapshotEntity = {
            generatedAt: nowIso(),
            groups: modelMix.groups,
            totalTokens: modelMix.totalTokens,
            window: modelMix.window,
          }
          const outcome = await projectTokensServedModelMixSnapshotBestEffort(
            client.sql,
            snapshot,
          )
          if (!outcome.ok) {
            failedSteps += 1
            deps.log?.(
              'khala_sync_tokens_served_aggregates_projection_failed',
              { messageSafe: outcome.diagnostic.messageSafe, window },
            )
          }
        }

        if (demandMix !== undefined) {
          const snapshot: TokensServedDemandMixSnapshotEntity = {
            generatedAt: nowIso(),
            groups: demandMix.groups,
            totalTokens: demandMix.totalTokens,
            window: demandMix.window,
          }
          const outcome = await projectTokensServedDemandMixSnapshotBestEffort(
            client.sql,
            snapshot,
          )
          if (!outcome.ok) {
            failedSteps += 1
            deps.log?.(
              'khala_sync_tokens_served_aggregates_projection_failed',
              { messageSafe: outcome.diagnostic.messageSafe, window },
            )
          }
        }

        if (channelMix !== undefined) {
          const snapshot: TokensServedChannelMixSnapshotEntity = {
            generatedAt: nowIso(),
            groups: channelMix.groups,
            totalTokens: channelMix.totalTokens,
            window: channelMix.window,
          }
          const outcome =
            await projectTokensServedChannelMixSnapshotBestEffort(
              client.sql,
              snapshot,
            )
          if (!outcome.ok) {
            failedSteps += 1
            deps.log?.(
              'khala_sync_tokens_served_aggregates_projection_failed',
              { messageSafe: outcome.diagnostic.messageSafe, window },
            )
          }
        }

        const history = await runEffect(
          deps.ledger.readPublicTokensServedHistory({
            bucket: 'day',
            timezone: TOKENS_SERVED_AGGREGATES_HISTORY_TIMEZONE,
            window,
          }),
        )
        if (history !== undefined) {
          const snapshot: TokensServedHistorySnapshotEntity = {
            bucket: 'day',
            generatedAt: nowIso(),
            series: history.series,
            timezone: TOKENS_SERVED_AGGREGATES_HISTORY_TIMEZONE,
            window: history.window,
          }
          const outcome = await projectTokensServedHistorySnapshotBestEffort(
            client.sql,
            snapshot,
          )
          if (!outcome.ok) {
            failedSteps += 1
            deps.log?.(
              'khala_sync_tokens_served_aggregates_projection_failed',
              { messageSafe: outcome.diagnostic.messageSafe, window },
            )
          }
        }
      }

      return failedSteps === 0
        ? { outcome: 'refreshed' }
        : { failedSteps, outcome: 'failed' }
    })
  } catch {
    deps.log?.('khala_sync_tokens_served_aggregates_refresh_failed', {
      messageSafe: 'tokens-served aggregates refresh client failed',
    })
    return { failedSteps: -1, outcome: 'failed' }
  }
}

// ---------------------------------------------------------------------------
// Readers (small in-isolate cache per snapshot, same TTL as KS-6.3)
// ---------------------------------------------------------------------------

export const TOKENS_SERVED_AGGREGATES_CACHE_TTL_MS = 2_000
export const TOKENS_SERVED_AGGREGATES_MAX_STALENESS_SECONDS =
  TOKENS_SERVED_AGGREGATES_CACHE_TTL_MS / 1000

type AggregateCacheState<A> = Readonly<{ snapshot: A; expiresAtMs: number }>

const aggregateCache = new Map<string, AggregateCacheState<unknown>>()

/** Drop all cached aggregate snapshots (repair path + tests). */
export const invalidateTokensServedAggregatesCache = (): void => {
  aggregateCache.clear()
}

export type TokensServedAggregatesReadDeps = TokensServedAggregatesDeps &
  Readonly<{ nowMs?: () => number }>

const readCached = async <A>(
  deps: TokensServedAggregatesReadDeps,
  cacheKey: string,
  read: (client: KhalaSyncPushSqlClient) => Promise<A | null>,
): Promise<A | undefined> => {
  const nowMs = (deps.nowMs ?? currentEpochMillis)()
  const cached = aggregateCache.get(cacheKey)
  if (cached !== undefined && cached.expiresAtMs > nowMs) {
    return cached.snapshot as A
  }
  const connectionString = bindingConnectionString(deps.binding)
  if (connectionString === undefined) {
    return undefined
  }
  try {
    const row = await withSqlClient(deps, connectionString, client =>
      read(client),
    )
    if (row === null) {
      // Not projected yet — fail open to the live ledger read.
      return undefined
    }
    aggregateCache.set(cacheKey, {
      expiresAtMs: nowMs + TOKENS_SERVED_AGGREGATES_CACHE_TTL_MS,
      snapshot: row,
    })
    return row
  } catch {
    return undefined
  }
}

export const readTokensServedModelMixSnapshotCached = (
  deps: TokensServedAggregatesReadDeps,
  window: TokensServedWindow,
): Promise<TokensServedModelMixSnapshotEntity | undefined> =>
  readCached(deps, `model-mix:${window}`, client =>
    readTokensServedModelMixSnapshot(client.sql, window),
  )

export const readTokensServedDemandMixSnapshotCached = (
  deps: TokensServedAggregatesReadDeps,
  window: TokensServedWindow,
): Promise<TokensServedDemandMixSnapshotEntity | undefined> =>
  readCached(deps, `demand-mix:${window}`, client =>
    readTokensServedDemandMixSnapshot(client.sql, window),
  )

export const readTokensServedChannelMixSnapshotCached = (
  deps: TokensServedAggregatesReadDeps,
  window: TokensServedWindow,
): Promise<TokensServedChannelMixSnapshotEntity | undefined> =>
  readCached(deps, `channel-mix:${window}`, client =>
    readTokensServedChannelMixSnapshot(client.sql, window),
  )

export const readTokensServedHistorySnapshotCached = (
  deps: TokensServedAggregatesReadDeps,
  window: TokensServedWindow,
  timezone: string,
): Promise<TokensServedHistorySnapshotEntity | undefined> =>
  readCached(deps, `history:${window}:${timezone}`, client =>
    readTokensServedHistorySnapshot(client.sql, window, timezone),
  )
