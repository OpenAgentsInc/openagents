// KS-8.2 (#8308): token ledger repository CONTRACT suite.
//
// One behavioral spec, TWO implementations of `TokenLedgerWriteStore`:
//   - D1: `makeD1TokenLedgerWriteStore` over real SQLite (node:sqlite —
//     the engine D1 is built on), schema from the worker migrations
//     (condensed in test/sqlite-d1.ts).
//   - Postgres: `makePostgresTokenLedgerStore` over a throwaway local
//     Postgres (initdb/pg_ctl), schema from khala-sync-server migration
//     0008. Skipped when no local Postgres binaries exist.
//
// Every case runs identically against both stores: idempotency-key replay
// (both keys: idempotency_key AND event id), dedupe reads, and — the
// KS-8.2 load-bearing property — ROLLUP CONSISTENCY: the daily/model/
// channel rollups move with the event insert (same transaction/batch),
// with identical day-keying and tokens-served math on both sides, and
// never move on a duplicate.
//
// A second Postgres-gated suite proves READ equivalence: the same event
// set seeded through both write stores yields byte-equal decoded payloads
// from the D1 ledger reads and `makePostgresPublicTokensReads` across the
// aggregate / history (default TZ + UTC) / model-mix / demand-mix /
// channel-mix paths — the evidence that licenses the compare/postgres
// read modes.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  makePostgresPublicTokensReads,
  makePostgresTokenLedgerStore,
  type PostgresTokenLedgerStore,
} from './token-ledger-store'
import {
  makeD1TokenLedgerWriteStore,
  makeD1TokenUsageLedger,
  systemTokenUsageLedgerRuntime,
  type TokenLedgerWriteStore,
  type TokenUsageEventRow,
  type TokenUsageLedgerRuntime,
} from './token-usage-ledger'
import { makeSqliteD1, TOKEN_LEDGER_D1_SCHEMA } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}.contract.${++refCounter}`

const eventRow = (
  overrides: Partial<TokenUsageEventRow> = {},
): TokenUsageEventRow => {
  const id = overrides.id ?? nextRef('token_event')
  return {
    account_ref: null,
    actor_team_id: null,
    actor_user_id: 'user_contract',
    anonymized_source_ref: null,
    backend_profile: 'contract-backend',
    cache_read_tokens: 0,
    cache_write_1h_tokens: 0,
    cache_write_5m_tokens: 0,
    cost_amount: null,
    currency: null,
    demand_channel: 'khala_api',
    demand_client: 'khala-cli',
    demand_kind: 'external',
    demand_source: 'contract-suite',
    id,
    idempotency_key: `idem:${id}`,
    ingested_at: '2026-07-02T12:00:01.000Z',
    input_tokens: 100,
    leaderboard_eligible: 1,
    model: 'glm-4.7',
    observed_at: '2026-07-02T12:00:00.000Z',
    output_tokens: 40,
    privacy_opt_out: 0,
    producer_system: 'probe',
    provider: 'zai',
    reasoning_tokens: 0,
    repository_ref: null,
    role_ref: null,
    run_ref: null,
    safe_metadata_json: '{}',
    session_ref: null,
    source_route: 'contract_route',
    task_ref: null,
    total_tokens: 140,
    usage_truth: 'exact',
    ...overrides,
  }
}

type RollupRow = Readonly<{
  key: string
  tokens_served: number
  usage_events: number
}>

type ContractHarness = Readonly<{
  store: TokenLedgerWriteStore
  readDailyRollups: () => Promise<ReadonlyArray<RollupRow>>
  readModelRollups: () => Promise<ReadonlyArray<RollupRow>>
  readChannelRollups: () => Promise<ReadonlyArray<RollupRow>>
}>

// ---------------------------------------------------------------------------
// The shared behavioral spec
// ---------------------------------------------------------------------------

const specContractSuite = (getHarness: () => ContractHarness) => {
  test('fresh insert lands and is found by idempotency key AND by id', async () => {
    const { store } = getHarness()
    const row = eventRow()

    expect(await store.insertEventRow(row)).toBe('inserted')

    const byKey = await store.findExistingRow({
      eventId: 'nonexistent-id',
      idempotencyKey: row.idempotency_key,
    })
    expect(byKey?.id).toBe(row.id)
    expect(byKey?.input_tokens).toBe(100)
    expect(byKey?.output_tokens).toBe(40)
    expect(byKey?.total_tokens).toBe(140)
    expect(byKey?.observed_at).toBe(row.observed_at)
    expect(byKey?.safe_metadata_json).toBe('{}')

    const byId = await store.findExistingRow({
      eventId: row.id,
      idempotencyKey: 'nonexistent-key',
    })
    expect(byId?.idempotency_key).toBe(row.idempotency_key)
  })

  test('same idempotency key with a different id replays as duplicate', async () => {
    const { store } = getHarness()
    const row = eventRow()
    expect(await store.insertEventRow(row)).toBe('inserted')

    const replay = eventRow({
      id: nextRef('token_event'),
      idempotency_key: row.idempotency_key,
      input_tokens: 999_999,
    })
    expect(await store.insertEventRow(replay)).toBe('duplicate')

    const stored = await store.findExistingRow({
      eventId: replay.id,
      idempotencyKey: row.idempotency_key,
    })
    expect(stored?.id).toBe(row.id)
    expect(stored?.input_tokens).toBe(100)
  })

  test('same event id with a different idempotency key replays as duplicate', async () => {
    const { store } = getHarness()
    const row = eventRow()
    expect(await store.insertEventRow(row)).toBe('inserted')

    const replay = eventRow({
      id: row.id,
      idempotency_key: `idem:${nextRef('other')}`,
    })
    expect(await store.insertEventRow(replay)).toBe('duplicate')
  })

  test('rollups move with the insert and NEVER move on a duplicate', async () => {
    const harness = getHarness()
    const { store } = harness

    // Two fresh events on the same UTC + America/Chicago day, same
    // provider/model/channel, plus a total_tokens-fallback row (zero
    // split counts): the public tokens-served math must use
    // input+output when positive, total_tokens otherwise.
    const observedAt = '2026-07-10T17:00:00.000Z'
    const a = eventRow({
      model: 'rollup-model',
      observed_at: observedAt,
      provider: 'rollup-provider',
    })
    const b = eventRow({
      input_tokens: 10,
      model: 'rollup-model',
      observed_at: observedAt,
      output_tokens: 5,
      provider: 'rollup-provider',
      total_tokens: 15,
    })
    const totalOnly = eventRow({
      input_tokens: 0,
      model: 'rollup-model',
      observed_at: observedAt,
      output_tokens: 0,
      provider: 'rollup-provider',
      total_tokens: 500,
    })

    expect(await store.insertEventRow(a)).toBe('inserted')
    expect(await store.insertEventRow(b)).toBe('inserted')
    expect(await store.insertEventRow(totalOnly)).toBe('inserted')

    const expectRollups = async () => {
      const daily = await harness.readDailyRollups()
      const dailyRow = daily.find(row =>
        row.key.startsWith('America/Chicago:2026-07-10'),
      )
      // 140 + 15 + 500 (total_tokens fallback for the zero-split row)
      expect(dailyRow?.tokens_served).toBe(655)
      expect(dailyRow?.usage_events).toBe(3)

      const model = await harness.readModelRollups()
      const modelRow = model.find(
        row => row.key === '2026-07-10:rollup-provider:rollup-model',
      )
      expect(modelRow?.tokens_served).toBe(655)
      expect(modelRow?.usage_events).toBe(3)

      const channel = await harness.readChannelRollups()
      const channelRow = channel.find(
        row => row.key === '2026-07-10:khala_api',
      )
      expect(channelRow?.tokens_served).toBe(655)
      expect(channelRow?.usage_events).toBe(3)
    }

    await expectRollups()

    // Duplicate replay (same idempotency key): rollups must not move.
    expect(
      await store.insertEventRow(
        eventRow({
          id: nextRef('token_event'),
          idempotency_key: a.idempotency_key,
          model: 'rollup-model',
          observed_at: observedAt,
          provider: 'rollup-provider',
        }),
      ),
    ).toBe('duplicate')

    await expectRollups()
  })

  test('daily rollup keys on the America/Chicago day; mix rollups on the UTC day', async () => {
    const harness = getHarness()
    // 02:00Z on July 12 is still July 11 in America/Chicago (UTC-5).
    const row = eventRow({
      model: 'tz-model',
      observed_at: '2026-07-12T02:00:00.000Z',
      provider: 'tz-provider',
    })
    expect(await harness.store.insertEventRow(row)).toBe('inserted')

    const daily = await harness.readDailyRollups()
    expect(
      daily.some(entry => entry.key === 'America/Chicago:2026-07-11'),
    ).toBe(true)

    const model = await harness.readModelRollups()
    expect(
      model.some(entry => entry.key === '2026-07-12:tz-provider:tz-model'),
    ).toBe(true)
  })
}

// ---------------------------------------------------------------------------
// D1 implementation (real SQLite)
// ---------------------------------------------------------------------------

const d1Harness = (sqlite: ReturnType<typeof makeSqliteD1>): ContractHarness => ({
  readChannelRollups: async () => {
    const rows = await sqlite.db
      .prepare(
        `SELECT day, demand_channel, tokens_served, usage_events
           FROM public_khala_tokens_served_channel_daily_rollups`,
      )
      .all<{
        day: string
        demand_channel: string
        tokens_served: number
        usage_events: number
      }>()
    return rows.results.map(row => ({
      key: `${row.day}:${row.demand_channel}`,
      tokens_served: Number(row.tokens_served),
      usage_events: Number(row.usage_events),
    }))
  },
  readDailyRollups: async () => {
    const rows = await sqlite.db
      .prepare(
        `SELECT timezone, day, tokens_served, usage_events
           FROM public_khala_tokens_served_daily_rollups`,
      )
      .all<{
        timezone: string
        day: string
        tokens_served: number
        usage_events: number
      }>()
    return rows.results.map(row => ({
      key: `${row.timezone}:${row.day}`,
      tokens_served: Number(row.tokens_served),
      usage_events: Number(row.usage_events),
    }))
  },
  readModelRollups: async () => {
    const rows = await sqlite.db
      .prepare(
        `SELECT day, provider, model, tokens_served, usage_events
           FROM public_khala_tokens_served_model_daily_rollups`,
      )
      .all<{
        day: string
        provider: string
        model: string
        tokens_served: number
        usage_events: number
      }>()
    return rows.results.map(row => ({
      key: `${row.day}:${row.provider}:${row.model}`,
      tokens_served: Number(row.tokens_served),
      usage_events: Number(row.usage_events),
    }))
  },
  store: makeD1TokenLedgerWriteStore(sqlite.db),
})

describe('token ledger repository contract — D1 (SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(TOKEN_LEDGER_D1_SCHEMA)
    harness = d1Harness(sqlite)
  })

  afterAll(() => {
    sqlite.close()
  })

  specContractSuite(() => harness)
})

// ---------------------------------------------------------------------------
// Postgres implementation (throwaway local instance)
// ---------------------------------------------------------------------------

type PgClient = {
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    text: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}

const MIGRATION_0008 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0008_token_usage_ledger.sql',
)

const pgHarness = (client: PgClient): ContractHarness & {
  postgres: PostgresTokenLedgerStore
} => {
  const postgres = makePostgresTokenLedgerStore({
    acquireSql: () =>
      Promise.resolve({
        end: () => Promise.resolve(),
        sql: client as never,
      }),
  })
  return {
    postgres,
    readChannelRollups: async () => {
      const rows = await client.unsafe(
        `SELECT day, demand_channel, tokens_served, usage_events
           FROM public_khala_tokens_served_channel_daily_rollups`,
      )
      return rows.map(row => ({
        key: `${String(row.day)}:${String(row.demand_channel)}`,
        tokens_served: Number(row.tokens_served),
        usage_events: Number(row.usage_events),
      }))
    },
    readDailyRollups: async () => {
      const rows = await client.unsafe(
        `SELECT timezone, day, tokens_served, usage_events
           FROM public_khala_tokens_served_daily_rollups`,
      )
      return rows.map(row => ({
        key: `${String(row.timezone)}:${String(row.day)}`,
        tokens_served: Number(row.tokens_served),
        usage_events: Number(row.usage_events),
      }))
    },
    readModelRollups: async () => {
      const rows = await client.unsafe(
        `SELECT day, provider, model, tokens_served, usage_events
           FROM public_khala_tokens_served_model_daily_rollups`,
      )
      return rows.map(row => ({
        key: `${String(row.day)}:${String(row.provider)}:${String(row.model)}`,
        tokens_served: Number(row.tokens_served),
        usage_events: Number(row.usage_events),
      }))
    },
    store: postgres,
  }
}

describe.skipIf(!hasLocalPostgres())(
  'token ledger repository contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness & { postgres: PostgresTokenLedgerStore }

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE token_ledger_contract')
      await admin.end({ timeout: 5 })

      const url = pg.urlFor('token_ledger_contract')
      const raw = postgres(url, { max: 4, prepare: false })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0008, 'utf8'))
      harness = pgHarness(client)
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    specContractSuite(() => harness)

    test('insertDirectEventRow lands the event WITHOUT rollup increments (D1 direct-path parity)', async () => {
      const before = await harness.readDailyRollups()
      const row = eventRow({ observed_at: '2026-07-15T12:00:00.000Z' })
      expect(await harness.postgres.insertDirectEventRow(row)).toBe('inserted')
      expect(await harness.postgres.insertDirectEventRow(row)).toBe(
        'duplicate',
      )
      const after = await harness.readDailyRollups()
      expect(after).toEqual(before)

      const stored = await harness.postgres.findExistingRow({
        eventId: row.id,
        idempotencyKey: row.idempotency_key,
      })
      expect(stored?.id).toBe(row.id)
    })

    test('mirrorLeaderboardPreference upserts and converges', async () => {
      const preference = {
        leaderboardParticipation: 'opted_out',
        leaderboardVisibility: 'private',
        subjectKind: 'user',
        subjectRef: 'user_contract_pref',
        updatedAt: '2026-07-15T12:00:00.000Z',
        updatedByUserId: 'user_contract_pref',
      }
      await harness.postgres.mirrorLeaderboardPreference(preference)
      await harness.postgres.mirrorLeaderboardPreference({
        ...preference,
        leaderboardParticipation: 'eligible',
        updatedAt: '2026-07-15T13:00:00.000Z',
      })
      const rows = await client!.unsafe(
        `SELECT leaderboard_participation, updated_at
           FROM token_usage_leaderboard_preferences
          WHERE subject_kind = 'user' AND subject_ref = 'user_contract_pref'`,
      )
      expect(rows[0]?.leaderboard_participation).toBe('eligible')
      expect(rows[0]?.updated_at).toBe('2026-07-15T13:00:00.000Z')
    })
  },
)

// ---------------------------------------------------------------------------
// Read equivalence: D1 ledger reads vs makePostgresPublicTokensReads
// ---------------------------------------------------------------------------

const FIXED_NOW = '2026-07-03T18:00:00.000Z'

const fixedRuntime: TokenUsageLedgerRuntime = {
  ...systemTokenUsageLedgerRuntime,
  nowIso: () => FIXED_NOW,
}

const readEquivalenceEvents: ReadonlyArray<TokenUsageEventRow> = [
  eventRow({
    id: 'read_eq_1',
    idempotency_key: 'idem:read_eq_1',
    model: 'glm-4.7',
    observed_at: '2026-06-10T12:00:00.000Z',
    provider: 'zai',
  }),
  eventRow({
    demand_channel: 'direct_local',
    demand_client: 'codex-cli',
    demand_kind: 'external',
    demand_source: 'codex-direct',
    id: 'read_eq_2',
    idempotency_key: 'idem:read_eq_2',
    input_tokens: 2_000,
    model: 'gpt-oss-120b',
    observed_at: '2026-07-01T05:00:00.000Z',
    output_tokens: 300,
    provider: 'openai',
    total_tokens: 2_300,
  }),
  eventRow({
    demand_kind: 'internal',
    demand_source: 'artanis',
    id: 'read_eq_3',
    idempotency_key: 'idem:read_eq_3',
    input_tokens: 0,
    model: 'glm-4.7',
    observed_at: '2026-07-02T23:30:00.000Z',
    output_tokens: 0,
    provider: 'zai',
    total_tokens: 750,
  }),
  eventRow({
    id: 'read_eq_4',
    idempotency_key: 'idem:read_eq_4',
    input_tokens: 55,
    model: 'gemini-2.5-pro',
    observed_at: '2026-07-03T15:00:00.000Z',
    output_tokens: 45,
    provider: 'google_gemini',
    total_tokens: 100,
  }),
]

describe.skipIf(!hasLocalPostgres())(
  'token ledger read equivalence — D1 reads vs Postgres reads',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1>
    let d1Ledger: ReturnType<typeof makeD1TokenUsageLedger>
    let postgresReads: ReturnType<typeof makePostgresPublicTokensReads>

    beforeAll(async () => {
      sqlite = makeSqliteD1()
      sqlite.exec(TOKEN_LEDGER_D1_SCHEMA)
      const d1Store = makeD1TokenLedgerWriteStore(sqlite.db)

      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE token_ledger_read_eq')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('token_ledger_read_eq'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0008, 'utf8'))
      const pgStore = makePostgresTokenLedgerStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: raw as never,
          }),
      })

      // Seed the SAME rows through BOTH write stores (rollups included).
      for (const row of readEquivalenceEvents) {
        expect(await d1Store.insertEventRow(row)).toBe('inserted')
        expect(await pgStore.insertEventRow(row)).toBe('inserted')
      }

      d1Ledger = makeD1TokenUsageLedger(sqlite.db, fixedRuntime)
      postgresReads = makePostgresPublicTokensReads(pgStore, fixedRuntime)
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    test('aggregate total matches exactly', async () => {
      const d1 = await Effect.runPromise(d1Ledger.readPublicTokensServed())
      const postgres = await Effect.runPromise(
        postgresReads.readPublicTokensServed(),
      )
      // 140 + 2300 + 750 (total fallback) + 100
      expect(d1.tokensServed).toBe(3_290)
      expect(postgres).toEqual(d1)
    })

    test.each(['today', '7d', '30d', 'all'] as const)(
      'history (default timezone, window %s) matches exactly',
      async window => {
        const d1 = await Effect.runPromise(
          d1Ledger.readPublicTokensServedHistory({ now: FIXED_NOW, window }),
        )
        const postgres = await Effect.runPromise(
          postgresReads.readPublicTokensServedHistory({
            now: FIXED_NOW,
            window,
          }),
        )
        expect(postgres).toEqual(d1)
        if (window === 'all') {
          expect(d1.series.length).toBeGreaterThan(0)
        }
      },
    )

    test('history (UTC timezone) matches exactly', async () => {
      const d1 = await Effect.runPromise(
        d1Ledger.readPublicTokensServedHistory({
          now: FIXED_NOW,
          timezone: 'UTC',
          window: '30d',
        }),
      )
      const postgres = await Effect.runPromise(
        postgresReads.readPublicTokensServedHistory({
          now: FIXED_NOW,
          timezone: 'UTC',
          window: '30d',
        }),
      )
      expect(postgres).toEqual(d1)
      expect(d1.series.length).toBeGreaterThan(0)
    })

    test.each(['7d', '30d', 'all'] as const)(
      'model mix (window %s) matches exactly',
      async window => {
        const d1 = await Effect.runPromise(
          d1Ledger.readPublicTokensServedModelMix({ now: FIXED_NOW, window }),
        )
        const postgres = await Effect.runPromise(
          postgresReads.readPublicTokensServedModelMix({
            now: FIXED_NOW,
            window,
          }),
        )
        expect(postgres).toEqual(d1)
        if (window === 'all') {
          expect(d1.totalTokens).toBe(3_290)
        }
      },
    )

    test('demand mix matches exactly', async () => {
      const d1 = await Effect.runPromise(
        d1Ledger.readPublicTokensServedDemandMix({
          now: FIXED_NOW,
          window: '30d',
        }),
      )
      const postgres = await Effect.runPromise(
        postgresReads.readPublicTokensServedDemandMix({
          now: FIXED_NOW,
          window: '30d',
        }),
      )
      expect(postgres).toEqual(d1)
      expect(d1.groups.length).toBeGreaterThan(1)
    })

    test('channel mix matches exactly', async () => {
      const d1 = await Effect.runPromise(
        d1Ledger.readPublicTokensServedChannelMix({
          now: FIXED_NOW,
          window: '30d',
        }),
      )
      const postgres = await Effect.runPromise(
        postgresReads.readPublicTokensServedChannelMix({
          now: FIXED_NOW,
          window: '30d',
        }),
      )
      expect(postgres).toEqual(d1)
      expect(
        d1.groups.some(group => group.channel === 'direct_local'),
      ).toBe(true)
    })
  },
)
