import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AdjutantEnrichmentOperationsRuntime,
  type ExaEnrichmentOperationsPolicy,
  exaCacheKey,
  exaRetryDecision,
  makeAdjutantEnrichmentOperationsService,
} from './adjutant-enrichment-operations'
import {
  ExaProviderHttpError,
  ExaProviderSchemaError,
  ExaProviderTimeout,
} from './exa'

type BudgetEvent = Readonly<{
  assignment_id: string
  created_at: string
  day_key: string
  id: string
  reason: string
  request_units: number
  run_id: string | null
}>

type CacheEntry = Readonly<{
  archived_at: string | null
  cache_key: string
  cost_dollars: number | null
  created_at: string
  expires_at: string
  freshness_max_age_hours: number
  id: string
  result_count: number
  results_json: string
  search_type: string
  source_category: string
}>

type MetricEvent = Readonly<{
  assignment_id: string
  cache_status: string | null
  cost_dollars: number | null
  created_at: string
  error_code: string | null
  event_name: string
  id: string
  latency_ms: number | null
  query_id: string | null
  result_count: number | null
  run_id: string | null
  search_type: string | null
  source_card_count: number | null
  source_category: string | null
  status: string
}>

type OperationsState = {
  budgetEvents: Array<BudgetEvent>
  cacheEntries: Array<CacheEntry>
  metricEvents: Array<MetricEvent>
}

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const textValue = (value: unknown): string => String(value)
const nullableText = (value: unknown): string | null =>
  value === null || value === undefined ? null : String(value)
const numberValue = (value: unknown): number => Number(value)

class OperationsStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly state: OperationsState,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM exa_enrichment_budget_events')) {
      const [assignmentId, dayKey] = this.values
      const events = this.state.budgetEvents.filter(
        event => event.day_key === dayKey,
      )
      const assignmentUnits = events
        .filter(event => event.assignment_id === assignmentId)
        .reduce((total, event) => total + event.request_units, 0)
      const dayUnits = events.reduce(
        (total, event) => total + event.request_units,
        0,
      )

      return Promise.resolve({
        assignment_units: assignmentUnits,
        day_units: dayUnits,
      } as T)
    }

    if (this.query.includes('FROM exa_enrichment_cache_entries')) {
      const [cacheKey, freshnessMaxAgeHours, nowIso] = this.values
      const row = this.state.cacheEntries
        .filter(
          entry =>
            entry.cache_key === cacheKey &&
            entry.freshness_max_age_hours === freshnessMaxAgeHours &&
            entry.expires_at > String(nowIso) &&
            entry.archived_at === null,
        )
        .sort((left, right) =>
          right.expires_at.localeCompare(left.expires_at),
        )[0]

      return Promise.resolve((row as T | undefined) ?? null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO exa_enrichment_budget_events')) {
      const [id, assignmentId, runId, dayKey, requestUnits, reason, createdAt] =
        this.values

      this.state.budgetEvents.push({
        assignment_id: textValue(assignmentId),
        created_at: textValue(createdAt),
        day_key: textValue(dayKey),
        id: textValue(id),
        reason: textValue(reason),
        request_units: numberValue(requestUnits),
        run_id: nullableText(runId),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE exa_enrichment_cache_entries')) {
      const [archivedAt, cacheKey] = this.values

      this.state.cacheEntries = this.state.cacheEntries.map(entry =>
        entry.cache_key === cacheKey && entry.archived_at === null
          ? { ...entry, archived_at: textValue(archivedAt) }
          : entry,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO exa_enrichment_cache_entries')) {
      const [
        id,
        cacheKey,
        sourceCategory,
        searchType,
        freshnessMaxAgeHours,
        resultsJson,
        resultCount,
        costDollars,
        createdAt,
        expiresAt,
      ] = this.values

      this.state.cacheEntries.push({
        archived_at: null,
        cache_key: textValue(cacheKey),
        cost_dollars: costDollars === null ? null : numberValue(costDollars),
        created_at: textValue(createdAt),
        expires_at: textValue(expiresAt),
        freshness_max_age_hours: numberValue(freshnessMaxAgeHours),
        id: textValue(id),
        result_count: numberValue(resultCount),
        results_json: textValue(resultsJson),
        search_type: textValue(searchType),
        source_category: textValue(sourceCategory),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO exa_enrichment_metric_events')) {
      const [
        id,
        assignmentId,
        runId,
        queryId,
        eventName,
        status,
        errorCode,
        searchType,
        sourceCategory,
        resultCount,
        sourceCardCount,
        latencyMs,
        costDollars,
        cacheStatus,
        createdAt,
      ] = this.values

      this.state.metricEvents.push({
        assignment_id: textValue(assignmentId),
        cache_status: nullableText(cacheStatus),
        cost_dollars: costDollars === null ? null : numberValue(costDollars),
        created_at: textValue(createdAt),
        error_code: nullableText(errorCode),
        event_name: textValue(eventName),
        id: textValue(id),
        latency_ms: latencyMs === null ? null : numberValue(latencyMs),
        query_id: nullableText(queryId),
        result_count: resultCount === null ? null : numberValue(resultCount),
        run_id: nullableText(runId),
        search_type: nullableText(searchType),
        source_card_count:
          sourceCardCount === null ? null : numberValue(sourceCardCount),
        source_category: nullableText(sourceCategory),
        status: textValue(status),
      })

      return Promise.resolve(makeResult<T>())
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

const operationsDb = (state: OperationsState): D1Database => ({
  batch: async <T = unknown>(
    statements: D1PreparedStatement[],
  ): Promise<Array<D1Result<T>>> =>
    Promise.all(statements.map(statement => statement.run<T>())),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OperationsStatement(query, state),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const makeState = (): OperationsState => ({
  budgetEvents: [],
  cacheEntries: [],
  metricEvents: [],
})

const runtime: AdjutantEnrichmentOperationsRuntime = {
  makeBudgetEventId: () => 'budget_event_1',
  makeCacheEntryId: () => 'cache_entry_1',
  makeMetricEventId: () => 'metric_event_1',
  nowIso: () => '2026-06-05T12:00:00.000Z',
}

const policy: ExaEnrichmentOperationsPolicy = {
  assignmentRequestBudget: 3,
  cacheTtlHours: 6,
  dailyRequestBudget: 5,
  rateLimitBackoffMs: 250,
  retryLimit: 2,
}

describe('Adjutant Exa enrichment operations', () => {
  test('reserves request budget and blocks assignment exhaustion', async () => {
    const state = makeState()
    state.budgetEvents.push({
      assignment_id: 'assignment_1',
      created_at: '2026-06-05T10:00:00.000Z',
      day_key: '2026-06-05',
      id: 'budget_prior',
      reason: 'run',
      request_units: 2,
      run_id: null,
    })
    const service = makeAdjutantEnrichmentOperationsService(
      operationsDb(state),
      runtime,
    )

    await expect(
      Effect.runPromise(
        service.reserveBudget({
          assignmentId: 'assignment_1',
          policy,
          reason: 'run',
          requestUnits: 2,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'ExaEnrichmentBudgetExhausted',
      scope: 'assignment',
    })
    expect(state.budgetEvents).toHaveLength(1)
  })

  test('blocks daily budget exhaustion before inserting an event', async () => {
    const state = makeState()
    state.budgetEvents.push(
      {
        assignment_id: 'assignment_a',
        created_at: '2026-06-05T10:00:00.000Z',
        day_key: '2026-06-05',
        id: 'budget_prior_a',
        reason: 'run',
        request_units: 3,
        run_id: null,
      },
      {
        assignment_id: 'assignment_b',
        created_at: '2026-06-05T11:00:00.000Z',
        day_key: '2026-06-05',
        id: 'budget_prior_b',
        reason: 'run',
        request_units: 2,
        run_id: null,
      },
    )
    const service = makeAdjutantEnrichmentOperationsService(
      operationsDb(state),
      runtime,
    )

    await expect(
      Effect.runPromise(
        service.reserveBudget({
          assignmentId: 'assignment_c',
          policy,
          reason: 'run',
          requestUnits: 1,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'ExaEnrichmentBudgetExhausted',
      scope: 'daily',
    })
    expect(state.budgetEvents).toHaveLength(2)
  })

  test('reads only fresh compatible cache entries and archives replaced rows', async () => {
    const state = makeState()
    const service = makeAdjutantEnrichmentOperationsService(
      operationsDb(state),
      runtime,
    )
    const cacheKey = await Effect.runPromise(
      exaCacheKey({
        freshnessMaxAgeHours: 24,
        includeDomains: [],
        query: 'OTEC floating datacenter',
        searchType: 'auto',
        sourceCategory: 'topic_web',
        urls: [],
      }),
    )
    state.cacheEntries.push({
      archived_at: null,
      cache_key: cacheKey,
      cost_dollars: null,
      created_at: '2026-06-04T00:00:00.000Z',
      expires_at: '2026-06-04T12:00:00.000Z',
      freshness_max_age_hours: 24,
      id: 'stale_cache',
      result_count: 1,
      results_json: JSON.stringify([
        {
          domain: 'example.com',
          highlightText: 'Stale OTEC context.',
          publishedDate: null,
          title: 'Stale',
          url: 'https://example.com/stale',
        },
      ]),
      search_type: 'auto',
      source_category: 'topic_web',
    })

    await expect(
      Effect.runPromise(
        service.readFreshCache({
          cacheKey,
          freshnessMaxAgeHours: 24,
        }),
      ),
    ).resolves.toBeNull()

    await Effect.runPromise(
      service.storeCache({
        cacheKey,
        costDollars: 0.01,
        freshnessMaxAgeHours: 24,
        policy,
        results: [
          {
            domain: 'otec.example',
            highlightText: 'Fresh OTEC and SWAC context.',
            publishedDate: '2026-06-01',
            title: 'OTEC reference',
            url: 'https://otec.example/reference',
          },
        ],
        searchType: 'auto',
        sourceCategory: 'topic_web',
      }),
    )

    expect(state.cacheEntries[0]?.archived_at).toBe('2026-06-05T12:00:00.000Z')
    await expect(
      Effect.runPromise(
        service.readFreshCache({
          cacheKey,
          freshnessMaxAgeHours: 24,
        }),
      ),
    ).resolves.toEqual([
      {
        domain: 'otec.example',
        highlightText: 'Fresh OTEC and SWAC context.',
        publishedDate: '2026-06-01',
        title: 'OTEC reference',
        url: 'https://otec.example/reference',
      },
    ])
  })

  test('records metric names and rejects secret-shaped metric payloads', async () => {
    const state = makeState()
    const service = makeAdjutantEnrichmentOperationsService(
      operationsDb(state),
      runtime,
    )

    await Effect.runPromise(
      service.recordMetric({
        assignmentId: 'assignment_1',
        cacheStatus: 'hit',
        eventName: 'exa.enrichment.cache.hit',
        resultCount: 1,
        runId: 'exa_run_1',
        sourceCardCount: 1,
        status: 'cached',
      }),
    )

    expect(state.metricEvents).toEqual([
      expect.objectContaining({
        cache_status: 'hit',
        event_name: 'exa.enrichment.cache.hit',
        status: 'cached',
      }),
    ])

    await expect(
      Effect.runPromise(
        service.recordMetric({
          assignmentId: 'assignment_1',
          errorCode: 'OPENAI_API_KEY=sk-provider-secret',
          eventName: 'exa.enrichment.search.failed',
          status: 'failed',
        }),
      ),
    ).rejects.toMatchObject({
      _tag: 'ExaEnrichmentMetricUnsafePayload',
    })
  })

  test('classifies retry decisions for rate limits and transient failures', () => {
    expect(
      exaRetryDecision(
        new ExaProviderHttpError({
          endpoint: '/search',
          message: 'rate limited',
          status: 429,
        }),
        0,
        policy,
      ),
    ).toEqual({
      delayMs: 250,
      reason: 'provider_rate_limited',
      retry: true,
    })
    expect(
      exaRetryDecision(
        new ExaProviderHttpError({
          endpoint: '/search',
          message: 'rate limited',
          status: 429,
        }),
        2,
        policy,
      ),
    ).toEqual({ delayMs: 0, reason: 'none', retry: false })
    expect(
      exaRetryDecision(
        new ExaProviderHttpError({
          endpoint: '/search',
          message: 'upstream unavailable',
          status: 503,
        }),
        0,
        policy,
      ),
    ).toMatchObject({ reason: 'provider_5xx', retry: true })
    expect(
      exaRetryDecision(
        new ExaProviderTimeout({
          endpoint: '/contents',
          timeoutMs: 25_000,
        }),
        0,
        policy,
      ),
    ).toMatchObject({ reason: 'provider_timeout', retry: true })
    expect(
      exaRetryDecision(
        new ExaProviderSchemaError({
          endpoint: '/search',
          error: 'schema mismatch',
        }),
        0,
        policy,
      ),
    ).toEqual({ delayMs: 0, reason: 'none', retry: false })
  })
})
