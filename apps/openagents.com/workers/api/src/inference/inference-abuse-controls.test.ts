import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../payments-ledger'
import { AgentRateLimitPolicy } from '../agent-rate-limit-policy'
import {
  clawbackInferenceCredits,
  decideAbuseResponse,
  decideFairShare,
  decideFiatKycGate,
  decideSpendCap,
  DEFAULT_FAIR_SHARE_LIMITS,
  DEFAULT_FIAT_KYC_THRESHOLD_CENTS,
  inferenceClawbackIdempotencyKey,
} from './inference-abuse-controls'

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

// ----------------------------------------------------------------------------
// Real-SQL D1 harness for the clawback ledger test (same pattern as
// metering-hook.test.ts). The load-bearing guards are the balance CHECK
// (never-negative) and the idempotency_key UNIQUE (no double-claw).
// ----------------------------------------------------------------------------
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
`

const NOW = '2026-06-19T12:00:00.000Z'
const ACCOUNT = 'agent:abuse-test'

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const seedBalance = async (db: D1Database, msat: number): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO agent_balances (actor_ref, balance_msat, created_at, updated_at)
       VALUES (?, ?, ?, ?)`,
    )
    .bind(ACCOUNT, msat, NOW, NOW)
    .run()
}

// ----------------------------------------------------------------------------
// 1. Per-customer rate / fair-share
// ----------------------------------------------------------------------------

describe('decideFairShare', () => {
  test('defaults are seeded from the shared AgentRateLimitPolicy', () => {
    expect(DEFAULT_FAIR_SHARE_LIMITS.maxRequests).toBe(AgentRateLimitPolicy.limit)
    expect(DEFAULT_FAIR_SHARE_LIMITS.windowSeconds).toBe(
      AgentRateLimitPolicy.windowSeconds,
    )
  })

  test('allows a request comfortably under both ceilings, echoing remaining', () => {
    const decision = decideFairShare({
      usage: { requestsInWindow: 5, tokensInWindow: 1000 },
      limits: { maxRequests: 60, maxTokens: 2_000_000, windowSeconds: 60 },
    })
    expect(decision.allowed).toBe(true)
    expect(decision.status).toBe('allow')
    expect(decision.statusCode).toBe(200)
    expect(decision.remainingRequests).toBe(55)
    expect(decision.remainingTokens).toBe(1_999_000)
  })

  test('rejects (429) when the per-window request ceiling is reached', () => {
    const decision = decideFairShare({
      usage: { requestsInWindow: 60, tokensInWindow: 0 },
      limits: { maxRequests: 60, maxTokens: 2_000_000, windowSeconds: 60 },
    })
    expect(decision.allowed).toBe(false)
    expect(decision.status).toBe('request_rate_exceeded')
    expect(decision.statusCode).toBe(429)
    expect(decision.remainingRequests).toBe(0)
  })

  test('enforces token fair-share so one customer cannot starve the shared pool', () => {
    // Under the request ceiling, but has already drawn the whole token budget.
    const decision = decideFairShare({
      usage: { requestsInWindow: 3, tokensInWindow: 2_000_000 },
      limits: { maxRequests: 60, maxTokens: 2_000_000, windowSeconds: 60 },
    })
    expect(decision.allowed).toBe(false)
    expect(decision.status).toBe('token_fair_share_exceeded')
    expect(decision.statusCode).toBe(429)
    expect(decision.remainingTokens).toBe(0)
    // request rate alone would have allowed it
    expect(decision.remainingRequests).toBe(57)
  })
})

// ----------------------------------------------------------------------------
// 2. Spend caps
// ----------------------------------------------------------------------------

describe('decideSpendCap', () => {
  test('no cap configured => open (distinct from balance gate)', () => {
    const decision = decideSpendCap({
      cap: { maxSpendMsatPerWindow: null, windowSeconds: 86_400 },
      spentMsatInWindow: 999_999_999,
    })
    expect(decision.allowed).toBe(true)
    expect(decision.status).toBe('no_cap_configured')
    expect(decision.remainingMsat).toBeNull()
  })

  test('allows when spent + estimate stays under the cap', () => {
    const decision = decideSpendCap({
      cap: { maxSpendMsatPerWindow: 1_000_000, windowSeconds: 86_400 },
      spentMsatInWindow: 600_000,
      estimatedChargeMsat: 300_000,
    })
    expect(decision.allowed).toBe(true)
    expect(decision.status).toBe('allow')
    expect(decision.remainingMsat).toBe(400_000)
  })

  test('rejects (402) when spent + estimate would exceed the cap', () => {
    const decision = decideSpendCap({
      cap: { maxSpendMsatPerWindow: 1_000_000, windowSeconds: 86_400 },
      spentMsatInWindow: 900_000,
      estimatedChargeMsat: 200_000,
    })
    expect(decision.allowed).toBe(false)
    expect(decision.status).toBe('spend_cap_exceeded')
    expect(decision.statusCode).toBe(402)
    expect(decision.remainingMsat).toBe(100_000)
  })

  test('pre-flight (no estimate) rejects once the cap is already blown', () => {
    const decision = decideSpendCap({
      cap: { maxSpendMsatPerWindow: 1_000_000, windowSeconds: 86_400 },
      spentMsatInWindow: 1_000_001,
    })
    expect(decision.allowed).toBe(false)
    expect(decision.status).toBe('spend_cap_exceeded')
  })
})

