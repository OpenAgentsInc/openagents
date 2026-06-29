import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import type { ExaConfig } from './config'
import {
  type ExaError,
  ExaProviderFetchError,
  ExaProviderHttpError,
  ExaProviderTimeout,
} from './exa'
import { parseJsonWithSchema } from './json-boundary'
import { openAgentsDatabase } from './runtime'
import {
  compactRandomId,
  currentIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

type AdjutantEnrichmentOperationsEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

export type AdjutantEnrichmentOperationsRuntime = Readonly<{
  makeBudgetEventId: () => string
  makeCacheEntryId: () => string
  makeMetricEventId: () => string
  nowIso: () => string
}>

export const systemAdjutantEnrichmentOperationsRuntime: AdjutantEnrichmentOperationsRuntime =
  {
    makeBudgetEventId: () => compactRandomId('exa_enrichment_budget'),
    makeCacheEntryId: () => compactRandomId('exa_enrichment_cache'),
    makeMetricEventId: () => compactRandomId('exa_enrichment_metric'),
    nowIso: currentIsoTimestamp,
  }

export type ExaEnrichmentOperationsPolicy = Readonly<{
  assignmentRequestBudget: number
  cacheTtlHours: number
  dailyRequestBudget: number
  rateLimitBackoffMs: number
  retryLimit: number
}>

export const exaEnrichmentOperationsPolicyFromConfig = (
  config: ExaConfig,
): ExaEnrichmentOperationsPolicy => ({
  assignmentRequestBudget: config.assignmentRequestBudget,
  cacheTtlHours: config.cacheTtlHours,
  dailyRequestBudget: config.dailyRequestBudget,
  rateLimitBackoffMs: config.rateLimitBackoffMs,
  retryLimit: config.retryLimit,
})

export const CachedExaSourceResult = S.Struct({
  domain: S.String,
  highlightText: S.NullOr(S.String),
  publishedDate: S.NullOr(S.String),
  title: S.String,
  url: S.String,
})
export type CachedExaSourceResult = typeof CachedExaSourceResult.Type

export type ReserveExaBudgetInput = Readonly<{
  assignmentId: string
  policy: ExaEnrichmentOperationsPolicy
  reason: string
  requestUnits: number
  runId?: string | null | undefined
}>

export type ExaCacheLookupInput = Readonly<{
  cacheKey: string
  freshnessMaxAgeHours: number
  nowIso?: string | undefined
}>

export type ExaCacheStoreInput = Readonly<{
  cacheKey: string
  costDollars?: number | null | undefined
  freshnessMaxAgeHours: number
  policy: ExaEnrichmentOperationsPolicy
  results: ReadonlyArray<CachedExaSourceResult>
  searchType: string
  sourceCategory: string
}>

export type RecordExaMetricInput = Readonly<{
  assignmentId: string
  cacheStatus?: 'bypass' | 'hit' | 'miss' | 'stale' | null | undefined
  costDollars?: number | null | undefined
  errorCode?: string | null | undefined
  eventName: string
  latencyMs?: number | null | undefined
  queryId?: string | null | undefined
  resultCount?: number | null | undefined
  runId?: string | null | undefined
  searchType?: string | null | undefined
  sourceCardCount?: number | null | undefined
  sourceCategory?: string | null | undefined
  status: string
}>

type ExaBudgetUsageRow = Readonly<{
  assignment_units: number | null
  day_units: number | null
}>

type ExaCacheEntryRow = Readonly<{
  cost_dollars: number | null
  created_at: string
  expires_at: string
  id: string
  result_count: number
  results_json: string
}>

export class ExaEnrichmentOperationsStorageError extends S.TaggedErrorClass<ExaEnrichmentOperationsStorageError>()(
  'ExaEnrichmentOperationsStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class ExaEnrichmentBudgetExhausted extends S.TaggedErrorClass<ExaEnrichmentBudgetExhausted>()(
  'ExaEnrichmentBudgetExhausted',
  {
    assignmentBudget: S.Number,
    assignmentUsed: S.Number,
    dailyBudget: S.Number,
    dailyUsed: S.Number,
    message: S.String,
    requested: S.Number,
    scope: S.Literals(['assignment', 'daily']),
  },
) {}

export class ExaEnrichmentMetricUnsafePayload extends S.TaggedErrorClass<ExaEnrichmentMetricUnsafePayload>()(
  'ExaEnrichmentMetricUnsafePayload',
  {
    reason: S.String,
  },
) {}

export type AdjutantEnrichmentOperationsError =
  | ExaEnrichmentBudgetExhausted
  | ExaEnrichmentMetricUnsafePayload
  | ExaEnrichmentOperationsStorageError

const MAX_CACHE_RESULTS_JSON_BYTES = 12_000

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ExaEnrichmentOperationsStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new ExaEnrichmentOperationsStorageError({ operation, error }),
  })

