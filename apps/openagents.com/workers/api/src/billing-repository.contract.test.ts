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
  ensureBillingAccount,
  readBillingBalanceCents,
  recordBillingAutoTopUpEvent,
  redeemBillingCoupon,
  systemBillingRuntime,
  upsertBillingAutoTopUpPolicy,
  type BillingRuntime,
} from './billing'
import {
  billingSyncFlagsFromEnv,
  billingRuntimeForEnv,
  makeBillingDomainMirror,
  makePostgresBillingStore,
  makeRoutedBillingBalanceRead,
  type BillingSyncDiagnosticEvent,
  type PostgresBillingStore,
} from './billing-store'
import {
  createPayInStatements,
  markPayInPaidStatements,
  runLedgerStatements,
} from './payments-ledger'
import { BILLING_DOMAIN_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

const MIGRATION_0015 = path.resolve(
  import.meta.dirname,
  '../../../../../packages/khala-sync-server/migrations/0015_billing_pay_ins.sql',
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

  test('compare mode serves D1 and logs cent-level divergence', async () => {
    const logs: Array<LogEntry> = []
    const read = makeRoutedBillingBalanceRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
      postgres: { readBalanceCents: () => Promise.resolve(999) },
      reads: 'compare',
    })
    expect(await read('user-x', () => Promise.resolve(1_000))).toBe(1_000)
    expect(
      logs.some(
        entry => entry.event === 'khala_sync_billing_read_compare_mismatch',
      ),
    ).toBe(true)

    logs.length = 0
    expect(await read('user-x', () => Promise.resolve(999))).toBe(999)
    expect(logs).toHaveLength(0)
  })

  test('compare mode: a Postgres failure logs and still serves D1', async () => {
    const logs: Array<LogEntry> = []
    const read = makeRoutedBillingBalanceRead({
      log: (event, fields) => logs.push({ event, op: fields.op }),
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

    test('pay-in lifecycle: create + paid transition mirror rows and legs byte-exactly', async () => {
      const payInId = nextRef('payin')
      const actorRef = `agent:${nextRef('actor')}`
      const now = '2026-07-04T02:00:00.000Z'

      // Fund the payer balance so the funding debit clears its CHECK.
      await sqlite.db
        .prepare(
          `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
           VALUES (?, 10000, ?, ?)`,
        )
        .bind(actorRef, now, now)
        .run()

      await runLedgerStatements(
        sqlite.db,
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
        runtime.mirror,
      )

      await expectRowParity('pay_ins', 'id', payInId)
      await expectRowParity('pay_in_legs', 'pay_in_id', payInId)

      // State machine advances on D1; the mirror converges the same bytes.
      await runLedgerStatements(
        sqlite.db,
        markPayInPaidStatements(
          {
            balancePayoutLegs: [],
            payInId,
          },
          '2026-07-04T02:00:05.000Z',
        ),
        runtime.mirror,
      )
      const pgState = await client!.unsafe(
        `SELECT state FROM pay_ins WHERE id = $1`,
        [payInId],
      )
      expect(String(pgState[0]?.state)).toBe('paid')
      await expectRowParity('pay_ins', 'id', payInId)
      await expectRowParity('pay_in_legs', 'pay_in_id', payInId)
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
  },
)
