// Owner-gated inference cost / provider-lane analytics (#6232) — SQL behavior
// test. Runs `readInferenceAnalytics` against a REAL node:sqlite database loaded
// with migration 0137, so the GROUP BY / SUM / cost-coverage SQL is exercised
// for real rather than against a hand-rolled query mock.
import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TokenUsageLedger,
  type TokenUsageLedgerFilters,
  systemTokenUsageLedgerRuntime,
} from './token-usage-ledger'

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []
  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}
  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }
  async first<T = Row>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }
  async all<T = Row>(): Promise<{ results: T[] }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[],
    }
  }
  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
  }
}

const migration = readFileSync(
  new URL('../migrations/0137_token_usage_events.sql', import.meta.url),
  'utf8',
)

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(migration)
  return new SqliteD1(raw) as unknown as D1Database
}

const NOW = '2026-06-25T12:00:00.000Z'

// Fireworks DeepSeek V4 Flash row (the real prod Khala lane) WITH a stored cost.
const fireworksEvent = (
  overrides: Readonly<{
    eventId: string
    observedAt: string
    inputTokens: number
    outputTokens: number
    costUsd?: number | undefined
  }>,
) => ({
  schemaVersion: 'openagents.token_usage_event.v1' as const,
  actor: { accountRef: 'agent:tester' },
  backendProfile: 'fireworks',
  ...(overrides.costUsd === undefined
    ? {}
    : { cost: { amount: overrides.costUsd, currency: 'USD' } }),
  eventId: overrides.eventId,
  idempotencyKey: `idem:${overrides.eventId}`,
  model: 'accounts/fireworks/models/deepseek-v4-flash',
  observedAt: overrides.observedAt,
  producerSystem: 'omega' as const,
  provider: 'fireworks',
  sourceRoute: 'omega_hosted_gemini' as const,
  tokenCounts: {
    cacheReadTokens: 0,
    cacheWrite1hTokens: 0,
    cacheWrite5mTokens: 0,
    inputTokens: overrides.inputTokens,
    outputTokens: overrides.outputTokens,
    reasoningTokens: 0,
    totalTokens: overrides.inputTokens + overrides.outputTokens,
  },
  usageTruth: 'exact' as const,
})

const runLedger = <A>(
  db: D1Database,
  effect: Effect.Effect<A, unknown, TokenUsageLedger>,
): Promise<A> =>
  Effect.runPromise(
    effect.pipe(
      Effect.provide(
        TokenUsageLedger.live(db, systemTokenUsageLedgerRuntime),
      ),
    ),
  )

const ingest = (body: unknown) =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.ingestEvent(body).pipe(Effect.orDie),
  )

const analytics = (filters?: TokenUsageLedgerFilters & { window?: string }) =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.readInferenceAnalytics({ now: NOW, ...filters }),
  )

describe('readInferenceAnalytics (#6232)', () => {
  test('aggregates tokens + cost by provider, model, route, and day', async () => {
    const db = makeDb()
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.18,
          eventId: 'e1',
          inputTokens: 200_000,
          observedAt: '2026-06-25T01:00:00.000Z',
          outputTokens: 500_000,
        }),
      ),
    )
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.09,
          eventId: 'e2',
          inputTokens: 121_065,
          observedAt: '2026-06-25T05:00:00.000Z',
          outputTokens: 310_787,
        }),
      ),
    )

    const result = await runLedger(db, analytics({ window: '7d' }))

    expect(result.schemaVersion).toBe('openagents.inference_analytics.v1')
    expect(result.window).toBe('7d')

    // byProvider collapses the two Fireworks rows into one group.
    expect(result.byProvider).toHaveLength(1)
    expect(result.byProvider[0]).toMatchObject({
      key: 'fireworks',
      inputTokens: 321_065,
      outputTokens: 810_787,
      totalTokens: 1_131_852,
      usageEvents: 2,
    })
    expect(result.byProvider[0]?.costUsd).toBeCloseTo(0.27, 6)

    expect(result.byModel[0]?.key).toBe(
      'accounts/fireworks/models/deepseek-v4-flash',
    )
    expect(result.byRoute[0]?.key).toBe('omega:omega_hosted_gemini')

    // byDay collapses both same-day rows into one ascending point.
    expect(result.byDay).toHaveLength(1)
    expect(result.byDay[0]).toMatchObject({
      day: '2026-06-25',
      totalTokens: 1_131_852,
      usageEvents: 2,
    })

    expect(result.totals.totalTokens).toBe(1_131_852)
    expect(result.totals.usageEvents).toBe(2)
    expect(result.totals.costUsd).toBeCloseTo(0.27, 6)
    // Every row carried a stored cost.
    expect(result.totals.costCoverage).toBe(1)
  })

  test('reports cost coverage < 1 when rows predate cost recording', async () => {
    const db = makeDb()
    // One row WITH cost, one row WITHOUT (NULL cost, the historical shape).
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.1,
          eventId: 'with-cost',
          inputTokens: 1_000,
          observedAt: '2026-06-25T01:00:00.000Z',
          outputTokens: 1_000,
        }),
      ),
    )
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          eventId: 'no-cost',
          inputTokens: 1_000,
          observedAt: '2026-06-25T02:00:00.000Z',
          outputTokens: 1_000,
        }),
      ),
    )

    const result = await runLedger(db, analytics({ window: '7d' }))

    expect(result.totals.usageEvents).toBe(2)
    expect(result.totals.costUsd).toBeCloseTo(0.1, 6)
    // Half the rows carry a stored cost.
    expect(result.totals.costCoverage).toBe(0.5)
  })

  test('window=today excludes rows before UTC start of day', async () => {
    const db = makeDb()
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.05,
          eventId: 'yesterday',
          inputTokens: 10,
          observedAt: '2026-06-24T23:00:00.000Z',
          outputTokens: 10,
        }),
      ),
    )
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.05,
          eventId: 'today',
          inputTokens: 10,
          observedAt: '2026-06-25T01:00:00.000Z',
          outputTokens: 10,
        }),
      ),
    )

    const result = await runLedger(db, analytics({ window: 'today' }))

    expect(result.window).toBe('today')
    expect(result.totals.usageEvents).toBe(1)
    expect(result.byDay).toHaveLength(1)
    expect(result.byDay[0]?.day).toBe('2026-06-25')
  })

  test('rejects an invalid window with a typed validation error', async () => {
    const db = makeDb()
    const outcome = await Effect.runPromise(
      analytics({ window: 'bogus' }).pipe(
        Effect.match({
          onFailure: error => error._tag,
          onSuccess: () => 'success',
        }),
        Effect.provide(
          TokenUsageLedger.live(db, systemTokenUsageLedgerRuntime),
        ),
      ),
    )

    expect(outcome).toBe('TokenUsageLedgerValidationError')
  })
})
