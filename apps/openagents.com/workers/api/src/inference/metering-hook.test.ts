import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { projectMdkPayoutModeGate } from '../mdk-payout-mode-gate'
import { readAgentBalance } from '../payments-ledger'
import {
  DEFAULT_BTC_USD,
  inferenceChargeContextRef,
  inferenceChargeIdempotencyKey,
  inferenceChargeReceiptRef,
  makeLedgerMeteringHook,
  parseInferenceChargeContextRef,
  type MeteringContext,
  usdToMsatCeil,
} from './metering-hook'
import { type ServingReceipt } from './openagents-network-adapter'
import { priceRequest } from './pricing'
import { type InferenceUsage } from './provider-adapter'
import { type ServingNodePayoutDecision } from './serving-node-payout'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

// Minimal real-SQL D1 adapter backed by node:sqlite (same pattern as the
// firm-up settleable-escrow harness). We exercise the live metering hook against
// genuine SQL so the balance CHECK (never-negative) and the idempotency_key
// UNIQUE (no double-charge) guards are REAL, not modeled.
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

// Real ledger schema: the load-bearing constraints are the balance CHECK
// (never-negative) and the idempotency_key UNIQUE (no double-charge). Both are
// copied verbatim from migration 0160 so the guards under test are genuine.
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
const ACCOUNT = 'agent:metering-test'

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

const sonnetUsage: InferenceUsage = {
  completionTokens: 1000,
  promptTokens: 4000,
  totalTokens: 5000,
}

const context = (
  overrides: Partial<MeteringContext> = {},
): MeteringContext => ({
  accountRef: ACCOUNT,
  adapterId: 'fireworks',
  fundingKind: 'card',
  requestId: 'req-1',
  requestedModel: 'sonnet',
  servedModel: 'sonnet',
  streamed: false,
  usage: sonnetUsage,
  ...overrides,
})

// A generous fixed balance so the charge always clears unless we test the gate.
const FUNDED = 1_000_000_000

describe('usdToMsatCeil', () => {
  test('converts USD to integer msat, rounding up so a nonzero charge is never free', () => {
    expect(usdToMsatCeil(0)).toBe(0)
    expect(usdToMsatCeil(-1)).toBe(0)
    expect(usdToMsatCeil(Number.NaN)).toBe(0)
    // $1 at $100k/BTC = 1e-5 BTC = 1_000_000 msat exactly.
    expect(usdToMsatCeil(1, 100_000)).toBe(1_000_000)
    // A sub-msat charge rounds UP to 1 msat (never rounds away to free).
    expect(usdToMsatCeil(1e-12, 100_000)).toBe(1)
  })
})

describe('inference charge context refs', () => {
  test('parse legacy and requested-model contexts', () => {
    expect(
      parseInferenceChargeContextRef(
        inferenceChargeContextRef({
          adapterId: 'fireworks',
          requestedModel: 'openagents/khala-code',
          servedModel: 'accounts/fireworks/models/qwen3',
          totalTokens: 42,
        }),
      ),
    ).toEqual({
      adapterId: 'fireworks',
      requestedModel: 'openagents/khala-code',
      servedModel: 'accounts/fireworks/models/qwen3',
      totalTokens: 42,
    })
    expect(
      parseInferenceChargeContextRef(
        'inference:fireworks:served:accounts%2Ffireworks%2Fmodels%2Fqwen3:tokens:42',
      ),
    ).toEqual({
      adapterId: 'fireworks',
      servedModel: 'accounts/fireworks/models/qwen3',
      totalTokens: 42,
    })
  })
})

