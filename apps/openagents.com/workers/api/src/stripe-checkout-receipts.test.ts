// KS-8.7 follow-up (#8337): the Stripe checkout public receipt read —
// ALREADY-SETTLED, immutable once fulfilled, never gates a live
// checkout/webhook/spend decision — is one of the four bounded Postgres-
// served read surfaces named in `billing-store.ts`'s
// `BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES`.
//
// This suite proves, against REAL SQLite (the D1 engine) and a throwaway
// REAL local Postgres carrying khala-sync migration 0015:
//   - PARITY: the Postgres-served store answers byte-identically to the D1
//     store for the same fixture.
//   - REAL SERVING: `postgres` mode reads back a value diverged directly on
//     the Postgres twin, proving this is genuine serving, not a shadow
//     compare that always answers D1.
//   - FAIL-SOFT: a broken Postgres query (or connection) NEVER fails the
//     read; it falls back to D1 and logs the typed fallback diagnostic.
//   - COMPARE MODE: serves D1 always, logs divergence only when the two
//     stores actually disagree.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  makeD1StripeCheckoutReceiptStore,
  makePostgresStripeCheckoutReceiptStore,
  makeReadsRoutedStripeCheckoutReceiptStore,
  stripeCheckoutReceiptRef,
  type PublicStripeCheckoutReceiptProjection,
} from './stripe-checkout-receipts'
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

