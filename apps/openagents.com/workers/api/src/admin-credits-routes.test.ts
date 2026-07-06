import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import {
  ADMIN_CREDITS_BALANCE_PATH,
  ADMIN_CREDITS_CLAWBACK_PATH,
  ADMIN_CREDITS_GRANT_PATH,
  ADMIN_CREDITS_HISTORY_PATH,
  ADMIN_CREDITS_RECENT_GRANTS_PATH,
  ADMIN_CREDITS_USERS_PATH,
  type AdminCaller,
  makeAdminCreditsRoutes,
} from './admin-credits-routes'
import { paymentsLedgerDbFromD1 } from './test/payments-ledger-sqlite'

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
  private queue: Promise<unknown> = Promise.resolve()

  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }

  batch(statements: ReadonlyArray<SqliteD1Statement>): Promise<Array<{ success: true }>> {
    const run = async (): Promise<Array<{ success: true }>> => {
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
    const result = this.queue.then(run, run)
    this.queue = result.catch(() => undefined)
    return result
  }
}

const SCHEMA = `
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  display_name TEXT NOT NULL,
  primary_email TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);
CREATE TABLE auth_identities (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  provider_subject TEXT NOT NULL,
  provider_username TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);
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
  pay_in_type TEXT NOT NULL,
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
CREATE TABLE admin_credit_grants (
  grant_ref TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  amount_usd_cents INTEGER NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  reason TEXT NOT NULL,
  granted_by_user_id TEXT NOT NULL,
  credit_receipt_ref TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);
CREATE TABLE github_signup_credit_grants (
  grant_ref TEXT PRIMARY KEY NOT NULL,
  github_user_id TEXT NOT NULL UNIQUE,
  user_id TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  amount_usd_cents INTEGER NOT NULL CHECK (amount_usd_cents > 0),
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  credit_receipt_ref TEXT NOT NULL UNIQUE,
  github_account_created_at TEXT,
  ip_hash TEXT,
  created_at TEXT NOT NULL
);
`

const NOW = '2026-07-06T12:00:00.000Z'

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  const db = new SqliteD1(raw) as unknown as D1Database
  db.prepare(
    `INSERT INTO users (id, kind, display_name, primary_email, created_at, updated_at)
     VALUES (?, 'human', ?, ?, ?, ?)`,
  )
    .bind('user_1', 'Octo Cat', 'octocat@example.com', NOW, NOW)
    .run()
  db.prepare(
    `INSERT INTO auth_identities (id, user_id, provider, provider_subject, provider_username, created_at, updated_at)
     VALUES (?, ?, 'github', '123', 'octocat', ?, ?)`,
  )
    .bind('auth_1', 'user_1', NOW, NOW)
    .run()
  return db
}

type Env = Readonly<{ OPENAGENTS_DB: D1Database }>

const makeRoutes = (db: D1Database, adminUserId: string | undefined) =>
  makeAdminCreditsRoutes<Env>({
    db: env => env.OPENAGENTS_DB,
    // CFG-4 (#8519): the credits ledger handle shares the same underlying
    // SQLite database as the D1 shim in tests.
    ledgerDb: env => paymentsLedgerDbFromD1(env.OPENAGENTS_DB as never),
    nowIso: () => NOW,
    requireAdminCaller: async (): Promise<AdminCaller | undefined> =>
      adminUserId === undefined ? undefined : { userId: adminUserId },
  })

const fakeCtx = {} as ExecutionContext

