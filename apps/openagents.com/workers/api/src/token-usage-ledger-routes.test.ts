import {
  InferenceAnalyticsResponse,
  TokenUsageAggregateResponse,
  TokenUsageEventRecord,
  TokenUsageLeaderboardPreferenceResponse,
  TokenUsageLeaderboardsResponse,
} from '@openagentsinc/sync-schema'
import { Effect, Layer } from 'effect'
import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TokenUsageLedger,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'
import { makeTokenUsageLedgerRoutes } from './token-usage-ledger-routes'
import {
  isoTimestampAfterIso,
  utcStartOfDayIsoTimestamp,
} from './runtime-primitives'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

const makeExecutionContext = (): ExecutionContext =>
  ({
    passThroughOnException: () => undefined,
    waitUntil: () => undefined,
  }) as unknown as ExecutionContext

const eventRecord = S.decodeUnknownSync(TokenUsageEventRecord)({
  schemaVersion: 'openagents.token_usage_event.record.v1' as const,
  actor: {
    teamId: 'team_openagents_core',
    userId: 'user_chris',
  },
  backendProfile: 'worker-secret',
  cost: null,
  eventId: 'token_event_route_1',
  idempotencyKey: 'route:event:1',
  ingestedAt: '2026-06-08T12:00:00.000Z',
  model: 'gemini-2.5-flash',
  observedAt: '2026-06-08T11:59:00.000Z',
  privacy: {
    leaderboardEligible: true,
    privacyOptOut: false,
  },
  producerSystem: 'omega' as const,
  provider: 'google_gemini',
  safeMetadata: {
    providerRequestStatus: 'succeeded',
  },
  sourceRefs: {
    anonymizedSourceRef: 'omega-request-hash:route',
  },
  sourceRoute: 'omega_provider_broker' as const,
  tokenCounts: {
    cacheReadTokens: 5,
    cacheWrite1hTokens: 0,
    cacheWrite5mTokens: 0,
    inputTokens: 50,
    outputTokens: 20,
    reasoningTokens: 10,
    totalTokens: 85,
  },
  usageTruth: 'exact' as const,
})

const aggregateResponse = S.decodeUnknownSync(TokenUsageAggregateResponse)({
  schemaVersion: 'openagents.token_usage_aggregate.v1' as const,
  byActor: [
    {
      accountRef: null,
      anonymous: false,
      teamId: 'team_openagents_core',
      tokenCounts: eventRecord.tokenCounts,
      usageEvents: 1,
      userId: 'user_chris',
    },
  ],
  byProviderModel: [
    {
      key: 'google_gemini:gemini-2.5-flash',
      label: 'google_gemini / gemini-2.5-flash',
      tokenCounts: eventRecord.tokenCounts,
      usageEvents: 1,
    },
  ],
  bySourceRoute: [
    {
      key: 'omega:omega_provider_broker',
      label: 'omega / omega_provider_broker',
      tokenCounts: eventRecord.tokenCounts,
      usageEvents: 1,
    },
  ],
  bySourceRef: [
    {
      key: 'anonymized:omega-request-hash:route',
      label: 'anonymized / omega-request-hash:route',
      tokenCounts: eventRecord.tokenCounts,
      usageEvents: 1,
    },
  ],
  byUsageTruth: [
    {
      key: 'exact',
      label: 'exact',
      tokenCounts: eventRecord.tokenCounts,
      usageEvents: 1,
    },
  ],
  filters: {},
  generatedAt: '2026-06-08T12:00:00.000Z',
  recentEvents: [eventRecord],
  totals: eventRecord.tokenCounts,
  usageEvents: 1,
})

const leaderboardsResponse = S.decodeUnknownSync(TokenUsageLeaderboardsResponse)({
  schemaVersion: 'openagents.token_usage_leaderboards.v1' as const,
  anonymousTotals: eventRecord.tokenCounts,
  filters: {
    since: '2026-06-01T12:00:00.000Z',
    window: '7d' as const,
  },
  generatedAt: '2026-06-08T12:00:00.000Z',
  globalTotals: eventRecord.tokenCounts,
  topProviderModels: aggregateResponse.byProviderModel,
  topProjects: [],
  topRuns: [],
  topTeams: aggregateResponse.byActor,
  topUsers: aggregateResponse.byActor,
})

