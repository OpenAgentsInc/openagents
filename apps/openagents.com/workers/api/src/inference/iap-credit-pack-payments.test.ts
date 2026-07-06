import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  claimIapWebhookEvent,
  fulfillIapCreditPackPurchase,
  readIapPurchaseByStoreTransactionId,
  refundIapCreditPackPurchase,
  type IapCreditPackPaymentDeps,
} from './iap-credit-pack-payments'
import { readAgentBalance, type AgentBalanceRow } from '../payments-ledger'
import { paymentsLedgerDbFromD1 } from '../test/payments-ledger-sqlite'

const requireAgentBalance = async (
  deps: IapCreditPackPaymentDeps,
  actorRef: string,
): Promise<AgentBalanceRow> => {
  const balance = await readAgentBalance(deps.ledgerDb, actorRef)
  expect(balance).not.toBeNull()
  return balance!
}

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
  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return { results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T> }
  }
  async run(): Promise<{ meta: { changes: number }; success: true }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
  async batch(statements: ReadonlyArray<SqliteD1Statement>): Promise<Array<{ success: true }>> {
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

const ledgerSchema = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  held_msat INTEGER NOT NULL DEFAULT 0 CHECK (held_msat >= 0),
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
  state TEXT NOT NULL CHECK (
    state IN ('pending', 'forwarding', 'paid', 'failed')
  ),
  failure_reason TEXT,
  rung TEXT CHECK (rung IN ('credited', 'direct_bolt12') OR rung IS NULL),
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
  direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
  kind TEXT NOT NULL CHECK (kind IN ('balance', 'lightning')),
  party_ref TEXT NOT NULL,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat INTEGER,
  external_ref TEXT,
  refund_of_leg_id TEXT,
  created_at TEXT NOT NULL
);
`

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', '..', 'migrations', name), 'utf8')

// CFG-4 (#8519): fulfillment/refund now take a deps pair — the intent tables
// stay on D1, the credit grant/clawback goes through the Postgres-authoritative
// `PaymentsLedgerDb` seam (backed by the same SQLite-D1 shim in tests).
const makeDeps = (): IapCreditPackPaymentDeps => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(ledgerSchema)
  raw.exec(migration('0306_iap_credit_pack_purchase_intents.sql'))
  const db = new SqliteD1(raw) as unknown as D1Database
  return { db, ledgerDb: paymentsLedgerDbFromD1(db as never) }
}

describe('fulfillIapCreditPackPurchase', () => {
  test('grants the catalog amount into Pool B, USD-origin', async () => {
    const deps = makeDeps()
    const outcome = await Effect.runPromise(
      fulfillIapCreditPackPurchase(deps, {
        amountUsdCents: 999,
        eventId: 'event-1',
        sku: 'credits_999',
        store: 'app_store',
        storeTransactionId: 'txn-1',
        userId: 'user-1',
      }),
    )
    expect(outcome.ok).toBe(true)
    if (!outcome.ok) return
    expect(outcome.alreadyFulfilled).toBe(false)
    expect(outcome.purchase.status).toBe('fulfilled')
    expect(outcome.purchase.amountUsdCents).toBe(999)

    const balance = await requireAgentBalance(deps, 'agent:user-1')
    expect(balance.usdCreditMsat).toBeGreaterThan(0)
    expect(balance.balanceMsat).toBe(balance.usdCreditMsat)
  })

  test('a replayed purchase for the SAME store_transaction_id is a no-op (never double-grants)', async () => {
    const deps = makeDeps()
    const input = {
      amountUsdCents: 999,
      eventId: 'event-1',
      sku: 'credits_999',
      store: 'app_store' as const,
      storeTransactionId: 'txn-1',
      userId: 'user-1',
    }
    await Effect.runPromise(fulfillIapCreditPackPurchase(deps, input))
    const balanceAfterFirst = await requireAgentBalance(deps, 'agent:user-1')

    const second = await Effect.runPromise(fulfillIapCreditPackPurchase(deps, input))
    expect(second.ok).toBe(true)
    if (second.ok) expect(second.alreadyFulfilled).toBe(true)

    const balanceAfterSecond = await requireAgentBalance(deps, 'agent:user-1')
    expect(balanceAfterSecond.balanceMsat).toBe(balanceAfterFirst.balanceMsat)
  })

  test('two DIFFERENT users each get their own grant', async () => {
    const deps = makeDeps()
    await Effect.runPromise(
      fulfillIapCreditPackPurchase(deps, {
        amountUsdCents: 499,
        eventId: 'event-1',
        sku: 'credits_499',
        store: 'app_store',
        storeTransactionId: 'txn-a',
        userId: 'user-a',
      }),
    )
    await Effect.runPromise(
      fulfillIapCreditPackPurchase(deps, {
        amountUsdCents: 1999,
        eventId: 'event-2',
        sku: 'credits_1999',
        store: 'play_store',
        storeTransactionId: 'txn-b',
        userId: 'user-b',
      }),
    )

    const balanceA = await requireAgentBalance(deps, 'agent:user-a')
    const balanceB = await requireAgentBalance(deps, 'agent:user-b')
    expect(balanceA.balanceMsat).toBeGreaterThan(0)
    expect(balanceB.balanceMsat).toBeGreaterThan(balanceA.balanceMsat)
  })
})

describe('refundIapCreditPackPurchase', () => {
  test('claws back the full granted amount and marks the purchase refunded', async () => {
    const deps = makeDeps()
    await Effect.runPromise(
      fulfillIapCreditPackPurchase(deps, {
        amountUsdCents: 999,
        eventId: 'event-1',
        sku: 'credits_999',
        store: 'app_store',
        storeTransactionId: 'txn-1',
        userId: 'user-1',
      }),
    )
    const balanceBeforeRefund = await requireAgentBalance(deps, 'agent:user-1')
    expect(balanceBeforeRefund.balanceMsat).toBeGreaterThan(0)

    const outcome = await Effect.runPromise(refundIapCreditPackPurchase(deps, 'txn-1'))
    expect(outcome.ok).toBe(true)
    if (!outcome.ok) return
    expect(outcome.alreadyRefunded).toBe(false)
    expect(outcome.clawback.clawedBack).toBe(true)

    const balanceAfterRefund = await requireAgentBalance(deps, 'agent:user-1')
    expect(balanceAfterRefund.balanceMsat).toBe(0)

    const purchase = await readIapPurchaseByStoreTransactionId(deps.db, 'txn-1')
    expect(purchase?.status).toBe('refunded')
  })

  test('refunding an already-refunded purchase is a no-op (never double-claws)', async () => {
    const deps = makeDeps()
    await Effect.runPromise(
      fulfillIapCreditPackPurchase(deps, {
        amountUsdCents: 999,
        eventId: 'event-1',
        sku: 'credits_999',
        store: 'app_store',
        storeTransactionId: 'txn-1',
        userId: 'user-1',
      }),
    )
    await Effect.runPromise(refundIapCreditPackPurchase(deps, 'txn-1'))
    const balanceAfterFirstRefund = await requireAgentBalance(deps, 'agent:user-1')

    const second = await Effect.runPromise(refundIapCreditPackPurchase(deps, 'txn-1'))
    expect(second.ok).toBe(true)
    if (second.ok) expect(second.alreadyRefunded).toBe(true)

    const balanceAfterSecondRefund = await requireAgentBalance(deps, 'agent:user-1')
    expect(balanceAfterSecondRefund.balanceMsat).toBe(balanceAfterFirstRefund.balanceMsat)
  })

  test('refunding a transaction that was never fulfilled reports purchase_not_found', async () => {
    const deps = makeDeps()
    const outcome = await Effect.runPromise(refundIapCreditPackPurchase(deps, 'never-existed'))
    expect(outcome).toEqual({ ok: false, reason: 'purchase_not_found' })
  })
})

describe('claimIapWebhookEvent — replay resistance', () => {
  test('the FIRST claim of an event id reports firstDelivery: true; a replay reports false', async () => {
    const deps = makeDeps()
    const first = await claimIapWebhookEvent(deps.db, {
      eventId: 'event-1',
      eventType: 'NON_RENEWING_PURCHASE',
      nowIso: '2026-07-06T00:00:00.000Z',
    })
    expect(first.firstDelivery).toBe(true)

    const replay = await claimIapWebhookEvent(deps.db, {
      eventId: 'event-1',
      eventType: 'NON_RENEWING_PURCHASE',
      nowIso: '2026-07-06T00:00:01.000Z',
    })
    expect(replay.firstDelivery).toBe(false)
  })
})
