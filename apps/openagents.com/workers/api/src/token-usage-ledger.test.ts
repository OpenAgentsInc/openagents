import { Effect } from 'effect'
import { readFile } from 'node:fs/promises'
import { describe, expect, test } from 'vitest'

import {
  dayKeyInTimezone,
  isoTimestampAfterIso,
  utcStartOfDayIsoTimestamp,
} from './runtime-primitives'
import {
  type TokenUsageHistoryFilters,
  type TokenUsageIngestResult,
  TokenUsageLedger,
  type TokenUsageLedgerFilters,
  type TokenUsageLedgerRuntime,
  type TokenUsageLeaderboardFilters,
} from './token-usage-ledger'

const nowIso = '2026-06-08T12:00:00.000Z'

type StoredTokenUsageRow = Record<string, string | number | null>

type TokenUsageLedgerMemory = Readonly<{
  preferences: Array<StoredTokenUsageRow>
  rows: Array<StoredTokenUsageRow>
}>

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

const makeResult = <T>(
  results: Array<T> = [],
  success = true,
): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: success as true,
})

const asNumber = (value: string | number | null | undefined): number =>
  typeof value === 'number' ? value : 0

const servedTokensFromRow = (row: StoredTokenUsageRow): number => {
  const splitTokens = asNumber(row.input_tokens) + asNumber(row.output_tokens)

  return splitTokens > 0 ? splitTokens : asNumber(row.total_tokens)
}

type TokenUsageCountRow = {
  cache_read_tokens: number
  cache_write_1h_tokens: number
  cache_write_5m_tokens: number
  input_tokens: number
  output_tokens: number
  reasoning_tokens: number
  total_tokens: number
  usage_events: number
}

const countRow = (
  rows: ReadonlyArray<StoredTokenUsageRow>,
): TokenUsageCountRow => ({
  cache_read_tokens: rows.reduce(
    (total, row) => total + asNumber(row.cache_read_tokens),
    0,
  ),
  cache_write_1h_tokens: rows.reduce(
    (total, row) => total + asNumber(row.cache_write_1h_tokens),
    0,
  ),
  cache_write_5m_tokens: rows.reduce(
    (total, row) => total + asNumber(row.cache_write_5m_tokens),
    0,
  ),
  input_tokens: rows.reduce(
    (total, row) => total + asNumber(row.input_tokens),
    0,
  ),
  output_tokens: rows.reduce(
    (total, row) => total + asNumber(row.output_tokens),
    0,
  ),
  reasoning_tokens: rows.reduce(
    (total, row) => total + asNumber(row.reasoning_tokens),
    0,
  ),
  total_tokens: rows.reduce(
    (total, row) => total + asNumber(row.total_tokens),
    0,
  ),
  usage_events: rows.length,
})

const filteredRows = (
  rows: ReadonlyArray<StoredTokenUsageRow>,
  query: string,
  values: ReadonlyArray<unknown>,
  preferences: ReadonlyArray<StoredTokenUsageRow> = [],
): ReadonlyArray<StoredTokenUsageRow> => {
  let valueIndex = 0
  const hasSince = query.includes('observed_at >= ?')
  const hasUntil = query.includes('observed_at <= ?')
  const hasBefore = query.includes('observed_at < ?')
  const since = hasSince ? String(values[valueIndex++]) : undefined
  const until = hasUntil ? String(values[valueIndex++]) : undefined
  const before = hasBefore ? String(values[valueIndex++]) : undefined
  const textFilters = [
    ['provider = ?', 'provider'],
    ['model = ?', 'model'],
    ['producer_system = ?', 'producer_system'],
    ['source_route = ?', 'source_route'],
    ['actor_user_id = ?', 'actor_user_id'],
    ['actor_team_id = ?', 'actor_team_id'],
    ['account_ref = ?', 'account_ref'],
    ['usage_truth = ?', 'usage_truth'],
  ] as const
  const activeTextFilters = textFilters.flatMap(([needle, column]) =>
    query.includes(needle)
      ? [{ column, value: String(values[valueIndex++]) }]
      : [],
  )
  const leaderboardEligible = query.includes('leaderboard_eligible = ?')
    ? Number(values[valueIndex++])
    : undefined
  const privacyOptOut = query.includes('privacy_opt_out = ?')
    ? Number(values[valueIndex++])
    : undefined
  const requireLeaderboardEligible = query.includes(
    'AND leaderboard_eligible = 1',
  )
  const requirePrivacyIncluded = query.includes('AND privacy_opt_out = 0')
  const nonNullColumns = [
    'anonymized_source_ref',
    'repository_ref',
    'run_ref',
    'session_ref',
    'task_ref',
  ].filter(column => query.includes(`${column} IS NOT NULL`))

  return rows.filter(row => {
    const observedAt = String(row.observed_at)

    return (
      (since === undefined || observedAt >= since) &&
      (until === undefined || observedAt <= until) &&
      (before === undefined || observedAt < before) &&
      activeTextFilters.every(
        filter => String(row[filter.column] ?? '') === filter.value,
      ) &&
      (leaderboardEligible === undefined ||
        asNumber(row.leaderboard_eligible) === leaderboardEligible) &&
      (privacyOptOut === undefined ||
        asNumber(row.privacy_opt_out) === privacyOptOut) &&
      (!requireLeaderboardEligible ||
        asNumber(row.leaderboard_eligible) === 1) &&
      (!requirePrivacyIncluded || asNumber(row.privacy_opt_out) === 0) &&
      nonNullColumns.every(column => row[column] !== null) &&
      !preferences.some(
        preference =>
          ((preference.subject_kind === 'user' &&
            preference.subject_ref === row.actor_user_id) ||
            (preference.subject_kind === 'team' &&
              preference.subject_ref === row.actor_team_id) ||
            (preference.subject_kind === 'account' &&
              preference.subject_ref === row.account_ref)) &&
          (preference.leaderboard_participation === 'opted_out' ||
            preference.leaderboard_visibility === 'private') &&
          query.includes('token_usage_leaderboard_preferences'),
      )
    )
  })
}

const groupRows = (
  rows: ReadonlyArray<StoredTokenUsageRow>,
  keyFor: (row: StoredTokenUsageRow) => string,
  labelFor: (row: StoredTokenUsageRow) => string,
): Array<Record<string, string | number>> =>
  Object.entries(
    rows.reduce<Record<string, Array<StoredTokenUsageRow>>>((groups, row) => {
      const key = keyFor(row)
      groups[key] = [...(groups[key] ?? []), row]

      return groups
    }, {}),
  )
    .map(
      ([key, groupedRows]): Record<string, string | number> => ({
        key,
        label: labelFor(groupedRows[0] ?? {}),
        ...countRow(groupedRows),
      }),
    )
    .sort(
      (left, right) =>
        Number(right.total_tokens ?? 0) - Number(left.total_tokens ?? 0),
    )

const actorGroupRows = (
  rows: ReadonlyArray<StoredTokenUsageRow>,
): Array<Record<string, string | number | null>> =>
  Object.values(
    rows.reduce<Record<string, Array<StoredTokenUsageRow>>>((groups, row) => {
      const key = [
        row.actor_user_id ?? 'anonymous',
        row.actor_team_id ?? 'none',
        row.account_ref ?? 'none',
      ].join(':')
      groups[key] = [...(groups[key] ?? []), row]

      return groups
    }, {}),
  )
    .map(
      (groupedRows): Record<string, string | number | null> => ({
        account_ref: groupedRows[0]?.account_ref ?? null,
        actor_team_id: groupedRows[0]?.actor_team_id ?? null,
        actor_user_id: groupedRows[0]?.actor_user_id ?? null,
        ...countRow(groupedRows),
      }),
    )
    .sort(
      (left, right) =>
        Number(right.total_tokens ?? 0) - Number(left.total_tokens ?? 0),
    )

