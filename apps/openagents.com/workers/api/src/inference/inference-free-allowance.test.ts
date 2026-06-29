import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { type VerifiedPublicIdentityClaim } from '../agent-owner-claim-routes'
import {
  type MeteringContext,
  type MeteringHook,
  type MeteringOutcome,
} from './metering-hook'
import { type InferenceUsage } from './provider-adapter'
import {
  type VerifiedOwnerIdentityResolver,
} from './inference-owner-identity'
import {
  EARNED_ALLOWANCE_CEILING_USD_MICROS,
  EARNED_ALLOWANCE_PER_REFERRED_SIGNUP_USD_MICROS,
  UNCLAIMED_TASTE_FREE_CAP_USD_MICROS,
  USD_MICROS_PER_USD,
  VERIFIED_OWNER_FREE_CAP_USD_MICROS,
  accrueEarnedAllowance,
  baseFreeCapUsdMicros,
  checkFreeAllowancePreflight,
  decideFreeAllowance,
  isFreeEligibleModel,
  usdToMicrosCeil,
  withFreeAllowance,
} from './inference-free-allowance'
import { KHALA_MINI_MODEL_ID } from './pricing'

// --- node:sqlite D1 adapter (same pattern as metering-hook.test.ts) ----------
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
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[],
    }
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

// Free-tier tables, copied verbatim from migration 0210 (the constraints under
// test are the request_id / accrual_event_ref UNIQUE PKs).
const SCHEMA = `
CREATE TABLE inference_free_usage_tally (
  owner_key TEXT PRIMARY KEY,
  identity_kind TEXT NOT NULL,
  cumulative_free_usd_micros INTEGER NOT NULL DEFAULT 0 CHECK (cumulative_free_usd_micros >= 0),
  free_request_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_free_usage_events (
  request_id TEXT PRIMARY KEY,
  owner_key TEXT NOT NULL,
  account_ref TEXT NOT NULL,
  served_model TEXT NOT NULL,
  free_usd_micros INTEGER NOT NULL CHECK (free_usd_micros >= 0),
  created_at TEXT NOT NULL
);
CREATE TABLE inference_premium_allowlist (
  owner_key TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'all_premium',
  granted_by TEXT,
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_earned_allowance (
  owner_key TEXT PRIMARY KEY,
  earned_free_usd_micros INTEGER NOT NULL DEFAULT 0 CHECK (earned_free_usd_micros >= 0),
  accrual_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_earned_allowance_events (
  accrual_event_ref TEXT PRIMARY KEY,
  owner_key TEXT NOT NULL,
  accrual_kind TEXT NOT NULL,
  earned_usd_micros INTEGER NOT NULL CHECK (earned_usd_micros >= 0),
  created_at TEXT NOT NULL
);
`

const NOW = '2026-06-19T12:00:00.000Z'

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

// A small Gemini usage (the priced charge is well under $0.50 so it is free for
// any identity kind in the default-state tests).
const smallUsage: InferenceUsage = {
  completionTokens: 100,
  promptTokens: 100,
  totalTokens: 200,
}

const meteringContext = (
  over: Partial<MeteringContext> = {},
): MeteringContext => ({
  accountRef: 'agent:user-a',
  adapterId: 'vertex-gemini',
  fundingKind: 'card',
  requestId: 'req-1',
  requestedModel: 'gemini-3.5-flash',
  servedModel: 'gemini-3.5-flash',
  streamed: false,
  usage: smallUsage,
  ...over,
})

// A spy inner hook recording how many times it was invoked (the "would charge"
// path). Returns a real metered outcome.
const makeSpyInner = (): {
  hook: MeteringHook
  calls: () => number
} => {
  let calls = 0
  const hook: MeteringHook = () =>
    Effect.sync(() => {
      calls += 1
      return {
        metered: true,
        receiptRef: 'receipt.inference.charge.req',
      } satisfies MeteringOutcome
    })
  return { calls: () => calls, hook }
}