// ----------------------------------------------------------------------------
// 3. Light KYC for fiat top-up (Bitcoin unaffected)
// ----------------------------------------------------------------------------

describe('decideFiatKycGate', () => {
  test('bitcoin top-up is always exempt, even far above the threshold', () => {
    const decision = decideFiatKycGate({
      rail: 'bitcoin',
      purchaseCents: 10_000_000,
      currentLevel: 'none',
    })
    expect(decision.allowed).toBe(true)
    expect(decision.status).toBe('bitcoin_exempt')
    expect(decision.requiredLevel).toBe('none')
  })

  test('card under the threshold needs no KYC', () => {
    const decision = decideFiatKycGate({
      rail: 'card',
      purchaseCents: DEFAULT_FIAT_KYC_THRESHOLD_CENTS - 1,
      currentLevel: 'none',
    })
    expect(decision.allowed).toBe(true)
    expect(decision.status).toBe('under_threshold')
  })

  test('card at/over the threshold with no verification is blocked (403) pending light KYC', () => {
    const decision = decideFiatKycGate({
      rail: 'card',
      purchaseCents: DEFAULT_FIAT_KYC_THRESHOLD_CENTS,
      currentLevel: 'none',
    })
    expect(decision.allowed).toBe(false)
    expect(decision.status).toBe('kyc_required')
    expect(decision.statusCode).toBe(403)
    expect(decision.requiredLevel).toBe('light')
    expect(decision.nextActionRef).toBe('next.inference_abuse.complete_light_kyc')
  })

  test('light verification clears the threshold gate for card', () => {
    const decision = decideFiatKycGate({
      rail: 'card',
      purchaseCents: DEFAULT_FIAT_KYC_THRESHOLD_CENTS + 5_000,
      currentLevel: 'light',
    })
    expect(decision.allowed).toBe(true)
    expect(decision.status).toBe('allow')
  })

  test('cumulative card spend (split purchases) cannot dodge the gate', () => {
    // Each purchase is under the threshold, but the cumulative crosses it.
    const decision = decideFiatKycGate({
      rail: 'card',
      purchaseCents: 10_000,
      priorCardCentsInWindow: DEFAULT_FIAT_KYC_THRESHOLD_CENTS - 5_000,
      currentLevel: 'none',
    })
    expect(decision.allowed).toBe(false)
    expect(decision.status).toBe('kyc_required')
  })
})

// ----------------------------------------------------------------------------
// 4. Abuse signals + chargeback/refund clawback
// ----------------------------------------------------------------------------

describe('decideAbuseResponse', () => {
  test('a chargeback implicating credits claws back AND freezes', () => {
    const decision = decideAbuseResponse({
      accountRef: ACCOUNT,
      kind: 'chargeback',
      severity: 'critical',
      disputedMsat: 500_000,
      sourceRef: 'dispute-ref-1',
    })
    expect(decision.action).toBe('clawback_credits')
    expect(decision.clawbackMsat).toBe(500_000)
    expect(decision.freeze).toBe(true)
  })

  test('a refund implicating credits claws back', () => {
    const decision = decideAbuseResponse({
      accountRef: ACCOUNT,
      kind: 'refund',
      severity: 'warn',
      disputedMsat: 123_456,
      sourceRef: 'refund-ref-1',
    })
    expect(decision.action).toBe('clawback_credits')
    expect(decision.clawbackMsat).toBe(123_456)
  })

  test('a critical prompt-injection signal freezes (no money move)', () => {
    const decision = decideAbuseResponse({
      accountRef: ACCOUNT,
      kind: 'prompt_injection_free_compute',
      severity: 'critical',
      disputedMsat: 0,
      sourceRef: 'pi-ref-1',
    })
    expect(decision.action).toBe('freeze_account')
    expect(decision.freeze).toBe(true)
    expect(decision.clawbackMsat).toBe(0)
  })

  test('a low-severity velocity signal is observe-only', () => {
    const decision = decideAbuseResponse({
      accountRef: ACCOUNT,
      kind: 'velocity',
      severity: 'info',
      disputedMsat: 0,
      sourceRef: 'v-ref-1',
    })
    expect(decision.action).toBe('observe')
    expect(decision.freeze).toBe(false)
  })
})

