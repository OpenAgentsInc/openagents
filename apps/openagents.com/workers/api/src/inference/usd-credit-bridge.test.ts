import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { validateAssetBoundary } from '../asset-bitcoin-boundary'
import { readAgentBalance } from '../payments-ledger'
import type { PaymentsLedgerDb } from '../payments-ledger-db'
import { paymentsLedgerDbFromD1 } from '../test/payments-ledger-sqlite'
import { selectSweepCandidates, sweepAmountSat } from '../tips-sweep'
import { makeLedgerMeteringHook, type MeteringContext } from './metering-hook'
import { priceRequest } from './pricing'
import { type InferenceUsage } from './provider-adapter'
import { parseCardCreditGrantContextRef } from './card-credit-provenance'
import {
  agentRefForUser,
  fundInferenceFromCredit,
  usdCreditDebitIdempotencyKey,
  usdCreditGrantIdempotencyKey,
  usdCreditGrantReceiptRef,
} from './usd-credit-bridge'
import {
  DEFAULT_BTC_USD,
  usdCentsToMsatFloor,
  usdToMsatCeil,
} from './usd-msat-conversion'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect)

// Real-SQL D1 adapter backed by node:sqlite (same pattern as the metering hook
// harness) so the load-bearing constraints — never-negative balance, idempotency
// UNIQUE, and the sweep's USD-credit exclusion — are genuine, not modeled.
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

// Schema: the agent_balances columns include `usd_credit_msat` (migration 0211,
// the USD-origin tag), the pay_ins/legs (constraints verbatim), the USD ledger
// (`billing_ledger_entries`, the bridge's debit side), and the tip wallet table
// the sweep joins. CHECKs that gate the asset boundary are copied so the test
// proves real guards.
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
CREATE TABLE forum_tip_recipient_wallets (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL UNIQUE,
  wallet_ref TEXT NOT NULL,
  lightning_address TEXT,
  bolt12_offer TEXT,
  state TEXT NOT NULL,
  archived_at TEXT
);
`

const NOW = '2026-06-19T12:00:00.000Z'
const USER = 'user-123'
const ACCOUNT = agentRefForUser(USER)

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

// CFG-4 (#8519): the msat grant side writes through the Postgres-authoritative
// `PaymentsLedgerDb` seam; tests back it with the same SQLite-D1 shim via the
// portability-checked adapter (one underlying database, two typed handles).
const makeLedgerDb = (db: D1Database): PaymentsLedgerDb =>
  paymentsLedgerDbFromD1(db as never)

// Seed a USD credit balance via a positive billing_ledger_entries row (mirrors a
// Stripe purchase: balance = SUM(amount_cents)).
const seedUsdCents = async (db: D1Database, cents: number): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO billing_ledger_entries
        (id, user_id, source, description, amount_cents, idempotency_key, created_at)
       VALUES (?, ?, 'stripe_checkout', 'seed', ?, ?, ?)`,
    )
    .bind(`seed-${cents}`, USER, cents, `seed:${USER}:${cents}`, NOW)
    .run()
  await db
    .prepare(
      `INSERT OR IGNORE INTO billing_accounts (user_id, status, created_at, updated_at)
       VALUES (?, 'active', ?, ?)`,
    )
    .bind(USER, NOW, NOW)
    .run()
}

const usdBalanceCents = async (db: D1Database): Promise<number> => {
  const row = (await db
    .prepare(
      `SELECT COALESCE(SUM(amount_cents), 0) AS c FROM billing_ledger_entries WHERE user_id = ?`,
    )
    .bind(USER)
    .first()) as { c?: number } | null
  return Number(row?.c ?? 0)
}

const deps = (db: D1Database) => ({ db, ledgerDb: makeLedgerDb(db), nowIso: () => NOW })

// ---------------------------------------------------------------------------
// 1. Conversion is the single source of truth
// ---------------------------------------------------------------------------
describe('usd-msat-conversion (single source, #5497)', () => {
  test('the bridge cents-conversion and the metering charge use the IDENTICAL rate', () => {
    // $1 (100c) at $100k/BTC = 1e-5 BTC = 1_000_000 msat exactly, both directions.
    expect(usdCentsToMsatFloor(100)).toBe(1_000_000)
    expect(usdToMsatCeil(1)).toBe(1_000_000)
    // Both default to the same shared rate constant.
    expect(usdCentsToMsatFloor(100, DEFAULT_BTC_USD)).toBe(usdToMsatCeil(1))
    expect(DEFAULT_BTC_USD).toBe(100_000)
  })

  test('cents-to-msat floors (never over-credits past paid USD) and rejects junk', () => {
    expect(usdCentsToMsatFloor(0)).toBe(0)
    expect(usdCentsToMsatFloor(-5)).toBe(0)
    expect(usdCentsToMsatFloor(Number.NaN)).toBe(0)
    // 1 cent @ $100k/BTC = 10_000 msat exactly.
    expect(usdCentsToMsatFloor(1)).toBe(10_000)
  })
})

