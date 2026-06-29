import {
  InferenceAnalyticsResponse,
  TokenUsageAggregateResponse,
  TokenUsageLeaderboardPreferenceResponse,
  TokenUsageLeaderboardsResponse,
} from '@openagentsinc/sync-schema'
import { Effect, Match as M, Option } from 'effect'
import { Command } from 'foldkit'
import { evo } from 'foldkit/struct'

import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedLoadTokenUsageStats,
  Message,
  SucceededLoadTokenUsageStats,
} from '../message'
import {
  Model,
  TokenUsageStatsFailed,
  TokenUsageStatsFilters,
  TokenUsageStatsLoaded,
  TokenUsageStatsLoading,
  type TokenUsageStatsFilterKey,
} from '../model'
import { type UpdateReturn, noUpdate } from '../transition'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const filterKeys: ReadonlyArray<TokenUsageStatsFilterKey> = [
  'actorTeamId',
  'actorUserId',
  'leaderboardEligible',
  'leaderboardWindow',
  'model',
  'producerSystem',
  'provider',
  'since',
  'sourceRoute',
  'until',
  'usageTruth',
]

const queryStringFromFilters = (filters: TokenUsageStatsFilters): string => {
  const params = new URLSearchParams()

  for (const key of filterKeys) {
    if (key === 'leaderboardWindow') {
      continue
    }

    const value = filters[key].trim()

    if (value !== '') {
      params.set(key, value)
    }
  }

  const query = params.toString()

  return query === '' ? '' : `?${query}`
}

const windowQueryStringFromFilters = (
  filters: TokenUsageStatsFilters,
): string => {
  const window = filters.leaderboardWindow.trim() || '7d'
  const params = new URLSearchParams({ window })

  return `?${params.toString()}`
}

export const LoadTokenUsageStats = Command.define(
  'LoadTokenUsageStats',
  { filters: TokenUsageStatsFilters },
  SucceededLoadTokenUsageStats,
  FailedLoadTokenUsageStats,
)(({ filters }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.stats.tokenUsage.load',
      request: `/api/stats/token-usage/aggregate${queryStringFromFilters(filters)}`,
      schema: TokenUsageAggregateResponse,
    })
    const leaderboards = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.stats.tokenUsage.leaderboards.load',
      request: `/api/stats/token-usage/leaderboards${windowQueryStringFromFilters(filters)}`,
      schema: TokenUsageLeaderboardsResponse,
    })
    const analytics = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.stats.inferenceAnalytics.load',
      request: `/api/admin/inference-analytics${windowQueryStringFromFilters(filters)}`,
      schema: InferenceAnalyticsResponse,
    })
    const preference = yield* requestJson({
      init: {
        cache: 'no-store',
        credentials: 'include',
        headers: { accept: 'application/json' },
      },
      name: 'loggedIn.stats.tokenUsage.preference.load',
      request: '/api/stats/token-usage/leaderboard-preference',
      schema: TokenUsageLeaderboardPreferenceResponse,
    })

    return SucceededLoadTokenUsageStats({
      analytics,
      filters,
      leaderboards,
      preference,
      response,
    })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedLoadTokenUsageStats({
          error: errorMessageFromUnknown(error),
          filters,
        }),
      ),
    ),
  ),
)

const tokenUsageStatsFiltersWithValue = (
  filters: TokenUsageStatsFilters,
  field: TokenUsageStatsFilterKey,
  value: string,
): TokenUsageStatsFilters =>
  TokenUsageStatsFilters({
    ...filters,
    [field]: value,
  })

export const updateStats = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      RequestedLoadTokenUsageStats: () => [
        evo(model, {
          tokenUsageStats: stats =>
            TokenUsageStatsLoading({ filters: stats.filters }),
        }),
        [LoadTokenUsageStats({ filters: model.tokenUsageStats.filters })],
        Option.none(),
      ],
      SucceededLoadTokenUsageStats: ({
        analytics,
        filters,
        leaderboards,
        preference,
        response,
      }) => [
        evo(model, {
          tokenUsageStats: () =>
            TokenUsageStatsLoaded({
              analytics,
              filters,
              leaderboards,
              preference,
              response,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedLoadTokenUsageStats: ({ error, filters }) => [
        evo(model, {
          tokenUsageStats: () => TokenUsageStatsFailed({ error, filters }),
        }),
        [],
        Option.none(),
      ],
      UpdatedTokenUsageStatsFilter: ({ field, value }) => [
        evo(model, {
          tokenUsageStats: stats =>
            evo(stats, {
              filters: filters =>
                tokenUsageStatsFiltersWithValue(filters, field, value),
            }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )
