import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type TokenUsageLedgerShape,
  TokenUsageLedgerStorageError,
  makeD1TokenUsageLedger,
} from '../token-usage-ledger'
import {
  buildServedTokensIngestBody,
  makeServedTokensRecorder,
  servedTokensEventId,
  servedTokensIdempotencyKey,
} from './served-tokens-recorder'

// ---------------------------------------------------------------------------
// Minimal in-memory D1 fake for the EXACT SQL the real `makeD1TokenUsageLedger`
// runs on the recorder write path + the public served-tokens counter read:
//   - findExisting: SELECT * FROM token_usage_events WHERE idempotency_key=? OR id=?
//   - insert:       INSERT INTO token_usage_events (...) VALUES (...)  (UNIQUE id/key)
//   - publicTokensServed: SELECT SUM(input)+SUM(output) AS tokens_served
// This exercises the REAL ledger (validation, idempotency, the public SUM) so
// the round trip is honest, not a stubbed projection.
// ---------------------------------------------------------------------------

type Row = Record<string, string | number | null>

const COLUMNS = [
  'id',
  'idempotency_key',
  'observed_at',
  'ingested_at',
  'producer_system',
  'source_route',
  'actor_user_id',
  'actor_team_id',
  'account_ref',
  'anonymized_source_ref',
  'run_ref',
  'session_ref',
  'task_ref',
  'repository_ref',
  'provider',
  'model',
  'backend_profile',
  'input_tokens',
  'output_tokens',
  'reasoning_tokens',
  'cache_read_tokens',
  'cache_write_5m_tokens',
  'cache_write_1h_tokens',
  'total_tokens',
  'usage_truth',
  'cost_amount',
  'currency',
  'leaderboard_eligible',
  'privacy_opt_out',
  'safe_metadata_json',
] as const

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: true,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeFakeDb = (rows: Array<Row> = []): D1Database => {
  const prepare = (sql: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const stmt = {
      bind: (...values: Array<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T>(): Promise<T | null> => {
        if (sql.includes('AS tokens_served')) {
          const tokensServed = rows.reduce(
            (sum, row) =>
              sum +
              Number(row.input_tokens ?? 0) +
              Number(row.output_tokens ?? 0),
            0,
          )
          return { tokens_served: tokensServed } as unknown as T
        }
        if (sql.includes('FROM token_usage_events') && sql.includes('WHERE')) {
          // findExisting: idempotency_key = ? OR id = ?
          const [idempotencyKey, id] = bound as [string, string]
          const found = rows.find(
            row =>
              row.idempotency_key === idempotencyKey || row.id === id,
          )
          return (found ?? null) as unknown as T
        }
        return null as unknown as T
      },
      all: async <T>(): Promise<D1Result<T>> => ({
        meta: d1Meta(),
        results: [] as Array<T>,
        success: true,
      }),
      run: async <T>(): Promise<D1Result<T>> => {
        if (sql.trimStart().startsWith('INSERT INTO token_usage_events')) {
          const row: Row = {}
          COLUMNS.forEach((column, index) => {
            row[column] = bound[index] as string | number | null
          })
          const clash = rows.some(
            existing =>
              existing.id === row.id ||
              existing.idempotency_key === row.idempotency_key,
          )
          if (clash) {
            throw new Error('UNIQUE constraint failed: token_usage_events.id')
          }
          rows.push(row)
        }
        return { meta: d1Meta(), results: [] as Array<T>, success: true }
      },
      raw: async () => [],
    }
    return stmt as unknown as D1PreparedStatement
  }

  return {
    prepare,
    batch: async () => [],
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    withSession: () => {
      throw new Error('not implemented')
    },
  } as unknown as D1Database
}

const readServed = (ledger: TokenUsageLedgerShape): Promise<number> =>
  Effect.runPromise(
    ledger.readPublicTokensServed().pipe(
      Effect.map(aggregate => aggregate.tokensServed),
    ),
  )

const fixedNow = () => '2026-06-24T00:00:00.000Z'

const runRecorder = (
  recorder: ReturnType<typeof makeServedTokensRecorder>,
  input: Parameters<ReturnType<typeof makeServedTokensRecorder>>[0],
): Promise<void> => Effect.runPromise(recorder(input))

describe('served-tokens-recorder', () => {
  test('a paid completion writes a ledger row and increases the served counter', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    const before = await readServed(ledger)
    expect(before).toBe(0)

    await runRecorder(recorder, {
      accountRef: 'agent:paid-1',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-paid-1',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 30, promptTokens: 12, totalTokens: 42 },
    })

    expect(rows).toHaveLength(1)
    expect(rows[0]!.id).toBe(servedTokensEventId('chatcmpl-paid-1'))
    expect(rows[0]!.idempotency_key).toBe(
      servedTokensIdempotencyKey('chatcmpl-paid-1'),
    )

    const after = await readServed(ledger)
    // The public counter sums input + output (12 + 30).
    expect(after).toBe(42)
  })

  test('a free-tier (zero-debit) completion still counts its served tokens', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    // Free-tier calls debit no credit, but the tokens are served and must count.
    await runRecorder(recorder, {
      accountRef: 'agent:free-1',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-free-1',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: true,
      usage: { completionTokens: 100, promptTokens: 50, totalTokens: 150 },
    })

    expect(rows).toHaveLength(1)
    const after = await readServed(ledger)
    expect(after).toBe(150)
  })

  test('the same request id is idempotent (one served completion = one row)', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    const call = () =>
      runRecorder(recorder, {
        accountRef: 'agent:dup-1',
        adapterId: 'hydralisk',
        requestId: 'chatcmpl-dup-1',
        requestedModel: 'openagents/khala',
        servedModel: 'openagents/khala',
        streamed: false,
        usage: { completionTokens: 20, promptTokens: 10, totalTokens: 30 },
      })

    await call()
    await call() // retry/replay for the SAME request id

    expect(rows).toHaveLength(1)
    const after = await readServed(ledger)
    // Counted exactly once, never double-counted.
    expect(after).toBe(30)
  })

  test('a zero-token completion is not recorded (nothing was served)', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    await runRecorder(recorder, {
      accountRef: 'agent:empty-1',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-empty-1',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 0, promptTokens: 0, totalTokens: 0 },
    })

    expect(rows).toHaveLength(0)
    const after = await readServed(ledger)
    expect(after).toBe(0)
  })

  test('a recorder persistence failure never throws (the completion is unaffected)', async () => {
    // A ledger whose ingest always fails: the recorder must swallow it.
    const failingLedger: TokenUsageLedgerShape = {
      ...makeD1TokenUsageLedger(makeFakeDb()),
      ingestEvent: () =>
        Effect.fail(
          new TokenUsageLedgerStorageError({
            error: new Error('boom'),
            operation: 'tokenUsageEvents.insert',
          }),
        ),
    }
    const recorder = makeServedTokensRecorder({
      ledger: failingLedger,
      nowIso: fixedNow,
    })

    // The recorder Effect swallows the ledger failure (it never fails), so
    // running it resolves cleanly — the completion is never affected.
    await expect(
      runRecorder(recorder, {
        accountRef: 'agent:fail-1',
        adapterId: 'hydralisk',
        requestId: 'chatcmpl-fail-1',
        requestedModel: 'openagents/khala',
        servedModel: 'openagents/khala',
        streamed: false,
        usage: { completionTokens: 5, promptTokens: 5, totalTokens: 10 },
      }),
    ).resolves.toBeUndefined()
  })

  test('the ingest body is public-safe (exact truth, no leaderboard, no secrets)', () => {
    const body = buildServedTokensIngestBody({
      accountRef: 'agent:body-1',
      adapterId: 'vertex-gemini',
      observedAt: fixedNow(),
      requestId: 'chatcmpl-body-1',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      usage: {
        cachedPromptTokens: 7,
        completionTokens: 40,
        promptTokens: 20,
        totalTokens: 60,
      },
    })

    expect(body.usageTruth).toBe('exact')
    expect(body.privacy.leaderboardEligible).toBe(false)
    expect(body.tokenCounts.inputTokens).toBe(20)
    expect(body.tokenCounts.outputTokens).toBe(40)
    expect(body.tokenCounts.cacheReadTokens).toBe(7)
    expect(body.producerSystem).toBe('omega')
    expect(body.sourceRoute).toBe('omega_hosted_gemini')
    expect(JSON.stringify(body)).not.toContain('prompt_text')
  })
})