// ---------------------------------------------------------------------------
// 2/3. The fund-inference action: grant, idempotency, bounded, never-negative
// ---------------------------------------------------------------------------
describe('fundInferenceFromCredit (#5497)', () => {
  test('debits USD and grants the equivalent USD-origin msat into the agent balance', async () => {
    const db = makeDb()
    await seedUsdCents(db, 500) // $5.00

    const outcome = await run(
      fundInferenceFromCredit(
        { amountCents: 300, grantRef: 'g1', userId: USER },
        deps(db),
      ),
    )

    expect(outcome.ok).toBe(true)
    if (!outcome.ok) return
    expect(outcome.grantedCents).toBe(300)
    expect(outcome.grantedMsat).toBe(usdCentsToMsatFloor(300))
    expect(outcome.receiptRef).toBe(usdCreditGrantReceiptRef('g1'))
    expect(outcome.remainingCreditCents).toBe(200)

    // USD ledger debited by exactly the granted cents (a negative row).
    expect(await usdBalanceCents(db)).toBe(200)

    // The agent balance gained the msat AND it is tagged USD-origin
    // (inference-spendable, NOT Bitcoin-withdrawable).
    const balance = await readAgentBalance(makeLedgerDb(db), ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(300))
    expect(balance?.availableMsat).toBe(usdCentsToMsatFloor(300))
    expect(balance?.usdCreditMsat).toBe(usdCentsToMsatFloor(300))
    expect(balance?.bitcoinWithdrawableMsat).toBe(0)

    // The grant pay-in is a `usd_credit_grant` marked paid with the receipt ref.
    const payIn = (await db
      .prepare(`SELECT pay_in_type, state, public_receipt_ref FROM pay_ins WHERE idempotency_key = ?`)
      .bind(usdCreditGrantIdempotencyKey('g1'))
      .first()) as Row | null
    expect(payIn?.pay_in_type).toBe('usd_credit_grant')
    expect(payIn?.state).toBe('paid')

    // KS-8.7 (#8318/#8337) FOUND + FIXED: the audit leg's `party_ref` and
    // `amount_msat` were bound in the WRONG order (party_ref got the msat
    // amount, amount_msat got the account ref string) — a real production
    // data bug that D1/SQLite's weak typing silently accepted for years.
    // Assert the actual column values so a param-order regression here is
    // caught in CI, not just by a strict-Postgres backfill sweep.
    const leg = (await db
      .prepare(
        `SELECT party_ref, amount_msat, direction, kind, external_ref
           FROM pay_in_legs WHERE pay_in_id = ?`,
      )
      .bind('inference:usd-credit:g1')
      .first()) as Row | null
    expect(leg?.party_ref).toBe(ACCOUNT)
    expect(leg?.amount_msat).toBe(usdCentsToMsatFloor(300))
    expect(leg?.direction).toBe('in')
    expect(leg?.kind).toBe('balance')
    expect(leg?.external_ref).toBe('usd_credit_grant')
  })

  test('stamps the funding Stripe session onto the grant context_ref when provided', async () => {
    const db = makeDb()
    await seedUsdCents(db, 500)

    const outcome = await run(
      fundInferenceFromCredit(
        {
          amountCents: 300,
          grantRef: 'g_session',
          sourceCheckoutSessionId: 'cs_test_xyz',
          userId: USER,
        },
        deps(db),
      ),
    )
    expect(outcome.ok).toBe(true)

    // The stored grant context_ref is dereferenceable back to its funding
    // session, so the card->credit->inference-spend chain can be proven.
    const payIn = (await db
      .prepare(`SELECT context_ref FROM pay_ins WHERE idempotency_key = ?`)
      .bind(usdCreditGrantIdempotencyKey('g_session'))
      .first()) as Row | null
    expect(
      parseCardCreditGrantContextRef(String(payIn?.context_ref ?? '')),
    ).toBe('cs_test_xyz')
  })

  test('falls back to the legacy generic context_ref when no session is provided', async () => {
    const db = makeDb()
    await seedUsdCents(db, 500)

    await run(
      fundInferenceFromCredit(
        { amountCents: 300, grantRef: 'g_nosession', userId: USER },
        deps(db),
      ),
    )

    const payIn = (await db
      .prepare(`SELECT context_ref FROM pay_ins WHERE idempotency_key = ?`)
      .bind(usdCreditGrantIdempotencyKey('g_nosession'))
      .first()) as Row | null
    // No card origin: the generic format carries the user, not a session.
    expect(String(payIn?.context_ref)).toBe(`inference:usd-credit:${USER}`)
    expect(
      parseCardCreditGrantContextRef(String(payIn?.context_ref ?? '')),
    ).toBeUndefined()
  })

  test('is idempotent per grant ref: replaying the same ref never double-grants or double-debits', async () => {
    const db = makeDb()
    await seedUsdCents(db, 500)

    const first = await run(
      fundInferenceFromCredit(
        { amountCents: 300, grantRef: 'dup', userId: USER },
        deps(db),
      ),
    )
    expect(first.ok).toBe(true)

    // Replay with the SAME grantRef. The UNIQUE idempotency keys (both ledgers)
    // make both writes no-ops, so balances do not move a second time.
    await run(
      fundInferenceFromCredit(
        { amountCents: 300, grantRef: 'dup', userId: USER },
        deps(db),
      ),
    )

    expect(await usdBalanceCents(db)).toBe(200)
    const balance = await readAgentBalance(makeLedgerDb(db), ACCOUNT)
    expect(balance?.balanceMsat).toBe(usdCentsToMsatFloor(300))
    expect(balance?.usdCreditMsat).toBe(usdCentsToMsatFloor(300))
    // Exactly one grant pay-in and one USD debit exist.
    const payIns = (await db
      .prepare(`SELECT COUNT(*) AS n FROM pay_ins WHERE idempotency_key = ?`)
      .bind(usdCreditGrantIdempotencyKey('dup'))
      .first()) as { n?: number } | null
    expect(Number(payIns?.n)).toBe(1)
    const debits = (await db
      .prepare(`SELECT COUNT(*) AS n FROM billing_ledger_entries WHERE idempotency_key = ?`)
      .bind(usdCreditDebitIdempotencyKey('dup'))
      .first()) as { n?: number } | null
    expect(Number(debits?.n)).toBe(1)
  })

  test('is bounded by the available USD balance: a request over balance clamps down', async () => {
    const db = makeDb()
    await seedUsdCents(db, 250) // only $2.50 available

    const outcome = await run(
      fundInferenceFromCredit(
        { amountCents: 1000, grantRef: 'clamp', userId: USER },
        deps(db),
      ),
    )

    expect(outcome.ok).toBe(true)
    if (!outcome.ok) return
    // Clamped to the available 250c — never overdraws the USD ledger.
    expect(outcome.grantedCents).toBe(250)
    expect(outcome.remainingCreditCents).toBe(0)
    // USD balance never goes negative.
    expect(await usdBalanceCents(db)).toBe(0)
  })

  test('refuses when there is no USD balance to fund from', async () => {
    const db = makeDb()
    const outcome = await run(
      fundInferenceFromCredit(
        { amountCents: 100, grantRef: 'broke', userId: USER },
        deps(db),
      ),
    )
    expect(outcome.ok).toBe(false)
    if (outcome.ok) return
    expect(outcome.reason).toBe('insufficient_credit')
    // No grant, no balance row.
    expect(await readAgentBalance(makeLedgerDb(db), ACCOUNT)).toBe(null)
  })

  test('rejects a non-positive amount without touching either ledger', async () => {
    const db = makeDb()
    await seedUsdCents(db, 500)
    const outcome = await run(
      fundInferenceFromCredit(
        { amountCents: 0, grantRef: 'zero', userId: USER },
        deps(db),
      ),
    )
    expect(outcome.ok).toBe(false)
    if (outcome.ok) return
    expect(outcome.reason).toBe('amount_invalid')
    expect(await usdBalanceCents(db)).toBe(500)
  })
})

