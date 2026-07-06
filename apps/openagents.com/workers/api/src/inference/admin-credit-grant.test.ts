import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { validateAssetBoundary } from '../asset-bitcoin-boundary'
import { readAgentBalance } from '../payments-ledger'
import {
  agentRefForUser,
  usdCreditGrantIdempotencyKey,
  usdCreditGrantReceiptRef,
} from './usd-credit-bridge'
import { inferenceClawbackIdempotencyKey } from './inference-abuse-controls'
import { usdCentsToMsatFloor } from './usd-msat-conversion'
import {
  adminCreditClawbackSourceRef,
  adminCreditGrantRef,
  clawbackAdminCredit,
  grantAdminCredit,
  readAdminCreditGrantsForUser,
  readRecentAdminCreditGrants,
} from './admin-credit-grant'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

// Real-SQL D1 adapter backed by node:sqlite — same pattern as
// usd-credit-bridge.test.ts / github-signup-credit-grant.test.ts, so the
// load-bearing constraints (never-negative balance, the double idempotency
// guard) are genuine, not modeled.
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

  batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
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
`

const NOW = '2026-07-06T12:00:00.000Z'
const USER = 'user_abc'
const ADMIN = 'user_owner'
const ACCOUNT = agentRefForUser(USER)

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const deps = (db: D1Database) => ({ db, nowIso: () => NOW })

// Issue #8505 (Part 2): a spy for the fail-soft Khala Sync credit-balance
// projection seam.
const makeRecorder = () => {
  const calls: Array<{
    accountRef: string
    idempotencyKey: string
    deltaUsdCents: number
    observedAt: string
  }> = []
  return {
    calls,
    recorder: async (event: (typeof calls)[number]) => {
      calls.push(event)
    },
  }
}

describe('grantAdminCredit (AIUR-2, #8500)', () => {
  test('grants exactly the requested USD-origin, inference-spendable credit', async () => {
    const db = makeDb()
    const outcome = await run(
      grantAdminCredit(
        {
          amountUsdCents: 500,
          grantedByUserId: ADMIN,
          grantRef: adminCreditGrantRef('grant-1'),
          reason: 'Beta tester welcome credit',
          userId: USER,
        },
        deps(db),
      ),
    )

    expect(outcome.ok).toBe(true)
    if (!outcome.ok) return
    expect(outcome.alreadyGranted).toBe(false)
    expect(outcome.grantedCents).toBe(500)
    expect(outcome.grantedMsat).toBe(usdCentsToMsatFloor(500))
    expect(outcome.receiptRef).toBe(
      usdCreditGrantReceiptRef(adminCreditGrantRef('grant-1')),
    )

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(500))
    expect(balance?.usdCreditMsat).toBe(usdCentsToMsatFloor(500))
    expect(balance?.bitcoinWithdrawableMsat).toBe(0)
  })

  test('is idempotent on the caller-supplied grantRef — a retry never double-grants', async () => {
    const db = makeDb()
    const grantRef = adminCreditGrantRef('grant-retry')
    const input = {
      amountUsdCents: 1000,
      grantedByUserId: ADMIN,
      grantRef,
      reason: 'Retry test',
      userId: USER,
    }

    const first = await run(grantAdminCredit(input, deps(db)))
    const second = await run(grantAdminCredit(input, deps(db)))

    expect(first.ok && !first.alreadyGranted).toBe(true)
    expect(second.ok && second.alreadyGranted).toBe(true)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(1000)) // NOT doubled
  })

  test('concurrent duplicate grant attempts (same grantRef) settle to exactly one credit', async () => {
    const db = makeDb()
    const grantRef = adminCreditGrantRef('grant-race')
    const input = {
      amountUsdCents: 250,
      grantedByUserId: ADMIN,
      grantRef,
      reason: 'Race test',
      userId: USER,
    }

    const [a, b] = await Promise.all([
      run(grantAdminCredit(input, deps(db))),
      run(grantAdminCredit(input, deps(db))),
    ])
    expect(a.ok).toBe(true)
    expect(b.ok).toBe(true)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(250))
  })

  test('refuses a blank reason', async () => {
    const db = makeDb()
    const outcome = await run(
      grantAdminCredit(
        {
          amountUsdCents: 500,
          grantedByUserId: ADMIN,
          grantRef: adminCreditGrantRef('grant-blank-reason'),
          reason: '   ',
          userId: USER,
        },
        deps(db),
      ),
    )
    expect(outcome.ok).toBe(false)
    if (outcome.ok) return
    expect(outcome.reason).toBe('reason_required')
  })

  test('refuses a non-positive amount', async () => {
    const db = makeDb()
    const outcome = await run(
      grantAdminCredit(
        {
          amountUsdCents: 0,
          grantedByUserId: ADMIN,
          grantRef: adminCreditGrantRef('grant-zero'),
          reason: 'Zero amount',
          userId: USER,
        },
        deps(db),
      ),
    )
    expect(outcome.ok).toBe(false)
    if (outcome.ok) return
    expect(outcome.reason).toBe('amount_invalid')
  })

  test('the shared RL-3 guard refuses a free/promotional grant from ever funding a Bitcoin share', () => {
    // Confirms the underlying invariant this module relies on (its own call
    // always uses `movement: 'spend'`, which the guard allows) — mirrors the
    // identical assertion in github-signup-credit-grant.test.ts.
    const violation = validateAssetBoundary({
      contributorAsset: 'bitcoin',
      movement: 'payout',
      revenueAsset: 'free',
    })
    expect(violation).not.toBe(null)
  })

  test('readAdminCreditGrantsForUser and readRecentAdminCreditGrants return the receipted grant', async () => {
    const db = makeDb()
    await run(
      grantAdminCredit(
        {
          amountUsdCents: 300,
          grantedByUserId: ADMIN,
          grantRef: adminCreditGrantRef('grant-history'),
          reason: 'History test',
          userId: USER,
        },
        deps(db),
      ),
    )

    const forUser = await readAdminCreditGrantsForUser(db, USER)
    expect(forUser).toHaveLength(1)
    expect(forUser[0]?.reason).toBe('History test')
    expect(forUser[0]?.grantedByUserId).toBe(ADMIN)

    const recent = await readRecentAdminCreditGrants(db, 10)
    expect(recent.some(row => row.grantRef === adminCreditGrantRef('grant-history'))).toBe(
      true,
    )
  })

  describe('recordCreditBalanceProjection seam (#8505)', () => {
    test('fires once for a fresh grant, with a positive delta and the D1 grant idempotency key', async () => {
      const db = makeDb()
      const { calls, recorder } = makeRecorder()
      const grantRef = adminCreditGrantRef('grant-projected')
      const outcome = await run(
        grantAdminCredit(
          {
            amountUsdCents: 500,
            grantedByUserId: ADMIN,
            grantRef,
            reason: 'Projection test',
            userId: USER,
          },
          { db, nowIso: () => NOW, recordCreditBalanceProjection: recorder },
        ),
      )
      expect(outcome.ok).toBe(true)
      expect(calls).toHaveLength(1)
      expect(calls[0]?.accountRef).toBe(ACCOUNT)
      expect(calls[0]?.idempotencyKey).toBe(usdCreditGrantIdempotencyKey(grantRef))
      expect(calls[0]?.deltaUsdCents).toBe(500)
    })

    test('never fires on a replay (already granted)', async () => {
      const db = makeDb()
      const { calls, recorder } = makeRecorder()
      const grantRef = adminCreditGrantRef('grant-projected-replay')
      const input = {
        amountUsdCents: 500,
        grantedByUserId: ADMIN,
        grantRef,
        reason: 'Projection replay test',
        userId: USER,
      }
      const withRecorder = { db, nowIso: () => NOW, recordCreditBalanceProjection: recorder }
      await run(grantAdminCredit(input, withRecorder))
      await run(grantAdminCredit(input, withRecorder))
      expect(calls).toHaveLength(1)
    })
  })
})

describe('clawbackAdminCredit (AIUR-2, #8500)', () => {
  test('claws back a previously granted amount', async () => {
    const db = makeDb()
    await run(
      grantAdminCredit(
        {
          amountUsdCents: 1000,
          grantedByUserId: ADMIN,
          grantRef: adminCreditGrantRef('grant-to-claw'),
          reason: 'Grant before clawback',
          userId: USER,
        },
        deps(db),
      ),
    )

    const outcome = await run(
      clawbackAdminCredit(
        {
          amountUsdCents: 400,
          clawbackRef: 'clawback-1',
          reason: 'Refund adjustment',
          userId: USER,
        },
        deps(db),
      ),
    )
    expect(outcome.clawedBack).toBe(true)
    expect(outcome.insufficientBalance).toBe(false)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(
      usdCentsToMsatFloor(1000) - usdCentsToMsatFloor(400),
    )
  })

  test('is idempotent on the caller-supplied clawbackRef', async () => {
    const db = makeDb()
    await run(
      grantAdminCredit(
        {
          amountUsdCents: 1000,
          grantedByUserId: ADMIN,
          grantRef: adminCreditGrantRef('grant-to-claw-2'),
          reason: 'Grant before clawback',
          userId: USER,
        },
        deps(db),
      ),
    )
    const input = {
      amountUsdCents: 400,
      clawbackRef: 'clawback-retry',
      reason: 'Refund adjustment',
      userId: USER,
    }
    const first = await run(clawbackAdminCredit(input, deps(db)))
    const second = await run(clawbackAdminCredit(input, deps(db)))
    expect(first.clawedBack).toBe(true)
    expect(second.clawedBack).toBe(true)

    const balance = await readAgentBalance(db, ACCOUNT)
    // Only clawed back ONCE, not twice.
    expect(balance?.balanceMsat).toBe(
      usdCentsToMsatFloor(1000) - usdCentsToMsatFloor(400),
    )
  })

  test('refuses to claw back more than the account holds (never goes negative)', async () => {
    const db = makeDb()
    await run(
      grantAdminCredit(
        {
          amountUsdCents: 100,
          grantedByUserId: ADMIN,
          grantRef: adminCreditGrantRef('grant-small'),
          reason: 'Small grant',
          userId: USER,
        },
        deps(db),
      ),
    )

    const outcome = await run(
      clawbackAdminCredit(
        {
          amountUsdCents: 10_000,
          clawbackRef: 'clawback-too-much',
          reason: 'Over-clawback attempt',
          userId: USER,
        },
        deps(db),
      ),
    )
    expect(outcome.clawedBack).toBe(false)
    expect(outcome.insufficientBalance).toBe(true)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(100)) // unchanged
  })

  test('adminCreditClawbackSourceRef namespaces the clawback ref distinctly from a grant ref', () => {
    expect(adminCreditClawbackSourceRef('x')).toBe('admin:credit-clawback:x')
    expect(adminCreditGrantRef('x')).toBe('admin:credit-grant:x')
  })

  describe('recordCreditBalanceProjection seam (#8505)', () => {
    test('fires once for a successful clawback, with a negative delta and the D1 clawback idempotency key', async () => {
      const db = makeDb()
      await run(
        grantAdminCredit(
          {
            amountUsdCents: 1000,
            grantedByUserId: ADMIN,
            grantRef: adminCreditGrantRef('grant-before-projected-clawback'),
            reason: 'Grant before clawback',
            userId: USER,
          },
          deps(db),
        ),
      )
      const { calls, recorder } = makeRecorder()
      const clawbackRef = 'clawback-projected'
      const outcome = await run(
        clawbackAdminCredit(
          { amountUsdCents: 400, clawbackRef, reason: 'Refund adjustment', userId: USER },
          { db, nowIso: () => NOW, recordCreditBalanceProjection: recorder },
        ),
      )
      expect(outcome.clawedBack).toBe(true)
      expect(calls).toHaveLength(1)
      expect(calls[0]?.accountRef).toBe(ACCOUNT)
      expect(calls[0]?.idempotencyKey).toBe(
        inferenceClawbackIdempotencyKey(adminCreditClawbackSourceRef(clawbackRef)),
      )
      expect(calls[0]?.deltaUsdCents).toBe(-400)
    })

    test('never fires when the balance CHECK aborts the clawback (insufficientBalance)', async () => {
      const db = makeDb()
      await run(
        grantAdminCredit(
          {
            amountUsdCents: 100,
            grantedByUserId: ADMIN,
            grantRef: adminCreditGrantRef('grant-small-projected'),
            reason: 'Small grant',
            userId: USER,
          },
          deps(db),
        ),
      )
      const { calls, recorder } = makeRecorder()
      const outcome = await run(
        clawbackAdminCredit(
          {
            amountUsdCents: 10_000,
            clawbackRef: 'clawback-too-much-projected',
            reason: 'Over-clawback attempt',
            userId: USER,
          },
          { db, nowIso: () => NOW, recordCreditBalanceProjection: recorder },
        ),
      )
      expect(outcome.clawedBack).toBe(false)
      expect(outcome.insufficientBalance).toBe(true)
      expect(calls).toHaveLength(0)
    })
  })
})