const makeMemoryD1 = (
  store: TokenUsageLedgerMemory = { preferences: [], rows: [] },
  options: Readonly<{
    dailyRollups?:
      | ReadonlyArray<
          Readonly<{
            day: string
            timezone: string
            tokens_served: number
          }>
        >
      | undefined
    failFirstTokenUsageEventInsertWithSuccessFalse?: boolean | undefined
    queryLog?: Array<string> | undefined
  }> = {},
): D1Database & TokenUsageLedgerMemory => {
  let failedTokenUsageInsert = false
  const prepare = (query: string) => {
    options.queryLog?.push(query)
    let values: ReadonlyArray<unknown> = []

    function raw<T = unknown[]>(options: {
      columnNames: true
    }): Promise<[Array<string>, ...Array<T>]>
    function raw<T = unknown[]>(options?: {
      columnNames?: false
    }): Promise<Array<T>>
    function raw<T = unknown[]>(options?: {
      columnNames?: boolean
    }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
      return options?.columnNames === true
        ? Promise.resolve([[]])
        : Promise.resolve([])
    }

    const statement: D1PreparedStatement = {
      all: <T = Record<string, unknown>>() => {
        if (
          query.includes('FROM public_khala_tokens_served_daily_rollups')
        ) {
          const timezone = String(values[0])
          const startDay = String(values[1])
          const endDay = String(values[2])
          if (options.dailyRollups !== undefined) {
            return Promise.resolve(
              makeResult<T>(
                options.dailyRollups
                  .filter(
                    rollup =>
                      rollup.timezone === timezone &&
                      rollup.day >= startDay &&
                      rollup.day <= endDay,
                  )
                  .sort((left, right) => left.day.localeCompare(right.day))
                  .map(rollup => ({
                    day: rollup.day,
                    tokens_served: rollup.tokens_served,
                  })) as Array<T>,
              ),
            )
          }
          const grouped = store.rows.reduce((days, row) => {
            const day = dayKeyInTimezone(String(row.observed_at), timezone)
            if (day === undefined || day < startDay || day > endDay) {
              return days
            }

            days.set(
              day,
              (days.get(day) ?? 0) + servedTokensFromRow(row),
            )
            return days
          }, new Map<string, number>())

          return Promise.resolve(
            makeResult<T>(
              [...grouped.entries()]
                .sort(([left], [right]) => left.localeCompare(right))
                .map(([day, tokens_served]) => ({
                  day,
                  tokens_served,
                })) as Array<T>,
            ),
          )
        }

        if (
          query.includes(
            'FROM public_khala_tokens_served_model_daily_rollups',
          )
        ) {
          const hasDayBounds = query.includes('WHERE day >= ?')
          const startDay = hasDayBounds ? String(values[0]) : undefined
          const endDay = hasDayBounds ? String(values[1]) : undefined
          const rollups = store.rows.reduce(
            (groups, row) => {
              const day = String(row.observed_at).slice(0, 10)
              if (
                (startDay !== undefined && day < startDay) ||
                (endDay !== undefined && day > endDay)
              ) {
                return groups
              }
              const provider = String(row.provider ?? '')
              const model = String(row.model ?? '')
              const key = `${provider}\u0000${model}`
              const previous = groups.get(key) ?? {
                model,
                provider,
                tokens: 0,
                usage_events: 0,
              }
              groups.set(key, {
                ...previous,
                tokens: previous.tokens + servedTokensFromRow(row),
                usage_events: previous.usage_events + 1,
              })

              return groups
            },
            new Map<
              string,
              {
                model: string
                provider: string
                tokens: number
                usage_events: number
              }
            >(),
          )

          return Promise.resolve(
            makeResult<T>(
              [...rollups.values()].map(row => ({
                model: row.model === '' ? null : row.model,
                provider: row.provider === '' ? null : row.provider,
                tokens: row.tokens,
                usage_events: row.usage_events,
              })) as Array<T>,
            ),
          )
        }

        if (
          query.includes(
            'FROM public_khala_tokens_served_channel_daily_rollups',
          )
        ) {
          const hasDayBounds = query.includes('WHERE day >= ?')
          const startDay = hasDayBounds ? String(values[0]) : undefined
          const endDay = hasDayBounds ? String(values[1]) : undefined
          const rollups = store.rows.reduce(
            (groups, row) => {
              const day = String(row.observed_at).slice(0, 10)
              if (
                (startDay !== undefined && day < startDay) ||
                (endDay !== undefined && day > endDay)
              ) {
                return groups
              }
              const demandChannel =
                String(row.demand_channel ?? '') === 'direct_local'
                  ? 'direct_local'
                  : 'khala_api'
              const previous = groups.get(demandChannel) ?? {
                demand_channel: demandChannel,
                tokens: 0,
                usage_events: 0,
              }
              groups.set(demandChannel, {
                ...previous,
                tokens: previous.tokens + servedTokensFromRow(row),
                usage_events: previous.usage_events + 1,
              })

              return groups
            },
            new Map<
              string,
              {
                demand_channel: string
                tokens: number
                usage_events: number
              }
            >(),
          )

          return Promise.resolve(
            makeResult<T>([...rollups.values()] as Array<T>),
          )
        }

        const rows = filteredRows(store.rows, query, values, store.preferences)

        if (query.includes('GROUP BY COALESCE(provider')) {
          return Promise.resolve(
            makeResult<T>(
              groupRows(
                rows,
                row => `${row.provider ?? 'unknown'}:${row.model ?? 'unknown'}`,
                row =>
                  `${row.provider ?? 'unknown'} / ${row.model ?? 'unknown'}`,
              ) as Array<T>,
            ),
          )
        }

        if (query.includes('GROUP BY producer_system, source_route')) {
          return Promise.resolve(
            makeResult<T>(
              groupRows(
                rows,
                row => `${row.producer_system}:${row.source_route}`,
                row => `${row.producer_system} / ${row.source_route}`,
              ) as Array<T>,
            ),
          )
        }

        if (query.includes('GROUP BY actor_user_id')) {
          return Promise.resolve(
            makeResult<T>(actorGroupRows(rows) as Array<T>),
          )
        }

        if (query.includes('GROUP BY actor_team_id')) {
          return Promise.resolve(
            makeResult<T>(actorGroupRows(rows) as Array<T>),
          )
        }

        if (query.includes('GROUP BY usage_truth')) {
          return Promise.resolve(
            makeResult<T>(
              groupRows(
                rows,
                row => String(row.usage_truth ?? 'unknown'),
                row => String(row.usage_truth ?? 'unknown'),
              ) as Array<T>,
            ),
          )
        }

        for (const column of [
          'anonymized_source_ref',
          'repository_ref',
          'run_ref',
          'session_ref',
          'task_ref',
        ]) {
          if (query.includes(`GROUP BY ${column}`)) {
            const kind =
              column === 'anonymized_source_ref'
                ? 'anonymized'
                : column.replace('_ref', '')

            return Promise.resolve(
              makeResult<T>(
                groupRows(
                  rows,
                  row => `${kind}:${row[column]}`,
                  row => `${kind} / ${row[column]}`,
                ) as Array<T>,
              ),
            )
          }
        }

        if (query.includes('ORDER BY observed_at DESC')) {
          return Promise.resolve(
            makeResult<T>(
              [...rows].sort((left, right) =>
                String(right.observed_at).localeCompare(
                  String(left.observed_at),
                ),
              ) as Array<T>,
            ),
          )
        }

        if (
          query.includes('SELECT observed_at, input_tokens, output_tokens') &&
          query.includes('ORDER BY observed_at ASC')
        ) {
          return Promise.resolve(
            makeResult<T>(
              [...rows]
                .sort((left, right) =>
                  String(left.observed_at).localeCompare(
                    String(right.observed_at),
                  ),
                )
                .map(row => ({
                  observed_at: row.observed_at,
                  input_tokens: row.input_tokens,
                  output_tokens: row.output_tokens,
                })) as Array<T>,
            ),
          )
        }

        if (
          query.includes('WITH bounded_token_usage_events AS') &&
          query.includes('CASE') &&
          query.includes('GROUP BY day')
        ) {
          const byDay = new Map<string, number>()
          const dayWindows: Array<{
            day: string
            endIso: string
            startIso: string
          }> = []

          for (let index = 2; index + 2 < values.length; index += 3) {
            dayWindows.push({
              day: String(values[index + 2]),
              endIso: String(values[index + 1]),
              startIso: String(values[index]),
            })
          }

          for (const row of rows) {
            const observedAt = String(row.observed_at)
            const dayWindow = dayWindows.find(
              window =>
                observedAt >= window.startIso && observedAt < window.endIso,
            )
            if (dayWindow === undefined) {
              continue
            }

            const served = servedTokensFromRow(row)
            byDay.set(dayWindow.day, (byDay.get(dayWindow.day) ?? 0) + served)
          }

          const series = [...byDay.entries()]
            .map(([day, tokens]) => ({ day, tokens }))
            .sort((left, right) => left.day.localeCompare(right.day))

          return Promise.resolve(makeResult<T>(series as Array<T>))
        }

        // Public tokens-served history: per-day SUM(input + output), ordered
        // ascending by UTC day. The `since` lower bound (when present) was
        // applied by filteredRows via the bound `observed_at >= ?` value.
        if (
          query.includes('date(observed_at) AS day') &&
          query.includes('GROUP BY day')
        ) {
          const byDay = new Map<string, number>()
          for (const row of rows) {
            const day = String(row.observed_at).slice(0, 10)
            const served = servedTokensFromRow(row)
            byDay.set(day, (byDay.get(day) ?? 0) + served)
          }

          const series = [...byDay.entries()]
            .map(([day, tokens]) => ({ day, tokens }))
            .sort((left, right) => left.day.localeCompare(right.day))

          return Promise.resolve(makeResult<T>(series as Array<T>))
        }

        if (
          query.includes('GROUP BY provider, model') &&
          query.includes('AS tokens') &&
          query.includes('AS usage_events')
        ) {
          const byProviderModel = new Map<
            string,
            {
              model: string | null
              provider: string | null
              tokens: number
              usage_events: number
            }
          >()

          for (const row of rows) {
            const key = `${row.provider ?? 'unknown'}:${row.model ?? 'unknown'}`
            const previous = byProviderModel.get(key) ?? {
              model: row.model as string | null,
              provider: row.provider as string | null,
              tokens: 0,
              usage_events: 0,
            }
            byProviderModel.set(key, {
              ...previous,
              tokens: previous.tokens + servedTokensFromRow(row),
              usage_events: previous.usage_events + 1,
            })
          }

          return Promise.resolve(
            makeResult<T>([...byProviderModel.values()] as Array<T>),
          )
        }

        if (
          query.includes('GROUP BY demand_channel') &&
          query.includes('AS tokens') &&
          query.includes('AS usage_events')
        ) {
          const byChannel = new Map<
            string,
            {
              demand_channel: string | null
              tokens: number
              usage_events: number
            }
          >()

          for (const row of rows) {
            const channel = String(row.demand_channel ?? 'khala_api')
            const previous = byChannel.get(channel) ?? {
              demand_channel: channel,
              tokens: 0,
              usage_events: 0,
            }
            byChannel.set(channel, {
              demand_channel: channel,
              tokens: previous.tokens + servedTokensFromRow(row),
              usage_events: previous.usage_events + 1,
            })
          }

          return Promise.resolve(
            makeResult<T>([...byChannel.values()] as Array<T>),
          )
        }

        if (
          query.includes('GROUP BY demand_kind, demand_source, demand_client') &&
          query.includes('AS tokens') &&
          query.includes('AS usage_events')
        ) {
          const byDemand = new Map<
            string,
            {
              demand_client: string | null
              demand_kind: string | null
              demand_source: string | null
              tokens: number
              usage_events: number
            }
          >()

          for (const row of rows) {
            const demandKind = String(row.demand_kind ?? 'unlabeled')
            const demandSource = String(row.demand_source ?? 'unknown')
            const demandClient = String(row.demand_client ?? 'unknown')
            const key = `${demandKind}:${demandSource}:${demandClient}`
            const previous = byDemand.get(key) ?? {
              demand_client: demandClient,
              demand_kind: demandKind,
              demand_source: demandSource,
              tokens: 0,
              usage_events: 0,
            }
            byDemand.set(key, {
              ...previous,
              tokens: previous.tokens + servedTokensFromRow(row),
              usage_events: previous.usage_events + 1,
            })
          }

          return Promise.resolve(
            makeResult<T>([...byDemand.values()] as Array<T>),
          )
        }

        return Promise.resolve(makeResult<T>())
      },
      bind: (...nextValues: ReadonlyArray<unknown>) => {
        values = nextValues

        return statement
      },
      first: <T = Record<string, unknown>>() => {
        if (
          query.includes('FROM token_usage_leaderboard_preferences') &&
          query.includes('subject_kind = ?')
        ) {
          const row = store.preferences.find(
            candidate =>
              candidate.subject_kind === values[0] &&
              candidate.subject_ref === values[1],
          )

          return Promise.resolve(row === undefined ? null : (row as T))
        }

        if (query.includes('WHERE idempotency_key = ? OR id = ?')) {
          const row = store.rows.find(
            candidate =>
              candidate.idempotency_key === values[0] ||
              candidate.id === values[1],
          )

          return Promise.resolve(row === undefined ? null : (row as T))
        }

        if (query.includes('AS tokens_served')) {
          const rows = filteredRows(store.rows, query, values, store.preferences)

          return Promise.resolve({
            tokens_served: rows.reduce(
              (total, row) => total + servedTokensFromRow(row),
              0,
            ),
          } as T)
        }

        if (query.includes('MIN(observed_at) AS first_observed_at')) {
          const rows = filteredRows(store.rows, query, values, store.preferences)
          const first = [...rows]
            .map(row => String(row.observed_at))
            .sort((left, right) => left.localeCompare(right))[0]

          return Promise.resolve({
            first_observed_at: first ?? null,
          } as T)
        }

        if (
          query.includes(' AS day') &&
          query.includes(' AS tokens') &&
          query.includes('COUNT(*) AS usage_events')
        ) {
          const rows = filteredRows(store.rows, query, values, store.preferences)
          const day = query.match(/SELECT\s+'([^']+)'\s+AS day/)?.[1] ?? null

          return Promise.resolve({
            day,
            tokens: rows.reduce(
              (total, row) => total + servedTokensFromRow(row),
              0,
            ),
            usage_events: rows.length,
          } as T)
        }

        if (query.includes('COUNT(*) AS usage_events')) {
          return Promise.resolve(
            countRow(
              filteredRows(store.rows, query, values, store.preferences),
            ) as T,
          )
        }

        return Promise.resolve(null)
      },
      raw,
      run: <T = Record<string, unknown>>() => {
        if (query.includes('INSERT INTO token_usage_leaderboard_preferences')) {
          const row = {
            leaderboard_participation: values[2] as string,
            leaderboard_visibility: values[3] as string,
            subject_kind: values[0] as string,
            subject_ref: values[1] as string,
            updated_at: values[4] as string,
            updated_by_user_id: values[5] as string | null,
          }
          const existingIndex = store.preferences.findIndex(
            candidate =>
              candidate.subject_kind === row.subject_kind &&
              candidate.subject_ref === row.subject_ref,
          )

          if (existingIndex < 0) {
            store.preferences.push(row)
          } else {
            store.preferences[existingIndex] = row
          }
        }

        if (query.includes('INSERT INTO token_usage_events')) {
          const id = String(values[0])
          const idempotencyKey = String(values[1])

          if (
            options.failFirstTokenUsageEventInsertWithSuccessFalse === true &&
            !failedTokenUsageInsert
          ) {
            failedTokenUsageInsert = true
            return Promise.resolve(makeResult<T>([], false))
          }

          if (
            store.rows.some(
              row => row.id === id || row.idempotency_key === idempotencyKey,
            )
          ) {
            throw new Error('UNIQUE constraint failed: token_usage_events.id')
          }

          store.rows.push({
            account_ref: values[8] as string | null,
            actor_team_id: values[7] as string | null,
            actor_user_id: values[6] as string | null,
            anonymized_source_ref: values[9] as string | null,
            backend_profile: values[16] as string | null,
            cache_read_tokens: values[20] as number,
            cache_write_1h_tokens: values[22] as number,
            cache_write_5m_tokens: values[21] as number,
            cost_amount: values[25] as number | null,
            currency: values[26] as string | null,
            demand_channel: values[27] as string,
            demand_client: values[30] as string | null,
            demand_kind: values[28] as string,
            demand_source: values[29] as string | null,
            id,
            idempotency_key: idempotencyKey,
            ingested_at: values[3] as string,
            input_tokens: values[17] as number,
            leaderboard_eligible: values[31] as number,
            model: values[15] as string | null,
            observed_at: values[2] as string,
            output_tokens: values[18] as number,
            privacy_opt_out: values[32] as number,
            producer_system: values[4] as string,
            provider: values[14] as string | null,
            reasoning_tokens: values[19] as number,
            repository_ref: values[13] as string | null,
            run_ref: values[10] as string | null,
            safe_metadata_json: values[33] as string,
            session_ref: values[11] as string | null,
            source_route: values[5] as string,
            task_ref: values[12] as string | null,
            total_tokens: values[23] as number,
            usage_truth: values[24] as string,
          })
        }

        return Promise.resolve(makeResult<T>())
      },
    }

    return statement
  }
  const db: D1Database & TokenUsageLedgerMemory = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare,
    preferences: store.preferences,
    rows: store.rows,
    withSession: () =>
      ({
        batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare,
      }) satisfies D1DatabaseSession,
  }

  return db
}

