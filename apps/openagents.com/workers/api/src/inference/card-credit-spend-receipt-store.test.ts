import { Effect } from 'effect'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../payments-ledger'
import { cardCreditSpendReceiptRef } from './card-credit-spend-receipt'
import { makeD1CardCreditSpendReceiptStore } from './card-credit-spend-receipt-store'
import { makeLedgerMeteringHook } from './metering-hook'
import {
  fundInferenceFromCredit,
  usdCreditGrantReceiptRef,
} from './usd-credit-bridge'

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
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: T[] }> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as T[]
    return { results }
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

const SCHEMA = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  held_msat INTEGER NOT NULL DEFAULT 0,
  usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL CHECK (
    pay_in_type IN ('tip','sweep','buffer_funding','reward','adjustment','usd_credit_grant')
  ),
  payer_ref TEXT NOT NULL,
  cost_msat INTEGER NOT NULL CHECK (cost_msat > 0),
  state TEXT NOT NULL,
  failure_reason TEXT,
  rung TEXT,
  context_ref TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  public_receipt_ref TEXT,
  genesis_id TEXT,
  successor_id TEXT,
  created_at TEXT NOT NULL,
  state_changed_at TEXT NOT NULL
);
CREATE TABLE pay_in_legs (
  id TEXT PRIMARY KEY,
  pay_in_id TEXT NOT NULL,
  direction TEXT NOT NULL,
  kind TEXT NOT NULL,
  party_ref TEXT NOT NULL,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat INTEGER,
  external_ref TEXT,
  refund_of_leg_id TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE billing_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  team_id TEXT,
  run_id TEXT,
  source TEXT NOT NULL,
  description TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  quantity INTEGER,
  unit TEXT,
  unit_rate_cents INTEGER,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);