// Verified-owner resolver: maps every agent to ONE verified owner (so two
// accounts share a pool). Unclaimed resolver always returns undefined.
const verifiedOwner = (ownerUserId: string): VerifiedOwnerIdentityResolver =>
  async () =>
    ({
      agentClaimRef: 'claim-1',
      claimRef: 'x-1',
      ownerUserId,
      provider: 'x',
      receiptRef: 'receipt.claim.1',
      state: 'approved',
      tweetRef: 'tweet-1',
      xAccountRef: 'x-acct-1',
    }) satisfies VerifiedPublicIdentityClaim
const unclaimed: VerifiedOwnerIdentityResolver = async () => undefined

const readTally = async (
  db: D1Database,
  ownerKey: string,
): Promise<{ cumulative: number; count: number } | null> => {
  const row = await db
    .prepare(
      `SELECT cumulative_free_usd_micros AS c, free_request_count AS n
         FROM inference_free_usage_tally WHERE owner_key = ?`,
    )
    .bind(ownerKey)
    .first<{ c: number; n: number }>()
  return row === null ? null : { count: row.n, cumulative: row.c }
}

describe('free-allowance constants + classification', () => {
  test('caps: verified owner $10, unclaimed $0.50', () => {
    expect(VERIFIED_OWNER_FREE_CAP_USD_MICROS).toBe(10 * USD_MICROS_PER_USD)
    expect(UNCLAIMED_TASTE_FREE_CAP_USD_MICROS).toBe(USD_MICROS_PER_USD / 2)
    expect(baseFreeCapUsdMicros('verified')).toBe(10 * USD_MICROS_PER_USD)
    expect(baseFreeCapUsdMicros('unclaimed')).toBe(USD_MICROS_PER_USD / 2)
  })

  test('only the Gemini Flash aliases are free-eligible', () => {
    expect(isFreeEligibleModel('gemini-3.5-flash')).toBe(true)
    expect(isFreeEligibleModel('gemini')).toBe(true)
    expect(isFreeEligibleModel(KHALA_MINI_MODEL_ID)).toBe(false)
    expect(isFreeEligibleModel('claude-sonnet')).toBe(false)
    expect(isFreeEligibleModel('opus')).toBe(false)
    expect(isFreeEligibleModel('gpt-oss-20b')).toBe(false)
    expect(isFreeEligibleModel('gpt-4o')).toBe(false)
  })

  test('usdToMicrosCeil rounds a nonzero charge up to >= 1 micro', () => {
    expect(usdToMicrosCeil(0)).toBe(0)
    expect(usdToMicrosCeil(-1)).toBe(0)
    expect(usdToMicrosCeil(1)).toBe(1_000_000)
    expect(usdToMicrosCeil(0.0000001)).toBe(1) // sub-micro nonzero -> 1
  })
})

describe('decideFreeAllowance (pure)', () => {
  test('under allowance => free; effective cap = base + earned', () => {
    const d = decideFreeAllowance({
      chargeUsdMicros: 1_000,
      state: {
        cumulativeFreeUsdMicros: 0,
        earnedFreeUsdMicros: 2 * USD_MICROS_PER_USD,
        identityKind: 'verified',
      },
    })
    expect(d.free).toBe(true)
    expect(d.effectiveCapUsdMicros).toBe(12 * USD_MICROS_PER_USD)
    expect(d.remainingUsdMicros).toBe(12 * USD_MICROS_PER_USD)
  })

  test('over allowance => not free (charge exceeds remaining)', () => {
    const d = decideFreeAllowance({
      chargeUsdMicros: 2_000_000, // $2
      state: {
        cumulativeFreeUsdMicros: 9_500_000, // $9.50 used of $10
        earnedFreeUsdMicros: 0,
        identityKind: 'verified',
      },
    })
    expect(d.free).toBe(false)
    expect(d.remainingUsdMicros).toBe(500_000) // $0.50 left, charge is $2
  })

  test('unclaimed taste exhausts after ~$0.50', () => {
    const within = decideFreeAllowance({
      chargeUsdMicros: 100_000, // $0.10
      state: {
        cumulativeFreeUsdMicros: 0,
        earnedFreeUsdMicros: 0,
        identityKind: 'unclaimed',
      },
    })
    expect(within.free).toBe(true)
    const exhausted = decideFreeAllowance({
      chargeUsdMicros: 100_000,
      state: {
        cumulativeFreeUsdMicros: UNCLAIMED_TASTE_FREE_CAP_USD_MICROS,
        earnedFreeUsdMicros: 0,
        identityKind: 'unclaimed',
      },
    })
    expect(exhausted.free).toBe(false)
  })
})

