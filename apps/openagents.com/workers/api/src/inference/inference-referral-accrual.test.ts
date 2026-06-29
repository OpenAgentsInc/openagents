import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type MeteringContext,
  type MeteringHook,
  type MeteringOutcome,
} from './metering-hook'
import { type InferenceUsage } from './provider-adapter'
import {
  accrueInferenceReferral,
  inferenceReferralIdempotencyKey,
  inferenceReferralMarginSplitRef,
  inferenceReferralPeriodKey,
  parseReferredParty,
  withReferralAccrual,
} from './inference-referral-accrual'
import type { ServingReceipt } from './openagents-network-adapter'
import { readInferenceReferralDashboard } from './inference-referral-dashboard'
import { dispatchInferenceReferralPayout } from './inference-referral-dispatch'
import {
  readCurrentReferralPayout,
  transitionReferralPayout,
} from '../site-referral-payout-ledger'
import { hostedMdkDirectPayoutDisabledGate } from '../mdk-payout-mode-gate'

// --- real node:sqlite D1 adapter (same pattern as metering-hook.test.ts) ----
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

// Minimal real schema: the referral attribution tables + sources (the join
// source for the referrer) + the payout ledger (with its UNIQUE idempotency key
// + state CHECK), copied from migrations 0067/0068/0069/0153 down to the columns
// these modules touch.
const SCHEMA = `
CREATE TABLE site_referral_sources (
  id TEXT PRIMARY KEY NOT NULL,
  referrer_user_id TEXT NOT NULL,
  policy_state TEXT NOT NULL DEFAULT 'active',
  archived_at TEXT
);
CREATE TABLE referral_attributions (
  id TEXT PRIMARY KEY NOT NULL
);
CREATE TABLE user_referral_attributions (
  user_id TEXT PRIMARY KEY NOT NULL,
  referral_attribution_id TEXT NOT NULL,
  referral_source_id TEXT NOT NULL,
  referral_invite_id TEXT,
  policy_state TEXT NOT NULL DEFAULT 'active',
  archived_at TEXT
);
CREATE TABLE agent_referral_attributions (
  agent_user_id TEXT PRIMARY KEY NOT NULL,
  owner_user_id TEXT,
  referral_attribution_id TEXT NOT NULL,
  referral_source_id TEXT NOT NULL,
  referral_invite_id TEXT,
  policy_state TEXT NOT NULL DEFAULT 'active',
  archived_at TEXT
);
CREATE TABLE site_referral_payout_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  payout_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  referral_attribution_id TEXT NOT NULL,
  referral_source_id TEXT NOT NULL,
  referral_invite_id TEXT,
  referrer_user_id TEXT NOT NULL,
  referred_user_id TEXT,
  qualifying_event_ref TEXT NOT NULL,
  qualifying_event_kind TEXT NOT NULL,
  qualifying_amount_sats INTEGER NOT NULL DEFAULT 0 CHECK (qualifying_amount_sats >= 0),
  amount_sats INTEGER NOT NULL,
  period_key TEXT NOT NULL,
  state TEXT NOT NULL CHECK (
    state IN ('eligible','approved','dispatched','settled','failed','refused','reversed')
  ),
  state_reason_ref TEXT,
  previous_entry_id TEXT,
  reversal_of_entry_id TEXT,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  archived_at TEXT
);
CREATE TABLE inference_referral_margin_splits (
  id TEXT PRIMARY KEY NOT NULL,
  request_id TEXT NOT NULL UNIQUE,
  account_ref TEXT NOT NULL,
  referred_user_id TEXT NOT NULL,
  referrer_user_id TEXT NOT NULL,
  referral_attribution_id TEXT NOT NULL,
  referral_source_id TEXT NOT NULL,
  referral_invite_id TEXT,
  payout_ref TEXT NOT NULL,
  qualifying_event_ref TEXT NOT NULL,
  charge_receipt_ref TEXT NOT NULL,
  funding_kind TEXT NOT NULL,
  adapter_id TEXT NOT NULL,
  requested_model TEXT NOT NULL,
  served_model TEXT NOT NULL,
  served_by_contributor INTEGER NOT NULL DEFAULT 0,
  serving_node_count INTEGER NOT NULL DEFAULT 0,
  charge_usd REAL NOT NULL,
  cost_usd REAL NOT NULL,
  margin_usd REAL NOT NULL,
  margin_sats INTEGER NOT NULL,
  openagents_usd REAL NOT NULL,
  openagents_sats INTEGER NOT NULL,
  serving_node_usd REAL NOT NULL,
  serving_node_sats INTEGER NOT NULL,
  referrer_usd REAL NOT NULL,
  referrer_sats INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);
`