const analyticsResponse = S.decodeUnknownSync(InferenceAnalyticsResponse)({
  schemaVersion: 'openagents.inference_analytics.v1' as const,
  window: '7d' as const,
  generatedAt: '2026-06-08T12:00:00.000Z',
  byProvider: [
    {
      key: 'fireworks',
      label: 'fireworks',
      inputTokens: 321065,
      outputTokens: 810787,
      totalTokens: 1131852,
      usageEvents: 560,
      costUsd: 0.272,
    },
  ],
  byModel: [
    {
      key: 'accounts/fireworks/models/deepseek-v4-flash',
      label: 'accounts/fireworks/models/deepseek-v4-flash',
      inputTokens: 321065,
      outputTokens: 810787,
      totalTokens: 1131852,
      usageEvents: 560,
      costUsd: 0.272,
    },
  ],
  byRoute: [
    {
      key: 'omega:omega_hosted_gemini',
      label: 'omega / omega_hosted_gemini',
      inputTokens: 321065,
      outputTokens: 810787,
      totalTokens: 1131852,
      usageEvents: 560,
      costUsd: 0.272,
    },
  ],
  byDay: [
    {
      day: '2026-06-25',
      inputTokens: 321065,
      outputTokens: 810787,
      totalTokens: 1131852,
      usageEvents: 560,
      costUsd: 0.272,
    },
  ],
  totals: {
    inputTokens: 321065,
    outputTokens: 810787,
    totalTokens: 1131852,
    usageEvents: 560,
    costUsd: 0.272,
    costCoverage: 1,
  },
})

const preferenceResponse = S.decodeUnknownSync(
  TokenUsageLeaderboardPreferenceResponse,
)({
  schemaVersion: 'openagents.token_usage_leaderboard_preference.v1' as const,
  preference: {
    leaderboardParticipation: 'eligible' as const,
    leaderboardVisibility: 'internal' as const,
    subjectKind: 'user' as const,
    subjectRef: 'user_chris',
    updatedAt: '2026-06-08T12:00:00.000Z',
    updatedByUserId: 'user_chris',
  },
})

const makeRoutes = (
  session: TestSession | undefined,
  input: Readonly<{
    authorized?: boolean
    inserted?: boolean
  }> = {},
) => {
  const calls: Array<unknown> = []
  const aggregateFilters: Array<unknown> = []
  const analyticsFilters: Array<unknown> = []
  const leaderboardFilters: Array<unknown> = []
  const preferenceInputs: Array<unknown> = []
  const ledger: TokenUsageLedgerShape = {
    ingestEvent: body => {
      calls.push(body)

      return Effect.succeed({
        event: eventRecord,
        inserted: input.inserted ?? true,
      })
    },
    readAggregates: filters => {
      aggregateFilters.push(filters ?? {})

      return Effect.succeed(aggregateResponse)
    },
    readInferenceAnalytics: filters => {
      analyticsFilters.push(filters ?? {})

      return Effect.succeed(analyticsResponse)
    },
    readPublicTokensServed: () => Effect.succeed({ tokensServed: 0 }),
    readPublicTokensServedHistory: () =>
      Effect.succeed({ window: '30d', bucket: 'day', series: [] }),
    readLeaderboardPreference: input => {
      preferenceInputs.push(input)

      return Effect.succeed(preferenceResponse)
    },
    readLeaderboards: filters => {
      leaderboardFilters.push(filters ?? {})

      return Effect.succeed(leaderboardsResponse)
    },
    updateLeaderboardPreference: (preferenceInput, body) => {
      preferenceInputs.push({ body, input: preferenceInput })

      return Effect.succeed(preferenceResponse)
    },
  }
  const routes = makeTokenUsageLedgerRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-test-session-refreshed', 'true')

      return response
    },
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    ledgerLayer: () => Layer.succeed(TokenUsageLedger, ledger),
    requireAdminApiToken: request =>
      Promise.resolve(
        input.authorized === true ||
          request.headers.get('authorization') === 'Bearer admin',
      ),
    requireBrowserSession: async () => session,
    runtime: {
      isoTimestampAfterIso,
      nowIso: () => '2026-06-08T12:00:00.000Z',
      utcStartOfDayIsoTimestamp,
    },
  })

  return {
    aggregateFilters,
    analyticsFilters,
    calls,
    leaderboardFilters,
    preferenceInputs,
    routes,
  }
}

