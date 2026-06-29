import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import { agentRefForUser } from './inference/usd-credit-bridge'
import { readAgentBalance } from './payments-ledger'
import { makeOperatorBillingHandlers } from './operator-billing-routes'
import type { OperatorTargetUser } from './operator-targets'

// Real-SQL D1 adapter backed by node:sqlite (same pattern as the bridge harness)
// so the operator inference-credit handler exercises the genuine USD debit + msat
// grant SQL, not a mock — the load-bearing claim is "an admin grant lands a
// SPENDABLE usd_credit_msat balance on an arbitrary target agent".
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
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[],
    }
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
  currency TEXT NOT NULL DEFAULT 'USD',
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
-- Tables the returned readBillingSummary touches. They only need to exist with
-- the queried columns; the handler returns an empty/zero summary in tests.
CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  goal TEXT,
  status TEXT NOT NULL,
  started_at TEXT,
  updated_at TEXT NOT NULL
);
CREATE TABLE billing_usage_cursors (
  run_id TEXT NOT NULL,
  meter TEXT NOT NULL,
  last_billed_at TEXT
);
CREATE TABLE stripe_saved_payment_methods (
  user_id TEXT NOT NULL,
  currency TEXT NOT NULL,
  livemode INTEGER NOT NULL DEFAULT 0,
  stripe_payment_method_id TEXT,
  brand TEXT,
  last4 TEXT,
  exp_month INTEGER,
  exp_year INTEGER,
  status TEXT,
  updated_at TEXT
);
CREATE TABLE billing_auto_top_up_policies (
  user_id TEXT NOT NULL,
  currency TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 0,
  threshold_cents INTEGER,
  amount_cents INTEGER,
  monthly_cap_cents INTEGER,
  spent_this_month_cents INTEGER,
  status TEXT,
  pause_reason TEXT,
  updated_at TEXT
);
CREATE TABLE billing_auto_top_up_events (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  status TEXT,
  amount_cents INTEGER,
  reason TEXT,
  created_at TEXT NOT NULL
);
`

type TestEnv = { OPENAGENTS_DB: D1Database }

const USER = 'user-target-1'

const makeEnv = (): TestEnv => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return { OPENAGENTS_DB: new SqliteD1(raw) as unknown as D1Database }
}

const targetUser: OperatorTargetUser = {
  userId: USER,
} as OperatorTargetUser

const makeHandlers = (
  over: Partial<{
    admin: boolean
    target: OperatorTargetUser | undefined
  }> = {},
) =>
  makeOperatorBillingHandlers<TestEnv>({
    readSelectedOperatorTargetUser: async () =>
      'target' in over ? over.target : targetUser,
    requireAdminApiToken: async () => over.admin ?? true,
  })

const postRequest = (body: unknown): Request =>
  new Request(
    'https://openagents.com/api/omni/operator/billing/inference-credit',
    { body: JSON.stringify(body), method: 'POST' },
  )

describe('handleOmniOperatorInferenceCreditApi', () => {
  test('401 without a valid admin token', async () => {
    const handlers = makeHandlers({ admin: false })
    const response = await handlers.handleOmniOperatorInferenceCreditApi(
      postRequest({ userId: USER, amountCents: 100 }),
      makeEnv(),
    )
    expect(response.status).toBe(401)
  })

  test('405 on a non-POST method', async () => {
    const handlers = makeHandlers()
    const response = await handlers.handleOmniOperatorInferenceCreditApi(
      new Request(
        'https://openagents.com/api/omni/operator/billing/inference-credit',
        { method: 'GET' },
      ),
      makeEnv(),
    )
    expect(response.status).toBe(405)
  })

  test('404 when the target user is not found', async () => {
    const handlers = makeHandlers({ target: undefined })
    const response = await handlers.handleOmniOperatorInferenceCreditApi(
      postRequest({ userId: 'missing', amountCents: 100 }),
      makeEnv(),
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('target_user_not_found')
  })

  test('400 on a non-positive amount', async () => {
    const handlers = makeHandlers()
    const response = await handlers.handleOmniOperatorInferenceCreditApi(
      postRequest({ userId: USER, amountCents: 0 }),
      makeEnv(),
    )
    expect(response.status).toBe(400)
  })

  test('grants a SPENDABLE usd_credit_msat balance to the target agent', async () => {
    const env = makeEnv()
    const handlers = makeHandlers()
    const response = await handlers.handleOmniOperatorInferenceCreditApi(
      postRequest({ userId: USER, amountCents: 1_000, grantRef: 'grant-fixed' }),
      env,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      status: string
      grantedCents: number
      grantedMsat: number
      receiptRef: string
    }
    expect(body.status).toBe('inference_credit_granted')
    expect(body.grantedCents).toBe(1_000)
    expect(body.grantedMsat).toBeGreaterThan(0)

    // The granted msat is spendable AND carries the USD-origin tag (so the
    // Bitcoin sweep will exclude it). availableMsat includes usd_credit_msat.
    const balance = await readAgentBalance(
      env.OPENAGENTS_DB,
      agentRefForUser(USER),
    )
    expect(balance).not.toBeNull()
    expect(balance?.availableMsat).toBe(body.grantedMsat)
    expect(balance?.usdCreditMsat).toBe(body.grantedMsat)
    expect(balance?.bitcoinWithdrawableMsat).toBe(0)
  })

  test('idempotent on a repeated grantRef (no double grant)', async () => {
    const env = makeEnv()
    const handlers = makeHandlers()
    const once = await handlers.handleOmniOperatorInferenceCreditApi(
      postRequest({ userId: USER, amountCents: 500, grantRef: 'grant-dupe' }),
      env,
    )
    expect(once.status).toBe(200)
    const firstMsat = (await readAgentBalance(
      env.OPENAGENTS_DB,
      agentRefForUser(USER),
    ))?.usdCreditMsat
    // Replay the SAME grantRef: the USD adjustment + msat grant are both keyed
    // by the ref, so the spendable balance must not grow.
    await handlers.handleOmniOperatorInferenceCreditApi(
      postRequest({ userId: USER, amountCents: 500, grantRef: 'grant-dupe' }),
      env,
    )
    const secondMsat = (await readAgentBalance(
      env.OPENAGENTS_DB,
      agentRefForUser(USER),
    ))?.usdCreditMsat
    expect(secondMsat).toBe(firstMsat)
  })
})