const NOW = '2026-06-19T12:00:00.000Z'
const REFERRER = 'user-referrer'
const AGENT = 'agent-referred'
const SOURCE = 'src-1'
const ATTRIBUTION = 'attr-1'

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

// Seed an active source + agent attribution so the referred agent resolves to
// REFERRER.
const seedReferredAgent = async (db: D1Database): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO site_referral_sources (id, referrer_user_id, policy_state) VALUES (?, ?, 'active')`,
    )
    .bind(SOURCE, REFERRER)
    .run()
  await db
    .prepare(`INSERT INTO referral_attributions (id) VALUES (?)`)
    .bind(ATTRIBUTION)
    .run()
  await db
    .prepare(
      `INSERT INTO agent_referral_attributions
         (agent_user_id, owner_user_id, referral_attribution_id, referral_source_id, policy_state)
       VALUES (?, NULL, ?, ?, 'active')`,
    )
    .bind(AGENT, ATTRIBUTION, SOURCE)
    .run()
}

// A large sonnet usage so the referrer share clears 1 sat (sub-sat tiny
// requests are tested separately).
const bigUsage: InferenceUsage = {
  completionTokens: 200_000,
  promptTokens: 200_000,
  totalTokens: 400_000,
}

const context = (
  overrides: Partial<MeteringContext> = {},
): MeteringContext => ({
  accountRef: `agent:${AGENT}`,
  adapterId: 'vertex',
  fundingKind: 'card',
  requestId: 'req-1',
  requestedModel: 'sonnet',
  servedModel: 'sonnet',
  streamed: false,
  usage: bigUsage,
  ...overrides,
})

const servingReceipt: ServingReceipt = {
  paidTrafficVerification: {
    blockerRefs: [],
    canaryPassed: true,
    parityPassed: true,
    payoutEligible: true,
    replayPassed: true,
  },
  parityMode: 'exact_greedy_parity',
  parityVerified: true,
  servedModel: 'sonnet',
  servingRunRef: 'serving.run.req-1',
  sharded: false,
  stages: [{ layerEnd: 32, layerStart: 0, nodeRef: 'pylon:one', role: 'stage' }],
}

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

describe('parseReferredParty', () => {
  test('parses agent and user principals; rejects garbage', () => {
    expect(parseReferredParty('agent:abc123')).toEqual({
      kind: 'agent',
      userId: 'abc123',
    })
    expect(parseReferredParty('user-7')).toEqual({
      kind: 'user',
      userId: 'user-7',
    })
    expect(parseReferredParty('agent:')).toBeNull()
    expect(parseReferredParty(' ')).toBeNull()
  })
})

describe('inferenceReferralPeriodKey', () => {
  test('buckets by UTC calendar month', () => {
    expect(inferenceReferralPeriodKey('2026-06-19T12:00:00.000Z')).toBe(
      'inference-2026-06',
    )
  })
})

describe('accrueInferenceReferral (#5487 attribution + #5488 ongoing accrual)', () => {
  test('records one eligibility row for a referred AGENT paid request', async () => {
    const db = makeDb()
    await seedReferredAgent(db)

    const result = await accrueInferenceReferral(db, {
      context: context(),
      nowIso: () => NOW,
    })

    expect(result._tag).toBe('recorded')
    if (result._tag !== 'recorded') return
    expect(result.entry.referrerUserId).toBe(REFERRER)
    expect(result.entry.referredUserId).toBe(AGENT)
    expect(result.entry.qualifyingEventKind).toBe('inference_paid_request')
    expect(result.entry.amountSats).toBeGreaterThan(0)
    expect(result.marginSplitRef).toBe(inferenceReferralMarginSplitRef('req-1'))
  })

  test('records the per-request OpenAgents / serving-node / referrer margin split', async () => {
    const db = makeDb()
    await seedReferredAgent(db)

    const result = await accrueInferenceReferral(db, {
      context: context({ servingReceipt }),
      nowIso: () => NOW,
    })

    expect(result._tag).toBe('recorded')
    const row = await db
      .prepare(
        `SELECT request_id, account_ref, referred_user_id, referrer_user_id,
                served_by_contributor, serving_node_count, margin_sats,
                openagents_sats, serving_node_sats, referrer_sats
           FROM inference_referral_margin_splits
          WHERE request_id = ?`,
      )
      .bind('req-1')
      .first<{
        account_ref: string
        margin_sats: number
        openagents_sats: number
        referred_user_id: string
        referrer_sats: number
        referrer_user_id: string
        request_id: string
        served_by_contributor: number
        serving_node_count: number
        serving_node_sats: number
      }>()

    expect(row).toMatchObject({
      account_ref: `agent:${AGENT}`,
      referred_user_id: AGENT,
      referrer_user_id: REFERRER,
      request_id: 'req-1',
      served_by_contributor: 1,
      serving_node_count: 1,
    })
    expect(Number(row?.margin_sats)).toBeGreaterThan(0)
    expect(Number(row?.openagents_sats)).toBeGreaterThan(0)
    expect(Number(row?.serving_node_sats)).toBeGreaterThan(0)
    expect(Number(row?.referrer_sats)).toBeGreaterThan(0)
  })

  test('ACCRUES ON EVERY PAID REQUEST (ongoing, not one-time)', async () => {
    const db = makeDb()
    await seedReferredAgent(db)

    // Three distinct paid requests => three distinct eligibility rows.
    for (const requestId of ['r1', 'r2', 'r3']) {
      const result = await accrueInferenceReferral(db, {
        context: context({ requestId }),
        nowIso: () => NOW,
      })
      expect(result._tag).toBe('recorded')
    }

    const countRow = await db
      .prepare(
        `SELECT COUNT(*) AS n FROM site_referral_payout_ledger_entries
          WHERE referrer_user_id = ? AND qualifying_event_kind = 'inference_paid_request'`,
      )
      .bind(REFERRER)
      .first<{ n: number }>()
    expect(Number(countRow?.n)).toBe(3)
  })

  test('is IDEMPOTENT per request id (a replayed settle does not double-accrue)', async () => {
    const db = makeDb()
    await seedReferredAgent(db)

    const first = await accrueInferenceReferral(db, {
      context: context({ requestId: 'dup' }),
      nowIso: () => NOW,
    })
    expect(first._tag).toBe('recorded')

    // Same request id again: the ledger's UNIQUE idempotency key makes the
    // create a no-op (returns the existing row), never a second charge.
    const again = await accrueInferenceReferral(db, {
      context: context({ requestId: 'dup' }),
      nowIso: () => NOW,
    })
    expect(again._tag).toBe('recorded')

    const countRow = await db
      .prepare(
        `SELECT COUNT(*) AS n FROM site_referral_payout_ledger_entries
          WHERE idempotency_key = ?`,
      )
      .bind(inferenceReferralIdempotencyKey('dup'))
      .first<{ n: number }>()
    expect(Number(countRow?.n)).toBe(1)

    const splitCountRow = await db
      .prepare(
        `SELECT COUNT(*) AS n FROM inference_referral_margin_splits
          WHERE id = ?`,
      )
      .bind(inferenceReferralMarginSplitRef('dup'))
      .first<{ n: number }>()
    expect(Number(splitCountRow?.n)).toBe(1)
  })

  test('no_attribution when the account was not referred', async () => {
    const db = makeDb() // no seeding
    const result = await accrueInferenceReferral(db, {
      context: context(),
      nowIso: () => NOW,
    })
    expect(result._tag).toBe('no_attribution')
  })

  test('self_attribution short-circuits (referrer == referred)', async () => {
    const db = makeDb()
    // Source owned by the agent itself.
    await db
      .prepare(
        `INSERT INTO site_referral_sources (id, referrer_user_id, policy_state) VALUES (?, ?, 'active')`,
      )
      .bind(SOURCE, AGENT)
      .run()
    await db
      .prepare(`INSERT INTO referral_attributions (id) VALUES (?)`)
      .bind(ATTRIBUTION)
      .run()
    await db
      .prepare(
        `INSERT INTO agent_referral_attributions
           (agent_user_id, referral_attribution_id, referral_source_id, policy_state)
         VALUES (?, ?, ?, 'active')`,
      )
      .bind(AGENT, ATTRIBUTION, SOURCE)
      .run()

    const result = await accrueInferenceReferral(db, {
      context: context(),
      nowIso: () => NOW,
    })
    expect(result._tag).toBe('self_attribution')
  })

  test('zero_referrer_share for a tiny sub-1-sat request', async () => {
    const db = makeDb()
    await seedReferredAgent(db)
    const result = await accrueInferenceReferral(db, {
      context: context({
        usage: { completionTokens: 1, promptTokens: 1, totalTokens: 2 },
      }),
      nowIso: () => NOW,
    })
    expect(result._tag).toBe('zero_referrer_share')
  })
})

describe('withReferralAccrual decorator', () => {
  const meteredHook: MeteringHook = () =>
    Effect.succeed({ metered: true, receiptRef: 'r' } satisfies MeteringOutcome)
  const stubHook: MeteringHook = () =>
    Effect.succeed({ metered: false, receiptRef: null } satisfies MeteringOutcome)
  const zeroHook: MeteringHook = () =>
    Effect.succeed({
      metered: true,
      receiptRef: 'r',
      zeroCharge: true,
    } satisfies MeteringOutcome)

  test('accrues only after a real, non-zero metered charge', async () => {
    const db = makeDb()
    await seedReferredAgent(db)

    const wrapped = withReferralAccrual(meteredHook, { db, nowIso: () => NOW })
    const outcome = await run(wrapped(context()))
    expect(outcome.metered).toBe(true)

    const countRow = await db
      .prepare(
        `SELECT COUNT(*) AS n FROM site_referral_payout_ledger_entries WHERE referrer_user_id = ?`,
      )
      .bind(REFERRER)
      .first<{ n: number }>()
    expect(Number(countRow?.n)).toBe(1)
  })

  test('is INERT on the stub hook (metered:false) and on zero-charge', async () => {
    const db = makeDb()
    await seedReferredAgent(db)

    await run(withReferralAccrual(stubHook, { db, nowIso: () => NOW })(context()))
    await run(
      withReferralAccrual(zeroHook, { db, nowIso: () => NOW })(
        context({ requestId: 'zero' }),
      ),
    )

    const countRow = await db
      .prepare(`SELECT COUNT(*) AS n FROM site_referral_payout_ledger_entries`)
      .first<{ n: number }>()
    expect(Number(countRow?.n)).toBe(0)
  })

  test('never fails the inference call when accrual errors', async () => {
    // A db whose insert path throws: the decorator swallows the error and still
    // returns the metering outcome unchanged.
    const brokenDb = {
      prepare: () => {
        throw new Error('boom')
      },
    } as unknown as D1Database
    const wrapped = withReferralAccrual(meteredHook, {
      db: brokenDb,
      nowIso: () => NOW,
    })
    const outcome = await run(wrapped(context()))
    expect(outcome.metered).toBe(true)
  })
})

describe('readInferenceReferralDashboard (#5491)', () => {
  test('rolls up referred accounts, ongoing earnings, and settled receipts', async () => {
    const db = makeDb()
    await seedReferredAgent(db)

    // Two paid requests accrue.
    await accrueInferenceReferral(db, {
      context: context({ requestId: 'd1' }),
      nowIso: () => NOW,
    })
    await accrueInferenceReferral(db, {
      context: context({ requestId: 'd2' }),
      nowIso: () => NOW,
    })

    const before = await readInferenceReferralDashboard(db, REFERRER)
    expect(before.referredAccountCount).toBe(1)
    expect(before.perReferredAccount[0]?.paidRequestCount).toBe(2)
    expect(before.totalAccruedSats).toBeGreaterThan(0)
    expect(before.totalSettledSats).toBe(0)
    expect(before.totalPendingSats).toBe(before.totalAccruedSats)
    expect(before.settledReceipts).toHaveLength(0)

    // Drive one payout to settled directly through the ledger transitions, then
    // re-read: it shows as a settled receipt with its evidence ref.
    const payoutRef = `inference.referral.payout.d1`
    await transitionReferralPayout(db, {
      action: 'approve_dispatch',
      idempotencyKey: 'k-approve',
      nowIso: '2026-06-19T12:00:01.000Z',
      payoutRef,
    })
    await transitionReferralPayout(db, {
      action: 'mark_dispatched',
      idempotencyKey: 'k-dispatch',
      nowIso: '2026-06-19T12:00:02.000Z',
      payoutRef,
    })
    await transitionReferralPayout(db, {
      action: 'mark_settled',
      evidenceRefs: ['receipt.test.settled.d1'],
      idempotencyKey: 'k-settled',
      nowIso: '2026-06-19T12:00:03.000Z',
      payoutRef,
    })

    const after = await readInferenceReferralDashboard(db, REFERRER)
    expect(after.totalSettledSats).toBeGreaterThan(0)
    expect(after.settledReceipts).toHaveLength(1)
    expect(after.settledReceipts[0]?.payoutRef).toBe(payoutRef)
    expect(after.settledReceipts[0]?.evidenceRefs).toContain(
      'receipt.test.settled.d1',
    )
    expect(after.totalPendingSats).toBe(
      after.totalAccruedSats - after.totalSettledSats,
    )
  })

  test('empty for an unknown referrer', async () => {
    const db = makeDb()
    const dashboard = await readInferenceReferralDashboard(db, 'nobody')
    expect(dashboard.referredAccountCount).toBe(0)
    expect(dashboard.totalAccruedSats).toBe(0)
  })
})

describe('dispatchInferenceReferralPayout (#5490 owner-armed)', () => {
  test('REFUSES (no money, adapter never called) under the owner-armed OFF gate', async () => {
    const db = makeDb()
    await seedReferredAgent(db)
    const accrued = await accrueInferenceReferral(db, {
      context: context({ requestId: 'p1' }),
      nowIso: () => NOW,
    })
    expect(accrued._tag).toBe('recorded')

    let adapterCalls = 0
    const outcome = await dispatchInferenceReferralPayout(
      db,
      {
        adapter: {
          adapterKind: 'test',
          dispatch: async () => {
            adapterCalls += 1
            return { receiptRef: 'receipt.test' }
          },
        },
        nowIso: () => NOW,
        // Owner-armed OFF gate: livePayoutClaimAllowed === false.
        readReadiness: async () => hostedMdkDirectPayoutDisabledGate(),
      },
      { payoutRef: 'inference.referral.payout.p1', revenueAsset: 'bitcoin' },
    )

    expect(outcome._tag).toBe('refused')
    expect(adapterCalls).toBe(0)
    // The ledger row never advanced past eligible.
    const current = await readCurrentReferralPayout(
      db,
      'inference.referral.payout.p1',
    )
    expect(current?.state).toBe('eligible')
  })

  test('refuses credit/USD revenue for a Bitcoin dispatch (RL-3 boundary)', async () => {
    const db = makeDb()
    await seedReferredAgent(db)
    await accrueInferenceReferral(db, {
      context: context({ requestId: 'p2' }),
      nowIso: () => NOW,
    })

    const outcome = await dispatchInferenceReferralPayout(
      db,
      {
        adapter: {
          adapterKind: 'test',
          dispatch: async () => ({ receiptRef: 'receipt.test' }),
        },
        nowIso: () => NOW,
        // Even if readiness allowed live, a usd-revenue Bitcoin dispatch is
        // refused by the asset boundary. Use the OFF gate is fine; boundary is
        // checked before readiness, so pass a "ready" stub to prove the boundary
        // path specifically.
        readReadiness: async () => ({
          activeMode: 'hosted_mdk_direct_payout',
          blockerRefs: [],
          caveatRefs: [],
          evidenceRefs: [],
          hostedDirectPayoutClaimAllowed: true,
          livePayoutClaimAllowed: true,
          localBridgePayoutClaimAllowed: false,
          modeLabel: 'test',
          state: 'ready',
        }),
      },
      { payoutRef: 'inference.referral.payout.p2', revenueAsset: 'usd' },
    )

    expect(outcome._tag).toBe('refused')
  })
})
