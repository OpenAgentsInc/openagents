import { DatabaseSync } from 'node:sqlite'

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../../payments-ledger'
import { makeLedgerMeteringHook, type MeteringContext } from '../metering-hook'
import { VERTEX_GEMINI_ADAPTER_ID, selectAdapterPlan } from '../model-router'
import { KHALA_MINI_MODEL_ID } from '../pricing'
import {
  type InferenceProviderAdapter,
  InferenceProviderRegistry,
} from '../provider-adapter'
import { stubEchoAdapter } from '../stub-echo-adapter'
import {
  type MppChatCompletionsDeps,
  handleMppChatCompletions,
  isKhalaMppEnabled,
} from './mpp-chat-completions-routes'
import { type StripeFetch } from './stripe-mpp-client'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect as Effect.Effect<A, never>)

// In-memory D1 (same harness as usd-credit-bridge.test.ts).
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
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

// The stub echo adapter registered under the Gemini lane id, so khala-mini
// resolves via `selectAdapterPlan` (khala-mini classifies to the Gemini lane).
// Reports receipt-first usage so the metering hook settles.
const echoAdapter = (id: string): InferenceProviderAdapter => ({
  ...stubEchoAdapter,
  id,
})

const khalaRegistry = (): InferenceProviderRegistry => {
  const registry = new InferenceProviderRegistry()
  registry.register(echoAdapter(VERTEX_GEMINI_ADAPTER_ID))
  return registry
}

// completionDeps mirroring the keyed route (minus auth/balance, which the MPP
// handler replaces). enabled = inference-gateway flag (on so the completion can
// run for a paid call).
const completionDeps = (
  db: D1Database,
): MppChatCompletionsDeps['completionDeps'] => ({
  enabled: true,
  lanePlan: selectAdapterPlan,
  meteringHook: makeLedgerMeteringHook({ db }),
  registry: khalaRegistry(),
})

const mppBody = (): unknown => ({
  messages: [{ content: 'hello', role: 'user' }],
  model: KHALA_MINI_MODEL_ID,
})

const mppRequest = (init: RequestInit = {}): Request =>
  new Request('https://openagents.com/mpp/v1/chat/completions', {
    body: JSON.stringify(mppBody()),
    method: 'POST',
    ...init,
  })

describe('isKhalaMppEnabled flag', () => {
  test('default OFF; on for explicit truthy tokens', () => {
    expect(isKhalaMppEnabled(undefined)).toBe(false)
    expect(isKhalaMppEnabled('')).toBe(false)
    expect(isKhalaMppEnabled('false')).toBe(false)
    expect(isKhalaMppEnabled('1')).toBe(true)
    expect(isKhalaMppEnabled('true')).toBe(true)
    expect(isKhalaMppEnabled('on')).toBe(true)
  })
})

describe('MPP endpoint — FAIL-SAFE inert (never charges when unconfigured)', () => {
  test('flag OFF => 503 not-configured, no Stripe call', async () => {
    const db = makeDb()
    let stripeCalled = false
    const fakeFetch: StripeFetch = async () => {
      stripeCalled = true
      return new Response('{}', { status: 200 })
    }
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: false,
        fetch: fakeFetch,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(503)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('mpp_not_configured')
    expect(stripeCalled).toBe(false)
  })

  test('no Stripe key => 503 not-configured, no Stripe call', async () => {
    const db = makeDb()
    let stripeCalled = false
    const fakeFetch: StripeFetch = async () => {
      stripeCalled = true
      return new Response('{}', { status: 200 })
    }
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: fakeFetch,
        stripeSecretKey: undefined,
      }),
    )
    expect(response.status).toBe(503)
    expect(stripeCalled).toBe(false)
  })
})

describe('MPP endpoint — 402 challenge (no payment credential)', () => {
  test('returns 402 with a WWW-Authenticate Payment challenge + problem+json body', async () => {
    const db = makeDb()
    // Fake Stripe create-PaymentIntent (the 402 needs a deposit address).
    const fakeFetch: StripeFetch = async () =>
      new Response(
        JSON.stringify({
          amount: 1,
          currency: 'usd',
          id: 'pi_quote_1',
          next_action: {
            crypto_display_details: {
              addresses: [{ address: '0xabc', network: 'base' }],
            },
          },
          status: 'requires_payment',
        }),
        { status: 200 },
      )

    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: fakeFetch,
        newId: () => 'fixed',
        stripeSecretKey: 'sk_test_x',
      }),
    )

    expect(response.status).toBe(402)
    expect(response.headers.get('content-type')).toBe('application/problem+json')
    const wwwAuth = response.headers.get('www-authenticate')
    expect(wwwAuth).toContain('Payment ')
    expect(wwwAuth).toContain('method="base"')
    expect(wwwAuth).toContain('recipient="0xabc"')
    expect(wwwAuth).toContain('payment_intent="pi_quote_1"')

    const problem = (await response.json()) as {
      type: string
      status: number
      challenges: Array<{ paymentIntentId?: string; recipient?: string }>
    }
    expect(problem.status).toBe(402)
    expect(problem.type).toBe(
      'https://paymentauth.org/problems/payment-required',
    )
    expect(problem.challenges[0]?.paymentIntentId).toBe('pi_quote_1')
  })
})

