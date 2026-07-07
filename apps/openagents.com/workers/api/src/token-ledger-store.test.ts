// KS-8.2 (#8308): token ledger dual-write + flag routing unit suite.
//
// Covers: flag parsing, the fail-soft mirror (a Postgres failure NEVER
// fails an ingest; duplicates never mirror), the #8304 EXACTLY-ONCE
// public-counter regression (a dual-written event fires the projection
// observer exactly once — the mirror never re-triggers it), read routing
// (d1 / compare / postgres with bounded retry + D1 fallback), and
// compare-mode mismatch logging.

import { PublicKhalaTokensServedAggregate } from '@openagentsinc/sync-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeDualWriteTokenLedgerWriteStore,
  makeReadRoutedTokenUsageLedger,
  makeTokenUsageLedgerForEnv,
  tokenLedgerFlagsFromEnv,
  type PublicTokensServedReads,
  type TokenLedgerDiagnostic,
  type TokenLedgerDiagnosticEvent,
  type TokenLedgerFlags,
} from './token-ledger-store'
import {
  makeD1TokenLedgerWriteStore,
  makeD1TokenUsageLedger,
  TokenUsageLedgerStorageError,
  type TokenLedgerWriteStore,
  type TokenUsageEventRow,
  type TokenUsageLedgerShape,
} from './token-usage-ledger'
import { makeSqliteD1, TOKEN_LEDGER_D1_SCHEMA } from './test/sqlite-d1'

type LoggedDiagnostic = Readonly<{
  event: TokenLedgerDiagnosticEvent
  fields: TokenLedgerDiagnostic
}>

const makeLogCapture = () => {
  const entries: Array<LoggedDiagnostic> = []
  return {
    entries,
    log: (event: TokenLedgerDiagnosticEvent, fields: TokenLedgerDiagnostic) => {
      entries.push({ event, fields })
    },
  }
}

const sampleRow = (id: string): TokenUsageEventRow => ({
  account_ref: null,
  actor_team_id: null,
  actor_user_id: null,
  anonymized_source_ref: null,
  backend_profile: null,
  cache_read_tokens: 0,
  cache_write_1h_tokens: 0,
  cache_write_5m_tokens: 0,
  cost_amount: null,
  currency: null,
  demand_channel: 'khala_api',
  demand_client: null,
  demand_kind: 'unlabeled',
  demand_source: null,
  id,
  idempotency_key: `idem:${id}`,
  ingested_at: '2026-07-03T00:00:01.000Z',
  input_tokens: 10,
  leaderboard_eligible: 1,
  model: null,
  observed_at: '2026-07-03T00:00:00.000Z',
  output_tokens: 5,
  privacy_opt_out: 0,
  producer_system: 'probe',
  provider: null,
  reasoning_tokens: 0,
  repository_ref: null,
  role_ref: null,
  run_ref: null,
  safe_metadata_json: '{}',
  session_ref: null,
  source_route: 'test',
  task_ref: null,
  total_tokens: 15,
  usage_truth: 'exact',
})

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

