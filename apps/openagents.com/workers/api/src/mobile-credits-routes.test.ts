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
import type { PaymentsLedgerDb } from './payments-ledger-db'
import { makeLedgerSqliteDb } from './test/payments-ledger-sqlite'

// CFG-4 (#8519): the routes read the Postgres-authoritative credits ledger
// through `PaymentsLedgerDb`; tests back it with the shared SQLite ledger
// adapter (same credits-domain schema, portable-SQL guarded).
type FakeEnv = Readonly<{ ledger: PaymentsLedgerDb }>
type FakeUser = Readonly<{ userId: string }>

const makeEnv = (): FakeEnv => ({ ledger: makeLedgerSqliteDb() })

const insertBalance = async (
  env: FakeEnv,
  actorRef: string,
  balanceMsat: number,
  heldMsat = 0,
): Promise<void> => {
  await env.ledger.batch([
    {
      params: [actorRef, balanceMsat, heldMsat],
      sql: `INSERT INTO agent_balances (actor_ref, balance_msat, held_msat, created_at, updated_at)
     VALUES (?, ?, ?, '2026-07-01T00:00:00.000Z', '2026-07-01T00:00:00.000Z')`,
    },
  ])
}

const insertPayIn = async (
  env: FakeEnv,
  row: Readonly<{
    id: string
    payInType: string
    payerRef: string
    costMsat: number
    contextRef?: string | null
    createdAt: string
  }>,
): Promise<void> => {
  await env.ledger.batch([
    {
      params: [
        row.id,
        row.payInType,
        row.payerRef,
        row.costMsat,
        row.contextRef ?? null,
        `idem:${row.id}`,
        row.createdAt,
        row.createdAt,
      ],
      sql: `INSERT INTO pay_ins
       (id, pay_in_type, payer_ref, cost_msat, state, context_ref, idempotency_key, created_at, state_changed_at)
     VALUES (?, ?, ?, ?, 'paid', ?, ?, ?, ?)`,
    },
  ])
}

const makeDependencies = (
  sessionUserId: string | undefined,
): MobileCreditsRouteDependencies<FakeEnv, FakeUser> => ({
  ledgerDb: env => env.ledger,
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
    await insertBalance(env, 'agent:user-1', 5_000_000)
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
    await insertBalance(env, 'agent:user-1', 5_000_000, 2_000_000)
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
    await insertBalance(env, 'agent:user-1', 5_000_000)
    await insertBalance(env, 'agent:user-2', 9_000_000)
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
    await insertPayIn(env, {
      contextRef: 'github-signup:12345',
      costMsat: 10_000_000, // $10.00 grant
      createdAt: '2026-07-01T00:00:00.000Z',
      id: 'inference:usd-credit:signup:github:12345',
      payInType: 'usd_credit_grant',
      payerRef: 'agent:user-1',
    })
    await insertPayIn(env, {
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
    await insertPayIn(env, {
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
    await insertPayIn(env, {
      costMsat: 10_000_000,
      createdAt: '2026-07-01T00:00:00.000Z',
      id: 'row-a',
      payInType: 'usd_credit_grant',
      payerRef: 'agent:user-1',
    })
    await insertPayIn(env, {
      costMsat: 20_000,
      createdAt: '2026-07-02T00:00:00.000Z',
      id: 'row-b',
      payInType: 'adjustment',
      payerRef: 'agent:user-1',
    })
    await insertPayIn(env, {
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
