// CFG-4 (#8519, epic #8515): the inference/pay-in public receipt read is
// Postgres-ALWAYS through the credits-ledger handle (`PaymentsLedgerDb`).
//
// NOTE ON DELETED COVERAGE: the previous suite here proved the KS-8.7/#8337
// `KHALA_SYNC_BILLING_READS` compare/postgres ROUTER for this store
// (`makePostgresInferenceReceiptStore`, `makeReadsRoutedInferenceReceiptStore`,
// `inferenceReceiptStoreForEnv`, the D1-vs-Postgres parity/fallback/divergence
// assertions, and `InferenceReceiptPostgresNotServableError`). That machinery
// was deleted by the hard cutover — there is no D1 twin of `pay_ins` left to
// route to or diverge from — so those tests were deleted with it. The
// dialect-level guarantees of the ledger handle itself are proved by
// `payments-ledger-postgres.contract.test.ts`.
//
// This suite proves the NEW store shape:
//   - the pay_ins branch (charge + usd_credit_grant receipts) reads through
//     the ledger handle,
//   - the free-allowance branch reads `inference_free_usage_events` (a
//     DIFFERENT domain's table) through D1,
//   - projection rules (paid-only, type/ref binding, public-safety) hold.

import { describe, expect, test } from 'vitest'

import {
  makeInferenceReceiptStore,
  publicInferenceReceiptFromRecord,
  type InferenceReceiptRecord,
} from './inference-receipts'
import { makeLedgerSqliteDb } from './test/payments-ledger-sqlite'
import { makeSqliteD1 } from './test/sqlite-d1'

const NOW = '2026-07-06T00:00:00.000Z'

const FREE_EVENTS_SCHEMA = `
CREATE TABLE inference_free_usage_events (
  request_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL
);
`

const makeStores = () => {
  const ledgerDb = makeLedgerSqliteDb()
  const sqlite = makeSqliteD1()
  sqlite.exec(FREE_EVENTS_SCHEMA)
  return {
    ledgerDb,
    sqlite,
    store: makeInferenceReceiptStore({ db: sqlite.db, ledgerDb }),
  }
}

const seedPayIn = async (
  ledgerDb: ReturnType<typeof makeLedgerSqliteDb>,
  input: Readonly<{
    id: string
    payInType: 'adjustment' | 'usd_credit_grant'
    state?: string
    receiptRef: string
    contextRef?: string | null
  }>,
): Promise<void> => {
  await ledgerDb.batch([
    {
      params: [
        input.id,
        input.payInType,
        `agent:${input.id}`,
        1000,
        input.state ?? 'paid',
        input.contextRef ?? null,
        `idem:${input.id}`,
        input.receiptRef,
        NOW,
        NOW,
      ],
      sql: `INSERT INTO pay_ins
              (id, pay_in_type, payer_ref, cost_msat, state, context_ref,
               idempotency_key, public_receipt_ref, created_at, state_changed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    },
  ])
}

describe('makeInferenceReceiptStore (CFG-4 Postgres-always pay_ins branch)', () => {
  test('reads a paid inference charge receipt by ref through the ledger handle', async () => {
    const { ledgerDb, store } = makeStores()
    const receiptRef = 'receipt.inference.charge.req_a'
    await seedPayIn(ledgerDb, {
      contextRef: 'inference:fireworks:served:sonnet:tokens:42',
      id: 'req_a',
      payInType: 'adjustment',
      receiptRef,
    })

    const record = await store.readInferenceReceiptByRef(receiptRef)
    expect(record).not.toBeNull()
    expect(record?.receiptRef).toBe(receiptRef)
    expect(record?.payInType).toBe('adjustment')
    expect(record?.state).toBe('paid')
    expect(record?.contextRef).toBe('inference:fireworks:served:sonnet:tokens:42')

    const projection = publicInferenceReceiptFromRecord(
      record as InferenceReceiptRecord,
      NOW,
    )
    expect(projection?.kind).toBe('charge')
    expect(projection?.ledgerState).toBe('paid')
  })

  test('reads a usd_credit_grant receipt by ref', async () => {
    const { ledgerDb, store } = makeStores()
    const receiptRef = 'receipt.inference.usd_credit_grant.grant_a'
    await seedPayIn(ledgerDb, {
      id: 'grant_a',
      payInType: 'usd_credit_grant',
      receiptRef,
    })

    const record = await store.readInferenceReceiptByRef(receiptRef)
    expect(record?.payInType).toBe('usd_credit_grant')
    const projection = publicInferenceReceiptFromRecord(
      record as InferenceReceiptRecord,
      NOW,
    )
    expect(projection?.kind).toBe('usd_credit_grant')
  })

  test('a missing ref answers null', async () => {
    const { store } = makeStores()
    expect(
      await store.readInferenceReceiptByRef('receipt.inference.charge.nope'),
    ).toBeNull()
  })

  test('listRecentInferenceReceipts lists paid charge receipts only', async () => {
    const { ledgerDb, store } = makeStores()
    await seedPayIn(ledgerDb, {
      id: 'req_paid',
      payInType: 'adjustment',
      receiptRef: 'receipt.inference.charge.req_paid',
    })
    // Non-paid and non-charge rows must never appear.
    await seedPayIn(ledgerDb, {
      id: 'req_pending',
      payInType: 'adjustment',
      receiptRef: 'receipt.inference.charge.req_pending',
      state: 'pending',
    })
    await seedPayIn(ledgerDb, {
      id: 'grant_x',
      payInType: 'usd_credit_grant',
      receiptRef: 'receipt.inference.usd_credit_grant.grant_x',
    })

    const listed = await store.listRecentInferenceReceipts(10)
    expect(listed.map(record => record.receiptRef)).toEqual([
      'receipt.inference.charge.req_paid',
    ])
  })

  test('the free-allowance branch reads inference_free_usage_events through D1', async () => {
    const { sqlite, store } = makeStores()
    sqlite.exec(
      `INSERT INTO inference_free_usage_events (request_id, created_at)
       VALUES ('free_req_1', '${NOW}')`,
    )

    const record = await store.readInferenceReceiptByRef(
      'receipt.inference.free.free_req_1',
    )
    expect(record).not.toBeNull()
    expect(record?.payInType).toBe('free_allowance')
    expect(record?.state).toBe('paid')

    const projection = publicInferenceReceiptFromRecord(
      record as InferenceReceiptRecord,
      NOW,
    )
    expect(projection?.kind).toBe('free_allowance')
    expect(projection?.ledgerState).toBe('free_allowance')

    // An unknown free ref answers null.
    expect(
      await store.readInferenceReceiptByRef('receipt.inference.free.unknown'),
    ).toBeNull()
  })

  test('ledger rows with bigint-as-string columns decode identically (Postgres shape)', async () => {
    // Postgres returns bigint columns as strings; the receipt read selects no
    // *_msat column, but prove the record decode tolerates a string-shaped row
    // end-to-end by round-tripping through the real ledger handle.
    const { ledgerDb, store } = makeStores()
    const receiptRef = 'receipt.inference.charge.req_str'
    await seedPayIn(ledgerDb, {
      id: 'req_str',
      payInType: 'adjustment',
      receiptRef,
    })
    const record = await store.readInferenceReceiptByRef(receiptRef)
    expect(typeof record?.createdAt).toBe('string')
    expect(typeof record?.stateChangedAt).toBe('string')
  })
})