const dayKey = (iso: string): string => iso.slice(0, 10)

const addHours = (iso: string, hours: number): string =>
  isoTimestampAfterIso(iso, Math.max(0, hours) * 3_600_000)

const assertMetricSafe = (
  input: RecordExaMetricInput,
): Effect.Effect<void, ExaEnrichmentMetricUnsafePayload> =>
  containsProviderSecretMaterial(JSON.stringify(input))
    ? Effect.fail(
        new ExaEnrichmentMetricUnsafePayload({
          reason: 'Exa metric contains secret-shaped material.',
        }),
      )
    : Effect.void

const parseCachedResults = (
  row: ExaCacheEntryRow,
): Effect.Effect<
  ReadonlyArray<CachedExaSourceResult>,
  ExaEnrichmentOperationsStorageError
> =>
  Effect.try({
    catch: error =>
      new ExaEnrichmentOperationsStorageError({
        operation: 'adjutantEnrichment.cache.decode',
        error,
      }),
    try: () =>
      parseJsonWithSchema(S.Array(CachedExaSourceResult), row.results_json),
  })

export const exaCacheKey = (
  input: Readonly<{
    freshnessMaxAgeHours: number
    includeDomains: ReadonlyArray<string>
    query: string
    searchType: string
    sourceCategory: string
    urls: ReadonlyArray<string>
  }>,
): Effect.Effect<string, ExaEnrichmentOperationsStorageError> =>
  Effect.tryPromise({
    catch: error =>
      new ExaEnrichmentOperationsStorageError({
        operation: 'adjutantEnrichment.cache.key',
        error,
      }),
    try: async () => {
      const bytes = new TextEncoder().encode(
        JSON.stringify({
          freshnessMaxAgeHours: input.freshnessMaxAgeHours,
          includeDomains: [...input.includeDomains].sort(),
          query: input.query.trim().replace(/\s+/g, ' '),
          searchType: input.searchType,
          sourceCategory: input.sourceCategory,
          urls: [...input.urls].sort(),
        }),
      )
      const digest = await crypto.subtle.digest('SHA-256', bytes)

      return Array.from(new Uint8Array(digest))
        .map(byte => byte.toString(16).padStart(2, '0'))
        .join('')
    },
  })