// ---------------------------------------------------------------------------
// 4. RL-3 ASSET BOUNDARY: usd_credit_grant is inference-spendable but NOT
//    Bitcoin-withdrawable.
// ---------------------------------------------------------------------------
describe('RL-3 asset boundary: usd_credit_grant (#5497, #5460)', () => {
  const usage: InferenceUsage = {
    completionTokens: 1000,
    promptTokens: 4000,
    totalTokens: 5000,
  }
  const meteringContext = (): MeteringContext => ({
    accountRef: ACCOUNT,
    adapterId: 'fireworks',
    fundingKind: 'card',
    requestId: 'infer-1',
    requestedModel: 'sonnet',
    servedModel: 'sonnet',
    streamed: false,
    usage,
  })

  test('a USD-funded balance CAN be spent on inference (metering decrements it)', async () => {
    const db = makeDb()
    // Fund a large credit so the charge clears.
    await seedUsdCents(db, 100_000) // $1000
    await run(
      fundInferenceFromCredit(
        { amountCents: 100_000, grantRef: 'spend', userId: USER },
        deps(db),
      ),
    )
    const before = await readAgentBalance(makeLedgerDb(db), ACCOUNT)
    expect(before?.balanceMsat).toBeGreaterThan(0)

    const hook = makeLedgerMeteringHook({ ledgerDb: makeLedgerDb(db), nowIso: () => NOW })
    const outcome = await run(hook(meteringContext()))
    expect(outcome.metered).toBe(true)

    const expectedMsat = usdToMsatCeil(
      priceRequest({ fundingKind: 'card', model: 'sonnet', usage }).chargeUsd,
    )
    expect(expectedMsat).toBeGreaterThan(0)
    const after = await readAgentBalance(makeLedgerDb(db), ACCOUNT)
    // The USD-funded balance was spent on inference.
    expect(after?.balanceMsat).toBe((before?.balanceMsat ?? 0) - expectedMsat)
  })

  test('a USD-funded balance is NOT Bitcoin-withdrawable: the sweep excludes it', async () => {
    const db = makeDb()
    // Grant a big USD credit so balance_msat is well above the sweep threshold.
    await seedUsdCents(db, 1_000_000) // $10,000
    await run(
      fundInferenceFromCredit(
        { amountCents: 1_000_000, grantRef: 'noswp', userId: USER },
        deps(db),
      ),
    )
    // Register a ready Lightning wallet so the candidate JOIN matches.
    await db
      .prepare(
        `INSERT INTO forum_tip_recipient_wallets
          (id, actor_ref, wallet_ref, lightning_address, state)
         VALUES (?, ?, ?, ?, 'ready')`,
      )
      .bind('w1', ACCOUNT, 'wallet.public.user.redacted', 'user@spark.money')
      .run()

    const balance = await readAgentBalance(makeLedgerDb(db), ACCOUNT)
    // The whole balance is USD-origin => zero Bitcoin-withdrawable.
    expect(balance?.balanceMsat).toBeGreaterThan(210_000)
    expect(balance?.bitcoinWithdrawableMsat).toBe(0)

    // The sweep (the live Bitcoin-withdrawal path) selects NO candidate, because
    // it subtracts usd_credit_msat. A USD purchase can never leak into Bitcoin.
    const candidates = await selectSweepCandidates(db, makeLedgerDb(db), NOW)
    expect(candidates.length).toBe(0)
  })

  test('a USD-funded balance becomes withdrawable ONLY for the Bitcoin-funded portion added on top', async () => {
    const db = makeDb()
    await seedUsdCents(db, 100_000) // $1000
    await run(
      fundInferenceFromCredit(
        { amountCents: 100_000, grantRef: 'mix', userId: USER },
        deps(db),
      ),
    )
    // Add a Bitcoin-funded (e.g. a tip/reward) top-up directly to balance_msat
    // WITHOUT bumping usd_credit_msat (that is how Lightning pay-ins credit).
    await db
      .prepare(
        `UPDATE agent_balances SET balance_msat = balance_msat + ? WHERE actor_ref = ?`,
      )
      .bind(5_000_000, ACCOUNT) // +5000 sat of real Bitcoin
      .run()
    await db
      .prepare(
        `INSERT INTO forum_tip_recipient_wallets
          (id, actor_ref, wallet_ref, lightning_address, state)
         VALUES (?, ?, ?, ?, 'ready')`,
      )
      .bind('w2', ACCOUNT, 'wallet.public.user.redacted', 'user@spark.money')
      .run()

    const balance = await readAgentBalance(makeLedgerDb(db), ACCOUNT)
    // Only the Bitcoin-funded 5_000_000 msat is withdrawable; the USD portion is not.
    expect(balance?.bitcoinWithdrawableMsat).toBe(5_000_000)

    const candidates = await selectSweepCandidates(db, makeLedgerDb(db), NOW)
    expect(candidates.length).toBe(1)
    // The sweepable amount is exactly the Bitcoin-funded portion (5000 sat),
    // never the USD-funded portion.
    expect(candidates[0]?.balanceMsat).toBe(5_000_000)
    expect(sweepAmountSat(candidates[0]!)).toBe(5000 - 210)
  })

  test('the shared guard refuses turning a USD-origin spend into a Bitcoin payout', () => {
    // The same primitive the bridge calls: a USD revenue/spend may NOT create a
    // withdrawable Bitcoin share.
    const violation = validateAssetBoundary({
      contributorAsset: 'bitcoin',
      movement: 'payout',
      revenueAsset: 'usd',
    })
    expect(violation).not.toBe(null)
    expect(violation?.reasonRef).toContain('credit_revenue_no_bitcoin')
    // But a USD spend funding a USD/credit share is allowed.
    expect(
      validateAssetBoundary({
        contributorAsset: 'credit',
        movement: 'spend',
        revenueAsset: 'usd',
      }),
    ).toBe(null)
  })
})
