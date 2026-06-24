import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type ChatCompletionsDeps,
  type InferenceAuth,
  type InferenceBalanceReader,
  handleChatCompletions,
} from './chat-completions-routes'
import {
  DEFAULT_FREE_TIER_QUOTA,
  FREE_KEY_MINT_REASON_RATE_LIMITED,
  FREE_TIER_QUOTA_REASON_REQUESTS_EXCEEDED,
  FREE_TIER_QUOTA_REASON_TOKENS_EXCEEDED,
  FREE_TIER_QUOTA_REASON_WITHIN,
  FREE_TIER_REASON_ELIGIBLE,
  FREE_TIER_REASON_NOT_FREE_LANE,
  FREE_TIER_REASON_PREMIUM_DENIED,
  decideFreeKeyMint,
  decideFreeTierLane,
  decideFreeTierQuota,
  freeTierReceiptRef,
  freeTierUsageDay,
  isFreeTierEnabled,
  isFreeTierLaneModel,
  makeFreeTierGate,
  markAccountFreeTier,
  readAccountFreeTier,
  readFreeKeyMintsToday,
  readFreeTierUsage,
  recordFreeKeyMint,
  sanitizeFreeKeyLabel,
  withFreeTierKhala,
} from './inference-free-tier-key'
import { type MeteringContext, type MeteringHook } from './metering-hook'
import { KHALA_MODEL_ID } from './pricing'
import {
  InferenceProviderRegistry,
} from './provider-adapter'
import { stubEchoAdapter } from './stub-echo-adapter'

// --- node:sqlite D1 adapter (mirrors inference-operator-exemption.test.ts) ----
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
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
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
    // node:sqlite has no nested transaction here; run each statement and let a
    // UNIQUE-constraint failure throw so the idempotency guard behaves like D1.
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