describe('tokenLedgerFlagsFromEnv', () => {
  test('defaults: dual-write ON, reads d1', () => {
    expect(tokenLedgerFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
      writes: 'postgres',
    })
  })

  test.each(['off', '0', 'false', 'disabled', 'no', ' OFF '])(
    'dual-write disabled by %j',
    value => {
      expect(
        tokenLedgerFlagsFromEnv({ KHALA_SYNC_LEDGER_DUAL_WRITE: value })
          .dualWrite,
      ).toBe(false)
    },
  )

  test('reads accept postgres/compare; unknown values fall back to d1', () => {
    expect(
      tokenLedgerFlagsFromEnv({ KHALA_SYNC_LEDGER_READS: 'postgres' }).reads,
    ).toBe('postgres')
    expect(
      tokenLedgerFlagsFromEnv({ KHALA_SYNC_LEDGER_READS: ' Compare ' }).reads,
    ).toBe('compare')
    expect(
      tokenLedgerFlagsFromEnv({ KHALA_SYNC_LEDGER_READS: 'typo' }).reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Dual-write write store
// ---------------------------------------------------------------------------

const flagsOn: TokenLedgerFlags = { dualWrite: true, reads: 'd1', writes: 'd1' }

const memoryD1Store = () => {
  const rows = new Map<string, TokenUsageEventRow>()
  const store: TokenLedgerWriteStore = {
    findExistingRow: input =>
      Promise.resolve(
        [...rows.values()].find(
          row =>
            row.idempotency_key === input.idempotencyKey ||
            row.id === input.eventId,
        ),
      ),
    insertEventRow: row => {
      const exists = [...rows.values()].some(
        existing =>
          existing.id === row.id ||
          existing.idempotency_key === row.idempotency_key,
      )
      if (exists) {
        return Promise.resolve('duplicate' as const)
      }
      rows.set(row.id, row)
      return Promise.resolve('inserted' as const)
    },
  }
  return { rows, store }
}

describe('makeDualWriteTokenLedgerWriteStore', () => {
  test('fresh insert mirrors ONCE; duplicate does not mirror', async () => {
    const d1 = memoryD1Store()
    const mirrored: Array<string> = []
    const store = makeDualWriteTokenLedgerWriteStore({
      d1: d1.store,
      flags: flagsOn,
      postgres: {
        findExistingRow: () => Promise.resolve(undefined),
        insertEventRow: row => {
          mirrored.push(row.id)
          return Promise.resolve('inserted' as const)
        },
      },
    })

    const row = sampleRow('dw_1')
    expect(await store.insertEventRow(row)).toBe('inserted')
    expect(mirrored).toEqual(['dw_1'])

    expect(await store.insertEventRow(row)).toBe('duplicate')
    expect(mirrored).toEqual(['dw_1'])
  })

  test('a mirror failure NEVER fails the write; typed diagnostic logged', async () => {
    const d1 = memoryD1Store()
    const capture = makeLogCapture()
    const store = makeDualWriteTokenLedgerWriteStore({
      d1: d1.store,
      flags: flagsOn,
      log: capture.log,
      postgres: {
        findExistingRow: () => Promise.resolve(undefined),
        insertEventRow: () => Promise.reject(new Error('postgres down')),
      },
    })

    expect(await store.insertEventRow(sampleRow('dw_2'))).toBe('inserted')
    expect(d1.rows.has('dw_2')).toBe(true)
    expect(capture.entries).toEqual([
      {
        event: 'khala_sync_ledger_dual_write_failed',
        fields: {
          messageSafe: 'postgres down',
          op: 'insertEventRow',
          refs: ['dw_2'],
        },
      },
    ])
  })

  test('dual-write off or missing postgres degrades to plain D1', async () => {
    const d1 = memoryD1Store()
    const mirrored: Array<string> = []
    const offStore = makeDualWriteTokenLedgerWriteStore({
      d1: d1.store,
      flags: { dualWrite: false, reads: 'd1', writes: 'd1' },
      postgres: {
        findExistingRow: () => Promise.resolve(undefined),
        insertEventRow: row => {
          mirrored.push(row.id)
          return Promise.resolve('inserted' as const)
        },
      },
    })
    expect(offStore).toBe(d1.store)

    const missingStore = makeDualWriteTokenLedgerWriteStore({
      d1: d1.store,
      flags: flagsOn,
      postgres: undefined,
    })
    expect(missingStore).toBe(d1.store)
    expect(mirrored).toEqual([])
  })

  test('#8515 writes=postgres routes BOTH insert and dedupe read to Postgres, never D1', async () => {
    const d1 = memoryD1Store()
    const d1FindSpy: Array<string> = []
    const d1InsertSpy: Array<string> = []
    const guardedD1: TokenLedgerWriteStore = {
      findExistingRow: input => {
        d1FindSpy.push(input.eventId)
        return d1.store.findExistingRow(input)
      },
      insertEventRow: row => {
        d1InsertSpy.push(row.id)
        return d1.store.insertEventRow(row)
      },
    }
    const pgRows = new Map<string, TokenUsageEventRow>()
    const pgFindSpy: Array<string> = []
    const store = makeDualWriteTokenLedgerWriteStore({
      d1: guardedD1,
      flags: { dualWrite: true, reads: 'postgres', writes: 'postgres' },
      postgres: {
        findExistingRow: input => {
          pgFindSpy.push(input.eventId)
          return Promise.resolve(pgRows.get(input.eventId))
        },
        insertEventRow: row => {
          if (pgRows.has(row.id)) {
            return Promise.resolve('duplicate' as const)
          }
          pgRows.set(row.id, row)
          return Promise.resolve('inserted' as const)
        },
      },
    })

    const row = sampleRow('pg_auth_1')
    expect(await store.insertEventRow(row)).toBe('inserted')
    expect(await store.insertEventRow(row)).toBe('duplicate')
    expect(await store.findExistingRow({ eventId: 'pg_auth_1', idempotencyKey: row.idempotency_key })).toBeDefined()

    // Postgres is authoritative; the 401-dead D1 handle is never touched.
    expect(pgRows.has('pg_auth_1')).toBe(true)
    expect(pgFindSpy).toEqual(['pg_auth_1'])
    expect(d1InsertSpy).toEqual([])
    expect(d1FindSpy).toEqual([])
    expect(d1.rows.size).toBe(0)
  })
})
// ---------------------------------------------------------------------------
// #8304 interplay: EXACTLY-ONCE counter regression
// ---------------------------------------------------------------------------

describe('public counter exactly-once under dual-write (#8304 regression)', () => {
  const ingestBody = {
    schemaVersion: 'openagents.token_usage_event.v1',
    eventId: 'token_event_counter_once',
    idempotencyKey: 'counter:once:1',
    model: 'glm-4.7',
    observedAt: '2026-07-03T10:00:00.000Z',
    producerSystem: 'probe',
    provider: 'zai',
    sourceRoute: 'probe_direct_provider',
    tokenCounts: {
      cacheReadTokens: 0,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: 120,
      outputTokens: 30,
      reasoningTokens: 0,
      totalTokens: 150,
    },
    usageTruth: 'exact',
  }

  const setup = (mirror: {
    insertEventRow: (
      row: TokenUsageEventRow,
    ) => Promise<'inserted' | 'duplicate'>
  }) => {
    const sqlite = makeSqliteD1()
    sqlite.exec(TOKEN_LEDGER_D1_SCHEMA)
    const observed: Array<{ idempotencyKey: string; tokensServed: number }> = []
    const capture = makeLogCapture()
    const ledger = makeD1TokenUsageLedger(sqlite.db, undefined, {
      onIngestedEvent: event => {
        observed.push({
          idempotencyKey: event.idempotencyKey,
          tokensServed: event.tokensServed,
        })
        return Promise.resolve(undefined)
      },
      writeStore: makeDualWriteTokenLedgerWriteStore({
        d1: makeD1TokenLedgerWriteStore(sqlite.db),
        flags: flagsOn,
        log: capture.log,
        postgres: { findExistingRow: () => Promise.resolve(undefined), ...mirror },
      }),
    })
    return { capture, ledger, observed, sqlite }
  }

  test('a dual-written event increments the counter observer exactly once', async () => {
    const mirrored: Array<string> = []
    const { ledger, observed, sqlite } = setup({
      insertEventRow: row => {
        mirrored.push(row.idempotency_key)
        return Promise.resolve('inserted' as const)
      },
    })

    const first = await Effect.runPromise(ledger.ingestEvent(ingestBody))
    expect(first.inserted).toBe(true)
    // Mirror ran, observer fired ONCE — the mirror is invisible to it.
    expect(mirrored).toEqual(['counter:once:1'])
    expect(observed).toEqual([
      { idempotencyKey: 'counter:once:1', tokensServed: 150 },
    ])

    // Replay: duplicate — neither the observer nor the mirror re-fires.
    const replay = await Effect.runPromise(ledger.ingestEvent(ingestBody))
    expect(replay.inserted).toBe(false)
    expect(mirrored).toEqual(['counter:once:1'])
    expect(observed).toEqual([
      { idempotencyKey: 'counter:once:1', tokensServed: 150 },
    ])
    sqlite.close()
  })

  test('a FAILING mirror still yields exactly one observer fire and a successful ingest', async () => {
    const { capture, ledger, observed, sqlite } = setup({
      insertEventRow: () => Promise.reject(new Error('mirror unreachable')),
    })

    const result = await Effect.runPromise(ledger.ingestEvent(ingestBody))
    expect(result.inserted).toBe(true)
    expect(observed).toEqual([
      { idempotencyKey: 'counter:once:1', tokensServed: 150 },
    ])
    expect(capture.entries.map(entry => entry.event)).toEqual([
      'khala_sync_ledger_dual_write_failed',
    ])
    sqlite.close()
  })
})

// ---------------------------------------------------------------------------
// Read routing
// ---------------------------------------------------------------------------

const aggregate = (tokensServed: number) =>
  ({ tokensServed }) as typeof PublicKhalaTokensServedAggregate.Type

const stubLedger = (
  reads: Partial<PublicTokensServedReads>,
): TokenUsageLedgerShape => {
  const die = () => {
    throw new Error('not under test')
  }
  return {
    ingestEvent: die,
    readAggregates: die,
    readInferenceAnalytics: die,
    readLeaderboardPreference: die,
    readLeaderboards: die,
    readPublicTokensServed: () =>
      Effect.succeed(aggregate(0)),
    readPublicTokensServedChannelMix: die,
    readPublicTokensServedDemandMix: die,
    readPublicTokensServedHistory: die,
    readPublicTokensServedModelMix: die,
    updateLeaderboardPreference: die,
    ...reads,
  } as unknown as TokenUsageLedgerShape
}

const pgReads = (
  overrides: Partial<PublicTokensServedReads>,
): PublicTokensServedReads =>
  ({
    readPublicTokensServed: () => Effect.succeed(aggregate(0)),
    readPublicTokensServedChannelMix: () => Effect.die('unused'),
    readPublicTokensServedDemandMix: () => Effect.die('unused'),
    readPublicTokensServedHistory: () => Effect.die('unused'),
    readPublicTokensServedModelMix: () => Effect.die('unused'),
    ...overrides,
  }) as PublicTokensServedReads

describe('makeReadRoutedTokenUsageLedger', () => {
  test('reads=d1 passes straight through (postgres never called)', async () => {
    let postgresCalls = 0
    const routed = makeReadRoutedTokenUsageLedger({
      d1: stubLedger({
        readPublicTokensServed: () => Effect.succeed(aggregate(11)),
      }),
      flags: { dualWrite: true, reads: 'd1', writes: 'd1' },
      postgres: pgReads({
        readPublicTokensServed: () =>
          Effect.sync(() => {
            postgresCalls += 1
            return aggregate(99)
          }),
      }),
    })
    const result = await Effect.runPromise(routed.readPublicTokensServed())
    expect(result.tokensServed).toBe(11)
    expect(postgresCalls).toBe(0)
  })

  test('reads=postgres serves postgres', async () => {
    const routed = makeReadRoutedTokenUsageLedger({
      d1: stubLedger({
        readPublicTokensServed: () => Effect.succeed(aggregate(11)),
      }),
      flags: { dualWrite: true, reads: 'postgres', writes: 'd1' },
      postgres: pgReads({
        readPublicTokensServed: () => Effect.succeed(aggregate(99)),
      }),
    })
    const result = await Effect.runPromise(routed.readPublicTokensServed())
    expect(result.tokensServed).toBe(99)
  })

  test('reads=postgres retries (bounded), then falls back to D1 with diagnostics', async () => {
    const capture = makeLogCapture()
    const waits: Array<number> = []
    let attempts = 0
    const routed = makeReadRoutedTokenUsageLedger({
      d1: stubLedger({
        readPublicTokensServed: () => Effect.succeed(aggregate(11)),
      }),
      flags: { dualWrite: true, reads: 'postgres', writes: 'd1' },
      log: capture.log,
      postgres: pgReads({
        readPublicTokensServed: () =>
          Effect.suspend(() => {
            attempts += 1
            return Effect.fail(
              new TokenUsageLedgerStorageError({
                error: new Error('pg unavailable'),
                operation: 'test',
              }),
            )
          }),
      }),
      wait: ms => {
        waits.push(ms)
        return Promise.resolve()
      },
    })

    const result = await Effect.runPromise(routed.readPublicTokensServed())
    expect(result.tokensServed).toBe(11)
    expect(attempts).toBe(3)
    expect(waits).toEqual([50, 150])
    expect(capture.entries.map(entry => entry.event)).toEqual([
      'khala_sync_ledger_postgres_read_failed',
      'khala_sync_ledger_postgres_read_failed',
      'khala_sync_ledger_postgres_read_fallback',
    ])
    expect(capture.entries[0]?.fields.op).toBe('readPublicTokensServed')
  })

  test('reads=postgres transient failure recovers within the retry budget', async () => {
    let attempts = 0
    const routed = makeReadRoutedTokenUsageLedger({
      d1: stubLedger({
        readPublicTokensServed: () => Effect.succeed(aggregate(11)),
      }),
      flags: { dualWrite: true, reads: 'postgres', writes: 'd1' },
      postgres: pgReads({
        readPublicTokensServed: () =>
          Effect.suspend(() => {
            attempts += 1
            return attempts < 2
              ? Effect.fail(
                  new TokenUsageLedgerStorageError({
                    error: new Error('blip'),
                    operation: 'test',
                  }),
                )
              : Effect.succeed(aggregate(99))
          }),
      }),
      wait: () => Promise.resolve(),
    })
    const result = await Effect.runPromise(routed.readPublicTokensServed())
    expect(result.tokensServed).toBe(99)
  })

  test('reads=compare serves D1 and logs a mismatch with the op name', async () => {
    const capture = makeLogCapture()
    const routed = makeReadRoutedTokenUsageLedger({
      d1: stubLedger({
        readPublicTokensServed: () => Effect.succeed(aggregate(11)),
      }),
      flags: { dualWrite: true, reads: 'compare', writes: 'd1' },
      log: capture.log,
      postgres: pgReads({
        readPublicTokensServed: () => Effect.succeed(aggregate(99)),
      }),
    })
    const result = await Effect.runPromise(routed.readPublicTokensServed())
    expect(result.tokensServed).toBe(11)
    expect(capture.entries).toEqual([
      {
        event: 'khala_sync_ledger_read_compare_mismatch',
        fields: {
          messageSafe: 'postgres read differs from d1 authority',
          op: 'readPublicTokensServed',
          refs: [],
        },
      },
    ])
  })

  test('reads=compare with matching results logs nothing', async () => {
    const capture = makeLogCapture()
    const routed = makeReadRoutedTokenUsageLedger({
      d1: stubLedger({
        readPublicTokensServed: () => Effect.succeed(aggregate(42)),
      }),
      flags: { dualWrite: true, reads: 'compare', writes: 'd1' },
      log: capture.log,
      postgres: pgReads({
        readPublicTokensServed: () => Effect.succeed(aggregate(42)),
      }),
    })
    const result = await Effect.runPromise(routed.readPublicTokensServed())
    expect(result.tokensServed).toBe(42)
    expect(capture.entries).toEqual([])
  })

  test('reads=compare postgres failure is swallowed (D1 served) and logged', async () => {
    const capture = makeLogCapture()
    const routed = makeReadRoutedTokenUsageLedger({
      d1: stubLedger({
        readPublicTokensServed: () => Effect.succeed(aggregate(7)),
      }),
      flags: { dualWrite: true, reads: 'compare', writes: 'd1' },
      log: capture.log,
      postgres: pgReads({
        readPublicTokensServed: () =>
          Effect.fail(
            new TokenUsageLedgerStorageError({
              error: new Error('pg down'),
              operation: 'test',
            }),
          ),
      }),
    })
    const result = await Effect.runPromise(routed.readPublicTokensServed())
    expect(result.tokensServed).toBe(7)
    expect(capture.entries.map(entry => entry.event)).toEqual([
      'khala_sync_ledger_postgres_read_failed',
    ])
  })
})

// ---------------------------------------------------------------------------
// Env factory degradation
// ---------------------------------------------------------------------------

describe('makeTokenUsageLedgerForEnv', () => {
  test('without KHALA_SYNC_DB the ledger is plain D1 and fully functional', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(TOKEN_LEDGER_D1_SCHEMA)
    const ledger = makeTokenUsageLedgerForEnv({ OPENAGENTS_DB: sqlite.db })
    const result = await Effect.runPromise(
      ledger.ingestEvent({
        schemaVersion: 'openagents.token_usage_event.v1',
        eventId: 'token_event_no_binding',
        idempotencyKey: 'no-binding:1',
        observedAt: '2026-07-03T10:00:00.000Z',
        producerSystem: 'probe',
        sourceRoute: 'probe_direct_provider',
        tokenCounts: {
          cacheReadTokens: 0,
          cacheWrite1hTokens: 0,
          cacheWrite5mTokens: 0,
          inputTokens: 1,
          outputTokens: 2,
          reasoningTokens: 0,
          totalTokens: 3,
        },
        usageTruth: 'exact',
      }),
    )
    expect(result.inserted).toBe(true)
    const total = await Effect.runPromise(ledger.readPublicTokensServed())
    expect(total.tokensServed).toBe(3)
    sqlite.close()
  })
})
