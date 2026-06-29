import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../../payments-ledger'
import {
  mintLightningCredits,
  mintMppCredits,
  mppCreditGrantContextRef,
  mppGrantRef,
  mppLightningCreditGrantContextRef,
  mppLightningGrantRef,
  mppLightningPayerAccountRef,
  mppPayerAccountRef,
} from './mpp-credit-grant'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect as Effect.Effect<A, never>)

// Real-SQL in-memory D1, same harness as usd-credit-bridge.test.ts, so the
// never-negative balance CHECK + the idempotency UNIQUE are genuine.
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
  async first<T = Record<string, unknown>>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }
  async all<T = Record<string, unknown>>(): Promise<{ results: T[] }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[],
    }
  }
  async run(): Promise<{ success: true; results: [] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { results: [], success: true }
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
    pay_in_type IN ('tip','sweep','buffer_funding','reward','adjustment','usd_credit_grant','lightning_charge')
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
`

const NOW = '2026-06-22T12:00:00.000Z'

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

describe('mpp credit grant (Phase 3 — settled payment -> Khala credits)', () => {
  test('mints USD-origin credit into the payer-bound agent balance', async () => {
    const db = makeDb()
    const accountRef = mppPayerAccountRef('pi_abc')
    const outcome = await run(
      mintMppCredits(
        { db, nowIso: () => NOW },
        { accountRef, amountCents: 100, paymentIntentId: 'pi_abc' },
      ),
    )

    expect(outcome.grantedMsat).toBeGreaterThan(0)
    expect(outcome.grantRef).toBe(mppGrantRef('pi_abc'))
    expect(outcome.receiptRef).toBe(
      `receipt.inference.usd_credit_grant.${mppGrantRef('pi_abc')}`,
    )

    const balance = await readAgentBalance(db, accountRef)
    expect(balance?.balanceMsat).toBe(outcome.grantedMsat)
    // RL-3: the minted credit is recorded USD-origin (NOT Bitcoin-withdrawable).
    expect(balance?.usdCreditMsat).toBe(outcome.grantedMsat)

    // The grant binds back to the funding PaymentIntent.
    const payIn = (await db
      .prepare(`SELECT context_ref, pay_in_type FROM pay_ins WHERE payer_ref = ?`)
      .bind(accountRef)
      .first()) as { context_ref?: string; pay_in_type?: string } | null
    expect(payIn?.pay_in_type).toBe('usd_credit_grant')
    expect(payIn?.context_ref).toBe(mppCreditGrantContextRef('pi_abc'))
  })

  test('is idempotent per payment id (a replayed settlement never double-credits)', async () => {
    const db = makeDb()
    const accountRef = mppPayerAccountRef('pi_dup')
    const first = await run(
      mintMppCredits(
        { db, nowIso: () => NOW },
        { accountRef, amountCents: 100, paymentIntentId: 'pi_dup' },
      ),
    )
    await run(
      mintMppCredits(
        { db, nowIso: () => NOW },
        { accountRef, amountCents: 100, paymentIntentId: 'pi_dup' },
      ),
    )
    const balance = await readAgentBalance(db, accountRef)
    // Balance reflects ONE grant, not two.
    expect(balance?.balanceMsat).toBe(first.grantedMsat)
  })
})

describe('lightning credit grant (BITCOIN-ORIGIN, not usd_credit_msat)', () => {
  const HASH =
    '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff'

  test('mints Bitcoin-origin credit: balance_msat only, usd_credit_msat untouched', async () => {
    const db = makeDb()
    const accountRef = mppLightningPayerAccountRef(HASH)
    const outcome = await run(
      mintLightningCredits(
        { db, nowIso: () => NOW },
        { accountRef, amountSats: 100, paymentHash: HASH },
      ),
    )
    // 100 sats = 100_000 msat.
    expect(outcome.grantedMsat).toBe(100_000)
    expect(outcome.grantRef).toBe(mppLightningGrantRef(HASH))

    const balance = await readAgentBalance(db, accountRef)
    expect(balance?.balanceMsat).toBe(100_000)
    // RL-3 (the OPPOSITE of the USDC/card rails): real Bitcoin is NOT tagged
    // usd_credit_msat, so it stays Bitcoin-withdrawable.
    expect(balance?.usdCreditMsat).toBe(0)

    const payIn = (await db
      .prepare(`SELECT context_ref, pay_in_type FROM pay_ins WHERE payer_ref = ?`)
      .bind(accountRef)
      .first()) as { context_ref?: string; pay_in_type?: string } | null
    expect(payIn?.pay_in_type).toBe('lightning_charge')
    expect(payIn?.context_ref).toBe(mppLightningCreditGrantContextRef(HASH))
  })

  test('is idempotent per paymentHash (a replayed settlement never double-credits)', async () => {
    const db = makeDb()
    const accountRef = mppLightningPayerAccountRef(HASH)
    const first = await run(
      mintLightningCredits(
        { db, nowIso: () => NOW },
        { accountRef, amountSats: 100, paymentHash: HASH },
      ),
    )
    await run(
      mintLightningCredits(
        { db, nowIso: () => NOW },
        { accountRef, amountSats: 100, paymentHash: HASH },
      ),
    )
    const balance = await readAgentBalance(db, accountRef)
    expect(balance?.balanceMsat).toBe(first.grantedMsat)
  })
})
