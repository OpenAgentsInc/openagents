// Credited-rung tips must have a read path (issue #4753, instance 5 of
// the projection-staleness epic #4751): every paid ladder tip projects
// a citable receipt ref and an explicit credited/swept/settled bucket
// on the recipient's tip-earnings surface, and sweep completion
// transitions the bucket instead of freezing it.
//
// These tests run the REAL write path (creditedTipStatements, sweep
// create/paid statements from the payments ledger) against an in-memory
// SQLite database behind a D1 shim, then read back through the same
// projection code the Worker serves.
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import { creditedTipStatements, readCreditedTipTotals } from '../tip-ladder'
import {
  createPayInStatements,
  markPayInForwardingStatements,
  markPayInPaidStatements,
  runLedgerStatements,
} from '../payments-ledger'
import type { PaymentsLedgerDb } from '../payments-ledger-db'
import { paymentsLedgerDbFromD1 } from '../test/payments-ledger-sqlite'
import { lookupForumPaidActionReceipt } from './paid-actions'
import {
  readForumCreatorEarnings,
  readForumTipReconciliation,
} from './tip-earnings'

const ledgerMigrationSql = readFileSync(
  resolve(import.meta.dirname, '../../migrations/0160_payments_ledger.sql'),
  'utf8',
)

const forumFixtureSql = `
CREATE TABLE forum_posts (
  id TEXT PRIMARY KEY,
  topic_id TEXT NOT NULL,
  forum_id TEXT NOT NULL,
  actor_json TEXT,
  state TEXT NOT NULL DEFAULT 'visible',
  archived_at TEXT
);
CREATE TABLE forum_money_actions (
  id TEXT PRIMARY KEY,
  action_kind TEXT NOT NULL,
  target_forum_id TEXT,
  target_topic_id TEXT,
  target_post_id TEXT,
  amount_asset TEXT NOT NULL,
  amount_value INTEGER NOT NULL,
  payment_event_id TEXT,
  earning_actor_ref TEXT,
  receipt_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE forum_receipts (
  id TEXT PRIMARY KEY,
  receipt_ref TEXT NOT NULL,
  recipient_actor_ref TEXT,
  archived_at TEXT
);
CREATE TABLE forum_payment_events (
  id TEXT PRIMARY KEY,
  public_projection_json TEXT,
  archived_at TEXT
);
CREATE TABLE forum_tip_settlement_claims (
  id TEXT PRIMARY KEY,
  receipt_id TEXT NOT NULL,
  public_projection_json TEXT,
  archived_at TEXT
);
`

type SqlValue = string | number | null

class SqliteBackedStatement implements D1PreparedStatement {
  private values: ReadonlyArray<SqlValue> = []

  constructor(
    private readonly sqlite: DatabaseSync,
    private readonly query: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values as ReadonlyArray<SqlValue>
    return this
  }

  bound(): ReadonlyArray<SqlValue> {
    return this.values
  }

