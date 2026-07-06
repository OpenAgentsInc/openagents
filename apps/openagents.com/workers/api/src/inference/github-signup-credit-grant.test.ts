import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { validateAssetBoundary } from '../asset-bitcoin-boundary'
import { readAgentBalance } from '../payments-ledger'
import { selectSweepCandidates } from '../tips-sweep'
import {
  agentRefForUser,
  usdCreditGrantIdempotencyKey,
  usdCreditGrantReceiptRef,
} from './usd-credit-bridge'
import { usdCentsToMsatFloor } from './usd-msat-conversion'
import {
  GITHUB_SIGNUP_CREDIT_GRANT_CENTS,
  decideGithubAccountAge,
  decideGithubSignupCreditIpMintCap,
  githubSignupCreditGrantRef,
  grantGithubSignupCredit,
  readGithubSignupCreditGrantsForUser,
} from './github-signup-credit-grant'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

// Real-SQL D1 adapter backed by node:sqlite, the same pattern as
// usd-credit-bridge.test.ts, so the load-bearing constraints (never-negative
// balance, the double idempotency guard, the sweep exclusion) are genuine.
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
  // Real D1 serializes concurrent `.batch()` calls against one database (each
  // is one atomic RPC round-trip); this in-memory node:sqlite double is
  // otherwise reentrant across `await` points within one process, so a
  // same-process `Promise.all` of two `.batch()` calls needs the same
  // serialization to behave like the real thing rather than throwing
  // "transaction within a transaction". A simple FIFO queue reproduces that
  // without pretending SQLite itself is concurrent.
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
    // Keep the queue alive regardless of this call's outcome so a rejected
    // batch does not permanently wedge later callers.
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
CREATE TABLE forum_tip_recipient_wallets (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL UNIQUE,
  wallet_ref TEXT NOT NULL,
  lightning_address TEXT,
  bolt12_offer TEXT,
  state TEXT NOT NULL,
  archived_at TEXT
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
CREATE TABLE github_signup_credit_ip_mints (
  ip_hash TEXT NOT NULL,
  mint_day TEXT NOT NULL,
  mint_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (ip_hash, mint_day)
);
`

const NOW = '2026-07-05T12:00:00.000Z'
const OLD_GITHUB_ACCOUNT_CREATED_AT = '2020-01-01T00:00:00.000Z'
const USER = 'github:1'
const GITHUB_USER_ID = '1'
const ACCOUNT = agentRefForUser(USER)

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const deps = (db: D1Database) => ({ db, nowIso: () => NOW })

describe('decideGithubAccountAge (MM-D1, #8478)', () => {
  test('is eligible when the account is older than the floor', () => {
    const decision = decideGithubAccountAge({
      githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT,
      nowIso: NOW,
    })
    expect(decision.eligible).toBe(true)
    expect(decision.ageSeconds).toBeGreaterThan(0)
  })

  test('defers a brand-new account created seconds ago', () => {
    const decision = decideGithubAccountAge({
      githubAccountCreatedAtIso: '2026-07-05T11:59:50.000Z',
      minAccountAgeSeconds: 3600,
      nowIso: NOW,
    })
    expect(decision.eligible).toBe(false)
    expect(decision.ageSeconds).toBe(10)
  })

  test('does not penalize a missing/unparseable created_at (defense-in-depth only)', () => {
    expect(
      decideGithubAccountAge({ githubAccountCreatedAtIso: undefined, nowIso: NOW })
        .eligible,
    ).toBe(true)
    expect(
      decideGithubAccountAge({
        githubAccountCreatedAtIso: 'not-a-date',
        nowIso: NOW,
      }).eligible,
    ).toBe(true)
  })
})

describe('decideGithubSignupCreditIpMintCap (MM-D1, #8478)', () => {
  test('allows under the cap and denies at/over it', () => {
    expect(
      decideGithubSignupCreditIpMintCap({ maxMintsPerDay: 5, mintsToday: 4 })
        .allowed,
    ).toBe(true)
    expect(
      decideGithubSignupCreditIpMintCap({ maxMintsPerDay: 5, mintsToday: 5 })
        .allowed,
    ).toBe(false)
  })
})

describe('grantGithubSignupCredit (MM-D1, #8478)', () => {
  test('grants exactly $10 of USD-origin, inference-spendable credit on first sign-in', async () => {
    const db = makeDb()
    const outcome = await run(
      grantGithubSignupCredit(
        {
          githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT,
          githubUserId: GITHUB_USER_ID,
          ipHash: 'iphash-1',
          userId: USER,
        },
        deps(db),
      ),
    )

    expect(outcome.ok).toBe(true)
    if (!outcome.ok) return
    expect(outcome.alreadyGranted).toBe(false)
    expect(outcome.grantedCents).toBe(GITHUB_SIGNUP_CREDIT_GRANT_CENTS)
    expect(outcome.grantedMsat).toBe(usdCentsToMsatFloor(1000))
    expect(outcome.receiptRef).toBe(
      usdCreditGrantReceiptRef(githubSignupCreditGrantRef(GITHUB_USER_ID)),
    )

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(1000))
    expect(balance?.usdCreditMsat).toBe(usdCentsToMsatFloor(1000))
    // RL-3: never Bitcoin-withdrawable.
    expect(balance?.bitcoinWithdrawableMsat).toBe(0)

    const history = await readGithubSignupCreditGrantsForUser(db, USER)
    expect(history).toHaveLength(1)
    expect(history[0]?.githubUserId).toBe(GITHUB_USER_ID)
    expect(history[0]?.amountUsdCents).toBe(GITHUB_SIGNUP_CREDIT_GRANT_CENTS)
  })

  // Issue #8505 (Part 2): the fail-soft Khala Sync credit-balance projection
  // seam.
  describe('recordCreditBalanceProjection seam (#8505)', () => {
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

    test('fires once for a fresh grant, with a positive delta and the D1 grant idempotency key', async () => {
      const db = makeDb()
      const { calls, recorder } = makeRecorder()
      const outcome = await run(
        grantGithubSignupCredit(
          { githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT, githubUserId: GITHUB_USER_ID, userId: USER },
          { db, nowIso: () => NOW, recordCreditBalanceProjection: recorder },
        ),
      )
      expect(outcome.ok).toBe(true)
      expect(calls).toHaveLength(1)
      expect(calls[0]?.accountRef).toBe(ACCOUNT)
      expect(calls[0]?.idempotencyKey).toBe(
        usdCreditGrantIdempotencyKey(githubSignupCreditGrantRef(GITHUB_USER_ID)),
      )
      expect(calls[0]?.deltaUsdCents).toBe(GITHUB_SIGNUP_CREDIT_GRANT_CENTS)
    })

    test('never fires on a replay (already granted)', async () => {
      const db = makeDb()
      const { calls, recorder } = makeRecorder()
      const withRecorder = { db, nowIso: () => NOW, recordCreditBalanceProjection: recorder }
      await run(
        grantGithubSignupCredit(
          { githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT, githubUserId: GITHUB_USER_ID, userId: USER },
          withRecorder,
        ),
      )
      await run(
        grantGithubSignupCredit(
          { githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT, githubUserId: GITHUB_USER_ID, userId: USER },
          withRecorder,
        ),
      )
      expect(calls).toHaveLength(1)
    })
  })

  test('is idempotent on the GitHub account id: replaying (even across processes) never double-grants', async () => {
    const db = makeDb()
    const first = await run(
      grantGithubSignupCredit(
        { githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT, githubUserId: GITHUB_USER_ID, userId: USER },
        deps(db),
      ),
    )
    expect(first.ok).toBe(true)

    const second = await run(
      grantGithubSignupCredit(
        { githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT, githubUserId: GITHUB_USER_ID, userId: USER },
        deps(db),
      ),
    )
    expect(second.ok).toBe(true)
    if (!second.ok) return
    expect(second.alreadyGranted).toBe(true)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(1000))
    const history = await readGithubSignupCreditGrantsForUser(db, USER)
    expect(history).toHaveLength(1)
  })

  test('is idempotent under a concurrent race (two simultaneous first-time calls)', async () => {
    const db = makeDb()
    const [first, second] = await Promise.all([
      run(
        grantGithubSignupCredit(
          {
            githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT,
            githubUserId: GITHUB_USER_ID,
            userId: USER,
          },
          deps(db),
        ),
      ),
      run(
        grantGithubSignupCredit(
          {
            githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT,
            githubUserId: GITHUB_USER_ID,
            userId: USER,
          },
          deps(db),
        ),
      ),
    ])
    expect(first.ok).toBe(true)
    expect(second.ok).toBe(true)

    // Whichever call "won", the account was credited EXACTLY ONCE.
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(1000))
    const history = await readGithubSignupCreditGrantsForUser(db, USER)
    expect(history).toHaveLength(1)
  })

  test('defers the grant for a brand-new GitHub account (heuristic gate)', async () => {
    const db = makeDb()
    const outcome = await run(
      grantGithubSignupCredit(
        {
          githubAccountCreatedAtIso: '2026-07-05T11:59:50.000Z',
          githubUserId: GITHUB_USER_ID,
          userId: USER,
        },
        { ...deps(db), minAccountAgeSeconds: 3600 },
      ),
    )
    expect(outcome.ok).toBe(false)
    if (outcome.ok || outcome.reason !== 'account_too_new') {
      throw new Error('expected an account_too_new outcome')
    }
    expect(outcome.ageSeconds).toBe(10)

    // No credit was granted and no grant row exists (so a later retry can
    // still succeed once the account ages past the floor).
    expect(await readAgentBalance(db, ACCOUNT)).toBe(null)
    expect(await readGithubSignupCreditGrantsForUser(db, USER)).toHaveLength(0)
  })

  test('a deferred account can be granted later once it ages past the floor', async () => {
    const db = makeDb()
    const deferred = await run(
      grantGithubSignupCredit(
        {
          githubAccountCreatedAtIso: '2026-07-05T11:59:50.000Z',
          githubUserId: GITHUB_USER_ID,
          userId: USER,
        },
        { ...deps(db), minAccountAgeSeconds: 3600 },
      ),
    )
    expect(deferred.ok).toBe(false)

    const later = await run(
      grantGithubSignupCredit(
        {
          githubAccountCreatedAtIso: '2026-07-05T11:59:50.000Z',
          githubUserId: GITHUB_USER_ID,
          userId: USER,
        },
        { db, minAccountAgeSeconds: 3600, nowIso: () => '2026-07-05T13:00:00.000Z' },
      ),
    )
    expect(later.ok).toBe(true)
    if (!later.ok) return
    expect(later.alreadyGranted).toBe(false)
  })

  test('denies over the per-IP-hash daily mint cap without touching the ledger', async () => {
    const db = makeDb()
    const outcome = await run(
      grantGithubSignupCredit(
        {
          githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT,
          githubUserId: 'gh-over-cap',
          ipHash: 'iphash-cap',
          userId: 'github:over-cap',
        },
        { ...deps(db), maxMintsPerIpPerDay: 1 },
      ),
    )
    expect(outcome.ok).toBe(true)

    const second = await run(
      grantGithubSignupCredit(
        {
          githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT,
          githubUserId: 'gh-over-cap-2',
          ipHash: 'iphash-cap',
          userId: 'github:over-cap-2',
        },
        { ...deps(db), maxMintsPerIpPerDay: 1 },
      ),
    )
    expect(second.ok).toBe(false)
    if (second.ok) return
    expect(second.reason).toBe('ip_mint_cap_exceeded')
    expect(await readAgentBalance(db, agentRefForUser('github:over-cap-2'))).toBe(
      null,
    )
  })

  test('never leaks into Bitcoin-withdrawable balance: the Lightning sweep excludes it', async () => {
    const db = makeDb()
    await run(
      grantGithubSignupCredit(
        {
          githubAccountCreatedAtIso: OLD_GITHUB_ACCOUNT_CREATED_AT,
          githubUserId: GITHUB_USER_ID,
          userId: USER,
        },
        deps(db),
      ),
    )
    await db
      .prepare(
        `INSERT INTO forum_tip_recipient_wallets
          (id, actor_ref, wallet_ref, lightning_address, state)
         VALUES (?, ?, ?, ?, 'ready')`,
      )
      .bind('w1', ACCOUNT, 'wallet.public.user.redacted', 'user@spark.money')
      .run()

    const candidates = await selectSweepCandidates(db, NOW)
    expect(candidates.length).toBe(0)
  })

  test('the shared RL-3 guard refuses a free/promotional grant from ever funding a Bitcoin share', () => {
    const violation = validateAssetBoundary({
      contributorAsset: 'bitcoin',
      movement: 'payout',
      revenueAsset: 'free',
    })
    expect(violation).not.toBe(null)
    expect(violation?.reasonRef).toContain('free_or_promo_no_withdrawable')
  })
})