const runtime: TokenUsageLedgerRuntime = {
  isoTimestampAfterIso,
  nowIso: () => nowIso,
  utcStartOfDayIsoTimestamp,
}

const runLedger = <A>(
  db: D1Database,
  effect: Effect.Effect<A, unknown, TokenUsageLedger>,
): Promise<A> =>
  Effect.runPromise(
    effect.pipe(Effect.provide(TokenUsageLedger.live(db, runtime))),
  )

const ingest = (
  body: unknown,
): Effect.Effect<TokenUsageIngestResult, never, TokenUsageLedger> =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.ingestEvent(body).pipe(Effect.orDie),
  )

const aggregate = (filters?: TokenUsageLedgerFilters) =>
  Effect.flatMap(TokenUsageLedger, ledger => ledger.readAggregates(filters))

const tokensServedHistory = (filters?: TokenUsageHistoryFilters) =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.readPublicTokensServedHistory(filters),
  )

const tokensServedAggregate = () =>
  Effect.flatMap(TokenUsageLedger, ledger => ledger.readPublicTokensServed())

const tokensServedModelMix = (filters?: TokenUsageLeaderboardFilters) =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.readPublicTokensServedModelMix(filters),
  )

const tokensServedChannelMix = (filters?: TokenUsageLeaderboardFilters) =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.readPublicTokensServedChannelMix(filters),
  )