// The schema mirrors migration 0231_inference_free_tier_keys.sql.
const SCHEMA = `
CREATE TABLE inference_free_tier_keys (
  account_ref TEXT PRIMARY KEY,
  scope TEXT NOT NULL DEFAULT 'free_khala_daily',
  mint_source TEXT NOT NULL DEFAULT 'self_serve_anonymous',
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE inference_free_tier_usage (
  account_ref TEXT NOT NULL,
  usage_day TEXT NOT NULL,
  free_request_count INTEGER NOT NULL DEFAULT 0,
  free_total_tokens INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (account_ref, usage_day)
);
CREATE TABLE inference_free_tier_usage_events (
  request_id TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  usage_day TEXT NOT NULL,
  served_model TEXT NOT NULL,
  total_tokens INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);
CREATE TABLE inference_free_key_mints (
  ip_hash TEXT NOT NULL,
  mint_day TEXT NOT NULL,
  mint_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (ip_hash, mint_day)
);
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const FIXED_NOW = '2026-06-24T12:00:00.000Z'
const fixedNow = () => FIXED_NOW

const meteringContext = (
  overrides: Partial<MeteringContext> = {},
): MeteringContext => ({
  accountRef: 'agent:free-user',
  adapterId: 'hydralisk-vllm',
  fundingKind: 'card',
  requestId: 'req-1',
  requestedModel: KHALA_MODEL_ID,
  servedModel: KHALA_MODEL_ID,
  streamed: false,
  usage: { completionTokens: 5, promptTokens: 10, totalTokens: 15 },
  ...overrides,
})

const makeInnerSpy = (): { hook: MeteringHook; calls: () => number } => {
  let calls = 0
  return {
    calls: () => calls,
    hook: () =>
      Effect.sync(() => {
        calls += 1
        return { metered: true, receiptRef: 'receipt.inference.charge.inner' }
      }),
  }
}

// ----------------------------------------------------------------------------
// Flag (fail-closed)
// ----------------------------------------------------------------------------

describe('isFreeTierEnabled (fail-closed flag)', () => {
  test('default OFF; only explicit on-tokens arm it', () => {
    expect(isFreeTierEnabled(undefined)).toBe(false)
    expect(isFreeTierEnabled('')).toBe(false)
    expect(isFreeTierEnabled('false')).toBe(false)
    expect(isFreeTierEnabled('0')).toBe(false)
    expect(isFreeTierEnabled('off')).toBe(false)
    expect(isFreeTierEnabled('true')).toBe(true)
    expect(isFreeTierEnabled('1')).toBe(true)
    expect(isFreeTierEnabled('on')).toBe(true)
    expect(isFreeTierEnabled('YES')).toBe(true)
  })
})

// ----------------------------------------------------------------------------
// Pure lane eligibility (Khala-only, never premium)
// ----------------------------------------------------------------------------

describe('decideFreeTierLane (pure)', () => {
  test('the single public Khala model is the free lane', () => {
    const d = decideFreeTierLane('openagents/khala')
    expect(d.freeLane).toBe(true)
    expect(d.premium).toBe(false)
    expect(d.reasonRef).toBe(FREE_TIER_REASON_ELIGIBLE)
    expect(isFreeTierLaneModel('openagents/khala')).toBe(true)
    // The bare "khala" slug normalizes to the public id.
    expect(isFreeTierLaneModel('khala')).toBe(true)
  })

  test('GUARDRAIL: a PREMIUM model is NEVER the free lane', () => {
    for (const premium of ['claude-sonnet', 'opus', 'gpt-4o']) {
      const d = decideFreeTierLane(premium)
      expect(d.freeLane).toBe(false)
      expect(d.premium).toBe(true)
      expect(d.reasonRef).toBe(FREE_TIER_REASON_PREMIUM_DENIED)
      expect(isFreeTierLaneModel(premium)).toBe(false)
    }
  })

  test('a non-Khala, non-premium model is not the free lane', () => {
    const d = decideFreeTierLane('gemini-3.5-flash')
    expect(d.freeLane).toBe(false)
    expect(d.premium).toBe(false)
    expect(d.reasonRef).toBe(FREE_TIER_REASON_NOT_FREE_LANE)
  })
})

// ----------------------------------------------------------------------------
// Pure quota decision
// ----------------------------------------------------------------------------

describe('decideFreeTierQuota (pure)', () => {
  test('within both ceilings => allowed', () => {
    const d = decideFreeTierQuota({
      usage: { requestsToday: 3, tokensToday: 100 },
    })
    expect(d.withinQuota).toBe(true)
    expect(d.reasonRef).toBe(FREE_TIER_QUOTA_REASON_WITHIN)
    expect(d.remainingRequests).toBe(DEFAULT_FREE_TIER_QUOTA.maxRequestsPerDay - 3)
  })

  test('at/over the daily request ceiling => over quota', () => {
    const d = decideFreeTierQuota({
      quota: { maxRequestsPerDay: 5, maxTokensPerDay: 1_000 },
      usage: { requestsToday: 5, tokensToday: 0 },
    })
    expect(d.withinQuota).toBe(false)
    expect(d.reasonRef).toBe(FREE_TIER_QUOTA_REASON_REQUESTS_EXCEEDED)
    expect(d.remainingRequests).toBe(0)
  })

  test('at/over the daily token ceiling => over quota', () => {
    const d = decideFreeTierQuota({
      quota: { maxRequestsPerDay: 100, maxTokensPerDay: 1_000 },
      usage: { requestsToday: 1, tokensToday: 1_000 },
    })
    expect(d.withinQuota).toBe(false)
    expect(d.reasonRef).toBe(FREE_TIER_QUOTA_REASON_TOKENS_EXCEEDED)
    expect(d.remainingTokens).toBe(0)
  })
})

describe('freeTierUsageDay', () => {
  test('buckets by UTC day', () => {
    expect(freeTierUsageDay('2026-06-24T23:59:59.999Z')).toBe('2026-06-24')
  })
})

// ----------------------------------------------------------------------------
// Pure mint rate decision
// ----------------------------------------------------------------------------

describe('decideFreeKeyMint (pure)', () => {
  test('under the daily IP ceiling => allowed', () => {
    const d = decideFreeKeyMint({ maxMintsPerDay: 5, mintsToday: 4 })
    expect(d.allowed).toBe(true)
  })

  test('at/over the daily IP ceiling => rate limited', () => {
    const d = decideFreeKeyMint({ maxMintsPerDay: 5, mintsToday: 5 })
    expect(d.allowed).toBe(false)
    expect(d.reasonRef).toBe(FREE_KEY_MINT_REASON_RATE_LIMITED)
  })
})

describe('sanitizeFreeKeyLabel', () => {
  test('defaults empty to a safe label and bounds length', () => {
    expect(sanitizeFreeKeyLabel(undefined)).toBe('Free API key')
    expect(sanitizeFreeKeyLabel('  ')).toBe('Free API key')
    expect(sanitizeFreeKeyLabel('x'.repeat(200)).length).toBe(80)
  })
})

// ----------------------------------------------------------------------------
// D1 store: mark + read + quota accrual
// ----------------------------------------------------------------------------

describe('free-tier key store (D1)', () => {
  test('markAccountFreeTier makes the account free-tier; absent accounts are not', async () => {
    const db = makeDb()
    expect(await readAccountFreeTier(db, 'agent:free-user')).toBe(false)
    const ok = await run(
      markAccountFreeTier(db, {
        accountRef: 'agent:free-user',
        nowIso: fixedNow,
      }),
    )
    expect(ok).toBe(true)
    expect(await readAccountFreeTier(db, 'agent:free-user')).toBe(true)
    expect(await readAccountFreeTier(db, 'agent:other')).toBe(false)
  })

  test('mint rate counter increments per IP-hash per day', async () => {
    const db = makeDb()
    expect(await readFreeKeyMintsToday(db, 'iphash', '2026-06-24')).toBe(0)
    await run(
      recordFreeKeyMint(db, {
        ipHash: 'iphash',
        mintDay: '2026-06-24',
        nowIso: fixedNow,
      }),
    )
    await run(
      recordFreeKeyMint(db, {
        ipHash: 'iphash',
        mintDay: '2026-06-24',
        nowIso: fixedNow,
      }),
    )
    expect(await readFreeKeyMintsToday(db, 'iphash', '2026-06-24')).toBe(2)
    // A different day is a fresh bucket.
    expect(await readFreeKeyMintsToday(db, 'iphash', '2026-06-25')).toBe(0)
  })
})

// ----------------------------------------------------------------------------
// Balance-gate seam (the free-tier bypass)
// ----------------------------------------------------------------------------

describe('makeFreeTierGate (balance-gate bypass)', () => {
  test('free-tier key + Khala + within quota => free', async () => {
    const db = makeDb()
    await run(
      markAccountFreeTier(db, { accountRef: 'agent:free-user', nowIso: fixedNow }),
    )
    const gate = makeFreeTierGate({ db, nowIso: fixedNow })
    const d = await gate('agent:free-user', KHALA_MODEL_ID)
    expect(d.free).toBe(true)
  })

  test('non-free-tier account => not free (the 402 stands)', async () => {
    const db = makeDb()
    const gate = makeFreeTierGate({ db, nowIso: fixedNow })
    const d = await gate('agent:unknown', KHALA_MODEL_ID)
    expect(d.free).toBe(false)
  })

  test('free-tier key but a PREMIUM model => never free', async () => {
    const db = makeDb()
    await run(
      markAccountFreeTier(db, { accountRef: 'agent:free-user', nowIso: fixedNow }),
    )
    const gate = makeFreeTierGate({ db, nowIso: fixedNow })
    const d = await gate('agent:free-user', 'claude-sonnet')
    expect(d.free).toBe(false)
    expect(d.reasonRef).toBe(FREE_TIER_REASON_PREMIUM_DENIED)
  })

  test('free-tier key over the daily request quota => not free', async () => {
    const db = makeDb()
    await run(
      markAccountFreeTier(db, { accountRef: 'agent:free-user', nowIso: fixedNow }),
    )
    const gate = makeFreeTierGate({
      db,
      nowIso: fixedNow,
      quota: { maxRequestsPerDay: 1, maxTokensPerDay: 1_000_000 },
    })
    // First request is within quota.
    expect((await gate('agent:free-user', KHALA_MODEL_ID)).free).toBe(true)
    // Accrue one request via the metering wrapper so the counter advances.
    const inner = makeInnerSpy()
    const wrapped = withFreeTierKhala(inner.hook, {
      db,
      nowIso: fixedNow,
      quota: { maxRequestsPerDay: 1, maxTokensPerDay: 1_000_000 },
    })
    await run(wrapped(meteringContext({ requestId: 'q-1' })))
    // Now the key has hit its 1/day request ceiling => over quota.
    expect((await gate('agent:free-user', KHALA_MODEL_ID)).free).toBe(false)
  })
})

// ----------------------------------------------------------------------------
// Metering wrapper (zero-debit + idempotent quota accrual)
// ----------------------------------------------------------------------------

describe('withFreeTierKhala (metering wrapper)', () => {
  test('free-tier Khala within quota => zero-debit free receipt, inner NOT called, quota accrued', async () => {
    const db = makeDb()
    await run(
      markAccountFreeTier(db, { accountRef: 'agent:free-user', nowIso: fixedNow }),
    )
    const inner = makeInnerSpy()
    const wrapped = withFreeTierKhala(inner.hook, { db, nowIso: fixedNow })
    const outcome = await run(wrapped(meteringContext({ requestId: 'free-1' })))
    expect(outcome.metered).toBe(false)
    expect(outcome.receiptRef).toBe(freeTierReceiptRef('free-1'))
    expect(inner.calls()).toBe(0)
    const usage = await readFreeTierUsage(db, 'agent:free-user', '2026-06-24')
    expect(usage.requestsToday).toBe(1)
    expect(usage.tokensToday).toBe(15)
  })

  test('idempotent: the SAME request id never double-counts the quota', async () => {
    const db = makeDb()
    await run(
      markAccountFreeTier(db, { accountRef: 'agent:free-user', nowIso: fixedNow }),
    )
    const inner = makeInnerSpy()
    const wrapped = withFreeTierKhala(inner.hook, { db, nowIso: fixedNow })
    await run(wrapped(meteringContext({ requestId: 'dup' })))
    await run(wrapped(meteringContext({ requestId: 'dup' })))
    const usage = await readFreeTierUsage(db, 'agent:free-user', '2026-06-24')
    expect(usage.requestsToday).toBe(1)
    expect(usage.tokensToday).toBe(15)
  })

  test('non-free-tier account => meters normally (inner called)', async () => {
    const db = makeDb()
    const inner = makeInnerSpy()
    const wrapped = withFreeTierKhala(inner.hook, { db, nowIso: fixedNow })
    const outcome = await run(wrapped(meteringContext({ accountRef: 'agent:paid' })))
    expect(outcome.metered).toBe(true)
    expect(inner.calls()).toBe(1)
  })

  test('GUARDRAIL: a PREMIUM served model always meters (inner called)', async () => {
    const db = makeDb()
    await run(
      markAccountFreeTier(db, { accountRef: 'agent:free-user', nowIso: fixedNow }),
    )
    const inner = makeInnerSpy()
    const wrapped = withFreeTierKhala(inner.hook, { db, nowIso: fixedNow })
    const outcome = await run(
      wrapped(meteringContext({ servedModel: 'claude-sonnet' })),
    )
    expect(outcome.metered).toBe(true)
    expect(inner.calls()).toBe(1)
  })

  test('over quota => meters normally (inner called)', async () => {
    const db = makeDb()
    await run(
      markAccountFreeTier(db, { accountRef: 'agent:free-user', nowIso: fixedNow }),
    )
    const inner = makeInnerSpy()
    const wrapped = withFreeTierKhala(inner.hook, {
      db,
      nowIso: fixedNow,
      quota: { maxRequestsPerDay: 1, maxTokensPerDay: 1_000_000 },
    })
    await run(wrapped(meteringContext({ requestId: 'within-1' })))
    const outcome = await run(wrapped(meteringContext({ requestId: 'over-1' })))
    expect(outcome.metered).toBe(true)
    expect(inner.calls()).toBe(1)
  })
})

// ----------------------------------------------------------------------------
// Integration: the chat-completions balance gate honors the free-tier bypass
// ----------------------------------------------------------------------------

const authFree: InferenceAuth = async () => ({ accountRef: 'agent:free-user' })
const emptyBalance: InferenceBalanceReader = async () => 0
const fundedBalance: InferenceBalanceReader = async () => 100_000

const registryWithStub = (): InferenceProviderRegistry => {
  const registry = new InferenceProviderRegistry()
  registry.register(stubEchoAdapter)
  return registry
}

const baseDeps = (
  overrides: Partial<ChatCompletionsDeps> = {},
): ChatCompletionsDeps => ({
  authenticate: authFree,
  enabled: true,
  nowEpochMillis: () => 0,
  readAvailableMsat: fundedBalance,
  registry: registryWithStub(),
  ...overrides,
})

const chatRequest = (body: unknown): Request =>
  new Request('https://openagents.com/v1/chat/completions', {
    body: JSON.stringify(body),
    method: 'POST',
  })

const khalaBody = {
  messages: [{ content: 'hello world', role: 'user' }],
  model: KHALA_MODEL_ID,
}

describe('POST /v1/chat/completions free-tier bypass (issue #6228)', () => {
  test('(b) free-tier key bypasses the balance-402 for free-lane Khala within quota', async () => {
    const seen: Array<{ accountRef: string; model: string }> = []
    const response = await run(
      handleChatCompletions(
        chatRequest(khalaBody),
        baseDeps({
          checkFreeTier: async (accountRef, model) => {
            seen.push({ accountRef, model })
            return { free: true }
          },
          readAvailableMsat: emptyBalance,
        }),
      ),
    )
    expect(response.status).toBe(200)
    expect(seen).toEqual([{ accountRef: 'agent:free-user', model: KHALA_MODEL_ID }])
  })

  test('(c) over-quota free-tier key => still 402 insufficient_credits', async () => {
    const response = await run(
      handleChatCompletions(
        chatRequest(khalaBody),
        baseDeps({
          checkFreeTier: async () => ({ free: false }),
          readAvailableMsat: emptyBalance,
        }),
      ),
    )
    expect(response.status).toBe(402)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('insufficient_credits')
  })

  test('(d) paid lanes still require credits when no free bypass applies', async () => {
    // No free-tier seam wired (the flag-off shape): a zero balance still 402s.
    const response = await run(
      handleChatCompletions(
        chatRequest(khalaBody),
        baseDeps({ readAvailableMsat: emptyBalance }),
      ),
    )
    expect(response.status).toBe(402)
  })

  test('funded balance never consults the free-tier seam', async () => {
    let calls = 0
    const response = await run(
      handleChatCompletions(
        chatRequest(khalaBody),
        baseDeps({
          checkFreeTier: async () => {
            calls += 1
            return { free: true }
          },
          readAvailableMsat: fundedBalance,
        }),
      ),
    )
    expect(response.status).toBe(200)
    expect(calls).toBe(0)
  })
})