describe('makeLedgerMeteringHook (#5477, real SQL)', () => {
  test('computes the charge from usage via priceRequest and decrements the balance', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    const hook = makeLedgerMeteringHook({ db, nowIso: () => NOW })

    const outcome = await run(hook(context()))

    expect(outcome.metered).toBe(true)
    expect(outcome.receiptRef).toBe(inferenceChargeReceiptRef('req-1'))

    // The decrement equals exactly the priced charge converted to msat.
    const expectedUsd = priceRequest({
      fundingKind: 'card',
      model: 'sonnet',
      usage: sonnetUsage,
    }).chargeUsd
    const expectedMsat = usdToMsatCeil(expectedUsd)
    expect(expectedMsat).toBeGreaterThan(0)

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(FUNDED - expectedMsat)

    // The pay-in is a debit-only adjustment with the public receipt ref.
    const payIn = (await db
      .prepare(
        `SELECT pay_in_type, cost_msat, state, public_receipt_ref, idempotency_key, context_ref
           FROM pay_ins WHERE idempotency_key = ?`,
      )
      .bind(inferenceChargeIdempotencyKey('req-1'))
      .first()) as Row | null
    expect(payIn?.pay_in_type).toBe('adjustment')
    expect(payIn?.state).toBe('paid')
    expect(payIn?.cost_msat).toBe(expectedMsat)
    expect(payIn?.public_receipt_ref).toBe(inferenceChargeReceiptRef('req-1'))
    expect(
      parseInferenceChargeContextRef(String(payIn?.context_ref)),
    ).toMatchObject({
      requestedModel: 'sonnet',
      servedModel: 'sonnet',
      totalTokens: sonnetUsage.totalTokens,
    })
  })

  // KS-8.7 (#8318/#8337): a wired `mirror` must see the pay_ins/pay_in_legs
  // rows this charge just created — the RUNBOOK coverage list called out
  // `inference/metering-hook.ts` as D1-only pending this decommission pass
  // (converged only by periodic backfill sweeps, never in real time).
  test('a settled charge mirrors its pay_ins + pay_in_legs refs when a mirror is wired', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    const calls: Array<ReadonlyArray<{ table: string; key: unknown }>> = []
    const mirror = async (
      _db: unknown,
      refs: ReadonlyArray<{ table: string; key: unknown }>,
    ) => {
      calls.push(refs)
    }
    const hook = makeLedgerMeteringHook({ db, mirror, nowIso: () => NOW })

    const outcome = await run(hook(context()))

    expect(outcome.metered).toBe(true)
    const mirroredTables = calls.flat().map(ref => ref.table)
    expect(mirroredTables).toContain('pay_ins')
    expect(mirroredTables).toContain('pay_in_legs')
    const payInRefs = calls
      .flat()
      .filter(ref => ref.table === 'pay_ins')
      .map(ref => ref.key)
    expect(payInRefs).toContainEqual({ id: 'inference:payin:req-1' })
  })

  // Issue #8505 (Part 2): the fail-soft Khala Sync credit-balance projection
  // seam. Fires exactly once per FRESH charge with a negative delta and the
  // SAME idempotency key the D1 charge used; never fires on a replay.
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

    test('fires once for a fresh charge, with a negative delta and the D1 charge idempotency key', async () => {
      const db = makeDb()
      await seedBalance(db, FUNDED)
      const { calls, recorder } = makeRecorder()
      const hook = makeLedgerMeteringHook({
        db,
        nowIso: () => NOW,
        recordCreditBalanceProjection: recorder,
      })

      const outcome = await run(hook(context()))
      expect(outcome.metered).toBe(true)

      expect(calls).toHaveLength(1)
      expect(calls[0]?.accountRef).toBe(ACCOUNT)
      expect(calls[0]?.idempotencyKey).toBe(inferenceChargeIdempotencyKey('req-1'))
      expect(calls[0]?.observedAt).toBe(NOW)
      expect(calls[0]?.deltaUsdCents).toBeLessThan(0)
    })

    test('never fires on an idempotent replay of the same request', async () => {
      const db = makeDb()
      await seedBalance(db, FUNDED)
      const { calls, recorder } = makeRecorder()
      const hook = makeLedgerMeteringHook({
        db,
        nowIso: () => NOW,
        recordCreditBalanceProjection: recorder,
      })

      await run(hook(context()))
      await run(hook(context()))

      expect(calls).toHaveLength(1)
    })

    test('a failure inside the recorder never fails or affects the charge', async () => {
      const db = makeDb()
      await seedBalance(db, FUNDED)
      const hook = makeLedgerMeteringHook({
        db,
        nowIso: () => NOW,
        recordCreditBalanceProjection: async () => {
          throw new Error('simulated Khala Sync projection failure')
        },
      })

      const outcome = await run(hook(context()))
      expect(outcome.metered).toBe(true)
      const balance = await readAgentBalance(db, ACCOUNT)
      expect(balance?.balanceMsat).toBeLessThan(FUNDED)
    })
  })

  test('is idempotent per request: a replayed settle never double-charges', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    const hook = makeLedgerMeteringHook({ db, nowIso: () => NOW })

    const first = await run(hook(context()))
    const afterFirst = await readAgentBalance(db, ACCOUNT)

    // Same request id => same idempotency key => second settle is a no-op.
    const second = await run(hook(context()))
    const afterSecond = await readAgentBalance(db, ACCOUNT)

    expect(first.metered).toBe(true)
    expect(second.metered).toBe(true)
    expect(second.receiptRef).toBe(first.receiptRef)
    expect(afterSecond?.balanceMsat).toBe(afterFirst?.balanceMsat)

    // Exactly one charge row exists for the request.
    const count = (await db
      .prepare('SELECT COUNT(*) AS n FROM pay_ins WHERE idempotency_key = ?')
      .bind(inferenceChargeIdempotencyKey('req-1'))
      .first()) as { n: number } | null
    expect(count?.n).toBe(1)
  })

  test('a distinct request id charges a separate ledger row', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    const hook = makeLedgerMeteringHook({ db, nowIso: () => NOW })

    await run(hook(context({ requestId: 'req-a' })))
    await run(hook(context({ requestId: 'req-b' })))

    const count = (await db
      .prepare(
        `SELECT COUNT(*) AS n FROM pay_ins WHERE idempotency_key IN (?, ?)`,
      )
      .bind(
        inferenceChargeIdempotencyKey('req-a'),
        inferenceChargeIdempotencyKey('req-b'),
      )
      .first()) as { n: number } | null
    expect(count?.n).toBe(2)
  })

  test('never goes negative: an over-charge fails closed and does not decrement', async () => {
    const db = makeDb()
    // Fund with 1 msat — far below any real charge — so the CHECK aborts.
    await seedBalance(db, 1)
    const hook = makeLedgerMeteringHook({ db, nowIso: () => NOW })

    const outcome = await run(hook(context()))

    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBe(null)

    // Balance untouched (never negative); no charge row written.
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(1)
    const payIn = await db
      .prepare('SELECT id FROM pay_ins WHERE idempotency_key = ?')
      .bind(inferenceChargeIdempotencyKey('req-1'))
      .first()
    expect(payIn).toBe(null)
  })

  test('bitcoin funding charges less than card funding for the same usage', async () => {
    const cardDb = makeDb()
    const btcDb = makeDb()
    await seedBalance(cardDb, FUNDED)
    await seedBalance(btcDb, FUNDED)

    await run(
      makeLedgerMeteringHook({ db: cardDb, nowIso: () => NOW })(
        context({ fundingKind: 'card' }),
      ),
    )
    await run(
      makeLedgerMeteringHook({ db: btcDb, nowIso: () => NOW })(
        context({ fundingKind: 'bitcoin' }),
      ),
    )

    const cardBalance = await readAgentBalance(cardDb, ACCOUNT)
    const btcBalance = await readAgentBalance(btcDb, ACCOUNT)
    const cardCharge = FUNDED - (cardBalance?.balanceMsat ?? 0)
    const btcCharge = FUNDED - (btcBalance?.balanceMsat ?? 0)

    expect(cardCharge).toBeGreaterThan(0)
    // Bitcoin funding applies the ~5% discount => strictly cheaper.
    expect(btcCharge).toBeLessThan(cardCharge)
  })

  test('zero-token usage is metered but writes no ledger row', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    const hook = makeLedgerMeteringHook({ db, nowIso: () => NOW })

    const outcome = await run(
      hook(
        context({
          usage: { completionTokens: 0, promptTokens: 0, totalTokens: 0 },
        }),
      ),
    )

    expect(outcome.metered).toBe(true)
    expect(outcome.zeroCharge).toBe(true)
    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(FUNDED)
    const payIn = await db
      .prepare('SELECT id FROM pay_ins WHERE idempotency_key = ?')
      .bind(inferenceChargeIdempotencyKey('req-1'))
      .first()
    expect(payIn).toBe(null)
  })

  test('an injected usdToMsat conversion is honored (oracle seam)', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    const hook = makeLedgerMeteringHook({
      db,
      nowIso: () => NOW,
      // Fixed, deterministic conversion independent of the default rate.
      usdToMsat: () => 12_345,
    })

    await run(hook(context()))

    const balance = await readAgentBalance(db, ACCOUNT)
    expect(balance?.balanceMsat).toBe(FUNDED - 12_345)
  })

  test('DEFAULT_BTC_USD is the documented fixed reference rate', () => {
    expect(DEFAULT_BTC_USD).toBe(100_000)
  })
})

