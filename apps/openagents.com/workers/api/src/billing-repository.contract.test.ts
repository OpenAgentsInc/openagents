// KS-8.7 (#8318): billing / Stripe / pay-ins repository CONTRACT suite.
//
// One behavioral spec, TWO stores: the D1 write paths (`billing.ts`,
// `payments-ledger.ts`, the webhook ingest statements) run against REAL
// SQLite (node:sqlite — the engine D1 is built on, schema condensed in
// test/sqlite-d1.ts), with the KS-8.7 fail-soft mirror armed against a
// throwaway local Postgres carrying khala-sync migration 0015.
//
// The money contract this suite proves:
//   - PER-USER BALANCE EQUALITY: after any op sequence, D1's
//     SUM(amount_cents) per user equals the Postgres store's balance read
//     to the cent, and the ledger rows are BYTE-equal column by column.
//   - IDEMPOTENCY KEYS ROUND-TRIP EXACTLY: a replayed Stripe checkout
//     credit / auto-top-up event / webhook event id lands ONE row on both
//     sides — the dedupe decision is D1's and the mirror only copies.
//   - WEBHOOK REPLAY REGRESSION: the `stripe_webhook_events` INSERT OR
//     IGNORE gate replays as a no-op and the mirrored row (including the
//     event_id bytes) never changes.
//   - PAY-IN LIFECYCLE PARITY: create → forwarding → paid transitions
//     mirror the full pay_ins row + legs byte-exactly (converge).
//   - FAIL-SOFT: a Postgres outage NEVER fails a billing operation; it
//     logs `khala_sync_billing_dual_write_failed` and D1 stands.
//   - READ ROUTING: compare serves D1 and logs cent-level divergence;
//     postgres mode serves Postgres and falls back to D1 on failure.

import {
  BILLING_DOMAIN_TABLE_SPECS,
  normalizeBillingValue,
  type BillingDomainTable,
  type CompareSoakSample,
} from '@openagentsinc/khala-sync-server'
import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  applyManualBillingCredit,
  applyStripeCheckoutCredit,
  billingAutoTopUpStateFromRows,
  billingLedgerEntryFromRow,
  ensureBillingAccount,
  readBillingAutoTopUpState,
  readBillingBalanceCents,
  readBillingRecentLedgerEntries,
  recordBillingAutoTopUpEvent,
  redeemBillingCoupon,
  systemBillingRuntime,
  upsertBillingAutoTopUpPolicy,
  type BillingRuntime,
} from './billing'
import {
  BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES,
  billingPostgresServesTable,
  billingSyncFlagsFromEnv,
  billingRuntimeForEnv,
  makeBillingDomainMirror,
  makePostgresBillingStore,
  makeRoutedBillingAutoTopUpStateRead,
  makeRoutedBillingBalanceRead,
  makeRoutedBillingRecentEntriesRead,
  type BillingSyncDiagnosticEvent,
  type PostgresBillingStore,
} from './billing-store'
import {
  createPayInStatements,
  markPayInPaidStatements,
  runLedgerStatements,
} from './payments-ledger'
import { makePostgresPaymentsLedgerDb } from './payments-ledger-db'
import { BILLING_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

const MIGRATION_0015 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0015_billing_pay_ins.sql',
)
const MIGRATION_0016 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0016_treasury_domain.sql',
)
const MIGRATION_0034 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0034_billing_bounded_read_indexes.sql',
)

type PgClient = {
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    text: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}