const leaderboards = () =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.readLeaderboards({ now: nowIso, window: '7d' }),
  )

const updatePreference = (
  body: unknown,
): Effect.Effect<unknown, never, TokenUsageLedger> =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger
      .updateLeaderboardPreference(
        {
          actorUserId: 'user_chris',
          subjectKind: 'user',
          subjectRef: 'user_chris',
        },
        body,
      )
      .pipe(Effect.orDie),
  )

const validProbeEvent = {
  schemaVersion: 'openagents.token_usage_event.v1',
  actor: {
    teamId: 'team_openagents_core',
    userId: 'user_chris',
  },
  eventId: 'token_event_probe_1',
  idempotencyKey: 'probe:event:1',
  model: 'gemini-2.5-pro',
  observedAt: '2026-06-08T11:59:00.000Z',
  producerSystem: 'probe',
  provider: 'google_gemini',
  safeMetadata: {
    providerRequestStatus: 'succeeded',
  },
  sourceRefs: {
    anonymizedSourceRef: 'probe-session-hash:abc123',
    runRef: 'probe-run:artanis-gepa-1',
  },
  sourceRoute: 'probe_direct_provider',
  tokenCounts: {
    cacheReadTokens: 25,
    cacheWrite1hTokens: 0,
    cacheWrite5mTokens: 0,
    inputTokens: 100,
    outputTokens: 40,
    reasoningTokens: 15,
    totalTokens: 180,
  },
  usageTruth: 'exact',
}