describe('withFreeAllowance decorator (real D1)', () => {
  test('non-free model always falls through to the inner hook', async () => {
    const db = makeDb()
    const spy = makeSpyInner()
    const hook = withFreeAllowance(spy.hook, {
      db,
      nowIso: () => NOW,
      resolveOwnerIdentity: verifiedOwner('owner-1'),
    })
    const outcome = await run(
      hook(meteringContext({ servedModel: 'claude-sonnet' })),
    )
    expect(spy.calls()).toBe(1)
    expect(outcome.metered).toBe(true)
    // No free accrual for a non-free model.
    expect(await readTally(db, 'owner:owner-1')).toBeNull()
  })

  test('under allowance => free: no inner call, accrues to the owner pool', async () => {
    const db = makeDb()
    const spy = makeSpyInner()
    const hook = withFreeAllowance(spy.hook, {
      db,
      nowIso: () => NOW,
      resolveOwnerIdentity: verifiedOwner('owner-1'),
    })
    const outcome = await run(hook(meteringContext()))
    expect(spy.calls()).toBe(0) // inner NOT called => no decrement, no referral
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toMatch(/^receipt\.inference\.free\./)
    const tally = await readTally(db, 'owner:owner-1')
    expect(tally?.count).toBe(1)
    expect(tally?.cumulative).toBeGreaterThan(0)
  })

  test('SYBIL: two accounts under one owner SHARE the $10 pool', async () => {
    const db = makeDb()
    const spy = makeSpyInner()
    const hook = withFreeAllowance(spy.hook, {
      db,
      nowIso: () => NOW,
      resolveOwnerIdentity: verifiedOwner('owner-shared'),
    })
    // Account A makes a free request.
    await run(hook(meteringContext({ accountRef: 'agent:a', requestId: 'r-a' })))
    // Account B (DIFFERENT account, SAME owner) makes a free request.
    await run(hook(meteringContext({ accountRef: 'agent:b', requestId: 'r-b' })))
    // Both accrued into the SAME owner pool (count 2), not two separate pools.
    const tally = await readTally(db, 'owner:owner-shared')
    expect(tally?.count).toBe(2)
    // No per-account pool row exists.
    expect(await readTally(db, 'account:agent:a')).toBeNull()
    expect(await readTally(db, 'account:agent:b')).toBeNull()
  })

  test('SYBIL: unclaimed accounts each key to their own taste pool', async () => {
    const db = makeDb()
    const spy = makeSpyInner()
    const hook = withFreeAllowance(spy.hook, {
      db,
      nowIso: () => NOW,
      resolveOwnerIdentity: unclaimed,
    })
    await run(hook(meteringContext({ accountRef: 'agent:x', requestId: 'r-x' })))
    const tally = await readTally(db, 'account:agent:x')
    expect(tally?.count).toBe(1)
    // The taste pool is keyed to the account, identity_kind = unclaimed.
    const row = await db
      .prepare(
        `SELECT identity_kind AS k FROM inference_free_usage_tally WHERE owner_key = ?`,
      )
      .bind('account:agent:x')
      .first<{ k: string }>()
    expect(row?.k).toBe('unclaimed')
  })

  test('over allowance => falls through to the inner (metering) hook', async () => {
    const db = makeDb()
    const spy = makeSpyInner()
    // Pre-seed the owner pool at the full $10 cap so any new charge is over.
    await db
      .prepare(
        `INSERT INTO inference_free_usage_tally
           (owner_key, identity_kind, cumulative_free_usd_micros, free_request_count, created_at, updated_at)
         VALUES (?, 'verified', ?, 1, ?, ?)`,
      )
      .bind('owner:full', VERIFIED_OWNER_FREE_CAP_USD_MICROS, NOW, NOW)
      .run()
    const hook = withFreeAllowance(spy.hook, {
      db,
      nowIso: () => NOW,
      resolveOwnerIdentity: verifiedOwner('full'),
    })
    const outcome = await run(hook(meteringContext({ requestId: 'r-over' })))
    expect(spy.calls()).toBe(1) // metered normally
    expect(outcome.metered).toBe(true)
    // No NEW free event row for the over-allowance request.
    const ev = await db
      .prepare(
        `SELECT request_id FROM inference_free_usage_events WHERE request_id = ?`,
      )
      .bind('r-over')
      .first()
    expect(ev).toBeNull()
  })

  test('idempotent: a replayed request id never double-counts the pool', async () => {
    const db = makeDb()
    const spy = makeSpyInner()
    const hook = withFreeAllowance(spy.hook, {
      db,
      nowIso: () => NOW,
      resolveOwnerIdentity: verifiedOwner('owner-idem'),
    })
    const ctx = meteringContext({ requestId: 'r-dup' })
    await run(hook(ctx))
    await run(hook(ctx)) // same request id again
    const tally = await readTally(db, 'owner:owner-idem')
    expect(tally?.count).toBe(1) // accrued ONCE
    expect(spy.calls()).toBe(0) // still free, inner never called
  })
})