  query_(): string {
    return this.query
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    const row = this.sqlite
      .prepare(this.query)
      .get(...(this.values as Array<never>))

    return Promise.resolve(row === undefined ? null : ({ ...row } as T))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    this.sqlite.prepare(this.query).run(...(this.values as Array<never>))

    return Promise.resolve({
      meta: {},
      results: [],
      success: true,
    } as unknown as D1Result<T>)
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    const rows = this.sqlite
      .prepare(this.query)
      .all(...(this.values as Array<never>))

    return Promise.resolve({
      meta: {},
      results: rows.map(row => ({ ...row })),
      success: true,
    } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

// CFG-4 (#8519): the credits tables ride the PaymentsLedgerDb seam while
// the forum fixture tables stay D1-shaped — both wrap the SAME SQLite
// database, mirroring production where forum_posts has a Postgres twin
// (KS-8.10) in the same database as the ledger.
const makeLedgerDb = (): { db: D1Database; ledgerDb: PaymentsLedgerDb } => {
  const sqlite = new DatabaseSync(':memory:')
  sqlite.exec(ledgerMigrationSql)
  // Migration 0169 adds the stored public receipt ref column.
  sqlite.exec('ALTER TABLE pay_ins ADD COLUMN public_receipt_ref TEXT;')
  sqlite.exec(forumFixtureSql)

  const db = {
    batch: (statements: ReadonlyArray<D1PreparedStatement>) => {
      sqlite.exec('BEGIN')
      const results: Array<D1Result<unknown>> = []
      for (const statement of statements) {
        const bound = statement as SqliteBackedStatement
        sqlite
          .prepare(bound.query_())
          .run(...(bound.bound() as Array<never>))
        results.push({
          meta: {},
          results: [],
          success: true,
        } as unknown as D1Result<unknown>)
      }
      sqlite.exec('COMMIT')

      return Promise.resolve(results as never)
    },
    dump: () => Promise.reject(new Error('D1 dump should not be used')),
    exec: () => Promise.reject(new Error('D1 exec should not be used')),
    prepare: (query: string) => new SqliteBackedStatement(sqlite, query),
    withSession: () => {
      throw new Error('D1 session should not be used')
    },
  } as unknown as D1Database

  return { db, ledgerDb: paymentsLedgerDbFromD1(db as never) }
}

const POST_ID = 'post_orrery_probe'
const TOPIC_ID = '9e84b2ba-1328-4990-b06e-2afa44f2ccd8'
const SENDER = 'agent:artanis'
const RECIPIENT = 'agent:orrery'
const STORED_RECEIPT_REF = 'receipt.forum.tip_ladder.sha256.feedfacefeedface'

const nowAt = (second: number): string =>
  `2026-06-11T02:00:${String(second).padStart(2, '0')}.000Z`

const seedSenderBalance = (db: D1Database, msat: number): Promise<unknown> =>
  db
    .prepare(
      `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
       VALUES (?, ?, ?, ?)`,
    )
    .bind(SENDER, msat, nowAt(0), nowAt(0))
    .run()

const seedPost = (db: D1Database): Promise<unknown> =>
  db
    .prepare(
      `INSERT INTO forum_posts (id, topic_id, forum_id, state) VALUES (?, ?, ?, 'visible')`,
    )
    .bind(POST_ID, TOPIC_ID, 'forum_pylon')
    .run()

const writeCreditedTip = (
  ledgerDb: PaymentsLedgerDb,
  input: Readonly<{
    payInId: string
    amountSat: number
    publicReceiptRef: string | null
    atSecond: number
  }>,
): Promise<void> =>
  runLedgerStatements(
    ledgerDb,
    creditedTipStatements(
      {
        amountSat: input.amountSat,
        fundingLegId: `${input.payInId}_fund`,
        idempotencyKey: `tip:${input.payInId}`,
        ladderReason: 'recipient_destination_missing',
        payInId: input.payInId,
        payoutLegId: `${input.payInId}_payout`,
        postId: POST_ID,
        publicReceiptRef: input.publicReceiptRef,
        recipientRef: RECIPIENT,
        senderRef: SENDER,
      },
      nowAt(input.atSecond),
    ),
  )

const writeSettledSweep = async (
  ledgerDb: PaymentsLedgerDb,
  input: Readonly<{ payInId: string; amountSat: number; atSecond: number }>,
): Promise<void> => {
  const amountMsat = input.amountSat * 1000
  await runLedgerStatements(ledgerDb, [
    ...createPayInStatements(
      {
        contextRef: `sweep.${RECIPIENT}`,
        costMsat: amountMsat,
        genesisId: null,
        idempotencyKey: `sweep:${input.payInId}`,
        legs: [
          {
            amountMsat,
            direction: 'in',
            externalRef: null,
            kind: 'balance',
            legId: `${input.payInId}_fund`,
            partyRef: RECIPIENT,
          },
          {
            amountMsat,
            direction: 'out',
            externalRef: 'wallet_claim.orrery',
            kind: 'lightning',
            legId: `${input.payInId}_payout`,
            partyRef: RECIPIENT,
          },
        ],
        payInId: input.payInId,
        payInType: 'sweep',
        payerRef: RECIPIENT,
        publicReceiptRef: null,
        rung: null,
      },
      nowAt(input.atSecond),
    ),
    ...markPayInForwardingStatements(input.payInId, nowAt(input.atSecond)),
    ...markPayInPaidStatements(
      { balancePayoutLegs: [], payInId: input.payInId },
      nowAt(input.atSecond),
    ),
  ])
}

const earningsFor = (db: D1Database, ledgerDb: PaymentsLedgerDb) =>
  Effect.runPromise(
    readForumCreatorEarnings(
      db,
      ledgerDb,
      { actorRef: RECIPIENT, limit: 20 },
      { nowIso: () => nowAt(59) },
    ),
  )

describe('credited-rung tip read path (#4753)', () => {
  test('a credited tip is a visible earnings row with a citable receipt ref in the credited bucket, never paid/settled', async () => {
    const { db, ledgerDb } = makeLedgerDb()
    await seedSenderBalance(db, 1_000_000)
    await seedPost(db)
    await writeCreditedTip(ledgerDb, {
      amountSat: 50,
      atSecond: 1,
      payInId: 'payin_tip_one',
      publicReceiptRef: STORED_RECEIPT_REF,
    })

    const earnings = await earningsFor(db, ledgerDb)

    expect(earnings.earnings).toHaveLength(1)
    expect(earnings.earnings[0]).toMatchObject({
      amount: { amount: 50, asset: 'sats' },
      creatorReceivedSpendableValue: false,
      earningActorRef: RECIPIENT,
      receiptRef: STORED_RECEIPT_REF,
      settlementState: 'credited',
      target: { postId: POST_ID, topicId: TOPIC_ID },
    })
    expect(earnings.earnings[0]?.tipSettlement).toMatchObject({
      settlementAuthority: 'openagents_ledger_credited',
      state: 'credited',
    })
    expect(earnings.summary).toMatchObject({
      creditedCount: 1,
      paidCount: 0,
      settledCount: 0,
      sweptCount: 0,
      totalCount: 1,
      totalCreditedSats: 50,
      totalPaidSats: 50,
      totalSettledSats: 0,
      totalSweptSats: 0,
    })
    expect(earnings.generatedAt).toBe(nowAt(59))
  })

  test('a ladder row without a stored receipt ref projects the deterministic receipt-equivalent ref and resolves through the public receipt API', async () => {
    const { db, ledgerDb } = makeLedgerDb()
    await seedSenderBalance(db, 1_000_000)
    await seedPost(db)
    await writeCreditedTip(ledgerDb, {
      amountSat: 30,
      atSecond: 1,
      payInId: 'payin_tip_legacy',
      publicReceiptRef: null,
    })

    const earnings = await earningsFor(db, ledgerDb)
    const derivedRef = 'receipt.forum.tip_ladder.payin.payin_tip_legacy'

    expect(earnings.earnings).toHaveLength(1)
    expect(earnings.earnings[0]).toMatchObject({
      receiptRef: derivedRef,
      settlementState: 'credited',
    })

    const receipt = await Effect.runPromise(
      lookupForumPaidActionReceipt(db, ledgerDb, derivedRef),
    )

    expect(receipt).toMatchObject({
      actionKind: 'post_reward',
      amount: { amount: 30, asset: 'sats' },
      receiptRef: derivedRef,
      recipientActorRef: RECIPIENT,
    })
    expect(receipt?.tipSettlement).toMatchObject({ state: 'credited' })
  })

  test('sweep completion transitions the bucket oldest-credited-first: partial sweep covers the older tip only, full coverage sweeps both', async () => {
    const { db, ledgerDb } = makeLedgerDb()
    await seedSenderBalance(db, 1_000_000)
    await seedPost(db)
    await writeCreditedTip(ledgerDb, {
      amountSat: 50,
      atSecond: 1,
      payInId: 'payin_tip_one',
      publicReceiptRef: STORED_RECEIPT_REF,
    })
    await writeCreditedTip(ledgerDb, {
      amountSat: 30,
      atSecond: 2,
      payInId: 'payin_tip_two',
      publicReceiptRef: null,
    })

    // A settled 50-sat sweep covers exactly the older tip.
    await writeSettledSweep(ledgerDb, {
      amountSat: 50,
      atSecond: 10,
      payInId: 'payin_sweep_one',
    })

    const partiallySwept = await earningsFor(db, ledgerDb)
    const byPayIn = new Map(
      partiallySwept.earnings.map(earning => [
        earning.moneyActionRef,
        earning,
      ]),
    )

    expect(byPayIn.get('pay_in:payin_tip_one')).toMatchObject({
      settlementState: 'swept',
    })
    expect(byPayIn.get('pay_in:payin_tip_one')?.tipSettlement).toMatchObject({
      creatorReceivedSpendableValue: true,
      recipientSettlementEvidence: true,
      settlementAuthority: 'recipient_wallet_direct',
      state: 'swept',
    })
    expect(byPayIn.get('pay_in:payin_tip_two')).toMatchObject({
      settlementState: 'credited',
    })
    expect(partiallySwept.summary).toMatchObject({
      creditedCount: 1,
      sweptCount: 1,
      totalCreditedSats: 30,
      totalSweptSats: 50,
    })

    // The receipt lookup reads the same transition.
    const sweptReceipt = await Effect.runPromise(
      lookupForumPaidActionReceipt(db, ledgerDb, STORED_RECEIPT_REF),
    )
    expect(sweptReceipt?.tipSettlement).toMatchObject({ state: 'swept' })

    // A second settled sweep covers the rest.
    await writeSettledSweep(ledgerDb, {
      amountSat: 30,
      atSecond: 20,
      payInId: 'payin_sweep_two',
    })

    const fullySwept = await earningsFor(db, ledgerDb)
    expect(fullySwept.summary).toMatchObject({
      creditedCount: 0,
      sweptCount: 2,
      totalCreditedSats: 0,
      totalSweptSats: 80,
    })
  })

  test('credited, swept, and settled-direct buckets stay distinguishable on one earnings surface', async () => {
    const { db, ledgerDb } = makeLedgerDb()
    await seedSenderBalance(db, 1_000_000)
    await seedPost(db)
    await writeCreditedTip(ledgerDb, {
      amountSat: 50,
      atSecond: 1,
      payInId: 'payin_tip_one',
      publicReceiptRef: STORED_RECEIPT_REF,
    })
    await writeSettledSweep(ledgerDb, {
      amountSat: 50,
      atSecond: 5,
      payInId: 'payin_sweep_one',
    })
    await writeCreditedTip(ledgerDb, {
      amountSat: 21,
      atSecond: 10,
      payInId: 'payin_tip_three',
      publicReceiptRef: null,
    })

    // A direct BOLT 12 ladder tip that settled.
    const directMsat = 40 * 1000
    await runLedgerStatements(ledgerDb, [
      ...createPayInStatements(
        {
          contextRef: `forum.post.${POST_ID}`,
          costMsat: directMsat,
          genesisId: null,
          idempotencyKey: 'tip:payin_tip_direct',
          legs: [
            {
              amountMsat: directMsat,
              direction: 'in',
              externalRef: null,
              kind: 'balance',
              legId: 'payin_tip_direct_fund',
              partyRef: SENDER,
            },
            {
              amountMsat: directMsat,
              direction: 'out',
              externalRef: 'forum.tip_recipient_claim',
              kind: 'lightning',
              legId: 'payin_tip_direct_payout',
              partyRef: RECIPIENT,
            },
          ],
          payInId: 'payin_tip_direct',
          payInType: 'tip',
          payerRef: SENDER,
          publicReceiptRef: 'receipt.forum.tip_ladder.sha256.cafecafecafecafe',
          rung: 'direct_bolt12',
        },
        nowAt(11),
      ),
      ...markPayInForwardingStatements('payin_tip_direct', nowAt(11)),
      ...markPayInPaidStatements(
        { balancePayoutLegs: [], payInId: 'payin_tip_direct' },
        nowAt(12),
      ),
    ])

    const earnings = await earningsFor(db, ledgerDb)
    const states = new Map(
      earnings.earnings.map(earning => [
        earning.moneyActionRef,
        earning.settlementState,
      ]),
    )

    expect(states.get('pay_in:payin_tip_one')).toBe('swept')
    expect(states.get('pay_in:payin_tip_three')).toBe('credited')
    expect(states.get('pay_in:payin_tip_direct')).toBe('settled')
    expect(earnings.summary).toMatchObject({
      creditedCount: 1,
      settledCount: 1,
      sweptCount: 1,
      totalCreditedSats: 21,
      totalSettledSats: 40,
      totalSweptSats: 50,
    })
  })

  test('credited and swept totals reconcile with the post tipStats credited totals at every stage', async () => {
    const { db, ledgerDb } = makeLedgerDb()
    await seedSenderBalance(db, 1_000_000)
    await seedPost(db)
    await writeCreditedTip(ledgerDb, {
      amountSat: 50,
      atSecond: 1,
      payInId: 'payin_tip_one',
      publicReceiptRef: STORED_RECEIPT_REF,
    })
    await writeCreditedTip(ledgerDb, {
      amountSat: 30,
      atSecond: 2,
      payInId: 'payin_tip_two',
      publicReceiptRef: null,
    })

    const beforeSweep = await earningsFor(db, ledgerDb)
    const tipStatsBefore = await readCreditedTipTotals(ledgerDb, [POST_ID])

    expect(
      beforeSweep.summary.totalCreditedSats +
        beforeSweep.summary.totalSweptSats,
    ).toBe(tipStatsBefore.get(POST_ID))

    await writeSettledSweep(ledgerDb, {
      amountSat: 80,
      atSecond: 10,
      payInId: 'payin_sweep_all',
    })

    const afterSweep = await earningsFor(db, ledgerDb)
    const tipStatsAfter = await readCreditedTipTotals(ledgerDb, [POST_ID])

    expect(tipStatsAfter.get(POST_ID)).toBe(80)
    expect(
      afterSweep.summary.totalCreditedSats + afterSweep.summary.totalSweptSats,
    ).toBe(80)
    expect(afterSweep.summary.totalCreditedSats).toBe(0)
    expect(afterSweep.summary.totalSweptSats).toBe(80)
  })

  test('forwarding ladder pay-ins never project as paid, credited, or settled earnings', async () => {
    const { db, ledgerDb } = makeLedgerDb()
    await seedSenderBalance(db, 1_000_000)
    await seedPost(db)

    const pendingMsat = 25 * 1000
    await runLedgerStatements(ledgerDb, [
      ...createPayInStatements(
        {
          contextRef: `forum.post.${POST_ID}`,
          costMsat: pendingMsat,
          genesisId: null,
          idempotencyKey: 'tip:payin_tip_pending',
          legs: [
            {
              amountMsat: pendingMsat,
              direction: 'in',
              externalRef: null,
              kind: 'balance',
              legId: 'payin_tip_pending_fund',
              partyRef: SENDER,
            },
            {
              amountMsat: pendingMsat,
              direction: 'out',
              externalRef: 'forum.tip_recipient_claim',
              kind: 'lightning',
              legId: 'payin_tip_pending_payout',
              partyRef: RECIPIENT,
            },
          ],
          payInId: 'payin_tip_pending',
          payInType: 'tip',
          payerRef: SENDER,
          publicReceiptRef: 'receipt.forum.tip_ladder.sha256.beefbeefbeefbeef',
          rung: 'direct_bolt12',
        },
        nowAt(1),
      ),
      ...markPayInForwardingStatements('payin_tip_pending', nowAt(1)),
    ])

    const earnings = await earningsFor(db, ledgerDb)
    expect(earnings.earnings).toHaveLength(0)
    expect(earnings.summary.totalCount).toBe(0)

    // The receipt API still resolves it, but only as observed evidence,
    // never as a settlement claim.
    const receipt = await Effect.runPromise(
      lookupForumPaidActionReceipt(
        db,
        ledgerDb,
        'receipt.forum.tip_ladder.sha256.beefbeefbeefbeef',
      ),
    )
    expect(receipt?.paymentEvent?.status).toBe('observed')
    expect(receipt?.tipSettlement.creatorReceivedSpendableValue).toBe(false)
    expect(['settled', 'swept', 'paid', 'credited']).not.toContain(
      receipt?.tipSettlement.state,
    )
  })

  test('the auditor reconciliation surface carries the credited bucket with generatedAt honesty', async () => {
    const { db, ledgerDb } = makeLedgerDb()
    await seedSenderBalance(db, 1_000_000)
    await seedPost(db)
    await writeCreditedTip(ledgerDb, {
      amountSat: 50,
      atSecond: 1,
      payInId: 'payin_tip_one',
      publicReceiptRef: STORED_RECEIPT_REF,
    })

    const reconciliation = await Effect.runPromise(
      readForumTipReconciliation(
        db,
        ledgerDb,
        { actorRef: null, limit: 20 },
        { nowIso: () => nowAt(59) },
      ),
    )

    expect(reconciliation.generatedAt).toBe(nowAt(59))
    expect(reconciliation.earnings).toHaveLength(1)
    expect(reconciliation.earnings[0]).toMatchObject({
      receiptRef: STORED_RECEIPT_REF,
      settlementState: 'credited',
    })
    expect(reconciliation.summary).toMatchObject({
      creditedCount: 1,
      totalCreditedSats: 50,
    })
  })
})