type LogEntry = Readonly<{ event: string; op: string }>

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}_receipt_${++refCounter}`

const GENERATED_AT = '2026-07-05T00:00:00.000Z'

describe.skipIf(!hasLocalPostgres())(
  'stripe checkout receipt — D1 authority + #8337 bounded Postgres serving',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1>

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE stripe_checkout_receipt_contract')
      await admin.end({ timeout: 5 })

      const raw = postgres(
        pg.urlFor('stripe_checkout_receipt_contract'),
        { max: 4, prepare: false },
      )
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0015, 'utf8'))

      sqlite = makeSqliteD1()
      sqlite.exec(BILLING_DOMAIN_D1_SCHEMA)
    }, 120_000)

    afterAll(async () => {
      sqlite?.close()
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    /** Insert the SAME checkout session + ledger row on both stores. */
    const seedFulfilledCheckout = async (
      sessionId: string,
    ): Promise<void> => {
      const now = '2026-07-04T00:00:00.000Z'
      await sqlite.db
        .prepare(
          `INSERT INTO stripe_checkout_sessions
            (session_id, user_id, package_id, amount_cents, currency,
             payment_status, fulfillment_status, ledger_entry_id,
             stripe_customer_id, checkout_url, created_at, updated_at)
           VALUES (?, ?, 'pack_25', 2500, 'USD', 'paid', 'fulfilled', ?,
             'cus_test', NULL, ?, ?)`,
        )
        .bind(sessionId, `user_${sessionId}`, `ledger_${sessionId}`, now, now)
        .run()
      await sqlite.db
        .prepare(
          `INSERT INTO billing_ledger_entries
            (id, user_id, team_id, run_id, source, description, amount_cents,
             currency, quantity, unit, unit_rate_cents, metadata_json,
             idempotency_key, created_at)
           VALUES (?, ?, NULL, NULL, 'stripe_checkout', 'checkout credit', 2500,
             'USD', NULL, NULL, NULL, '{}', ?, ?)`,
        )
        .bind(
          `ledger_${sessionId}`,
          `user_${sessionId}`,
          `billing:stripe-checkout:${sessionId}`,
          now,
        )
        .run()

      const checkoutRows = await sqlite.db
        .prepare(`SELECT * FROM stripe_checkout_sessions WHERE session_id = ?`)
        .bind(sessionId)
        .all<Record<string, unknown>>()
      const ledgerRows = await sqlite.db
        .prepare(`SELECT * FROM billing_ledger_entries WHERE id = ?`)
        .bind(`ledger_${sessionId}`)
        .all<Record<string, unknown>>()

      await client!.unsafe(
        `INSERT INTO stripe_checkout_sessions
          (session_id, user_id, package_id, amount_cents, currency,
           payment_status, fulfillment_status, ledger_entry_id,
           stripe_customer_id, checkout_url, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (session_id) DO NOTHING`,
        [
          checkoutRows.results[0]!['session_id'],
          checkoutRows.results[0]!['user_id'],
          checkoutRows.results[0]!['package_id'],
          checkoutRows.results[0]!['amount_cents'],
          checkoutRows.results[0]!['currency'],
          checkoutRows.results[0]!['payment_status'],
          checkoutRows.results[0]!['fulfillment_status'],
          checkoutRows.results[0]!['ledger_entry_id'],
          checkoutRows.results[0]!['stripe_customer_id'],
          checkoutRows.results[0]!['checkout_url'],
          checkoutRows.results[0]!['created_at'],
          checkoutRows.results[0]!['updated_at'],
        ],
      )
      await client!.unsafe(
        `INSERT INTO billing_ledger_entries
          (id, user_id, team_id, run_id, source, description, amount_cents,
           currency, quantity, unit, unit_rate_cents, metadata_json,
           idempotency_key, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         ON CONFLICT (id) DO NOTHING`,
        [
          ledgerRows.results[0]!['id'],
          ledgerRows.results[0]!['user_id'],
          ledgerRows.results[0]!['team_id'],
          ledgerRows.results[0]!['run_id'],
          ledgerRows.results[0]!['source'],
          ledgerRows.results[0]!['description'],
          ledgerRows.results[0]!['amount_cents'],
          ledgerRows.results[0]!['currency'],
          ledgerRows.results[0]!['quantity'],
          ledgerRows.results[0]!['unit'],
          ledgerRows.results[0]!['unit_rate_cents'],
          ledgerRows.results[0]!['metadata_json'],
          ledgerRows.results[0]!['idempotency_key'],
          ledgerRows.results[0]!['created_at'],
        ],
      )
    }

    const query = (
      text: string,
      params: ReadonlyArray<unknown>,
    ): Promise<ReadonlyArray<Readonly<Record<string, unknown>>>> =>
      client!.unsafe(text, [...params])

    test('parity: the Postgres-served store answers byte-identically to D1', async () => {
      const sessionId = nextRef('cs')
      await seedFulfilledCheckout(sessionId)
      const receiptRef = stripeCheckoutReceiptRef(sessionId)

      const d1Store = makeD1StripeCheckoutReceiptStore(sqlite.db)
      const postgresStore = makePostgresStripeCheckoutReceiptStore(query)

      const d1Result = await d1Store.readStripeCheckoutReceipt(
        receiptRef,
        GENERATED_AT,
      )
      const postgresResult = await postgresStore.readStripeCheckoutReceipt(
        receiptRef,
        GENERATED_AT,
      )
      expect(postgresResult).toEqual(d1Result)
      expect(d1Result?.resolution.status).toBe('ok')
    })

    test('postgres mode SERVES the Postgres row, not D1 (real serving, not shadow compare)', async () => {
      const sessionId = nextRef('cs')
      await seedFulfilledCheckout(sessionId)
      const receiptRef = stripeCheckoutReceiptRef(sessionId)

      // Diverge the Postgres twin directly: mark it unpaid there only.
      await client!.unsafe(
        `UPDATE stripe_checkout_sessions SET payment_status = 'unpaid' WHERE session_id = $1`,
        [sessionId],
      )

      const d1Store = makeD1StripeCheckoutReceiptStore(sqlite.db)
      const postgresStore = makePostgresStripeCheckoutReceiptStore(query)
      const logs: Array<LogEntry> = []
      const routed = makeReadsRoutedStripeCheckoutReceiptStore({
        d1: d1Store,
        log: (event, fields) => logs.push({ event, op: fields.op }),
        postgres: postgresStore,
        reads: 'postgres',
      })

      const served = await routed.readStripeCheckoutReceipt(
        receiptRef,
        GENERATED_AT,
      )
      expect(served?.resolution.status).toBe('pending')
      const stillD1 = await d1Store.readStripeCheckoutReceipt(
        receiptRef,
        GENERATED_AT,
      )
      expect(stillD1?.resolution.status).toBe('ok')
      expect(
        logs.some(
          entry => entry.event === 'khala_sync_billing_postgres_read_fallback',
        ),
      ).toBe(false)
    })

    test('postgres mode: a broken Postgres query fails soft to D1', async () => {
      const sessionId = nextRef('cs')
      await seedFulfilledCheckout(sessionId)
      const receiptRef = stripeCheckoutReceiptRef(sessionId)

      const d1Store = makeD1StripeCheckoutReceiptStore(sqlite.db)
      const brokenPostgres = makePostgresStripeCheckoutReceiptStore(() =>
        Promise.reject(new Error('pg down')),
      )
      const logs: Array<LogEntry> = []
      const routed = makeReadsRoutedStripeCheckoutReceiptStore({
        d1: d1Store,
        log: (event, fields) => logs.push({ event, op: fields.op }),
        postgres: brokenPostgres,
        reads: 'postgres',
      })

      const served = await routed.readStripeCheckoutReceipt(
        receiptRef,
        GENERATED_AT,
      )
      expect(served?.resolution.status).toBe('ok')
      expect(
        logs.some(
          entry => entry.event === 'khala_sync_billing_postgres_read_fallback',
        ),
      ).toBe(true)
    })

    test('compare mode: serves D1 always; logs divergence only when the stores disagree', async () => {
      const sessionId = nextRef('cs')
      await seedFulfilledCheckout(sessionId)
      const receiptRef = stripeCheckoutReceiptRef(sessionId)

      const d1Store = makeD1StripeCheckoutReceiptStore(sqlite.db)
      const postgresStore = makePostgresStripeCheckoutReceiptStore(query)

      const agreeing: Array<LogEntry> = []
      const agreeingRouted = makeReadsRoutedStripeCheckoutReceiptStore({
        d1: d1Store,
        log: (event, fields) => agreeing.push({ event, op: fields.op }),
        postgres: postgresStore,
        reads: 'compare',
      })
      const agreeingResult = await agreeingRouted.readStripeCheckoutReceipt(
        receiptRef,
        GENERATED_AT,
      )
      expect(agreeingResult?.resolution.status).toBe('ok')
      expect(agreeing).toHaveLength(0)

      // Diverge the Postgres twin, then compare again: D1 still serves, but
      // the divergence is logged.
      await client!.unsafe(
        `UPDATE stripe_checkout_sessions SET fulfillment_status = 'pending' WHERE session_id = $1`,
        [sessionId],
      )
      const diverging: Array<LogEntry> = []
      const divergingRouted = makeReadsRoutedStripeCheckoutReceiptStore({
        d1: d1Store,
        log: (event, fields) => diverging.push({ event, op: fields.op }),
        postgres: postgresStore,
        reads: 'compare',
      })
      const divergingResult: PublicStripeCheckoutReceiptProjection | null =
        await divergingRouted.readStripeCheckoutReceipt(
          receiptRef,
          GENERATED_AT,
        )
      expect(divergingResult?.resolution.status).toBe('ok')
      expect(
        diverging.some(
          entry => entry.event === 'khala_sync_billing_read_compare_mismatch',
        ),
      ).toBe(true)
    })
  },
)