describe('clawbackInferenceCredits (live ledger)', () => {
  test('claws back the disputed credits through the existing PayIn ledger', async () => {
    const db = makeDb()
    await seedBalance(db, 1_000_000)

    const outcome = await run(
      clawbackInferenceCredits(
        { accountRef: ACCOUNT, sourceRef: 'dispute-1', clawbackMsat: 400_000 },
        { db, nowIso: () => NOW },
      ),
    )
    expect(outcome.clawedBack).toBe(true)
    expect(outcome.insufficientBalance).toBe(false)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(600_000)
  })

  // KS-8.7 (#8318/#8337): a wired `mirror` must see the pay_ins/pay_in_legs
  // rows this clawback just created — the RUNBOOK coverage list called out
  // `inference/inference-abuse-controls.ts` as D1-only pending this
  // decommission pass (converged only by periodic backfill sweeps).
  test('a clawback mirrors its pay_ins + pay_in_legs refs when a mirror is wired', async () => {
    const db = makeDb()
    await seedBalance(db, 1_000_000)
    const calls: Array<ReadonlyArray<{ table: string; key: unknown }>> = []
    const mirror = async (
      _db: unknown,
      refs: ReadonlyArray<{ table: string; key: unknown }>,
    ) => {
      calls.push(refs)
    }

    const outcome = await run(
      clawbackInferenceCredits(
        { accountRef: ACCOUNT, sourceRef: 'dispute-mirror-1', clawbackMsat: 400_000 },
        { db, mirror, nowIso: () => NOW },
      ),
    )
    expect(outcome.clawedBack).toBe(true)
    const mirroredTables = calls.flat().map(ref => ref.table)
    expect(mirroredTables).toContain('pay_ins')
    expect(mirroredTables).toContain('pay_in_legs')
    const payInRefs = calls
      .flat()
      .filter(ref => ref.table === 'pay_ins')
      .map(ref => ref.key)
    expect(payInRefs).toContainEqual({
      id: 'inference:clawback:dispute-mirror-1',
    })
  })

  test('is idempotent per source event (webhook replay never double-claws)', async () => {
    const db = makeDb()
    await seedBalance(db, 1_000_000)

    const first = await run(
      clawbackInferenceCredits(
        { accountRef: ACCOUNT, sourceRef: 'dispute-2', clawbackMsat: 300_000 },
        { db, nowIso: () => NOW },
      ),
    )
    const second = await run(
      clawbackInferenceCredits(
        { accountRef: ACCOUNT, sourceRef: 'dispute-2', clawbackMsat: 300_000 },
        { db, nowIso: () => NOW },
      ),
    )
    expect(first.clawedBack).toBe(true)
    expect(second.clawedBack).toBe(true) // idempotent no-op, not a re-charge

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(700_000) // decremented exactly once

    // The idempotency key is stable per source event.
    expect(inferenceClawbackIdempotencyKey('dispute-2')).toBe(
      'inference:clawback:dispute-2',
    )
  })

  test('never goes negative: a clawback larger than the balance fails with insufficientBalance', async () => {
    const db = makeDb()
    await seedBalance(db, 100_000)

    const outcome = await run(
      clawbackInferenceCredits(
        { accountRef: ACCOUNT, sourceRef: 'dispute-3', clawbackMsat: 500_000 },
        { db, nowIso: () => NOW },
      ),
    )
    expect(outcome.clawedBack).toBe(false)
    expect(outcome.insufficientBalance).toBe(true)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(100_000) // untouched
  })

  test('a zero/negative clawback amount is a no-op', async () => {
    const db = makeDb()
    await seedBalance(db, 100_000)

    const outcome = await run(
      clawbackInferenceCredits(
        { accountRef: ACCOUNT, sourceRef: 'dispute-4', clawbackMsat: 0 },
        { db, nowIso: () => NOW },
      ),
    )
    expect(outcome.clawedBack).toBe(false)
    expect(outcome.insufficientBalance).toBe(false)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.availableMsat).toBe(100_000)
  })
})