type LogEntry = Readonly<{ event: BillingSyncDiagnosticEvent; op: string }>

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}_contract_${++refCounter}`

// ---------------------------------------------------------------------------
// Flags (pure)
// ---------------------------------------------------------------------------

describe('billing sync flags', () => {
  test('dual-write defaults ON; reads default d1; typos never route reads', () => {
    expect(billingSyncFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
    expect(
      billingSyncFlagsFromEnv({ KHALA_SYNC_BILLING_DUAL_WRITE: 'off' })
        .dualWrite,
    ).toBe(false)
    expect(
      billingSyncFlagsFromEnv({ KHALA_SYNC_BILLING_READS: 'compare' }).reads,
    ).toBe('compare')
    expect(
      billingSyncFlagsFromEnv({ KHALA_SYNC_BILLING_READS: 'postgres' }).reads,
    ).toBe('postgres')
    expect(
      billingSyncFlagsFromEnv({ KHALA_SYNC_BILLING_READS: 'postgress' }).reads,
    ).toBe('d1')
  })

  test('no binding → plain system runtime (no mirror, no routed read)', () => {
    const runtime = billingRuntimeForEnv({}, { routeReads: true })
    expect(runtime).toBe(systemBillingRuntime)
  })

  test('dual-write off + reads d1 → plain system runtime even with binding', () => {
    const runtime = billingRuntimeForEnv(
      {
        KHALA_SYNC_BILLING_DUAL_WRITE: 'off',
        KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
      },
      {
        makeSqlClient: () => {
          throw new Error('must not connect')
        },
      },
    )
    expect(runtime.mirror).toBeUndefined()
    expect(runtime.balanceRead).toBeUndefined()
  })

  test('reads routing requires the explicit routeReads opt-in', () => {
    const env = {
      KHALA_SYNC_BILLING_READS: 'postgres',
      KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
    }
    const options = {
      makeSqlClient: () => Promise.reject(new Error('no connection in test')),
    }
    expect(billingRuntimeForEnv(env, options).balanceRead).toBeUndefined()
    expect(
      billingRuntimeForEnv(env, { ...options, routeReads: true }).balanceRead,
    ).toBeDefined()
  })

  // #8337: unlike balanceRead, the two bounded-allowlist reads are wired
  // unconditionally whenever reads !== 'd1' — no separate routeReads-style
  // opt-in, because only the display summary path ever calls either hook.
  test('#8337 recent-entries + auto-top-up-state reads are wired WITHOUT routeReads', () => {
    const env = {
      KHALA_SYNC_BILLING_READS: 'postgres',
      KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
    }
    const options = {
      makeSqlClient: () => Promise.reject(new Error('no connection in test')),
    }
    const runtime = billingRuntimeForEnv(env, options)
    expect(runtime.balanceRead).toBeUndefined()
    expect(runtime.recentEntriesRead).toBeDefined()
    expect(runtime.autoTopUpStateRead).toBeDefined()
  })

  test('#8337 reads d1 (the default) wires neither bounded-allowlist hook', () => {
    const runtime = billingRuntimeForEnv({
      KHALA_SYNC_DB: { connectionString: 'postgres://unused' },
    })
    expect(runtime.recentEntriesRead).toBeUndefined()
    expect(runtime.autoTopUpStateRead).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// #8337 bounded Postgres-served read allowlist (pure)
// ---------------------------------------------------------------------------

describe('#8337 BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES', () => {
  test('names exactly the allowlisted display surfaces, never a decision-critical table', () => {
    // CFG-4 (#8519): `pay_ins` left this allowlist — the receipt reads go
    // DIRECTLY to the Postgres-authoritative credits ledger with no flag
    // routing, so the flag-gated served-read registry no longer names it.
    expect([...BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES].sort()).toEqual([
      'billing_auto_top_up_events',
      'billing_auto_top_up_policies',
      'billing_ledger_entries',
      'stripe_checkout_sessions',
      'stripe_saved_payment_methods',
    ])
    expect(billingPostgresServesTable('billing_ledger_entries')).toBe(true)
    expect(billingPostgresServesTable('pay_ins')).toBe(false)
    // Decision-critical / idempotency-dedupe tables must NEVER be allowlisted.
    expect(billingPostgresServesTable('buyer_payment_challenges')).toBe(false)
    expect(billingPostgresServesTable('buyer_payment_receipts')).toBe(false)
    expect(
      billingPostgresServesTable('buyer_payment_reconciliation_events'),
    ).toBe(false)
    expect(billingPostgresServesTable('buyer_payment_redemptions')).toBe(false)
    expect(billingPostgresServesTable('stripe_webhook_events')).toBe(false)
    expect(billingPostgresServesTable('pay_in_legs')).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Fail-soft + read routing (pure, injected stores)
// ---------------------------------------------------------------------------

describe('fail-soft mirror + routed balance read', () => {
  const makeHarness = () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(BILLING_DOMAIN_D1_SCHEMA)
    return sqlite
  }

  test('a throwing Postgres NEVER fails the billing op; drift is logged', async () => {
    const sqlite = makeHarness()
    const logs: Array<LogEntry> = []
    const mirror = makeBillingDomainMirror({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: {
        upsertRows: () => Promise.reject(new Error('postgres down')),
      },
    })
    const runtime: BillingRuntime = { ...systemBillingRuntime, mirror }

    const userId = nextRef('user')
    const summary = await applyManualBillingCredit(
      sqlite.db,
      {
        amountCents: 500,
        idempotencyKey: `billing:test:${userId}`,
        reason: 'fail-soft test',
        userId,
      },
      runtime,
    )
    // The D1 authority stands: trial grant (1000) + manual credit (500).
    expect(summary.balanceCents).toBe(1_500)
    expect(
      logs.some(entry => entry.event === 'khala_sync_billing_dual_write_failed'),
    ).toBe(true)
    sqlite.close()
  })

  test('mirror ref for a missing row is a no-op, never an error', async () => {
    const sqlite = makeHarness()
    const upserted: Array<string> = []
    const mirror = makeBillingDomainMirror({
      log: () => {},
      postgres: {
        upsertRows: table => {
          upserted.push(table)
          return Promise.resolve()
        },
      },
    })
    await mirror(sqlite.db, [
      { key: { user_id: 'nope' }, table: 'billing_accounts' },
    ])
    expect(upserted).toHaveLength(0)
    sqlite.close()
  })

  test('compare mode serves D1 and logs cent-level divergence, plus a durable soak sample (#8282)', async () => {
    const logs: Array<LogEntry> = []
    const samples: CompareSoakSample[] = []
    const read = makeRoutedBillingBalanceRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      metrics: { record: sample => samples.push(sample) },
      postgres: { readBalanceCents: () => Promise.resolve(999) },
      reads: 'compare',
    })
    expect(await read('user-x', () => Promise.resolve(1_000))).toBe(1_000)
    expect(
      logs.some(
        entry => entry.event === 'khala_sync_billing_read_compare_mismatch',
      ),
    ).toBe(true)
    expect(samples).toEqual([
      { domain: 'billing', outcome: 'mismatch', readKind: 'readBalanceCents' },
    ])

    logs.length = 0
    samples.length = 0
    expect(await read('user-x', () => Promise.resolve(999))).toBe(999)
    expect(logs).toHaveLength(0)
    expect(samples).toEqual([
      { domain: 'billing', outcome: 'match', readKind: 'readBalanceCents' },
    ])
  })

  test('compare mode: a Postgres failure logs and still serves D1, and records an error soak sample', async () => {
    const logs: Array<LogEntry> = []
    const samples: CompareSoakSample[] = []
    const read = makeRoutedBillingBalanceRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      metrics: { record: sample => samples.push(sample) },
      postgres: {
        readBalanceCents: () => Promise.reject(new Error('pg down')),
      },
      reads: 'compare',
    })
    expect(await read('user-x', () => Promise.resolve(42))).toBe(42)
    expect(
      logs.some(
        entry => entry.event === 'khala_sync_billing_postgres_read_failed',
      ),
    ).toBe(true)
    expect(samples).toEqual([
      { domain: 'billing', outcome: 'error', readKind: 'readBalanceCents' },
    ])
  })

  test('postgres mode serves Postgres; exhausted retries fall back to D1', async () => {
    const logs: Array<LogEntry> = []
    const healthy = makeRoutedBillingBalanceRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: { readBalanceCents: () => Promise.resolve(777) },
      reads: 'postgres',
      wait: () => Promise.resolve(),
    })
    expect(await healthy('user-x', () => Promise.resolve(0))).toBe(777)

    const failing = makeRoutedBillingBalanceRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: {
        readBalanceCents: () => Promise.reject(new Error('pg down')),
      },
      reads: 'postgres',
      wait: () => Promise.resolve(),
    })
    expect(await failing('user-x', () => Promise.resolve(314))).toBe(314)
    expect(
      logs.filter(
        entry => entry.event === 'khala_sync_billing_postgres_read_failed',
      ).length,
    ).toBeGreaterThanOrEqual(2)
    expect(
      logs.some(
        entry => entry.event === 'khala_sync_billing_postgres_read_fallback',
      ),
    ).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// #8337 routed recent-entries + auto-top-up-state reads (pure, injected)
// ---------------------------------------------------------------------------

describe('#8337 routed recent-entries read', () => {
  const entry = (id: string) => ({
    amountCents: 100,
    amountFormatted: '$1.00',
    createdAt: '2026-07-05T00:00:00.000Z',
    description: 'd',
    id,
    quantity: null,
    source: 'manual_adjustment' as const,
    unit: null,
  })

  test('compare mode serves D1 and logs divergence', async () => {
    const logs: Array<LogEntry> = []
    const read = makeRoutedBillingRecentEntriesRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: {
        readRecentLedgerEntryRows: () =>
          Promise.resolve([
            {
              amount_cents: 999,
              created_at: '2026-07-05T00:00:00.000Z',
              description: 'd',
              id: 'pg-1',
              quantity: null,
              source: 'manual_adjustment',
              unit: null,
            },
          ]),
      },
      reads: 'compare',
    })
    const served = await read('user-x', () => Promise.resolve([entry('d1-1')]))
    expect(served).toEqual([entry('d1-1')])
    expect(
      logs.some(
        entry_ => entry_.event === 'khala_sync_billing_read_compare_mismatch',
      ),
    ).toBe(true)
  })

  test('compare mode: identical rows never log a mismatch', async () => {
    const logs: Array<LogEntry> = []
    const read = makeRoutedBillingRecentEntriesRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: {
        readRecentLedgerEntryRows: () =>
          Promise.resolve([
            {
              amount_cents: 100,
              created_at: '2026-07-05T00:00:00.000Z',
              description: 'd',
              id: 'match-1',
              quantity: null,
              source: 'manual_adjustment',
              unit: null,
            },
          ]),
      },
      reads: 'compare',
    })
    await read('user-x', () => Promise.resolve([entry('match-1')]))
    expect(logs).toHaveLength(0)
  })

  test('postgres mode serves Postgres; a failure falls back to D1', async () => {
    const logs: Array<LogEntry> = []
    const healthy = makeRoutedBillingRecentEntriesRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: {
        readRecentLedgerEntryRows: () =>
          Promise.resolve([
            {
              amount_cents: 100,
              created_at: '2026-07-05T00:00:00.000Z',
              description: 'd',
              id: 'pg-served-1',
              quantity: null,
              source: 'manual_adjustment',
              unit: null,
            },
          ]),
      },
      reads: 'postgres',
    })
    expect(
      (await healthy('user-x', () => Promise.resolve([]))).map(row => row.id),
    ).toEqual(['pg-served-1'])

    const failing = makeRoutedBillingRecentEntriesRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: {
        readRecentLedgerEntryRows: () => Promise.reject(new Error('pg down')),
      },
      reads: 'postgres',
    })
    expect(await failing('user-x', () => Promise.resolve([entry('d1-fallback')]))).toEqual([
      entry('d1-fallback'),
    ])
    expect(
      logs.some(
        entry_ => entry_.event === 'khala_sync_billing_postgres_read_fallback',
      ),
    ).toBe(true)
  })
})

describe('#8337 routed auto-top-up-state read', () => {
  const d1State = billingAutoTopUpStateFromRows(
    { events: [], paymentMethod: null, policy: null },
    systemBillingRuntime,
  )

  test('compare mode serves D1 and logs divergence', async () => {
    const logs: Array<LogEntry> = []
    const read = makeRoutedBillingAutoTopUpStateRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: {
        readAutoTopUpStateRows: () =>
          Promise.resolve({
            events: [],
            paymentMethod: null,
            policy: {
              amount_cents: 2_500,
              enabled: 1,
              monthly_cap_cents: 10_000,
              pause_reason: null,
              spent_this_month_cents: 0,
              status: 'active',
              threshold_cents: 500,
              updated_at: '2026-07-05T00:00:00.000Z',
            },
          }),
      },
      reads: 'compare',
    })
    const served = await read('user-x', () => Promise.resolve(d1State))
    expect(served).toEqual(d1State)
    expect(
      logs.some(
        entry => entry.event === 'khala_sync_billing_read_compare_mismatch',
      ),
    ).toBe(true)
  })

  test('postgres mode serves Postgres; a failure falls back to D1', async () => {
    const logs: Array<LogEntry> = []
    const failing = makeRoutedBillingAutoTopUpStateRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: {
        readAutoTopUpStateRows: () => Promise.reject(new Error('pg down')),
      },
      reads: 'postgres',
    })
    expect(await failing('user-x', () => Promise.resolve(d1State))).toEqual(
      d1State,
    )
    expect(
      logs.some(
        entry => entry.event === 'khala_sync_billing_postgres_read_fallback',
      ),
    ).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// The D1 ↔ Postgres contract (local Postgres required)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  'billing repository contract — D1 authority + Postgres mirror',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1>
    let postgresStore: PostgresBillingStore
    let runtime: BillingRuntime
    let logs: Array<LogEntry>

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE billing_contract')
      await admin.end({ timeout: 5 })

      const raw = postgres(pg.urlFor('billing_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0015, 'utf8'))
      // CFG-4 (#8519): agent_balances lives in the treasury twin set.
      await raw.unsafe(readFileSync(MIGRATION_0016, 'utf8'))
      await raw.unsafe(readFileSync(MIGRATION_0034, 'utf8'))

      sqlite = makeSqliteD1()
      sqlite.exec(BILLING_DOMAIN_D1_SCHEMA)

      postgresStore = makePostgresBillingStore({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: client as never,
          }),
      })
      logs = []
      runtime = {
        ...systemBillingRuntime,
        mirror: makeBillingDomainMirror({
          log: (event, fields) => logs.push({ event, op: fields.op }),
          postgres: postgresStore,
        }),
      }
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    /** Byte-level row parity: D1 row(s) vs Postgres row(s) by key. */
    const expectRowParity = async (
      table: BillingDomainTable,
      keyColumn: string,
      keyValue: string,
    ) => {
      const spec = BILLING_DOMAIN_TABLE_SPECS[table]
      const d1Rows = (
        await sqlite.db
          .prepare(`SELECT * FROM ${table} WHERE ${keyColumn} = ?`)
          .bind(keyValue)
          .all<Record<string, unknown>>()
      ).results
      const pgRows = await client!.unsafe(
        `SELECT * FROM ${table} WHERE ${keyColumn} = $1 ORDER BY ${spec.keyColumns.join(', ')}`,
        [keyValue],
      )
      // postgres.js returns bigint columns as strings; D1 returns numbers.
      // The byte contract is over the canonical string form (exactly what
      // the backfill row hash consumes), so compare String() forms.
      const normalize = (rows: ReadonlyArray<Record<string, unknown>>) =>
        [...rows]
          .map(row =>
            spec.columns.map(column => {
              const value = normalizeBillingValue(row[column])
              return value === null ? null : String(value)
            }),
          )
          .sort((left, right) =>
            JSON.stringify(left) < JSON.stringify(right) ? -1 : 1,
          )
      expect(pgRows.length).toBe(d1Rows.length)
      expect(normalize(pgRows)).toEqual(normalize(d1Rows))
    }

    test('credits sequence: per-user balance reconciles to the cent, rows byte-equal', async () => {
      const userId = nextRef('user')

      await ensureBillingAccount(sqlite.db, userId, runtime)
      await applyStripeCheckoutCredit(
        sqlite.db,
        {
          amountCents: 2_400,
          bonusCents: 100,
          packageId: 'pack_25',
          sessionId: `cs_${userId}`,
          userId,
        },
        runtime,
      )
      await applyManualBillingCredit(
        sqlite.db,
        {
          amountCents: 250,
          idempotencyKey: `billing:manual:${userId}`,
          reason: 'contract credit',
          userId,
        },
        runtime,
      )
      const coupon = await redeemBillingCoupon(
        sqlite.db,
        { couponCode: 'SHC-SMOKE', userId },
        runtime,
      )
      expect(coupon.ok).toBe(true)

      const d1Balance = await readBillingBalanceCents(sqlite.db, userId)
      // trial 1000 + checkout 2500 + manual 250 + coupon 1000
      expect(d1Balance).toBe(4_750)
      expect(await postgresStore.readBalanceCents(userId)).toBe(d1Balance)

      await expectRowParity('billing_ledger_entries', 'user_id', userId)
      await expectRowParity('billing_accounts', 'user_id', userId)
      await expectRowParity('billing_coupon_redemptions', 'user_id', userId)
      expect(logs).toHaveLength(0)
    })

    test('replayed Stripe checkout credit lands ONE ledger row on both sides', async () => {
      const userId = nextRef('user')
      const sessionId = `cs_replay_${userId}`
      const apply = () =>
        applyStripeCheckoutCredit(
          sqlite.db,
          {
            amountCents: 500,
            packageId: 'pack_5',
            sessionId,
            userId,
          },
          runtime,
        )
      const first = await apply()
      const second = await apply()
      expect(second.balanceCents).toBe(first.balanceCents)

      const d1Count = await sqlite.db
        .prepare(
          `SELECT COUNT(*) AS n FROM billing_ledger_entries WHERE idempotency_key = ?`,
        )
        .bind(`billing:stripe-checkout:${sessionId}`)
        .first<{ n: number }>()
      expect(Number(d1Count?.n)).toBe(1)
      const pgCount = await client!.unsafe(
        `SELECT COUNT(*) AS n FROM billing_ledger_entries WHERE idempotency_key = $1`,
        [`billing:stripe-checkout:${sessionId}`],
      )
      expect(Number(pgCount[0]?.n)).toBe(1)
      await expectRowParity('billing_ledger_entries', 'user_id', userId)
    })

    test('webhook idempotency regression: replayed event id is a no-op, bytes stable', async () => {
      const eventId = `evt_${nextRef('wh')}_§byte-exact§`
      const ingest = async (type: string, receivedAt: string) => {
        // The EXACT statement shape processStripeWebhook runs.
        await sqlite.db
          .prepare(
            `INSERT OR IGNORE INTO stripe_webhook_events
              (event_id, type, processing_status, checkout_session_id, received_at, processed_at)
             VALUES (?, ?, 'received', ?, ?, NULL)`,
          )
          .bind(eventId, type, null, receivedAt)
          .run()
        await runtime.mirror?.(sqlite.db, [
          { key: { event_id: eventId }, table: 'stripe_webhook_events' },
        ])
      }

      await ingest('checkout.session.completed', '2026-07-04T01:00:00.000Z')
      // Stripe redelivery: same event id, later timestamp — MUST NOT win.
      await ingest('checkout.session.completed', '2026-07-04T09:00:00.000Z')

      const pgRows = await client!.unsafe(
        `SELECT * FROM stripe_webhook_events WHERE event_id = $1`,
        [eventId],
      )
      expect(pgRows).toHaveLength(1)
      expect(String(pgRows[0]?.event_id)).toBe(eventId)
      expect(String(pgRows[0]?.received_at)).toBe('2026-07-04T01:00:00.000Z')
      await expectRowParity('stripe_webhook_events', 'event_id', eventId)

      // The processed-status UPDATE converges too.
      await sqlite.db
        .prepare(
          `UPDATE stripe_webhook_events
           SET processing_status = 'processed', processed_at = ?
           WHERE event_id = ?`,
        )
        .bind('2026-07-04T01:00:05.000Z', eventId)
        .run()
      await runtime.mirror?.(sqlite.db, [
        { key: { event_id: eventId }, table: 'stripe_webhook_events' },
      ])
      await expectRowParity('stripe_webhook_events', 'event_id', eventId)
      expect(logs).toHaveLength(0)
    })

    test('auto-top-up policy + replayed event: one row per idempotency key, byte parity', async () => {
      const userId = nextRef('user')
      await upsertBillingAutoTopUpPolicy(
        sqlite.db,
        {
          amountCents: 2_500,
          enabled: true,
          monthlyCapCents: 10_000,
          thresholdCents: 500,
          userId,
        },
        runtime,
      )
      const record = () =>
        recordBillingAutoTopUpEvent(
          sqlite.db,
          {
            amountCents: 2_500,
            idempotencyKey: `topup:${userId}`,
            status: 'succeeded',
            userId,
          },
          runtime,
        )
      await record()
      await record()

      const pgEvents = await client!.unsafe(
        `SELECT COUNT(*) AS n FROM billing_auto_top_up_events WHERE user_id = $1`,
        [userId],
      )
      expect(Number(pgEvents[0]?.n)).toBe(1)
      await expectRowParity('billing_auto_top_up_events', 'user_id', userId)
      await expectRowParity('billing_auto_top_up_policies', 'user_id', userId)
    })

    test('pay-in lifecycle (CFG-4 #8519): create + paid transition land DIRECTLY on Postgres', async () => {
      // The credits tables left the dual-write/mirror posture entirely —
      // `runLedgerStatements` executes ONE Postgres transaction and D1 is
      // not involved. This proves the lifecycle lands on the Postgres twins
      // this contract suite provisions (full executor semantics live in
      // payments-ledger-postgres.contract.test.ts).
      const payInId = nextRef('payin')
      const actorRef = `agent:${nextRef('actor')}`
      const now = '2026-07-04T02:00:00.000Z'
      const ledgerDb = makePostgresPaymentsLedgerDb({
        acquireSql: () =>
          Promise.resolve({
            end: () => Promise.resolve(),
            sql: client as never,
          }),
      })

      // Fund the payer balance so the funding debit clears its CHECK.
      await ledgerDb.batch([
        {
          params: [actorRef, now, now],
          sql: `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
                VALUES (?, 10000, ?, ?)`,
        },
      ])

      await runLedgerStatements(
        ledgerDb,
        createPayInStatements(
          {
            contextRef: `forum.post.${payInId}`,
            costMsat: 5_000,
            genesisId: null,
            idempotencyKey: `payin:${payInId}`,
            legs: [
              {
                amountMsat: 5_000,
                direction: 'in',
                externalRef: null,
                kind: 'balance',
                legId: `${payInId}:funding`,
                partyRef: actorRef,
              },
              {
                amountMsat: 5_000,
                direction: 'out',
                externalRef: 'forum.tip_recipient_claim',
                kind: 'balance',
                legId: `${payInId}:payout`,
                partyRef: `agent:${nextRef('recipient')}`,
              },
            ],
            payInId,
            payInType: 'tip',
            payerRef: actorRef,
            publicReceiptRef: null,
            rung: 'credited',
          },
          now,
        ),
      )

      const pgLegs = await client!.unsafe(
        `SELECT COUNT(*) AS n FROM pay_in_legs WHERE pay_in_id = $1`,
        [payInId],
      )
      expect(Number(pgLegs[0]?.n)).toBe(2)
      const pgBalance = await client!.unsafe(
        `SELECT balance_msat FROM agent_balances WHERE actor_ref = $1`,
        [actorRef],
      )
      expect(Number(pgBalance[0]?.balance_msat)).toBe(5_000)

      await runLedgerStatements(
        ledgerDb,
        markPayInPaidStatements(
          {
            balancePayoutLegs: [],
            payInId,
          },
          '2026-07-04T02:00:05.000Z',
        ),
      )
      const pgState = await client!.unsafe(
        `SELECT state FROM pay_ins WHERE id = $1`,
        [payInId],
      )
      expect(String(pgState[0]?.state)).toBe('paid')
      expect(logs).toHaveLength(0)
    })

    test('routed balance read: postgres mode returns the mirrored (equal) balance', async () => {
      const userId = nextRef('user')
      await ensureBillingAccount(sqlite.db, userId, runtime)
      const read = makeRoutedBillingBalanceRead({
        log: () => {},
        postgres: postgresStore,
        reads: 'postgres',
        wait: () => Promise.resolve(),
      })
      const viaPostgres = await read(userId, () =>
        readBillingBalanceCents(sqlite.db, userId),
      )
      expect(viaPostgres).toBe(
        await readBillingBalanceCents(sqlite.db, userId),
      )
    })

    // -----------------------------------------------------------------
    // #8337: recent-entries + auto-top-up-state bounded read serving
    // -----------------------------------------------------------------

    test('#8337 recent-entries: mirrored rows read back byte-identical via the real Postgres store', async () => {
      const userId = nextRef('user')
      await ensureBillingAccount(sqlite.db, userId, runtime)
      await applyManualBillingCredit(
        sqlite.db,
        {
          amountCents: 750,
          idempotencyKey: `billing:test:${userId}:recent`,
          reason: 'recent-entries contract credit',
          userId,
        },
        runtime,
      )

      const d1Entries = await readBillingRecentLedgerEntries(sqlite.db, userId)
      const postgresEntries = (
        await postgresStore.readRecentLedgerEntryRows(userId)
      ).map(billingLedgerEntryFromRow)
      const sortById = (
        entries: ReadonlyArray<{ id: string }>,
      ) => [...entries].sort((left, right) => left.id.localeCompare(right.id))
      expect(sortById(postgresEntries)).toEqual(sortById(d1Entries))
    })

    test('#8337 recent-entries: postgres mode SERVES the Postgres row set, not D1', async () => {
      const userId = nextRef('user')
      await ensureBillingAccount(sqlite.db, userId, runtime)
      await applyManualBillingCredit(
        sqlite.db,
        {
          amountCents: 300,
          idempotencyKey: `billing:test:${userId}:serve`,
          reason: 'recent-entries serve-proof credit',
          userId,
        },
        runtime,
      )
      const d1EntryId = (
        await readBillingRecentLedgerEntries(sqlite.db, userId)
      ).find(entry => entry.description === 'recent-entries serve-proof credit')
        ?.id
      expect(d1EntryId).toBeDefined()

      // Diverge the Postgres twin directly: a DIFFERENT description on the
      // SAME row. Real serving must read this back; D1 must still show the
      // original.
      await client!.unsafe(
        `UPDATE billing_ledger_entries SET description = $1 WHERE id = $2`,
        ['postgres_only_marker', d1EntryId],
      )

      const read = makeRoutedBillingRecentEntriesRead({
        log: () => {},
        postgres: postgresStore,
        reads: 'postgres',
      })
      const served = await read(userId, () =>
        readBillingRecentLedgerEntries(sqlite.db, userId),
      )
      expect(
        served.find(entry => entry.id === d1EntryId)?.description,
      ).toBe('postgres_only_marker')
      const stillD1 = await readBillingRecentLedgerEntries(sqlite.db, userId)
      expect(
        stillD1.find(entry => entry.id === d1EntryId)?.description,
      ).toBe('recent-entries serve-proof credit')

      // Restore the row so later parity-sensitive tests are unaffected.
      await client!.unsafe(
        `UPDATE billing_ledger_entries SET description = $1 WHERE id = $2`,
        ['recent-entries serve-proof credit', d1EntryId],
      )
    })

    test('#8337 auto-top-up state: mirrored rows compose byte-identically via the real Postgres store', async () => {
      const userId = nextRef('user')
      await upsertBillingAutoTopUpPolicy(
        sqlite.db,
        {
          amountCents: 1_500,
          enabled: true,
          monthlyCapCents: 5_000,
          thresholdCents: 300,
          userId,
        },
        runtime,
      )
      await recordBillingAutoTopUpEvent(
        sqlite.db,
        {
          amountCents: 1_500,
          idempotencyKey: `topup:contract:${userId}`,
          status: 'succeeded',
          userId,
        },
        runtime,
      )

      const d1State = await readBillingAutoTopUpState(sqlite.db, userId, runtime)
      const postgresState = billingAutoTopUpStateFromRows(
        await postgresStore.readAutoTopUpStateRows(userId),
        runtime,
      )
      expect(postgresState).toEqual(d1State)
    })

    test('#8337 auto-top-up state: postgres mode SERVES the Postgres state, not D1', async () => {
      const userId = nextRef('user')
      await upsertBillingAutoTopUpPolicy(
        sqlite.db,
        {
          amountCents: 2_000,
          enabled: true,
          monthlyCapCents: 8_000,
          thresholdCents: 400,
          userId,
        },
        runtime,
      )

      // Diverge the Postgres twin directly: pause the policy there only.
      await client!.unsafe(
        `UPDATE billing_auto_top_up_policies
            SET status = 'paused', pause_reason = 'postgres_only_marker'
          WHERE user_id = $1`,
        [userId],
      )

      const read = makeRoutedBillingAutoTopUpStateRead({
        log: () => {},
        postgres: postgresStore,
        reads: 'postgres',
      })
      const served = await read(userId, () =>
        readBillingAutoTopUpState(sqlite.db, userId, runtime),
      )
      expect(served.policy.status).toBe('paused')
      expect(served.policy.pauseReason).toBe('postgres_only_marker')
      const stillD1 = await readBillingAutoTopUpState(sqlite.db, userId, runtime)
      expect(stillD1.policy.status).toBe('active')
    })

    test('#8337: a real Postgres CONNECTION failure fails soft to D1 for both bounded reads', async () => {
      const userId = nextRef('user')
      await ensureBillingAccount(sqlite.db, userId, runtime)
      // Seed a REAL, persisted auto-top-up policy row so the D1 read
      // returns a stored `updatedAt` rather than the synthesized default
      // policy (which stamps `runtime.nowIso()` fresh on every call —
      // comparing two independent wall-clock reads would be flaky).
      await upsertBillingAutoTopUpPolicy(
        sqlite.db,
        {
          amountCents: 1_000,
          enabled: false,
          monthlyCapCents: 4_000,
          thresholdCents: 200,
          userId,
        },
        runtime,
      )
      const brokenPostgres = makePostgresBillingStore({
        acquireSql: () => Promise.reject(new Error('connection refused')),
      })
      const logs: Array<LogEntry> = []
      const log = (event: BillingSyncDiagnosticEvent, fields: { op: string }) =>
        logs.push({ event, op: fields.op })

      const recentEntriesRead = makeRoutedBillingRecentEntriesRead({
        log,
        postgres: brokenPostgres,
        reads: 'postgres',
      })
      const entries = await recentEntriesRead(userId, () =>
        readBillingRecentLedgerEntries(sqlite.db, userId),
      )
      expect(entries).toEqual(
        await readBillingRecentLedgerEntries(sqlite.db, userId),
      )

      const autoTopUpStateRead = makeRoutedBillingAutoTopUpStateRead({
        log,
        postgres: brokenPostgres,
        reads: 'postgres',
      })
      const state = await autoTopUpStateRead(userId, () =>
        readBillingAutoTopUpState(sqlite.db, userId, runtime),
      )
      expect(state).toEqual(
        await readBillingAutoTopUpState(sqlite.db, userId, runtime),
      )
      expect(
        logs.some(
          entry =>
            entry.event === 'khala_sync_billing_postgres_read_fallback',
        ),
      ).toBe(true)
    })
  },
)