describe('Aiur admin credits routes — auth matrix (fail closed)', () => {
  test('balance/history/grant/clawback/users/recent-grants all 401 without an admin caller', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, undefined)

    const balance = await routes.handleAdminCreditsBalanceApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_BALANCE_PATH}?userId=user_1`),
      env,
      fakeCtx,
    )
    expect(balance.status).toBe(401)

    const history = await routes.handleAdminCreditsHistoryApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_HISTORY_PATH}?userId=user_1`),
      env,
      fakeCtx,
    )
    expect(history.status).toBe(401)

    const users = await routes.handleAdminCreditsUsersApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_USERS_PATH}`),
      env,
      fakeCtx,
    )
    expect(users.status).toBe(401)

    const recentGrants = await routes.handleAdminCreditsRecentGrantsApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_RECENT_GRANTS_PATH}`),
      env,
      fakeCtx,
    )
    expect(recentGrants.status).toBe(401)

    const grant = await routes.handleAdminCreditsGrantApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_GRANT_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ userId: 'user_1', grantRef: 'g1', amountUsdCents: 500, reason: 'x' }),
      }),
      env,
      fakeCtx,
    )
    expect(grant.status).toBe(401)

    const clawback = await routes.handleAdminCreditsClawbackApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_CLAWBACK_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ userId: 'user_1', clawbackRef: 'c1', amountUsdCents: 100, reason: 'x' }),
      }),
      env,
      fakeCtx,
    )
    expect(clawback.status).toBe(401)
  })

  test('an owner caller can grant, read balance/history, and see the recent-grants ledger', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    const grantResponse = await routes.handleAdminCreditsGrantApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_GRANT_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          amountUsdCents: 1000,
          githubLogin: 'octocat',
          grantRef: 'grant-e2e-1',
          reason: 'Beta welcome credit',
        }),
      }),
      env,
      fakeCtx,
    )
    expect(grantResponse.status).toBe(201)
    const grantBody = (await grantResponse.json()) as { ok: boolean; grantedCents: number }
    expect(grantBody.ok).toBe(true)
    expect(grantBody.grantedCents).toBe(1000)

    const balanceResponse = await routes.handleAdminCreditsBalanceApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_BALANCE_PATH}?userId=user_1`),
      env,
      fakeCtx,
    )
    expect(balanceResponse.status).toBe(200)
    const balanceBody = (await balanceResponse.json()) as {
      balance: { balanceUsdCents: number }
    }
    expect(balanceBody.balance.balanceUsdCents).toBe(1000)

    const historyResponse = await routes.handleAdminCreditsHistoryApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_HISTORY_PATH}?githubLogin=octocat`),
      env,
      fakeCtx,
    )
    expect(historyResponse.status).toBe(200)
    const historyBody = (await historyResponse.json()) as {
      history: ReadonlyArray<{ kind: string; reason: string }>
    }
    expect(historyBody.history).toHaveLength(1)
    expect(historyBody.history[0]?.reason).toBe('Beta welcome credit')

    const recentGrantsResponse = await routes.handleAdminCreditsRecentGrantsApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_RECENT_GRANTS_PATH}`),
      env,
      fakeCtx,
    )
    const recentGrantsBody = (await recentGrantsResponse.json()) as {
      grants: ReadonlyArray<{ grantRef: string; grantedByUserId: string }>
    }
    expect(recentGrantsBody.grants[0]?.grantRef).toBe('grant-e2e-1')
    expect(recentGrantsBody.grants[0]?.grantedByUserId).toBe('user_owner')

    const usersResponse = await routes.handleAdminCreditsUsersApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_USERS_PATH}`),
      env,
      fakeCtx,
    )
    const usersBody = (await usersResponse.json()) as {
      users: ReadonlyArray<{ userId: string; hasAdminCreditGrant: boolean }>
    }
    expect(usersBody.users[0]?.hasAdminCreditGrant).toBe(true)
  })

  test('a grant with no reason is refused (400), never silently mutates', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    const response = await routes.handleAdminCreditsGrantApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_GRANT_PATH}`, {
        method: 'POST',
        body: JSON.stringify({ userId: 'user_1', grantRef: 'no-reason', amountUsdCents: 500 }),
      }),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(400)
  })

  test('resolving a nonexistent target user returns 404, not a silent no-op', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    const response = await routes.handleAdminCreditsBalanceApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_BALANCE_PATH}?userId=nonexistent`),
      env,
      fakeCtx,
    )
    expect(response.status).toBe(404)
  })

  test('clawback path: grant then claw back, receipted and reflected in balance', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const routes = makeRoutes(db, 'user_owner')

    await routes.handleAdminCreditsGrantApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_GRANT_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          amountUsdCents: 1000,
          userId: 'user_1',
          grantRef: 'grant-before-claw',
          reason: 'Initial grant',
        }),
      }),
      env,
      fakeCtx,
    )

    const clawbackResponse = await routes.handleAdminCreditsClawbackApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_CLAWBACK_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          amountUsdCents: 400,
          userId: 'user_1',
          clawbackRef: 'clawback-e2e-1',
          reason: 'Refund',
        }),
      }),
      env,
      fakeCtx,
    )
    expect(clawbackResponse.status).toBe(200)
    const clawbackBody = (await clawbackResponse.json()) as { clawedBack: boolean }
    expect(clawbackBody.clawedBack).toBe(true)

    const balanceResponse = await routes.handleAdminCreditsBalanceApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_BALANCE_PATH}?userId=user_1`),
      env,
      fakeCtx,
    )
    const balanceBody = (await balanceResponse.json()) as {
      balance: { balanceUsdCents: number }
    }
    expect(balanceBody.balance.balanceUsdCents).toBe(600)
  })

  test('recordCreditBalanceProjection (#8505 Part 2) is threaded through to the grant and clawback', async () => {
    const db = makeDb()
    const env: Env = { OPENAGENTS_DB: db }
    const calls: Array<{ accountRef: string; deltaUsdCents: number }> = []
    const routes = makeAdminCreditsRoutes<Env>({
      db: e => e.OPENAGENTS_DB,
      ledgerDb: e => paymentsLedgerDbFromD1(e.OPENAGENTS_DB as never),
      nowIso: () => NOW,
      recordCreditBalanceProjection: () => async event => {
        calls.push({ accountRef: event.accountRef, deltaUsdCents: event.deltaUsdCents })
      },
      requireAdminCaller: async (): Promise<AdminCaller | undefined> => ({ userId: 'user_owner' }),
    })

    await routes.handleAdminCreditsGrantApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_GRANT_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          amountUsdCents: 500,
          userId: 'user_1',
          grantRef: 'grant-projection-wiring',
          reason: 'Wiring test',
        }),
      }),
      env,
      fakeCtx,
    )
    await routes.handleAdminCreditsClawbackApi(
      new Request(`https://openagents.com${ADMIN_CREDITS_CLAWBACK_PATH}`, {
        method: 'POST',
        body: JSON.stringify({
          amountUsdCents: 200,
          userId: 'user_1',
          clawbackRef: 'clawback-projection-wiring',
          reason: 'Wiring test',
        }),
      }),
      env,
      fakeCtx,
    )

    expect(calls).toEqual([
      { accountRef: 'agent:user_1', deltaUsdCents: 500 },
      { accountRef: 'agent:user_1', deltaUsdCents: -200 },
    ])
  })
})
