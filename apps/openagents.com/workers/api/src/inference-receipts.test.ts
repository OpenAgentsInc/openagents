// KS-8.7 follow-up (#8337): the inference/pay-in public receipt read
// (`readInferenceReceiptByRef` / `listRecentInferenceReceipts`) — scoped to
// `pay_in_type IN ('adjustment', 'usd_credit_grant')` and an immutable,
// already-settled `public_receipt_ref` — is one of the four bounded
// Postgres-served read surfaces named in `billing-store.ts`'s
// `BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES`.
//
// This suite proves, against REAL SQLite (the D1 engine) and a throwaway
// REAL local Postgres carrying khala-sync migration 0015:
//   - PARITY: the Postgres-served store answers byte-identically to the D1
//     store for the same fixture, for both `readInferenceReceiptByRef` and
//     `listRecentInferenceReceipts`.
//   - REAL SERVING: `postgres` mode reads back a value diverged directly on
//     the Postgres twin.
//   - FAIL-SOFT: a broken Postgres query never fails the read; it falls
//     back to D1 and logs the typed fallback diagnostic.
//   - COMPARE MODE: serves D1 always, logs divergence only on real
//     disagreement.
//   - THE FREE-ALLOWANCE CARVE-OUT: a `receipt.inference.free.*` ref reads
//     a DIFFERENT domain's table (`inference_free_usage_events`, no live
//     Postgres mirror in this lane) — the Postgres store must refuse to
//     serve it (`InferenceReceiptPostgresNotServableError`) and the router
//     must transparently fall back to D1 for that shape, in EVERY mode.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  InferenceReceiptPostgresNotServableError,
  makeD1InferenceReceiptStore,
  makePostgresInferenceReceiptStore,
  makeReadsRoutedInferenceReceiptStore,
  type InferenceReceiptStore,
} from './inference-receipts'
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