describe('token usage ledger', () => {
  test('migration creates canonical event ledger and aggregate indexes', async () => {
    const migration = await readFile(
      new URL('../migrations/0137_token_usage_events.sql', import.meta.url),
      'utf8',
    )

    expect(migration).toContain('CREATE TABLE IF NOT EXISTS token_usage_events')
    expect(migration).toContain('idempotency_key TEXT NOT NULL UNIQUE')
    expect(migration).toContain('idx_token_usage_events_provider_model')
    expect(migration).toContain('idx_token_usage_events_leaderboard')

    const preferenceMigration = await readFile(
      new URL(
        '../migrations/0138_token_usage_leaderboard_preferences.sql',
        import.meta.url,
      ),
      'utf8',
    )

    expect(preferenceMigration).toContain(
      'CREATE TABLE IF NOT EXISTS token_usage_leaderboard_preferences',
    )
    expect(preferenceMigration).toContain(
      "leaderboard_participation IN ('eligible', 'opted_out')",
    )

    const demandMigration = await readFile(
      new URL(
        '../migrations/0232_token_usage_demand_attribution.sql',
        import.meta.url,
      ),
      'utf8',
    )
    expect(demandMigration).toContain('ADD COLUMN demand_kind')
    expect(demandMigration).toContain('ADD COLUMN demand_source')
    expect(demandMigration).toContain('ADD COLUMN demand_client')

    const demandChannelMigration = await readFile(
      new URL(
        '../migrations/0262_token_usage_demand_channel.sql',
        import.meta.url,
      ),
      'utf8',
    )
    expect(demandChannelMigration).toContain('ADD COLUMN demand_channel')
    expect(demandChannelMigration).toContain("'direct_local'")
    expect(demandChannelMigration).toContain(
      'idx_token_usage_events_demand_channel',
    )

    const publicStatsMixRollupMigration = await readFile(
      new URL(
        '../migrations/0265_public_khala_tokens_served_mix_rollups.sql',
        import.meta.url,
      ),
      'utf8',
    )
    expect(publicStatsMixRollupMigration).toContain(
      'public_khala_tokens_served_model_daily_rollups',
    )
    expect(publicStatsMixRollupMigration).toContain(
      'public_khala_tokens_served_channel_daily_rollups',
    )

    const publicStatsTotalFallbackMigration = await readFile(
      new URL(
        '../migrations/0267_public_khala_tokens_served_total_fallback.sql',
        import.meta.url,
      ),
      'utf8',
    )
    expect(publicStatsTotalFallbackMigration).toContain(
      'idx_token_usage_events_public_observed_tokens_total',
    )
    expect(publicStatsTotalFallbackMigration).toContain(
      'ELSE COALESCE(total_tokens, 0)',
    )
    expect(publicStatsTotalFallbackMigration).toContain(
      'DELETE FROM public_khala_tokens_served_daily_rollups',
    )
  })

  test('stores one redacted Probe event across repeated idempotent submissions', async () => {
    const db = makeMemoryD1()
    const first = await runLedger(db, ingest(validProbeEvent))
    const second = await runLedger(db, ingest(validProbeEvent))

    expect(first.inserted).toBe(true)
    expect(second.inserted).toBe(false)
    expect(db.rows).toHaveLength(1)
    expect(db.rows[0]).toMatchObject({
      id: 'token_event_probe_1',
      idempotency_key: 'probe:event:1',
      demand_channel: 'khala_api',
      producer_system: 'probe',
      source_route: 'probe_direct_provider',
      total_tokens: 180,
    })
    expect(JSON.stringify(db.rows)).not.toContain('prompt')
    expect(JSON.stringify(db.rows)).not.toContain('completion')
  })

  test('persists internal_stress demand kind from typed ingest text (#6318 slice)', async () => {
    const db = makeMemoryD1()
    await runLedger(
      db,
      ingest({
        ...validProbeEvent,
        demand: {
          demandClient: 'stress-harness',
          demandKind: 'internal_stress',
          demandSource: 'glm-saturation',
        },
        eventId: 'token_event_internal_stress',
        idempotencyKey: 'probe:event:internal-stress',
      }),
    )

    expect(db.rows).toHaveLength(1)
    expect(db.rows[0]).toMatchObject({
      demand_channel: 'khala_api',
      demand_client: 'stress-harness',
      demand_kind: 'internal_stress',
      demand_source: 'glm-saturation',
    })
  })

  test('persists explicit direct-local demand channel separately from demand kind', async () => {
    const db = makeMemoryD1()
    await runLedger(
      db,
      ingest({
        ...validProbeEvent,
        demand: {
          demandChannel: 'direct_local',
          demandClient: 'pylon',
          demandKind: 'own_capacity',
          demandSource: 'direct_local_codex',
        },
        eventId: 'token_event_direct_local_codex',
        idempotencyKey: 'pylon:codex-direct-local:1',
        producerSystem: 'pylon',
        provider: 'pylon-codex-direct-local',
        model: 'openagents/codex-direct-local',
        sourceRoute: 'pylon_codex_direct_local',
      }),
    )

    expect(db.rows).toHaveLength(1)
    expect(db.rows[0]).toMatchObject({
      demand_channel: 'direct_local',
      demand_client: 'pylon',
      demand_kind: 'own_capacity',
      demand_source: 'direct_local_codex',
      producer_system: 'pylon',
      source_route: 'pylon_codex_direct_local',
    })
  })

  test('treats D1 insert success=false as a storage failure, not a recorded event (#6317 closeout)', async () => {
    const store: TokenUsageLedgerMemory = { preferences: [], rows: [] }
    const db = makeMemoryD1(store, {
      failFirstTokenUsageEventInsertWithSuccessFalse: true,
    })
    const result = await Effect.runPromiseExit(
      Effect.flatMap(TokenUsageLedger, ledger =>
        ledger.ingestEvent(validProbeEvent),
      ).pipe(Effect.provide(TokenUsageLedger.live(db, runtime))),
    )

    expect(result._tag).toBe('Failure')
    expect(store.rows).toHaveLength(0)
  })

  test('public tokens-served scalar includes internal, own-capacity, and stress rows (#6358 regression)', async () => {
    const db = makeMemoryD1()
    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest({
          ...validProbeEvent,
          demand: {
            demandKind: 'external',
            demandSource: 'public-api',
          },
          eventId: 'token_event_public_external',
          idempotencyKey: 'probe:event:public-external',
          tokenCounts: {
            ...validProbeEvent.tokenCounts,
            inputTokens: 10,
            outputTokens: 5,
            totalTokens: 15,
          },
        })
        yield* ingest({
          ...validProbeEvent,
          demand: {
            demandKind: 'internal',
            demandSource: 'heartbeat',
          },
          eventId: 'token_event_public_internal',
          idempotencyKey: 'probe:event:public-internal',
          tokenCounts: {
            ...validProbeEvent.tokenCounts,
            inputTokens: 1_000,
            outputTokens: 1_000,
            totalTokens: 2_000,
          },
        })
        yield* ingest({
          ...validProbeEvent,
          demand: {
            demandKind: 'own_capacity',
            demandSource: 'khala_coding_delegation',
          },
          eventId: 'token_event_public_own_capacity',
          idempotencyKey: 'probe:event:public-own-capacity',
          tokenCounts: {
            ...validProbeEvent.tokenCounts,
            inputTokens: 20,
            outputTokens: 10,
            totalTokens: 30,
          },
        })
        yield* ingest({
          ...validProbeEvent,
          demand: {
            demandKind: 'internal_stress',
            demandSource: 'glm-saturation',
          },
          eventId: 'token_event_public_internal_stress',
          idempotencyKey: 'probe:event:public-internal-stress',
          tokenCounts: {
            ...validProbeEvent.tokenCounts,
            inputTokens: 7,
            outputTokens: 3,
            totalTokens: 10,
          },
        })
      }),
    )

    const aggregate = await runLedger(db, tokensServedAggregate())

    expect(aggregate.tokensServed).toBe(2_055)
  })

  test('rejects unsafe prompt, provider payload, private path, and bearer material', async () => {
    const db = makeMemoryD1()
    const unsafe = {
      ...validProbeEvent,
      eventId: 'token_event_probe_unsafe',
      idempotencyKey: 'probe:event:unsafe',
      rawPrompt: 'Bearer sk-live-should-not-persist from /Users/chris/private',
    }
    const result = await Effect.runPromise(
      Effect.flatMap(TokenUsageLedger, ledger =>
        ledger.ingestEvent(unsafe),
      ).pipe(
        Effect.match({
          onFailure: error => error._tag,
          onSuccess: () => 'success',
        }),
        Effect.provide(TokenUsageLedger.live(db, runtime)),
      ),
    )

    expect(result).toBe('TokenUsageLedgerUnsafePayload')
    expect(db.rows).toHaveLength(0)
  })

  test('accepts public GLM adapter refs without mistaking hydralisk for an sk secret', async () => {
    const db = makeMemoryD1()
    await runLedger(
      db,
      ingest({
        ...validProbeEvent,
        backendProfile: 'hydralisk-vllm-glm-5p2-reap-504b',
        eventId: 'token_event_hydralisk_glm',
        idempotencyKey: 'probe:event:hydralisk-glm',
        model: 'openagents/glm-5.2-reap-504b',
        provider: 'hydralisk-vllm-glm-5p2-reap-504b',
        safeMetadata: {
          replicaCapacityClass: 'spot',
          selectedReplicaRef: 'replica.hydralisk.glm_52_reap_504b.second',
          supplyLane: 'hydralisk',
        },
      }),
    )

    expect(db.rows).toHaveLength(1)
    expect(db.rows[0]).toMatchObject({
      backend_profile: 'hydralisk-vllm-glm-5p2-reap-504b',
      provider: 'hydralisk-vllm-glm-5p2-reap-504b',
    })
  })

  test('aggregates anonymous and identified events while preserving privacy flags', async () => {
    const db = makeMemoryD1()
    await runLedger(db, ingest(validProbeEvent))
    const { actor: _actor, ...anonymousProbeEvent } = validProbeEvent
    await runLedger(
      db,
      ingest({
        ...anonymousProbeEvent,
        eventId: 'token_event_probe_2',
        idempotencyKey: 'probe:event:2',
        observedAt: '2026-06-08T12:01:00.000Z',
        privacy: {
          leaderboardEligible: false,
          privacyOptOut: true,
        },
        sourceRefs: {
          anonymizedSourceRef: 'probe-session-hash:anonymous',
        },
        tokenCounts: {
          cacheReadTokens: 0,
          cacheWrite1hTokens: 0,
          cacheWrite5mTokens: 0,
          inputTokens: 10,
          outputTokens: 5,
          reasoningTokens: 0,
          totalTokens: 15,
        },
      }),
    )

    const result = await runLedger(db, aggregate())

    expect(result.usageEvents).toBe(2)
    expect(result.totals.totalTokens).toBe(195)
    expect(result.byActor).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          anonymous: false,
          userId: 'user_chris',
        }),
        expect.objectContaining({
          anonymous: true,
          userId: null,
        }),
      ]),
    )
    expect(result.byUsageTruth).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          key: 'exact',
          usageEvents: 2,
        }),
      ]),
    )
    expect(result.bySourceRef).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          key: 'anonymized:probe-session-hash:abc123',
        }),
        expect.objectContaining({
          key: 'anonymized:probe-session-hash:anonymous',
        }),
        expect.objectContaining({
          key: 'run:probe-run:artanis-gepa-1',
        }),
      ]),
    )
    expect(result.recentEvents[0]).toMatchObject({
      eventId: 'token_event_probe_2',
      privacy: {
        leaderboardEligible: false,
        privacyOptOut: true,
      },
    })

    const filtered = await runLedger(
      db,
      aggregate({
        leaderboardEligible: false,
        privacyOptOut: true,
        producerSystem: 'probe',
        sourceRoute: 'probe_direct_provider',
      }),
    )

    expect(filtered.usageEvents).toBe(1)
    expect(filtered.totals.totalTokens).toBe(15)
    expect(filtered.filters).toEqual({
      leaderboardEligible: false,
      privacyOptOut: true,
      producerSystem: 'probe',
      sourceRoute: 'probe_direct_provider',
    })
  })

  test('builds opt-out-aware leaderboards without removing global totals', async () => {
    const db = makeMemoryD1()
    await runLedger(db, ingest(validProbeEvent))
    await runLedger(
      db,
      ingest({
        ...validProbeEvent,
        actor: {
          teamId: 'team_openagents_core',
          userId: 'user_alex',
        },
        eventId: 'token_event_probe_alex',
        idempotencyKey: 'probe:event:alex',
        observedAt: '2026-06-08T12:02:00.000Z',
        sourceRefs: {
          repositoryRef: 'OpenAgentsInc/autopilot-omega',
          runRef: 'probe-run:scene',
        },
        tokenCounts: {
          cacheReadTokens: 0,
          cacheWrite1hTokens: 0,
          cacheWrite5mTokens: 0,
          inputTokens: 200,
          outputTokens: 50,
          reasoningTokens: 0,
          totalTokens: 250,
        },
      }),
    )
    await runLedger(
      db,
      updatePreference({
        leaderboardParticipation: 'opted_out',
        leaderboardVisibility: 'private',
      }),
    )

    const result = await runLedger(db, leaderboards())

    expect(result.globalTotals.totalTokens).toBe(430)
    expect(result.topUsers.map(row => row.userId)).toEqual(['user_alex'])
    expect(result.topTeams[0]).toMatchObject({
      teamId: 'team_openagents_core',
      tokenCounts: {
        totalTokens: 250,
      },
    })
    expect(result.topProviderModels[0]).toMatchObject({
      key: 'google_gemini:gemini-2.5-pro',
      tokenCounts: {
        totalTokens: 430,
      },
    })
    expect(result.topRuns[0]).toMatchObject({
      key: 'run:probe-run:scene',
      tokenCounts: {
        totalTokens: 250,
      },
    })
  })
})