describe('accrueEarnedAllowance (real D1)', () => {
  test('a referred signup grants the per-signup amount, idempotent per source', async () => {
    const db = makeDb()
    const first = await run(
      accrueEarnedAllowance(db, {
        kind: 'referred_signup',
        nowIso: () => NOW,
        ownerKey: 'owner:earn',
        sourceRef: 'referred-user-1',
      }),
    )
    expect(first).toBe(true)
    // Replay the SAME contribution ref => no double grant.
    const replay = await run(
      accrueEarnedAllowance(db, {
        kind: 'referred_signup',
        nowIso: () => NOW,
        ownerKey: 'owner:earn',
        sourceRef: 'referred-user-1',
      }),
    )
    expect(replay).toBe(false)
    const row = await db
      .prepare(
        `SELECT earned_free_usd_micros AS e, accrual_count AS n
           FROM inference_earned_allowance WHERE owner_key = ?`,
      )
      .bind('owner:earn')
      .first<{ e: number; n: number }>()
    expect(row?.e).toBe(EARNED_ALLOWANCE_PER_REFERRED_SIGNUP_USD_MICROS)
    expect(row?.n).toBe(1)
  })

  test('earned allowance is capped at the ceiling', async () => {
    const db = makeDb()
    // Pre-seed near the ceiling.
    await db
      .prepare(
        `INSERT INTO inference_earned_allowance
           (owner_key, earned_free_usd_micros, accrual_count, created_at, updated_at)
         VALUES (?, ?, 1, ?, ?)`,
      )
      .bind(
        'owner:cap',
        EARNED_ALLOWANCE_CEILING_USD_MICROS -
          EARNED_ALLOWANCE_PER_REFERRED_SIGNUP_USD_MICROS / 2,
        NOW,
        NOW,
      )
      .run()
    await run(
      accrueEarnedAllowance(db, {
        kind: 'referred_signup',
        nowIso: () => NOW,
        ownerKey: 'owner:cap',
        sourceRef: 'referred-user-2',
      }),
    )
    const row = await db
      .prepare(
        `SELECT earned_free_usd_micros AS e FROM inference_earned_allowance WHERE owner_key = ?`,
      )
      .bind('owner:cap')
      .first<{ e: number }>()
    // Capped at the ceiling, never exceeding it.
    expect(row?.e).toBe(EARNED_ALLOWANCE_CEILING_USD_MICROS)
  })

  test('earned allowance extends the effective free cap', async () => {
    const d = decideFreeAllowance({
      chargeUsdMicros: 1_000,
      state: {
        cumulativeFreeUsdMicros: VERIFIED_OWNER_FREE_CAP_USD_MICROS, // base used up
        earnedFreeUsdMicros: EARNED_ALLOWANCE_PER_REFERRED_SIGNUP_USD_MICROS,
        identityKind: 'verified',
      },
    })
    // Base is exhausted but the earned $1 still covers a small request.
    expect(d.free).toBe(true)
    expect(d.remainingUsdMicros).toBe(
      EARNED_ALLOWANCE_PER_REFERRED_SIGNUP_USD_MICROS,
    )
  })
})