describe.skipIf(!hasLocalPostgres())(
  'inference receipt — D1 authority + #8337 bounded Postgres serving',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let sqlite: ReturnType<typeof makeSqliteD1>

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE inference_receipt_contract')
      await admin.end({ timeout: 5 })

      const raw = postgres(pg.urlFor('inference_receipt_contract'), {
        max: 4,
        prepare: false,
      })
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

    /** Insert the SAME paid `pay_ins` charge row on both stores. */
    const seedInferenceChargePayIn = async (payInId: string): Promise<void> => {
      const now = '2026-07-04T00:00:00.000Z'
      const receiptRef = `receipt.inference.charge.${payInId}`
      const idempotencyKey = `inference:charge:${payInId}`

      const insert = (
        db: Readonly<{
          prepare: (sql: string) => {
            bind: (...values: Array<unknown>) => { run: () => Promise<unknown> }
          }
        }>,
      ) =>
        db
          .prepare(
            `INSERT INTO pay_ins
              (id, pay_in_type, payer_ref, cost_msat, state, failure_reason,
               rung, context_ref, idempotency_key, genesis_id, successor_id,
               created_at, state_changed_at, public_receipt_ref)
             VALUES (?, 'adjustment', ?, 1000, 'paid', NULL, NULL, ?, ?, NULL,
               NULL, ?, ?, ?)`,
          )
          .bind(
            payInId,
            `agent:${payInId}`,
            `inference.charge.${payInId}`,
            idempotencyKey,
            now,
            now,
            receiptRef,
          )
          .run()

      await insert(sqlite.db as never)

      const row = await sqlite.db
        .prepare(`SELECT * FROM pay_ins WHERE id = ?`)
        .bind(payInId)
        .all<Record<string, unknown>>()
      const r = row.results[0]!

      await client!.unsafe(
        `INSERT INTO pay_ins
          (id, pay_in_type, payer_ref, cost_msat, state, failure_reason,
           rung, context_ref, idempotency_key, genesis_id, successor_id,
           created_at, state_changed_at, public_receipt_ref)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         ON CONFLICT (id) DO NOTHING`,
        [
          r['id'],
          r['pay_in_type'],
          r['payer_ref'],
          r['cost_msat'],
          r['state'],
          r['failure_reason'],
          r['rung'],
          r['context_ref'],
          r['idempotency_key'],
          r['genesis_id'],
          r['successor_id'],
          r['created_at'],
          r['state_changed_at'],
          r['public_receipt_ref'],
        ],
      )

      return
    }

    const query = (
      text: string,
      params: ReadonlyArray<unknown>,
    ): Promise<ReadonlyArray<Readonly<Record<string, unknown>>>> =>
      client!.unsafe(text, [...params])

    test('parity: readInferenceReceiptByRef answers byte-identically to D1', async () => {
      const payInId = nextRef('payin')
      await seedInferenceChargePayIn(payInId)
      const receiptRef = `receipt.inference.charge.${payInId}`

      const d1Store = makeD1InferenceReceiptStore(sqlite.db)
      const postgresStore = makePostgresInferenceReceiptStore(query)

      const d1Result = await d1Store.readInferenceReceiptByRef(receiptRef)
      const postgresResult =
        await postgresStore.readInferenceReceiptByRef(receiptRef)
      expect(postgresResult).toEqual(d1Result)
      expect(d1Result?.state).toBe('paid')
    })

    test('parity: listRecentInferenceReceipts answers byte-identically to D1', async () => {
      const payInId = nextRef('payin')
      await seedInferenceChargePayIn(payInId)

      const d1Store = makeD1InferenceReceiptStore(sqlite.db)
      const postgresStore = makePostgresInferenceReceiptStore(query)

      const d1Result = await d1Store.listRecentInferenceReceipts(50)
      const postgresResult =
        await postgresStore.listRecentInferenceReceipts(50)
      const sortByRef = (rows: ReadonlyArray<{ receiptRef: string }>) =>
        [...rows].sort((left, right) =>
          left.receiptRef.localeCompare(right.receiptRef),
        )
      expect(sortByRef(postgresResult)).toEqual(sortByRef(d1Result))
      expect(d1Result.length).toBeGreaterThan(0)
    })

    test('postgres mode SERVES the Postgres row, not D1 (real serving)', async () => {
      const payInId = nextRef('payin')
      await seedInferenceChargePayIn(payInId)
      const receiptRef = `receipt.inference.charge.${payInId}`

      // Diverge the Postgres twin directly.
      await client!.unsafe(
        `UPDATE pay_ins SET state = 'failed' WHERE id = $1`,
        [payInId],
      )

      const d1Store = makeD1InferenceReceiptStore(sqlite.db)
      const postgresStore = makePostgresInferenceReceiptStore(query)
      const logs: Array<LogEntry> = []
      const routed = makeReadsRoutedInferenceReceiptStore({
        d1: d1Store,
        log: (event, fields) => logs.push({ event, op: fields.op }),
        postgres: postgresStore,
        reads: 'postgres',
      })

      const served = await routed.readInferenceReceiptByRef(receiptRef)
      expect(served?.state).toBe('failed')
      const stillD1 = await d1Store.readInferenceReceiptByRef(receiptRef)
      expect(stillD1?.state).toBe('paid')
      expect(
        logs.some(
          entry => entry.event === 'khala_sync_billing_postgres_read_fallback',
        ),
      ).toBe(false)
    })

    test('postgres mode: a broken Postgres query fails soft to D1', async () => {
      const payInId = nextRef('payin')
      await seedInferenceChargePayIn(payInId)
      const receiptRef = `receipt.inference.charge.${payInId}`

      const d1Store = makeD1InferenceReceiptStore(sqlite.db)
      const brokenPostgres = makePostgresInferenceReceiptStore(() =>
        Promise.reject(new Error('pg down')),
      )
      const logs: Array<LogEntry> = []
      const routed = makeReadsRoutedInferenceReceiptStore({
        d1: d1Store,
        log: (event, fields) => logs.push({ event, op: fields.op }),
        postgres: brokenPostgres,
        reads: 'postgres',
      })

      const served = await routed.readInferenceReceiptByRef(receiptRef)
      expect(served?.state).toBe('paid')
      const listed = await routed.listRecentInferenceReceipts(10)
      expect(listed.some(r => r.receiptRef === receiptRef)).toBe(true)
      expect(
        logs.filter(
          entry => entry.event === 'khala_sync_billing_postgres_read_fallback',
        ).length,
      ).toBeGreaterThanOrEqual(2)
    })

    test('compare mode: serves D1 always; logs divergence only when the stores disagree', async () => {
      const payInId = nextRef('payin')
      await seedInferenceChargePayIn(payInId)
      const receiptRef = `receipt.inference.charge.${payInId}`

      const d1Store = makeD1InferenceReceiptStore(sqlite.db)
      const postgresStore = makePostgresInferenceReceiptStore(query)

      const agreeing: Array<LogEntry> = []
      const agreeingRouted = makeReadsRoutedInferenceReceiptStore({
        d1: d1Store,
        log: (event, fields) => agreeing.push({ event, op: fields.op }),
        postgres: postgresStore,
        reads: 'compare',
      })
      const agreeingResult =
        await agreeingRouted.readInferenceReceiptByRef(receiptRef)
      expect(agreeingResult?.state).toBe('paid')
      expect(agreeing).toHaveLength(0)

      await client!.unsafe(
        `UPDATE pay_ins SET state = 'failed' WHERE id = $1`,
        [payInId],
      )
      const diverging: Array<LogEntry> = []
      const divergingRouted = makeReadsRoutedInferenceReceiptStore({
        d1: d1Store,
        log: (event, fields) => diverging.push({ event, op: fields.op }),
        postgres: postgresStore,
        reads: 'compare',
      })
      const divergingResult =
        await divergingRouted.readInferenceReceiptByRef(receiptRef)
      expect(divergingResult?.state).toBe('paid')
      expect(
        diverging.some(
          entry => entry.event === 'khala_sync_billing_read_compare_mismatch',
        ),
      ).toBe(true)
    })

    test('the free-allowance carve-out: Postgres refuses to serve; the router always falls back to D1', async () => {
      const postgresStore = makePostgresInferenceReceiptStore(query)
      await expect(
        postgresStore.readInferenceReceiptByRef(
          'receipt.inference.free.some_request_id',
        ),
      ).rejects.toBeInstanceOf(InferenceReceiptPostgresNotServableError)

      // The D1-only fixture: a free-allowance-style receipt exists ONLY as
      // a hand-built D1 double (no `inference_free_usage_events` table in
      // this suite's SQLite schema — this proves the ROUTER'S fallback
      // behavior, not the D1 free-lookup SQL itself, which is covered by
      // `makeD1InferenceReceiptStore`'s own production wiring).
      const freeRef = 'receipt.inference.free.req_1'
      const fakeD1: InferenceReceiptStore = {
        listRecentInferenceReceipts: () => Promise.resolve([]),
        readInferenceReceiptByRef: async ref =>
          ref === freeRef
            ? {
                contextRef: null,
                createdAt: '2026-07-04T00:00:00.000Z',
                payInType: 'free_allowance',
                receiptRef: freeRef,
                state: 'paid',
                stateChangedAt: '2026-07-04T00:00:00.000Z',
              }
            : null,
      }
      const logs: Array<LogEntry> = []
      const routedPostgresMode = makeReadsRoutedInferenceReceiptStore({
        d1: fakeD1,
        log: (event, fields) => logs.push({ event, op: fields.op }),
        postgres: postgresStore,
        reads: 'postgres',
      })
      const served = await routedPostgresMode.readInferenceReceiptByRef(freeRef)
      expect(served?.payInType).toBe('free_allowance')
      expect(
        logs.some(
          entry => entry.event === 'khala_sync_billing_postgres_read_fallback',
        ),
      ).toBe(true)

      logs.length = 0
      const routedCompareMode = makeReadsRoutedInferenceReceiptStore({
        d1: fakeD1,
        log: (event, fields) => logs.push({ event, op: fields.op }),
        postgres: postgresStore,
        reads: 'compare',
      })
      const compareServed =
        await routedCompareMode.readInferenceReceiptByRef(freeRef)
      expect(compareServed?.payInType).toBe('free_allowance')
      expect(
        logs.some(
          entry => entry.event === 'khala_sync_billing_postgres_read_failed',
        ),
      ).toBe(true)
      // Never a mismatch — the "not servable" case is a known limitation,
      // not real drift between the two stores.
      expect(
        logs.some(
          entry => entry.event === 'khala_sync_billing_read_compare_mismatch',
        ),
      ).toBe(false)
    })
  },
)
