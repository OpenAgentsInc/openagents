import { Effect } from 'effect'
import { readFile } from 'node:fs/promises'
import { describe, expect, test } from 'vitest'

import {
  type TokenUsageHistoryFilters,
  type TokenUsageIngestResult,
  TokenUsageLedger,
  type TokenUsageLedgerFilters,
  type TokenUsageLedgerRuntime,
} from './token-usage-ledger'
import {
  isoTimestampAfterIso,
  utcStartOfDayIsoTimestamp,
} from './runtime-primitives'

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

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const asNumber = (value: string | number | null | undefined): number =>
  typeof value === 'number' ? value : 0

const countRow = (
  rows: ReadonlyArray<StoredTokenUsageRow>,
): Record<string, number> => ({
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
  const since = hasSince ? String(values[valueIndex++]) : undefined
  const until = hasUntil ? String(values[valueIndex++]) : undefined
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
): D1Database & TokenUsageLedgerMemory => {
  const prepare = (query: string) => {
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
            const served = Number(row.input_tokens ?? 0) + Number(row.output_tokens ?? 0)
            byDay.set(day, (byDay.get(day) ?? 0) + served)
          }

          const series = [...byDay.entries()]
            .map(([day, tokens]) => ({ day, tokens }))
            .sort((left, right) => left.day.localeCompare(right.day))

          return Promise.resolve(makeResult<T>(series as Array<T>))
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
        if (
          query.includes('INSERT INTO token_usage_leaderboard_preferences')
        ) {
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
            id,
            idempotency_key: idempotencyKey,
            ingested_at: values[3] as string,
            input_tokens: values[17] as number,
            leaderboard_eligible: values[27] as number,
            model: values[15] as string | null,
            observed_at: values[2] as string,
            output_tokens: values[18] as number,
            privacy_opt_out: values[28] as number,
            producer_system: values[4] as string,
            provider: values[14] as string | null,
            reasoning_tokens: values[19] as number,
            repository_ref: values[13] as string | null,
            run_ref: values[10] as string | null,
            safe_metadata_json: values[29] as string,
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

const leaderboards = () =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.readLeaderboards({ now: nowIso, window: '7d' }),
  )

const updatePreference = (
  body: unknown,
): Effect.Effect<
  unknown,
  never,
  TokenUsageLedger
> =>
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
      producer_system: 'probe',
      source_route: 'probe_direct_provider',
      total_tokens: 180,
    })
    expect(JSON.stringify(db.rows)).not.toContain('prompt')
    expect(JSON.stringify(db.rows)).not.toContain('completion')
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

  test('returns correct per-day sums over a window from the ledger', async () => {
    const db = makeMemoryD1()

    await runLedger(
      db,
      Effect.gen(function* () {
        // Two events on 2026-06-06 → that day sums to (100+40) + (10+5) = 155.
        yield* ingest(eventOnDay('e1', '2026-06-06T09:00:00.000Z', 100, 40))
        yield* ingest(eventOnDay('e2', '2026-06-06T20:30:00.000Z', 10, 5))
        // One event on 2026-06-07 → 60.
        yield* ingest(eventOnDay('e3', '2026-06-07T01:00:00.000Z', 40, 20))
        // One event on 2026-06-08 → 9.
        yield* ingest(eventOnDay('e4', '2026-06-08T11:00:00.000Z', 6, 3))
      }),
    )

    const history = await runLedger(db, tokensServedHistory({ window: '30d' }))

    expect(history.window).toBe('30d')
    expect(history.bucket).toBe('day')
    // Ascending by day; per-day input + output sums.
    expect(history.series).toEqual([
      { day: '2026-06-06', tokensServed: 155 },
      { day: '2026-06-07', tokensServed: 60 },
      { day: '2026-06-08', tokensServed: 9 },
    ])
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

    expect(history.series).toEqual([
      { day: '2026-06-07', tokensServed: 100 },
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
