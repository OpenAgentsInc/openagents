import { DatabaseSync } from 'node:sqlite'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleMobileCreditsBalanceRequest,
  handleMobileCreditsTransactionsRequest,
  mobileCreditsTransactionDescription,
  mobileCreditsTransactionKind,
  MOBILE_CREDITS_BALANCE_PATH,
  MOBILE_CREDITS_TRANSACTIONS_PATH,
  type MobileCreditsRouteDependencies,
} from './mobile-credits-routes'

type Row = Record<string, unknown>
type FakeEnv = Readonly<{ db: D1Database }>
type FakeUser = Readonly<{ userId: string }>

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
}

const schema = `
CREATE TABLE agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  held_msat INTEGER NOT NULL DEFAULT 0,
  usd_credit_msat INTEGER NOT NULL DEFAULT 0,
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
  genesis_id TEXT,
  successor_id TEXT,
  created_at TEXT NOT NULL,
  state_changed_at TEXT NOT NULL
);

CREATE TABLE pay_in_legs (
  id TEXT PRIMARY KEY,
  pay_in_id TEXT NOT NULL REFERENCES pay_ins (id),
  direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
  kind TEXT NOT NULL CHECK (kind IN ('balance', 'lightning')),
  party_ref TEXT NOT NULL,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat INTEGER,
  external_ref TEXT,
  refund_of_leg_id TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_pay_ins_payer ON pay_ins (payer_ref);
CREATE INDEX idx_pay_in_legs_party ON pay_in_legs (party_ref);
`

const makeEnv = (): FakeEnv => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(schema)
  return { db: new SqliteD1(raw) as unknown as D1Database }
}

const insertBalance = (env: FakeEnv, actorRef: string, balanceMsat: number, heldMsat = 0): void => {
  const stmt = (env.db as unknown as SqliteD1).prepare(
    `INSERT INTO agent_balances (actor_ref, balance_msat, held_msat, created_at, updated_at)
     VALUES (?, ?, ?, '2026-07-01T00:00:00.000Z', '2026-07-01T00:00:00.000Z')`,
  )
  void stmt.bind(actorRef, balanceMsat, heldMsat).run()
}

const insertPayIn = (
  env: FakeEnv,
  row: Readonly<{
    id: string
    payInType: string
    payerRef: string
    costMsat: number
    contextRef?: string | null
    createdAt: string
  }>,
): void => {
  const stmt = (env.db as unknown as SqliteD1).prepare(
    `INSERT INTO pay_ins
       (id, pay_in_type, payer_ref, cost_msat, state, context_ref, idempotency_key, created_at, state_changed_at)
     VALUES (?, ?, ?, ?, 'paid', ?, ?, ?, ?)`,
  )
  void stmt
    .bind(
      row.id,
      row.payInType,
      row.payerRef,
      row.costMsat,
      row.contextRef ?? null,
      `idem:${row.id}`,
      row.createdAt,
      row.createdAt,
    )
    .run()
}

const makeDependencies = (
  sessionUserId: string | undefined,
): MobileCreditsRouteDependencies<FakeEnv, FakeUser> => ({
  db: env => env.db,
  requireUserBearerSession: async () =>
    sessionUserId === undefined ? undefined : { user: { userId: sessionUserId } },
  userIdFromSession: session => session.user.userId,
})

const ctx = {} as ExecutionContext

const runBalance = (
  dependencies: MobileCreditsRouteDependencies<FakeEnv, FakeUser>,
  request: Request,
  env: FakeEnv,
): Promise<Response> =>
  Effect.runPromise(handleMobileCreditsBalanceRequest(dependencies, request, env, ctx))

const runTransactions = (
  dependencies: MobileCreditsRouteDependencies<FakeEnv, FakeUser>,
  request: Request,
  env: FakeEnv,
): Promise<Response> =>
  Effect.runPromise(handleMobileCreditsTransactionsRequest(dependencies, request, env, ctx))