const reserveBudget = (
  db: D1Database,
  runtime: AdjutantEnrichmentOperationsRuntime,
  input: ReserveExaBudgetInput,
): Effect.Effect<void, AdjutantEnrichmentOperationsError> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()
    const requested = Math.max(0, Math.trunc(input.requestUnits))

    if (requested === 0) {
      return
    }

    const usage = yield* d1Effect('adjutantEnrichment.budget.read', () =>
      db
        .prepare(
          `SELECT COALESCE(SUM(CASE WHEN assignment_id = ? THEN request_units ELSE 0 END), 0) AS assignment_units,
                  COALESCE(SUM(request_units), 0) AS day_units
             FROM exa_enrichment_budget_events
            WHERE day_key = ?`,
        )
        .bind(input.assignmentId, dayKey(now))
        .first<ExaBudgetUsageRow>(),
    )
    const assignmentUsed = usage?.assignment_units ?? 0
    const dailyUsed = usage?.day_units ?? 0

    if (
      requested > input.policy.assignmentRequestBudget ||
      assignmentUsed + requested > input.policy.assignmentRequestBudget
    ) {
      return yield* new ExaEnrichmentBudgetExhausted({
        assignmentBudget: input.policy.assignmentRequestBudget,
        assignmentUsed,
        dailyBudget: input.policy.dailyRequestBudget,
        dailyUsed,
        message:
          'Exa assignment request budget would be exceeded by this enrichment run.',
        requested,
        scope: 'assignment',
      })
    }

    if (dailyUsed + requested > input.policy.dailyRequestBudget) {
      return yield* new ExaEnrichmentBudgetExhausted({
        assignmentBudget: input.policy.assignmentRequestBudget,
        assignmentUsed,
        dailyBudget: input.policy.dailyRequestBudget,
        dailyUsed,
        message:
          'Exa daily request budget would be exceeded by this enrichment run.',
        requested,
        scope: 'daily',
      })
    }

    yield* d1Effect('adjutantEnrichment.budget.insert', () =>
      db
        .prepare(
          `INSERT INTO exa_enrichment_budget_events
             (id,
              assignment_id,
              run_id,
              day_key,
              request_units,
              reason,
              created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          runtime.makeBudgetEventId(),
          input.assignmentId,
          input.runId ?? null,
          dayKey(now),
          requested,
          input.reason,
          now,
        )
        .run(),
    )
  })

const readFreshCache = (
  db: D1Database,
  runtime: AdjutantEnrichmentOperationsRuntime,
  input: ExaCacheLookupInput,
): Effect.Effect<
  ReadonlyArray<CachedExaSourceResult> | null,
  ExaEnrichmentOperationsStorageError
> =>
  d1Effect('adjutantEnrichment.cache.read', () =>
    db
      .prepare(
        `SELECT id,
                results_json,
                result_count,
                cost_dollars,
                created_at,
                expires_at
           FROM exa_enrichment_cache_entries
          WHERE cache_key = ?
            AND freshness_max_age_hours = ?
            AND expires_at > ?
            AND archived_at IS NULL
          ORDER BY expires_at DESC
          LIMIT 1`,
      )
      .bind(
        input.cacheKey,
        Math.max(0, Math.trunc(input.freshnessMaxAgeHours)),
        input.nowIso ?? runtime.nowIso(),
      )
      .first<ExaCacheEntryRow>(),
  ).pipe(
    Effect.flatMap(row =>
      row === null ? Effect.succeed(null) : parseCachedResults(row),
    ),
  )

const storeCache = (
  db: D1Database,
  runtime: AdjutantEnrichmentOperationsRuntime,
  input: ExaCacheStoreInput,
): Effect.Effect<void, AdjutantEnrichmentOperationsError> =>
  Effect.gen(function* () {
    if (containsProviderSecretMaterial(JSON.stringify(input.results))) {
      return yield* new ExaEnrichmentMetricUnsafePayload({
        reason: 'Exa cache contains secret-shaped material.',
      })
    }

    const now = runtime.nowIso()
    const resultsJson = JSON.stringify(input.results)

    if (resultsJson.length > MAX_CACHE_RESULTS_JSON_BYTES) {
      return yield* new ExaEnrichmentMetricUnsafePayload({
        reason: 'Exa cache payload exceeds the bounded storage limit.',
      })
    }

    yield* d1Effect('adjutantEnrichment.cache.archiveExisting', () =>
      db
        .prepare(
          `UPDATE exa_enrichment_cache_entries
              SET archived_at = ?
            WHERE cache_key = ?
              AND archived_at IS NULL`,
        )
        .bind(now, input.cacheKey)
        .run(),
    )
    yield* d1Effect('adjutantEnrichment.cache.insert', () =>
      db
        .prepare(
          `INSERT INTO exa_enrichment_cache_entries
             (id,
              cache_key,
              source_category,
              search_type,
              freshness_max_age_hours,
              results_json,
              result_count,
              cost_dollars,
              created_at,
              expires_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          runtime.makeCacheEntryId(),
          input.cacheKey,
          input.sourceCategory,
          input.searchType,
          Math.max(0, Math.trunc(input.freshnessMaxAgeHours)),
          resultsJson,
          input.results.length,
          input.costDollars ?? null,
          now,
          addHours(now, input.policy.cacheTtlHours),
        )
        .run(),
    )
  })