describe('public tokens-served history', () => {
  const eventOnDay = (
    eventId: string,
    observedAt: string,
    inputTokens: number,
    outputTokens: number,
    demandKind?: 'external' | 'internal' | 'internal_stress' | 'own_capacity',
  ) => ({
    schemaVersion: 'openagents.token_usage_event.v1',
    actor: { userId: 'user_chris' },
    eventId,
    idempotencyKey: `history:${eventId}`,
    model: 'openagents/khala',
    observedAt,
    producerSystem: 'omega',
    provider: 'openagents',
    sourceRoute: 'omega_hosted_gemini',
    ...(demandKind === undefined
      ? {}
      : {
          demand: {
            demandKind,
            demandSource: 'history-test',
          },
        }),
    tokenCounts: {
      cacheReadTokens: 0,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens,
      outputTokens,
      reasoningTokens: 0,
      totalTokens: inputTokens + outputTokens,
    },
    usageTruth: 'exact',
  })

  const totalOnlyEventOnDay = (
    eventId: string,
    observedAt: string,
    totalTokens: number,
  ) => ({
    ...eventOnDay(eventId, observedAt, 0, 0),
    tokenCounts: {
      cacheReadTokens: 0,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: 0,
      outputTokens: 0,
      reasoningTokens: 0,
      totalTokens,
    },
  })

  test('returns correct per-day sums over a window from the ledger', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        // Two events on 2026-06-06 → that day sums to (100+40) + (10+5) = 155.
        yield* ingest(eventOnDay('e1', '2026-06-06T09:00:00.000Z', 100, 40))
        yield* ingest(eventOnDay('e2', '2026-06-06T20:30:00.000Z', 10, 5))
        // One event at 2026-06-07T01:00Z is still 2026-06-06 in Central time.
        yield* ingest(eventOnDay('e3', '2026-06-07T01:00:00.000Z', 40, 20))
        // One event on 2026-06-08 → 9.
        yield* ingest(eventOnDay('e4', '2026-06-08T11:00:00.000Z', 6, 3))
      }),
    )

    const history = await runLedger(db, tokensServedHistory({ window: '30d' }))

    expect(history.window).toBe('30d')
    expect(history.bucket).toBe('day')
    expect(history.timezone).toBe('America/Chicago')
    // Ascending by day; per-day input + output sums.
    expect(history.series).toEqual([
      { day: '2026-06-06', tokensServed: 215 },
      { day: '2026-06-08', tokensServed: 9 },
    ])
  })

  test('Central-time history is aggregated in D1 instead of raw-scanning rows', async () => {
    const queryLog: Array<string> = []
    const db = makeMemoryD1(
      { preferences: [], rows: [] },
      { queryLog },
    )

    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest(eventOnDay('central-1', '2026-06-07T01:00:00.000Z', 4, 6))
        yield* ingest(eventOnDay('central-2', '2026-06-07T13:00:00.000Z', 7, 8))
      }),
    )

    const history = await runLedger(db, tokensServedHistory({ window: '30d' }))

    expect(history.series).toEqual([
      { day: '2026-06-06', tokensServed: 10 },
      { day: '2026-06-07', tokensServed: 15 },
    ])
    expect(
      queryLog.some(
        query =>
          query.includes('SELECT observed_at, input_tokens, output_tokens') &&
          query.includes('ORDER BY observed_at ASC'),
      ),
    ).toBe(false)
    expect(
      queryLog.some(query =>
        query.includes('FROM public_khala_tokens_served_daily_rollups'),
      ),
    ).toBe(true)
  })

  test('Central-time history reads the current day live when the rollup is stale', async () => {
    const queryLog: Array<string> = []
    const db = makeMemoryD1(
      { preferences: [], rows: [] },
      {
        dailyRollups: [
          {
            day: '2026-06-07',
            timezone: 'America/Chicago',
            tokens_served: 15,
          },
          {
            day: '2026-06-08',
            timezone: 'America/Chicago',
            tokens_served: 1,
          },
        ],
        queryLog,
      },
    )

    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest(
          eventOnDay('previous-day', '2026-06-07T13:00:00.000Z', 7, 8),
        )
        yield* ingest(
          totalOnlyEventOnDay(
            'current-total-only',
            '2026-06-08T11:00:00.000Z',
            250,
          ),
        )
      }),
    )

    const history = await runLedger(db, tokensServedHistory({ window: '30d' }))

    expect(history.series).toEqual([
      { day: '2026-06-07', tokensServed: 15 },
      { day: '2026-06-08', tokensServed: 250 },
    ])
    expect(
      queryLog.some(
        query =>
          query.includes('SELECT') &&
          query.includes('COUNT(*) AS usage_events') &&
          query.includes('FROM token_usage_events'),
      ),
    ).toBe(true)
  })

  test('the window lower bound excludes older days', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        // Inside a 7d window (now = 2026-06-08T12:00Z, since = 2026-06-01).
        yield* ingest(eventOnDay('in1', '2026-06-07T10:00:00.000Z', 50, 50))
        // Older than 7d → must be excluded from the 7d window.
        yield* ingest(eventOnDay('old1', '2026-05-20T10:00:00.000Z', 999, 999))
      }),
    )

    const history = await runLedger(db, tokensServedHistory({ window: '7d' }))

    expect(history.series).toEqual([{ day: '2026-06-07', tokensServed: 100 }])
  })

  test('history includes internal, own-capacity, and stress rows', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest(
          eventOnDay(
            'history-external',
            '2026-06-07T10:00:00.000Z',
            50,
            50,
            'external',
          ),
        )
        yield* ingest(
          eventOnDay(
            'history-internal',
            '2026-06-07T11:00:00.000Z',
            1_000,
            1_000,
            'internal',
          ),
        )
        yield* ingest(
          eventOnDay(
            'history-own-capacity',
            '2026-06-07T12:00:00.000Z',
            20,
            10,
            'own_capacity',
          ),
        )
        yield* ingest(
          eventOnDay(
            'history-internal-stress',
            '2026-06-07T13:00:00.000Z',
            7,
            3,
            'internal_stress',
          ),
        )
      }),
    )

    const history = await runLedger(
      db,
      tokensServedHistory({ now: '2026-06-08T12:00:00.000Z', window: '7d' }),
    )

    expect(history.series).toEqual([
      { day: '2026-06-07', tokensServed: 2_140 },
    ])
  })

  test('history counts exact total-only Khala Code rows', async () => {
    const db = makeMemoryD1()
    const base = eventOnDay(
      'history-total-only-khala-code',
      '2026-06-07T13:00:00.000Z',
      0,
      0,
      'own_capacity',
    )

    await runLedger(
      db,
      ingest({
        ...base,
        demand: {
          demandChannel: 'direct_local',
          demandClient: 'khala_code_desktop',
          demandKind: 'own_capacity',
          demandSource: 'direct_local_codex',
        },
        model: 'openagents/codex-direct-local',
        producerSystem: 'pylon',
        provider: 'pylon-codex-direct-local',
        sourceRoute: 'pylon_codex_direct_local',
        tokenCounts: {
          ...base.tokenCounts,
          totalTokens: 8_333_893,
        },
      }),
    )

    const history = await runLedger(
      db,
      tokensServedHistory({ now: '2026-06-08T12:00:00.000Z', window: '7d' }),
    )

    expect(history.series).toEqual([
      { day: '2026-06-07', tokensServed: 8_333_893 },
    ])
  })

  test('can bucket history by America/Chicago local day across a UTC boundary', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest(
          eventOnDay('late-central', '2026-06-25T04:30:00.000Z', 6, 4),
        )
        yield* ingest(
          eventOnDay('next-central', '2026-06-25T05:30:00.000Z', 20, 5),
        )
      }),
    )

    const utcHistory = await runLedger(
      db,
      tokensServedHistory({
        now: '2026-06-26T12:00:00.000Z',
        timezone: 'UTC',
        window: '7d',
      }),
    )
    const chicagoHistory = await runLedger(
      db,
      tokensServedHistory({
        now: '2026-06-26T12:00:00.000Z',
        timezone: 'America/Chicago',
        window: '7d',
      }),
    )

    expect(utcHistory.series).toEqual([
      { day: '2026-06-25', tokensServed: 35 },
    ])
    expect(chicagoHistory.timezone).toBe('America/Chicago')
    expect(chicagoHistory.series).toEqual([
      { day: '2026-06-24', tokensServed: 10 },
      { day: '2026-06-25', tokensServed: 25 },
    ])
  })

  test('window=today starts at America/Chicago midnight by default', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest(
          eventOnDay('previous-central', '2026-06-25T04:30:00.000Z', 6, 4),
        )
        yield* ingest(
          eventOnDay('today-central', '2026-06-25T05:30:00.000Z', 20, 5),
        )
      }),
    )

    const history = await runLedger(
      db,
      tokensServedHistory({
        now: '2026-06-25T06:00:00.000Z',
        window: 'today',
      }),
    )

    expect(history.timezone).toBe('America/Chicago')
    expect(history.series).toEqual([
      { day: '2026-06-25', tokensServed: 25 },
    ])
  })

  test('empty ledger → empty series', async () => {
    const db = makeMemoryD1()
    const history = await runLedger(db, tokensServedHistory({ window: '30d' }))

    expect(history.series).toEqual([])
  })

  test('rejects an unsupported bucket', async () => {
    const db = makeMemoryD1()
    const exit = await Effect.runPromise(
      Effect.exit(
        tokensServedHistory({ bucket: 'hour', window: '30d' }).pipe(
          Effect.provide(TokenUsageLedger.live(db, runtime)),
        ),
      ),
    )

    expect(exit._tag).toBe('Failure')
  })
})