describe('MPP endpoint — valid credential serves + meters', () => {
  test('verifies the settled payment, mints credit, runs Khala, meters, returns the completion', async () => {
    const db = makeDb()
    const metered: Array<MeteringContext> = []

    // Stripe fetch routes by method: GET /payment_intents/<id> = verify
    // (settled); we should NOT need a create call on the paid path.
    const fakeFetch: StripeFetch = async (url, init) => {
      if ((init.method ?? 'GET') === 'GET' && url.includes('/payment_intents/')) {
        return new Response(
          JSON.stringify({
            amount: 100,
            currency: 'usd',
            id: 'pi_paid_1',
            metadata: { model: KHALA_MINI_MODEL_ID },
            status: 'succeeded',
          }),
          { status: 200 },
        )
      }
      return new Response('{}', { status: 200 })
    }

    // Spy metering hook wrapping the real ledger hook so we assert it fired.
    const ledger = makeLedgerMeteringHook({ db })
    const spyMetering = (context: MeteringContext) =>
      Effect.gen(function* () {
        metered.push(context)
        return yield* ledger(context)
      })

    const deps: MppChatCompletionsDeps = {
      completionDeps: { ...completionDeps(db), meteringHook: spyMetering },
      db,
      enabled: true,
      fetch: fakeFetch,
      nowIso: () => '2026-06-22T12:00:00.000Z',
      stripeSecretKey: 'sk_test_x',
    }

    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          headers: {
            authorization: 'Payment id="c:crypto", payment_intent="pi_paid_1"',
            'content-type': 'application/json',
          },
        }),
        deps,
      ),
    )

    // Served the completion (OpenAI shape).
    expect(response.status).toBe(200)
    const completion = (await response.json()) as {
      object: string
      choices: Array<{ message: { content: string } }>
      openagents?: { served_model?: string }
    }
    expect(completion.object).toBe('chat.completion')
    // The stub echoes the last user message back ('hello').
    expect(completion.choices[0]?.message.content).toBe('hello')
    // Khala disclosure block present.
    expect(completion.openagents).toBeDefined()

    // The metering hook fired (the SAME receipt-first metering as the keyed route).
    expect(metered.length).toBe(1)
    expect(metered[0]?.requestedModel).toBe(KHALA_MINI_MODEL_ID)

    // Phase 3: credit was minted into the payer-bound balance (USD-origin).
    const balance = await readAgentBalance(db, 'agent:mpp:pi_paid_1')
    expect(balance).not.toBeNull()
    expect(balance!.usdCreditMsat).toBeGreaterThan(0)
  })

  test('an unsettled payment returns 402 and serves nothing (never serves unpaid)', async () => {
    const db = makeDb()
    let completionRan = false
    const fakeFetch: StripeFetch = async (url, init) => {
      if ((init.method ?? 'GET') === 'GET' && url.includes('/payment_intents/')) {
        return new Response(
          JSON.stringify({ amount: 100, id: 'pi_pending', status: 'processing' }),
          { status: 200 },
        )
      }
      return new Response('{}', { status: 200 })
    }
    const spyMetering = (context: MeteringContext) =>
      Effect.sync(() => {
        completionRan = true
        return { metered: false, receiptRef: null }
      })

    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          headers: {
            authorization: 'Payment payment_intent="pi_pending"',
            'content-type': 'application/json',
          },
        }),
        {
          completionDeps: { ...completionDeps(db), meteringHook: spyMetering },
          db,
          enabled: true,
          fetch: fakeFetch,
          stripeSecretKey: 'sk_test_x',
        },
      ),
    )

    expect(response.status).toBe(402)
    const body = (await response.json()) as { error: string }
    expect(body.error).toBe('payment_not_settled')
    expect(completionRan).toBe(false)
  })
})