describe('handleMobileCreditsBalanceRequest', () => {
  test('401s without a valid mobile bearer session', async () => {
    const env = makeEnv()
    const response = await runBalance(
      makeDependencies(undefined),
      new Request(`https://openagents.com${MOBILE_CREDITS_BALANCE_PATH}`),
      env,
    )
    expect(response.status).toBe(401)
  })

  test('a non-GET method is 405 with an Allow header', async () => {
    const env = makeEnv()
    const response = await runBalance(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_BALANCE_PATH}`, { method: 'POST' }),
      env,
    )
    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })

  test('reports 0 for a user with no agent_balances row yet (never fabricates a balance)', async () => {
    const env = makeEnv()
    const response = await runBalance(
      makeDependencies('brand-new-user'),
      new Request(`https://openagents.com${MOBILE_CREDITS_BALANCE_PATH}`),
      env,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { balanceUsdCents: number }
    expect(body.balanceUsdCents).toBe(0)
  })

  test('reads the real D1 agent_balances row, converted to USD cents at the shared rate', async () => {
    const env = makeEnv()
    // $100,000/BTC reference rate (DEFAULT_BTC_USD): 1_000_000 msat == $1.00.
    insertBalance(env, 'agent:user-1', 5_000_000)
    const response = await runBalance(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_BALANCE_PATH}`),
      env,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { balanceUsdCents: number }
    expect(body.balanceUsdCents).toBe(500)
  })

  test('reports the available balance (minus escrow-held), matching the coding-admission gate', async () => {
    const env = makeEnv()
    insertBalance(env, 'agent:user-1', 5_000_000, 2_000_000)
    const response = await runBalance(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_BALANCE_PATH}`),
      env,
    )
    const body = (await response.json()) as { balanceUsdCents: number }
    expect(body.balanceUsdCents).toBe(300)
  })

  test('one user never sees another user\'s balance', async () => {
    const env = makeEnv()
    insertBalance(env, 'agent:user-1', 5_000_000)
    insertBalance(env, 'agent:user-2', 9_000_000)
    const response = await runBalance(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_BALANCE_PATH}`),
      env,
    )
    const body = (await response.json()) as { balanceUsdCents: number }
    expect(body.balanceUsdCents).toBe(500)
  })
})

describe('mobileCreditsTransactionKind / mobileCreditsTransactionDescription', () => {
  test('maps usd_credit_grant to grant', () => {
    expect(mobileCreditsTransactionKind('usd_credit_grant')).toBe('grant')
  })

  test('maps adjustment to charge', () => {
    expect(mobileCreditsTransactionKind('adjustment')).toBe('charge')
  })

  test('maps an unrecognized pay_in_type to other', () => {
    expect(mobileCreditsTransactionKind('tip')).toBe('other')
  })

  test('describes an admin grant, signup grant, clawback, inference charge, and cloud charge', () => {
    expect(mobileCreditsTransactionDescription('usd_credit_grant', 'admin-credit-grant:user-1')).toBe(
      'Admin credit grant',
    )
    expect(mobileCreditsTransactionDescription('usd_credit_grant', 'github-signup:12345')).toBe(
      'GitHub signup credit',
    )
    expect(mobileCreditsTransactionDescription('adjustment', 'admin-credit-clawback:user-1:abuse')).toBe(
      'Credit clawback',
    )
    expect(mobileCreditsTransactionDescription('adjustment', 'inference:vertex:served:gemini:tokens:100')).toBe(
      'Inference usage',
    )
    expect(mobileCreditsTransactionDescription('adjustment', 'cloud.coding_session.run:adapter-1')).toBe(
      'Cloud compute usage',
    )
  })

  test('falls back to an empty description for an unrecognized context_ref (client renders the kind label)', () => {
    expect(mobileCreditsTransactionDescription('tip', null)).toBe('')
  })
})

describe('handleMobileCreditsTransactionsRequest', () => {
  test('401s without a valid mobile bearer session', async () => {
    const env = makeEnv()
    const response = await runTransactions(
      makeDependencies(undefined),
      new Request(`https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}`),
      env,
    )
    expect(response.status).toBe(401)
  })

  test('a non-GET method is 405 with an Allow header', async () => {
    const env = makeEnv()
    const response = await runTransactions(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}`, { method: 'POST' }),
      env,
    )
    expect(response.status).toBe(405)
  })

  test('an empty history returns an empty list with no next cursor', async () => {
    const env = makeEnv()
    const response = await runTransactions(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}`),
      env,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { transactions: Array<unknown>; nextCursor: string | null }
    expect(body.transactions).toEqual([])
    expect(body.nextCursor).toBeNull()
  })

  test('lists a real signup grant and inference charge, newest first, in USD cents', async () => {
    const env = makeEnv()
    insertPayIn(env, {
      contextRef: 'github-signup:12345',
      costMsat: 10_000_000, // $10.00 grant
      createdAt: '2026-07-01T00:00:00.000Z',
      id: 'inference:usd-credit:signup:github:12345',
      payInType: 'usd_credit_grant',
      payerRef: 'agent:user-1',
    })
    insertPayIn(env, {
      contextRef: 'inference:vertex:served:gemini-2.5-pro:tokens:500',
      costMsat: 50_000, // $0.05 charge
      createdAt: '2026-07-02T00:00:00.000Z',
      id: 'inference:payin:req-1',
      payInType: 'adjustment',
      payerRef: 'agent:user-1',
    })

    const response = await runTransactions(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}`),
      env,
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      transactions: ReadonlyArray<{
        id: string
        kind: string
        amountUsdCents: number
        description: string
        occurredAt: string
      }>
      nextCursor: string | null
    }
    expect(body.transactions).toHaveLength(2)
    expect(body.transactions[0]?.id).toBe('inference:payin:req-1')
    expect(body.transactions[0]?.kind).toBe('charge')
    expect(body.transactions[0]?.amountUsdCents).toBe(5)
    expect(body.transactions[0]?.description).toBe('Inference usage')
    expect(body.transactions[1]?.id).toBe('inference:usd-credit:signup:github:12345')
    expect(body.transactions[1]?.kind).toBe('grant')
    expect(body.transactions[1]?.amountUsdCents).toBe(1000)
    expect(body.nextCursor).toBeNull()
  })

  test('never leaks another user\'s transactions', async () => {
    const env = makeEnv()
    insertPayIn(env, {
      costMsat: 10_000_000,
      createdAt: '2026-07-01T00:00:00.000Z',
      id: 'grant-user-2',
      payInType: 'usd_credit_grant',
      payerRef: 'agent:user-2',
    })
    const response = await runTransactions(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}`),
      env,
    )
    const body = (await response.json()) as { transactions: Array<unknown> }
    expect(body.transactions).toEqual([])
  })

  test('paginates with a keyset cursor: limit=1 then Load more resumes correctly', async () => {
    const env = makeEnv()
    insertPayIn(env, {
      costMsat: 10_000_000,
      createdAt: '2026-07-01T00:00:00.000Z',
      id: 'row-a',
      payInType: 'usd_credit_grant',
      payerRef: 'agent:user-1',
    })
    insertPayIn(env, {
      costMsat: 20_000,
      createdAt: '2026-07-02T00:00:00.000Z',
      id: 'row-b',
      payInType: 'adjustment',
      payerRef: 'agent:user-1',
    })
    insertPayIn(env, {
      costMsat: 30_000,
      createdAt: '2026-07-03T00:00:00.000Z',
      id: 'row-c',
      payInType: 'adjustment',
      payerRef: 'agent:user-1',
    })

    const firstPage = await runTransactions(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}?limit=1`),
      env,
    )
    const firstBody = (await firstPage.json()) as {
      transactions: ReadonlyArray<{ id: string }>
      nextCursor: string | null
    }
    expect(firstBody.transactions.map(t => t.id)).toEqual(['row-c'])
    expect(firstBody.nextCursor).not.toBeNull()

    const secondPage = await runTransactions(
      makeDependencies('user-1'),
      new Request(
        `https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}?limit=1&cursor=${encodeURIComponent(firstBody.nextCursor as string)}`,
      ),
      env,
    )
    const secondBody = (await secondPage.json()) as {
      transactions: ReadonlyArray<{ id: string }>
      nextCursor: string | null
    }
    expect(secondBody.transactions.map(t => t.id)).toEqual(['row-b'])
    expect(secondBody.nextCursor).not.toBeNull()

    const thirdPage = await runTransactions(
      makeDependencies('user-1'),
      new Request(
        `https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}?limit=1&cursor=${encodeURIComponent(secondBody.nextCursor as string)}`,
      ),
      env,
    )
    const thirdBody = (await thirdPage.json()) as {
      transactions: ReadonlyArray<{ id: string }>
      nextCursor: string | null
    }
    expect(thirdBody.transactions.map(t => t.id)).toEqual(['row-a'])
    expect(thirdBody.nextCursor).toBeNull()
  })

  test('a malformed cursor is a 400, not a silent empty page', async () => {
    const env = makeEnv()
    const response = await runTransactions(
      makeDependencies('user-1'),
      new Request(`https://openagents.com${MOBILE_CREDITS_TRANSACTIONS_PATH}?cursor=%%%not-base64%%%`),
      env,
    )
    expect(response.status).toBe(400)
  })
})