describe('public tokens-served model mix', () => {
  const eventForFamily = (
    eventId: string,
    input: Readonly<{
      demandKind?: 'external' | 'internal' | 'internal_stress' | 'own_capacity'
      model: string
      observedAt: string
      outputTokens: number
      provider: string
      inputTokens: number
      totalTokens?: number
    }>,
  ) => ({
    schemaVersion: 'openagents.token_usage_event.v1',
    actor: { userId: 'user_chris' },
    eventId,
    idempotencyKey: `model-mix:${eventId}`,
    model: input.model,
    observedAt: input.observedAt,
    producerSystem: 'omega',
    provider: input.provider,
    sourceRoute: 'omega_hosted_gemini',
    ...(input.demandKind === undefined
      ? {}
      : {
          demand: {
            demandKind: input.demandKind,
            demandSource: 'model-mix-test',
          },
        }),
    tokenCounts: {
      cacheReadTokens: 0,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: input.inputTokens,
      outputTokens: input.outputTokens,
      reasoningTokens: 0,
      totalTokens: input.totalTokens ?? input.inputTokens + input.outputTokens,
    },
    usageTruth: 'exact',
  })

  test('collapses raw provider and model ids into canonical public groups', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest(
          eventForFamily('glm-1', {
            inputTokens: 100,
            model: 'openagents/glm-5.2-reap-504b',
            observedAt: '2026-06-07T10:00:00.000Z',
            outputTokens: 50,
            provider: 'hydralisk-g4',
          }),
        )
        yield* ingest(
          eventForFamily('glm-2', {
            inputTokens: 30,
            model: 'z-ai/glm-4.5',
            observedAt: '2026-06-07T11:00:00.000Z',
            outputTokens: 20,
            provider: 'z.ai',
          }),
        )
        yield* ingest(
          eventForFamily('fireworks-1', {
            inputTokens: 40,
            model: 'accounts/fireworks/models/deepseek-v4-flash',
            observedAt: '2026-06-07T12:00:00.000Z',
            outputTokens: 10,
            provider: 'fireworks',
          }),
        )
        yield* ingest(
          eventForFamily('codex-1', {
            inputTokens: 20,
            model: 'openagents/pylon-codex',
            observedAt: '2026-06-07T13:00:00.000Z',
            outputTokens: 5,
            provider: 'pylon-codex-own-capacity',
          }),
        )
        yield* ingest(
          eventForFamily('gpt-oss-1', {
            inputTokens: 10,
            model: 'gpt-oss-120b',
            observedAt: '2026-06-07T14:00:00.000Z',
            outputTokens: 5,
            provider: 'openai-compatible',
          }),
        )
        yield* ingest(
          eventForFamily('gemini-1', {
            inputTokens: 7,
            model: 'gemini-2.5-pro',
            observedAt: '2026-06-07T15:00:00.000Z',
            outputTokens: 3,
            provider: 'google_vertex',
          }),
        )
      }),
    )

    const mix = await runLedger(
      db,
      tokensServedModelMix({ now: nowIso, window: '30d' }),
    )

    expect(mix).toEqual({
      window: '30d',
      totalTokens: 300,
      groups: [
        {
          family: 'glm',
          label: 'GLM family',
          pct: 66.666667,
          reqs: 2,
          tokens: 200,
        },
        {
          family: 'fireworks_deepseek',
          label: 'Fireworks DeepSeek',
          pct: 16.666667,
          reqs: 1,
          tokens: 50,
        },
        {
          family: 'pylon_codex',
          label: 'Pylon-Codex',
          pct: 8.333333,
          reqs: 1,
          tokens: 25,
        },
        {
          family: 'gpt_oss',
          label: 'GPT-OSS',
          pct: 5,
          reqs: 1,
          tokens: 15,
        },
        {
          family: 'gemini',
          label: 'Gemini',
          pct: 3.333333,
          reqs: 1,
          tokens: 10,
        },
      ],
    })
    expect(
      mix.groups.reduce((sum, group) => sum + group.pct, 0),
    ).toBeCloseTo(100, 6)
  })

  test('default 30d window excludes older rows but includes exact internal rows', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest(
          eventForFamily('inside', {
            inputTokens: 50,
            model: 'gpt-oss-20b',
            observedAt: '2026-06-07T10:00:00.000Z',
            outputTokens: 50,
            provider: 'openai-compatible',
          }),
        )
        yield* ingest(
          eventForFamily('internal', {
            demandKind: 'internal',
            inputTokens: 1_000,
            model: 'openagents/glm-5.2-reap-504b',
            observedAt: '2026-06-07T10:00:00.000Z',
            outputTokens: 1_000,
            provider: 'hydralisk-g4',
          }),
        )
        yield* ingest(
          eventForFamily('old', {
            inputTokens: 999,
            model: 'deepseek-chat',
            observedAt: '2026-05-01T10:00:00.000Z',
            outputTokens: 999,
            provider: 'deepseek',
          }),
        )
      }),
    )

    const mix = await runLedger(
      db,
      tokensServedModelMix({ now: nowIso, window: '30d' }),
    )

    expect(mix.groups).toEqual([
      {
        family: 'glm',
        label: 'GLM family',
        pct: 95.238095,
        reqs: 1,
        tokens: 2_000,
      },
      {
        family: 'gpt_oss',
        label: 'GPT-OSS',
        pct: 4.761905,
        reqs: 1,
        tokens: 100,
      },
    ])
  })

  test('model mix counts exact total-only Khala Code rows', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      ingest(
        eventForFamily('codex-total-only', {
          demandKind: 'own_capacity',
          inputTokens: 0,
          model: 'openagents/codex-direct-local',
          observedAt: '2026-06-07T13:00:00.000Z',
          outputTokens: 0,
          provider: 'pylon-codex-direct-local',
          totalTokens: 8_333_893,
        }),
      ),
    )

    const mix = await runLedger(
      db,
      tokensServedModelMix({ now: nowIso, window: '30d' }),
    )

    expect(mix).toEqual({
      groups: [
        {
          family: 'codex_direct',
          label: 'Codex (direct)',
          pct: 100,
          reqs: 1,
          tokens: 8_333_893,
        },
      ],
      totalTokens: 8_333_893,
      window: '30d',
    })
  })
})