const recordMetric = (
  db: D1Database,
  runtime: AdjutantEnrichmentOperationsRuntime,
  input: RecordExaMetricInput,
): Effect.Effect<void, AdjutantEnrichmentOperationsError> =>
  Effect.gen(function* () {
    yield* assertMetricSafe(input)

    yield* d1Effect('adjutantEnrichment.metric.insert', () =>
      db
        .prepare(
          `INSERT INTO exa_enrichment_metric_events
             (id,
              assignment_id,
              run_id,
              query_id,
              event_name,
              status,
              error_code,
              search_type,
              source_category,
              result_count,
              source_card_count,
              latency_ms,
              cost_dollars,
              cache_status,
              created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          runtime.makeMetricEventId(),
          input.assignmentId,
          input.runId ?? null,
          input.queryId ?? null,
          input.eventName,
          input.status,
          input.errorCode ?? null,
          input.searchType ?? null,
          input.sourceCategory ?? null,
          input.resultCount ?? null,
          input.sourceCardCount ?? null,
          input.latencyMs ?? null,
          input.costDollars ?? null,
          input.cacheStatus ?? null,
          runtime.nowIso(),
        )
        .run(),
    )
  })

export type ExaRetryDecision = Readonly<{
  delayMs: number
  retry: boolean
  reason: 'none' | 'provider_5xx' | 'provider_rate_limited' | 'provider_timeout'
}>

export const exaRetryDecision = (
  error: ExaError,
  attemptIndex: number,
  policy: ExaEnrichmentOperationsPolicy,
): ExaRetryDecision => {
  if (attemptIndex >= policy.retryLimit) {
    return { delayMs: 0, reason: 'none', retry: false }
  }

  if (error instanceof ExaProviderHttpError && error.status === 429) {
    return {
      delayMs: policy.rateLimitBackoffMs,
      reason: 'provider_rate_limited',
      retry: true,
    }
  }

  if (error instanceof ExaProviderHttpError && error.status >= 500) {
    return {
      delayMs: Math.max(0, Math.trunc(policy.rateLimitBackoffMs / 2)),
      reason: 'provider_5xx',
      retry: true,
    }
  }

  if (
    error instanceof ExaProviderTimeout ||
    error instanceof ExaProviderFetchError
  ) {
    return {
      delayMs: Math.max(0, Math.trunc(policy.rateLimitBackoffMs / 2)),
      reason: 'provider_timeout',
      retry: true,
    }
  }

  return { delayMs: 0, reason: 'none', retry: false }
}

export const retryExaEffect = <A>(
  policy: ExaEnrichmentOperationsPolicy,
  effect: Effect.Effect<A, ExaError>,
): Effect.Effect<A, ExaError> => {
  const run = (attemptIndex: number): Effect.Effect<A, ExaError> =>
    effect.pipe(
      Effect.catch((error: ExaError) => {
        const decision = exaRetryDecision(error, attemptIndex, policy)

        return decision.retry
          ? Effect.sleep(`${decision.delayMs} millis`).pipe(
              Effect.flatMap(() => run(attemptIndex + 1)),
            )
          : Effect.fail(error)
      }),
    )

  return run(0)
}

export const makeAdjutantEnrichmentOperationsService = (
  db: D1Database,
  runtime: AdjutantEnrichmentOperationsRuntime = systemAdjutantEnrichmentOperationsRuntime,
) => ({
  readFreshCache: Effect.fn('AdjutantEnrichmentOperations.readFreshCache')(
    (input: ExaCacheLookupInput) => readFreshCache(db, runtime, input),
  ),
  recordMetric: Effect.fn('AdjutantEnrichmentOperations.recordMetric')(
    (input: RecordExaMetricInput) => recordMetric(db, runtime, input),
  ),
  reserveBudget: Effect.fn('AdjutantEnrichmentOperations.reserveBudget')(
    (input: ReserveExaBudgetInput) => reserveBudget(db, runtime, input),
  ),
  storeCache: Effect.fn('AdjutantEnrichmentOperations.storeCache')(
    (input: ExaCacheStoreInput) => storeCache(db, runtime, input),
  ),
})

export class AdjutantEnrichmentOperationsService extends Context.Service<
  AdjutantEnrichmentOperationsService,
  ReturnType<typeof makeAdjutantEnrichmentOperationsService>
>()('@openagentsinc/autopilot-omega/AdjutantEnrichmentOperationsService') {
  static layer = (
    env: AdjutantEnrichmentOperationsEnv,
    runtime: AdjutantEnrichmentOperationsRuntime = systemAdjutantEnrichmentOperationsRuntime,
  ) =>
    Layer.succeed(
      AdjutantEnrichmentOperationsService,
      makeAdjutantEnrichmentOperationsService(openAgentsDatabase(env), runtime),
    )
}