CREATE TABLE billing_accounts (
  user_id TEXT PRIMARY KEY NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const NOW = '2026-06-28T12:00:00.000Z'
const USER = 'user-card-credit-spend'
const ACCOUNT = `agent:${USER}`
const SESSION = 'cs_test_card_credit_spend_1'

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const seedStripeCheckoutCredit = async (db: D1Database): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO billing_accounts (user_id, status, created_at, updated_at)
       VALUES (?, 'active', ?, ?)`,
    )
    .bind(USER, NOW, NOW)
    .run()
  await db
    .prepare(
      `INSERT INTO billing_ledger_entries
        (id, user_id, source, description, amount_cents, currency, quantity,
         unit, metadata_json, idempotency_key, created_at)
       VALUES (?, ?, 'stripe_checkout', 'Stripe credit purchase', 500, 'USD',
         500, 'credit_cents', ?, ?, ?)`,
    )
    .bind(
      'bill-card-credit-spend',
      USER,
      JSON.stringify({ packageId: 'starter', sessionId: SESSION }),
      `billing:stripe-checkout:${SESSION}`,
      NOW,
    )
    .run()
}

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

describe('D1 card-credit-spend receipt store', () => {
  test('pending projection names the purchase ledger key when checkout is missing', async () => {
    const db = makeDb()

    const projection = await makeD1CardCreditSpendReceiptStore(
      db,
    ).readCardCreditSpendReceipt(
      cardCreditSpendReceiptRef(SESSION),
      '2026-06-28T12:01:00.000Z',
    )

    expect(projection?.resolution).toEqual({
      missing: 'purchase',
      nextEvidenceRef: `billing:stripe-checkout:${SESSION}`,
      status: 'pending',
    })
  })

  test('pending projection names the card-origin grant context when checkout exists', async () => {
    const db = makeDb()
    await seedStripeCheckoutCredit(db)

    const projection = await makeD1CardCreditSpendReceiptStore(
      db,
    ).readCardCreditSpendReceipt(
      cardCreditSpendReceiptRef(SESSION),
      '2026-06-28T12:01:00.000Z',
    )

    expect(projection?.resolution).toEqual({
      missing: 'grant',
      nextEvidenceRef: `inference:usd-credit:card:${SESSION}`,
      status: 'pending',
    })
  })

  test('pending projection names the inference charge context when grant exists', async () => {
    const db = makeDb()
    await seedStripeCheckoutCredit(db)

    const grant = await run(
      fundInferenceFromCredit(
        {
          amountCents: 500,
          grantRef: 'grant-card-credit-spend',
          sourceCheckoutSessionId: SESSION,
          userId: USER,
        },
        { db, nowIso: () => NOW },
      ),
    )
    expect(grant.ok).toBe(true)

    const projection = await makeD1CardCreditSpendReceiptStore(
      db,
    ).readCardCreditSpendReceipt(
      cardCreditSpendReceiptRef(SESSION),
      '2026-06-28T12:01:00.000Z',
    )

    expect(projection?.resolution).toEqual({
      missing: 'spend',
      nextEvidenceRef: 'ledger.pay_ins.inference_charge.context_ref',
      status: 'pending',
    })
  })

  test('resolves ok after card credit is bridged and spent by real metering', async () => {
    const db = makeDb()
    await seedStripeCheckoutCredit(db)

    const grant = await run(
      fundInferenceFromCredit(
        {
          amountCents: 500,
          grantRef: 'grant-card-credit-spend',
          sourceCheckoutSessionId: SESSION,
          userId: USER,
        },
        { db, nowIso: () => NOW },
      ),
    )
    expect(grant.ok).toBe(true)
    if (!grant.ok) {
      throw new Error('expected credit grant')
    }

    const beforeSpend = await readAgentBalance(db, ACCOUNT)
    const metering = await run(
      makeLedgerMeteringHook({ db, nowIso: () => NOW })({
        accountRef: ACCOUNT,
        adapterId: 'fireworks',
        fundingKind: 'card',
        requestId: 'chatcmpl_card_credit_spend',
        requestedModel: 'sonnet',
        servedModel: 'sonnet',
        streamed: false,
        usage: {
          completionTokens: 1000,
          promptTokens: 4000,
          totalTokens: 5000,
        },
      }),
    )
    expect(metering.metered).toBe(true)
    expect(metering.receiptRef).toBe(
      'receipt.inference.charge.chatcmpl_card_credit_spend',
    )

    const afterSpend = await readAgentBalance(db, ACCOUNT)
    expect(afterSpend?.balanceMsat).toBeLessThan(beforeSpend?.balanceMsat ?? 0)

    const projection = await makeD1CardCreditSpendReceiptStore(
      db,
    ).readCardCreditSpendReceipt(
      cardCreditSpendReceiptRef(SESSION),
      '2026-06-28T12:01:00.000Z',
    )

    expect(projection).not.toBeNull()
    expect(projection?.resolution.status).toBe('ok')
    if (projection?.resolution.status !== 'ok') {
      throw new Error('expected ok card-credit-spend receipt')
    }

    expect(projection.resolution.receipt.chain).toEqual([
      {
        evidenceRef: `evidence.stripe_checkout_paid.${SESSION}`,
        receiptRef: `billing:stripe-checkout:${SESSION}`,
        step: 'card_to_credit',
      },
      {
        evidenceRef: `inference:usd-credit:card:${SESSION}`,
        receiptRef: usdCreditGrantReceiptRef('grant-card-credit-spend'),
        step: 'credit_to_msat',
      },
      {
        receiptRef: 'receipt.inference.charge.chatcmpl_card_credit_spend',
        step: 'msat_to_inference',
      },
    ])
    expect(
      projection.resolution.receipt.conservation.spentMsat,
    ).toBeGreaterThan(0)
    expect(JSON.stringify(projection)).not.toMatch(
      /invoice|lnbc|payment_hash|preimage|wallet|secret|api[_-]?key|bearer/i,
    )
  })
})