describe('public tokens-served channel mix', () => {
  test('splits product-wide tokens by Khala API and direct-local channels', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        yield* ingest({
          ...validProbeEvent,
          eventId: 'token_event_khala_api_channel',
          idempotencyKey: 'probe:event:khala-api-channel',
          tokenCounts: {
            cacheReadTokens: 0,
            cacheWrite1hTokens: 0,
            cacheWrite5mTokens: 0,
            inputTokens: 100,
            outputTokens: 50,
            reasoningTokens: 0,
            totalTokens: 150,
          },
        })
        yield* ingest({
          ...validProbeEvent,
          demand: {
            demandChannel: 'direct_local',
            demandClient: 'pylon',
            demandKind: 'own_capacity',
            demandSource: 'direct_local_codex',
          },
          eventId: 'token_event_direct_local_channel',
          idempotencyKey: 'pylon:codex-direct-local:channel',
          model: 'openagents/codex-direct-local',
          producerSystem: 'pylon',
          provider: 'pylon-codex-direct-local',
          sourceRoute: 'pylon_codex_direct_local',
          tokenCounts: {
            cacheReadTokens: 0,
            cacheWrite1hTokens: 0,
            cacheWrite5mTokens: 0,
            inputTokens: 25,
            outputTokens: 25,
            reasoningTokens: 0,
            totalTokens: 50,
          },
        })
      }),
    )

    const mix = await runLedger(
      db,
      tokensServedChannelMix({ now: nowIso, window: '30d' }),
    )

    expect(mix).toEqual({
      window: '30d',
      totalTokens: 200,
      groups: [
        {
          channel: 'khala_api',
          label: 'Khala API',
          pct: 75,
          reqs: 1,
          tokens: 150,
        },
        {
          channel: 'direct_local',
          label: 'Direct local',
          pct: 25,
          reqs: 1,
          tokens: 50,
        },
      ],
    })
  })

  test('channel mix counts exact total-only direct-local Khala Code rows', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      ingest({
        ...validProbeEvent,
        demand: {
          demandChannel: 'direct_local',
          demandClient: 'khala_code_desktop',
          demandKind: 'own_capacity',
          demandSource: 'direct_local_codex',
        },
        eventId: 'token_event_direct_local_total_only',
        idempotencyKey: 'pylon:codex-direct-local:total-only',
        model: 'openagents/codex-direct-local',
        producerSystem: 'pylon',
        provider: 'pylon-codex-direct-local',
        sourceRoute: 'pylon_codex_direct_local',
        tokenCounts: {
          cacheReadTokens: 0,
          cacheWrite1hTokens: 0,
          cacheWrite5mTokens: 0,
          inputTokens: 0,
          outputTokens: 0,
          reasoningTokens: 0,
          totalTokens: 8_333_893,
        },
      }),
    )

    const mix = await runLedger(
      db,
      tokensServedChannelMix({ now: nowIso, window: '30d' }),
    )

    expect(mix).toEqual({
      groups: [
        {
          channel: 'direct_local',
          label: 'Direct local',
          pct: 100,
          reqs: 1,
          tokens: 8_333_893,
        },
      ],
      totalTokens: 8_333_893,
      window: '30d',
    })
  })
})
