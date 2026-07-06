import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleIapRevenueCatWebhookRequest,
  IAP_REVENUECAT_WEBHOOK_PATH,
  type IapWebhookRouteDependencies,
} from './iap-webhook-routes'
import { readAgentBalance, type AgentBalanceRow } from './payments-ledger'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { paymentsLedgerDbFromD1, type D1LikeDatabase } from './test/payments-ledger-sqlite'

const requireAgentBalance = async (db: PaymentsLedgerDb, actorRef: string): Promise<AgentBalanceRow> => {
  const balance = await readAgentBalance(db, actorRef)
  expect(balance).not.toBeNull()
  return balance!
}

type Row = Record<string, unknown>
type FakeEnv = Readonly<{ db: D1Database
  ledgerDb: PaymentsLedgerDb; secret: string | undefined }>

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
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const DEFAULT_TEST_SECRET = 'test-webhook-secret'

const makeEnv = (input: Readonly<{ noSecret?: true }> = {}): FakeEnv => {
  const db = new DatabaseSync(':memory:')
  db.exec(ledgerSchema)
  db.exec(migration('0306_iap_credit_pack_purchase_intents.sql'))
  const d1 = new SqliteD1(db) as unknown as D1Database
  return {
    db: d1,
    ledgerDb: paymentsLedgerDbFromD1(d1 as unknown as D1LikeDatabase),
    secret: input.noSecret === true ? undefined : DEFAULT_TEST_SECRET,
  }
}

const dependencies: IapWebhookRouteDependencies<FakeEnv> = {
  db: env => env.db,
  ledgerDb: env => env.ledgerDb,
  webhookSecret: env => env.secret,
}

const post = (body: unknown, authorization = 'test-webhook-secret') =>
  new Request(`https://openagents.com${IAP_REVENUECAT_WEBHOOK_PATH}`, {
    body: JSON.stringify(body),
    headers: { authorization },
    method: 'POST',
  })

const purchaseEvent = (overrides: Record<string, unknown> = {}) => ({
  event: {
    app_user_id: 'user-1',
    environment: 'SANDBOX',
    id: 'event-1',
    original_transaction_id: 'txn-1',
    product_id: 'credits_999',
    store: 'APP_STORE',
    transaction_id: 'txn-1',
    type: 'NON_RENEWING_PURCHASE',
    ...overrides,
  },
})

describe('handleIapRevenueCatWebhookRequest — auth + method gating', () => {
  test('rejects a non-POST method', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(dependencies, new Request(`https://x/${IAP_REVENUECAT_WEBHOOK_PATH}`, { method: 'GET' }), env),
    )
    expect(response.status).toBe(405)
  })

  test('rejects a wrong or missing webhook secret', async () => {
    const env = makeEnv()
    const wrongSecret = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(dependencies, post(purchaseEvent(), 'wrong'), env),
    )
    expect(wrongSecret.status).toBe(401)

    const missingHeader = new Request(`https://openagents.com${IAP_REVENUECAT_WEBHOOK_PATH}`, {
      body: JSON.stringify(purchaseEvent()),
      method: 'POST',
    })
    const noHeaderResponse = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(dependencies, missingHeader, env),
    )
    expect(noHeaderResponse.status).toBe(401)
  })

  test('fails closed when no secret is configured on this deployment', async () => {
    const env = makeEnv({ noSecret: true })
    const response = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(dependencies, post(purchaseEvent()), env),
    )
    expect(response.status).toBe(401)
  })
})

describe('handleIapRevenueCatWebhookRequest — purchase fulfillment', () => {
  test('a NON_RENEWING_PURCHASE for a catalog SKU grants credits into Pool B', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(dependencies, post(purchaseEvent()), env),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { ok: boolean; action: string }
    expect(body).toEqual({ action: 'fulfilled', ok: true, purchaseRef: expect.any(String) })

    const balance = await requireAgentBalance(env.ledgerDb, 'agent:user-1')
    expect(balance.balanceMsat).toBeGreaterThan(0)
    expect(balance.usdCreditMsat).toBe(balance.balanceMsat)
  })

  test('the SAME webhook event delivered twice is a no-op the SECOND time (replay resistance)', async () => {
    const env = makeEnv()
    await Effect.runPromise(handleIapRevenueCatWebhookRequest(dependencies, post(purchaseEvent()), env))
    const balanceAfterFirst = await requireAgentBalance(env.ledgerDb, 'agent:user-1')

    const secondResponse = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(dependencies, post(purchaseEvent()), env),
    )
    const secondBody = (await secondResponse.json()) as { ok: boolean; reason: string }
    expect(secondBody).toEqual({ action: 'ignored', ok: true, reason: 'duplicate_event_id' })

    const balanceAfterSecond = await requireAgentBalance(env.ledgerDb, 'agent:user-1')
    expect(balanceAfterSecond.balanceMsat).toBe(balanceAfterFirst.balanceMsat)
  })

  test('a non-catalog SKU is acknowledged as ignored, never granted, never a hard error', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(
        dependencies,
        post(purchaseEvent({ product_id: 'some_subscription_sku', transaction_id: 'txn-2' })),
        env,
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { action: string }
    expect(body.action).toBe('ignored')

    // No balance row was ever created for this user — nothing was granted.
    expect(await readAgentBalance(env.ledgerDb, 'agent:user-1')).toBeNull()
  })

  test('an unhandled lifecycle event type is acknowledged as ignored', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(dependencies, post(purchaseEvent({ type: 'RENEWAL' })), env),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { action: string; reason: string }
    expect(body).toEqual({ action: 'ignored', ok: true, reason: 'unhandled_event_type' })
  })

  test('a malformed body is a 400, not a crash', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(dependencies, post({ not: 'a recognized shape' }), env),
    )
    expect(response.status).toBe(400)
  })
})

describe('handleIapRevenueCatWebhookRequest — refund clawback', () => {
  test('a REFUND webhook claws back a previously-fulfilled purchase', async () => {
    const env = makeEnv()
    await Effect.runPromise(handleIapRevenueCatWebhookRequest(dependencies, post(purchaseEvent()), env))
    const balanceBeforeRefund = await requireAgentBalance(env.ledgerDb, 'agent:user-1')
    expect(balanceBeforeRefund.balanceMsat).toBeGreaterThan(0)

    const response = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(
        dependencies,
        post(purchaseEvent({ id: 'event-refund-1', type: 'REFUND' })),
        env,
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { action: string; clawedBack: boolean }
    expect(body).toEqual({ action: 'refunded', clawedBack: true, insufficientBalance: false, ok: true })

    const balanceAfterRefund = await requireAgentBalance(env.ledgerDb, 'agent:user-1')
    expect(balanceAfterRefund.balanceMsat).toBe(0)
  })

  test('refunding a transaction that was never fulfilled here is acknowledged as ignored', async () => {
    const env = makeEnv()
    const response = await Effect.runPromise(
      handleIapRevenueCatWebhookRequest(
        dependencies,
        post(purchaseEvent({ original_transaction_id: 'never-fulfilled', type: 'REFUND' })),
        env,
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { action: string; reason: string }
    expect(body).toEqual({ action: 'ignored', ok: true, reason: 'purchase_not_found' })
  })
})