// ==========================================================================
// Serving-node payout seam (#5484) — wired-but-dormant
// ==========================================================================

const servingReceipt: ServingReceipt = {
  parityMode: 'exact_greedy_parity',
  parityVerified: true,
  servedModel: 'kimi-k2p6',
  sharded: false,
  servingRunRef: 'serve.run.metering',
  stages: [{ layerEnd: 32, layerStart: 0, nodeRef: 'pylon.alpha', role: 'stage' }],
}

const armedHostedGate = projectMdkPayoutModeGate({
  hostedFundedKeyVerified: true,
  hostedProgrammaticPayoutsEnabled: true,
  requestedMode: 'hosted_mdk_direct_payout',
})

describe('serving-node payout seam in the metering hook (#5484)', () => {
  test('no serving receipt => the seam never fires (ordinary charge only)', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    let sinkCalls = 0
    const hook = makeLedgerMeteringHook({
      db,
      nowIso: () => NOW,
      recordServingPayout: () =>
        Effect.sync(() => {
          sinkCalls += 1
        }),
      servingPayoutGate: armedHostedGate,
    })
    const outcome = await run(hook(context()))
    expect(outcome.metered).toBe(true)
    // No serving receipt on the context => the sink is never invoked.
    expect(sinkCalls).toBe(0)
  })

  test('serving receipt present but default DISABLED gate => decision NOT armed, sink not called', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    let sinkCalls = 0
    // Default gate (no servingPayoutGate supplied) is DISABLED => no live payout.
    const hook = makeLedgerMeteringHook({
      db,
      nowIso: () => NOW,
      recordServingPayout: () =>
        Effect.sync(() => {
          sinkCalls += 1
        }),
    })
    const outcome = await run(
      hook(
        context({
          adapterId: 'openagents-network',
          fundingKind: 'bitcoin',
          servedModel: 'kimi-k2p6',
          servingReceipt,
        }),
      ),
    )
    expect(outcome.metered).toBe(true)
    // The decision is computed + logged, but unarmed => the sink never dispatches.
    expect(sinkCalls).toBe(0)
  })

  test('even with an armed owner gate + bitcoin funding, the hook never self-arms a live payout', async () => {
    const db = makeDb()
    await seedBalance(db, FUNDED)
    let dispatched: ServingNodePayoutDecision | undefined
    const hook = makeLedgerMeteringHook({
      db,
      nowIso: () => NOW,
      recordServingPayout: decision =>
        Effect.sync(() => {
          dispatched = decision
        }),
      servingPayoutGate: armedHostedGate,
    })
    const outcome = await run(
      hook(
        context({
          adapterId: 'openagents-network',
          fundingKind: 'bitcoin',
          servedModel: 'kimi-k2p6',
          servingReceipt,
        }),
      ),
    )
    expect(outcome.metered).toBe(true)
    // The RL-3 resale ref chain (provider-grant / settlement-receipt / ...) does
    // NOT exist inside the metering hook, so the api_inference_gateway_resale lane
    // cannot authorize from here: the decision is NOT armed and the sink never
    // dispatches. This is the honest gate — the first real dispatched payout is
    // owner-armed with the full ref chain, never auto-armed from metering.
    expect(dispatched).toBeUndefined()
  })
})