describe('checkFreeAllowancePreflight (balance-gate bypass)', () => {
  const seedExhaustedTally = async (
    db: D1Database,
    ownerKey: string,
    identityKind: 'verified' | 'unclaimed',
    cumulative: number,
  ): Promise<void> => {
    await db
      .prepare(
        `INSERT INTO inference_free_usage_tally
           (owner_key, identity_kind, cumulative_free_usd_micros, free_request_count, created_at, updated_at)
         VALUES (?, ?, ?, 1, ?, ?)`,
      )
      .bind(ownerKey, identityKind, cumulative, NOW, NOW)
      .run()
  }

  test('eligible for a free-eligible model when the owner has fresh allowance', async () => {
    const db = makeDb()
    const decision = await checkFreeAllowancePreflight({
      db,
      resolveOwnerIdentity: verifiedOwner('owner-1'),
    })('agent:user-a', 'gemini-3.5-flash')
    expect(decision.eligible).toBe(true)
    expect(decision.remainingUsdMicros).toBe(VERIFIED_OWNER_FREE_CAP_USD_MICROS)
    expect(decision.identityKind).toBe('verified')
  })

  test('an unclaimed account still gets the tiny taste pool', async () => {
    const db = makeDb()
    const decision = await checkFreeAllowancePreflight({
      db,
      resolveOwnerIdentity: unclaimed,
    })('agent:user-a', 'gemini-3.5-flash')
    expect(decision.eligible).toBe(true)
    expect(decision.remainingUsdMicros).toBe(UNCLAIMED_TASTE_FREE_CAP_USD_MICROS)
    expect(decision.identityKind).toBe('unclaimed')
  })

  test('NOT eligible for a non-free model (premium/open lanes still gate on balance)', async () => {
    const db = makeDb()
    for (const model of ['claude-sonnet', 'gpt-4o', 'gpt-oss-20b']) {
      const decision = await checkFreeAllowancePreflight({
        db,
        resolveOwnerIdentity: verifiedOwner('owner-1'),
      })('agent:user-a', model)
      expect(decision.eligible).toBe(false)
    }
  })

  test('NOT eligible once the owner pool is exhausted', async () => {
    const db = makeDb()
    // Verified owner key is `owner:<ownerUserId>`; exhaust the whole $10 base.
    await seedExhaustedTally(
      db,
      'owner:owner-1',
      'verified',
      VERIFIED_OWNER_FREE_CAP_USD_MICROS,
    )
    const decision = await checkFreeAllowancePreflight({
      db,
      resolveOwnerIdentity: verifiedOwner('owner-1'),
    })('agent:user-a', 'gemini-3.5-flash')
    expect(decision.eligible).toBe(false)
    expect(decision.remainingUsdMicros).toBe(0)
  })

  test('resolution error => not eligible (the balance gate stands)', async () => {
    const db = makeDb()
    const throwingResolver: VerifiedOwnerIdentityResolver = async () => {
      throw new Error('owner resolver unavailable')
    }
    const decision = await checkFreeAllowancePreflight({
      db,
      resolveOwnerIdentity: throwingResolver,
    })('agent:user-a', 'gemini-3.5-flash')
    expect(decision.eligible).toBe(false)
  })
})