const env = { OPENAGENTS_DB: {} as D1Database }

const readJson = async (response: Response): Promise<Record<string, unknown>> =>
  (await response.json()) as Record<string, unknown>

describe('token usage ledger routes', () => {
  test('rejects ingestion without trusted producer bearer auth', async () => {
    const { routes } = makeRoutes(undefined)
    const response = await Effect.runPromise(
      routes.handleTokenUsageEventsApi(
        new Request('https://openagents.com/api/stats/token-usage/events', {
          body: JSON.stringify({}),
          method: 'POST',
        }),
        env,
      ),
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('ingests token usage events with trusted producer bearer auth', async () => {
    const { calls, routes } = makeRoutes(undefined)
    const response = await Effect.runPromise(
      routes.handleTokenUsageEventsApi(
        new Request('https://openagents.com/api/stats/token-usage/events', {
          body: JSON.stringify({
            schemaVersion: 'openagents.token_usage_event.v1',
            eventId: 'token_event_route_1',
            idempotencyKey: 'route:event:1',
            observedAt: '2026-06-08T11:59:00.000Z',
            producerSystem: 'omega',
            sourceRoute: 'omega_provider_broker',
            tokenCounts: eventRecord.tokenCounts,
            usageTruth: 'exact',
          }),
          headers: {
            authorization: 'Bearer admin',
            'content-type': 'application/json',
          },
          method: 'POST',
        }),
        env,
      ),
    )
    const body = await readJson(response)

    expect(response.status).toBe(201)
    expect(calls).toHaveLength(1)
    expect(body).toMatchObject({
      inserted: true,
      event: {
        eventId: 'token_event_route_1',
        provider: 'google_gemini',
      },
    })
  })

  test('requires an admin browser session for aggregate reads', async () => {
    const nonAdmin = makeRoutes({
      user: {
        email: 'operator@example.com',
        userId: 'user_operator',
      },
    })
    const forbiddenResponse = await Effect.runPromise(
      nonAdmin.routes.handleTokenUsageAggregateApi(
        new Request('https://openagents.com/api/stats/token-usage/aggregate'),
        env,
        makeExecutionContext(),
      ),
    )

    expect(forbiddenResponse.status).toBe(403)

    const admin = makeRoutes({
      user: {
        email: 'chris@openagents.com',
        userId: 'user_chris',
      },
    })
    const okResponse = await Effect.runPromise(
      admin.routes.handleTokenUsageAggregateApi(
        new Request(
          'https://openagents.com/api/stats/token-usage/aggregate?since=2026-06-08T00:00:00.000Z&provider=google_gemini&model=gemini-2.5-flash&producerSystem=omega&sourceRoute=omega_provider_broker&actorUserId=user_chris&leaderboardEligible=true&usageTruth=exact',
        ),
        env,
        makeExecutionContext(),
      ),
    )
    const okBody = await readJson(okResponse)

    expect(okResponse.status).toBe(200)
    expect(okResponse.headers.get('x-test-session-refreshed')).toBe('true')
    expect(okBody).toMatchObject({
      schemaVersion: 'openagents.token_usage_aggregate.v1',
      totals: {
        totalTokens: 85,
      },
      usageEvents: 1,
    })
    expect(admin.aggregateFilters).toEqual([
      {
        actorUserId: 'user_chris',
        leaderboardEligible: 'true',
        model: 'gemini-2.5-flash',
        producerSystem: 'omega',
        provider: 'google_gemini',
        since: '2026-06-08T00:00:00.000Z',
        sourceRoute: 'omega_provider_broker',
        usageTruth: 'exact',
      },
    ])
  })

  test('requires an admin/owner session for inference analytics reads', async () => {
    const anonymous = makeRoutes(undefined)
    const unauthorized = await Effect.runPromise(
      anonymous.routes.handleInferenceAnalyticsApi(
        new Request('https://openagents.com/api/admin/inference-analytics'),
        env,
        makeExecutionContext(),
      ),
    )

    expect(unauthorized.status).toBe(401)

    const nonAdmin = makeRoutes({
      user: {
        email: 'operator@example.com',
        userId: 'user_operator',
      },
    })
    const forbidden = await Effect.runPromise(
      nonAdmin.routes.handleInferenceAnalyticsApi(
        new Request('https://openagents.com/api/admin/inference-analytics'),
        env,
        makeExecutionContext(),
      ),
    )

    expect(forbidden.status).toBe(403)
  })

  test('returns aggregate inference analytics and passes the window filter for an admin', async () => {
    const admin = makeRoutes({
      user: {
        email: 'chris@openagents.com',
        userId: 'user_chris',
      },
    })
    const response = await Effect.runPromise(
      admin.routes.handleInferenceAnalyticsApi(
        new Request(
          'https://openagents.com/api/admin/inference-analytics?window=7d',
        ),
        env,
        makeExecutionContext(),
      ),
    )
    const body = await readJson(response)

    expect(response.status).toBe(200)
    expect(response.headers.get('x-test-session-refreshed')).toBe('true')
    expect(body).toMatchObject({
      schemaVersion: 'openagents.inference_analytics.v1',
      window: '7d',
      byProvider: [{ key: 'fireworks', costUsd: 0.272 }],
      totals: { costCoverage: 1, totalTokens: 1131852 },
    })
    expect(admin.analyticsFilters).toEqual([{ window: '7d' }])
  })

  test('rejects a non-GET inference analytics request', async () => {
    const admin = makeRoutes({
      user: {
        email: 'chris@openagents.com',
        userId: 'user_chris',
      },
    })
    const response = await Effect.runPromise(
      admin.routes.handleInferenceAnalyticsApi(
        new Request('https://openagents.com/api/admin/inference-analytics', {
          method: 'POST',
        }),
        env,
        makeExecutionContext(),
      ),
    )

    expect(response.status).toBe(405)
  })

  test('requires admin access for leaderboard reads and passes window filters', async () => {
    const nonAdmin = makeRoutes({
      user: {
        email: 'operator@example.com',
        userId: 'user_operator',
      },
    })
    const forbiddenResponse = await Effect.runPromise(
      nonAdmin.routes.handleTokenUsageLeaderboardsApi(
        new Request('https://openagents.com/api/stats/token-usage/leaderboards'),
        env,
        makeExecutionContext(),
      ),
    )

    expect(forbiddenResponse.status).toBe(403)

    const admin = makeRoutes({
      user: {
        email: 'chris@openagents.com',
        userId: 'user_chris',
      },
    })
    const okResponse = await Effect.runPromise(
      admin.routes.handleTokenUsageLeaderboardsApi(
        new Request(
          'https://openagents.com/api/stats/token-usage/leaderboards?window=30d',
        ),
        env,
        makeExecutionContext(),
      ),
    )
    const okBody = await readJson(okResponse)

    expect(okResponse.status).toBe(200)
    expect(okBody).toMatchObject({
      schemaVersion: 'openagents.token_usage_leaderboards.v1',
      topUsers: [
        {
          userId: 'user_chris',
        },
      ],
    })
    expect(admin.leaderboardFilters).toEqual([{ window: '30d' }])
  })

  test('reads and updates the current user leaderboard preference', async () => {
    const currentUser = makeRoutes({
      user: {
        email: 'operator@example.com',
        userId: 'user_chris',
      },
    })
    const getResponse = await Effect.runPromise(
      currentUser.routes.handleTokenUsageLeaderboardPreferenceApi(
        new Request(
          'https://openagents.com/api/stats/token-usage/leaderboard-preference',
        ),
        env,
        makeExecutionContext(),
      ),
    )
    const putResponse = await Effect.runPromise(
      currentUser.routes.handleTokenUsageLeaderboardPreferenceApi(
        new Request(
          'https://openagents.com/api/stats/token-usage/leaderboard-preference',
          {
            body: JSON.stringify({
              leaderboardParticipation: 'opted_out',
              leaderboardVisibility: 'private',
            }),
            headers: {
              'content-type': 'application/json',
            },
            method: 'PUT',
          },
        ),
        env,
        makeExecutionContext(),
      ),
    )

    expect(getResponse.status).toBe(200)
    expect(putResponse.status).toBe(200)
    expect(currentUser.preferenceInputs).toEqual([
      {
        actorUserId: 'user_chris',
        subjectKind: 'user',
        subjectRef: 'user_chris',
      },
      {
        body: {
          leaderboardParticipation: 'opted_out',
          leaderboardVisibility: 'private',
        },
        input: {
          actorUserId: 'user_chris',
          subjectKind: 'user',
          subjectRef: 'user_chris',
        },
      },
    ])
  })
})
