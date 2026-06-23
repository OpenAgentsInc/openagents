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
import { base64UrlEncode, jcsBase64Url } from './mpp-canonical'
import {
  type MppChatCompletionsDeps,
  handleMppChatCompletions,
  isKhalaMppEnabled,
} from './mpp-chat-completions-routes'
import { buildChallenge, type MppChallenge } from './mpp-protocol'
import { type StripeFetch } from './stripe-mpp-client'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect as Effect.Effect<A, never>)

const SIGNING_SECRET = 'test-signing-secret'
const REALM = 'openagents.com'

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
  async run(): Promise<{ success: true; results: []; meta: { changes: number } }> {
    const info = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return {
      meta: { changes: Number(info.changes ?? 0) },
      results: [],
      success: true,
    }
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
CREATE TABLE mpp_spt_replay (
  spt TEXT PRIMARY KEY,
  challenge_id TEXT NOT NULL,
  payment_intent_id TEXT,
  consumed_at TEXT NOT NULL
);
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const echoAdapter = (id: string): InferenceProviderAdapter => ({
  ...stubEchoAdapter,
  id,
})

const khalaRegistry = (): InferenceProviderRegistry => {
  const registry = new InferenceProviderRegistry()
  registry.register(echoAdapter(VERTEX_GEMINI_ADAPTER_ID))
  return registry
}

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

// Compose a wire credential that echoes a challenge + carries a payload.
const credentialHeader = (
  challenge: MppChallenge,
  payload: Record<string, unknown> = {},
): string =>
  `Payment ${base64UrlEncode(
    JSON.stringify({
      challenge: {
        expires: challenge.expires,
        id: challenge.id,
        intent: challenge.intent,
        method: challenge.method,
        opaque: challenge.opaque,
        realm: challenge.realm,
        request: challenge.request,
      },
      payload,
    }),
  )}`

// A valid crypto challenge whose opaque carries the deposit PaymentIntent id.
const cryptoChallenge = (pi: string, amountCents = 100): Promise<MppChallenge> =>
  buildChallenge(SIGNING_SECRET, {
    amountCents,
    currency: 'usdc',
    expires: '2099-01-15T12:05:00.000Z',
    method: 'base',
    network: 'base',
    opaque: { amount: String(amountCents), network: 'base', pi },
    paymentIntentId: pi,
    realm: REALM,
    recipient: '0xabc',
    request: { amount: String(amountCents), currency: 'usdc', network: 'base' },
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
  const noStripeCall = (): {
    fetch: StripeFetch
    called: () => boolean
  } => {
    let called = false
    return {
      called: () => called,
      fetch: async () => {
        called = true
        return new Response('{}', { status: 200 })
      },
    }
  }

  test('flag OFF => 503, no Stripe call', async () => {
    const db = makeDb()
    const spy = noStripeCall()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: false,
        fetch: spy.fetch,
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(503)
    expect(((await response.json()) as { error: string }).error).toBe(
      'mpp_not_configured',
    )
    expect(spy.called()).toBe(false)
  })

  test('no Stripe key => 503, no Stripe call', async () => {
    const db = makeDb()
    const spy = noStripeCall()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: spy.fetch,
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: undefined,
      }),
    )
    expect(response.status).toBe(503)
    expect(spy.called()).toBe(false)
  })

  test('no signing secret => 503, no Stripe call', async () => {
    const db = makeDb()
    const spy = noStripeCall()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: spy.fetch,
        signingSecret: undefined,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(503)
    expect(((await response.json()) as { reason: string }).reason).toContain(
      'signing secret',
    )
    expect(spy.called()).toBe(false)
  })
})

describe('MPP endpoint — 402 challenge (no payment credential)', () => {
  const quoteFetch = (id: string): StripeFetch => async () =>
    new Response(
      JSON.stringify({
        amount: 1,
        currency: 'usd',
        id,
        next_action: {
          crypto_display_details: {
            deposit_addresses: { base: { address: '0xabc', supported_tokens: [] } },
          },
        },
        status: 'requires_payment',
      }),
      { status: 200 },
    )

  test('returns 402 with an HMAC-bound WWW-Authenticate Payment challenge', async () => {
    const db = makeDb()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: quoteFetch('pi_quote_1'),
        newId: () => 'fixed',
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(402)
    expect(response.headers.get('content-type')).toBe('application/problem+json')
    const wwwAuth = response.headers.get('www-authenticate')!
    expect(wwwAuth).toContain('Payment ')
    expect(wwwAuth).toContain('method="base"')
    expect(wwwAuth).toContain('realm="openagents.com"')
    expect(wwwAuth).toContain('request=')
    expect(wwwAuth).toContain('opaque=')
    // The id is an HMAC, NOT a plaintext "pi_…:crypto".
    expect(wwwAuth).not.toContain('pi_quote_1:crypto')

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
    expect(problem.challenges[0]?.recipient).toBe('0xabc')
  })

  test('NO network profile id => crypto-only (no stripe/card challenge)', async () => {
    const db = makeDb()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: quoteFetch('pi_quote_2'),
        newId: () => 'fixed',
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(402)
    const problem = (await response.json()) as {
      challenges: Array<{ method: string }>
    }
    expect(problem.challenges.every(c => c.method !== 'stripe')).toBe(true)
  })

  test('WITH network profile id => adds the card/SPT (stripe) challenge', async () => {
    const db = makeDb()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: quoteFetch('pi_quote_3'),
        newId: () => 'fixed',
        signingSecret: SIGNING_SECRET,
        stripeNetworkProfileId: 'profile_test',
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(402)
    const wwwAuth = response.headers.get('www-authenticate')!
    expect(wwwAuth).toContain('method="stripe"')
    const problem = (await response.json()) as {
      challenges: Array<{ method: string }>
    }
    expect(problem.challenges.some(c => c.method === 'base')).toBe(true)
    expect(problem.challenges.some(c => c.method === 'stripe')).toBe(true)
  })
})

describe('MPP endpoint — credential verification + crypto settlement', () => {
  // Stripe fetch that verifies a settled crypto deposit on GET and provides a
  // quote on the re-challenge POST.
  const settledCryptoFetch = (pi: string, amount = 100): StripeFetch => async (
    url,
    init,
  ) => {
    if ((init.method ?? 'GET') === 'GET' && url.includes('/payment_intents/')) {
      return new Response(
        JSON.stringify({
          amount,
          currency: 'usd',
          id: pi,
          metadata: { model: KHALA_MINI_MODEL_ID },
          status: 'succeeded',
        }),
        { status: 200 },
      )
    }
    // re-challenge quote
    return new Response(
      JSON.stringify({
        amount: 1,
        currency: 'usd',
        id: 'pi_requote',
        next_action: {
          crypto_display_details: {
            deposit_addresses: { base: { address: '0xabc' } },
          },
        },
        status: 'requires_payment',
      }),
      { status: 200 },
    )
  }

  test('verifies the bound credential, mints credit, runs Khala, meters, serves + Payment-Receipt', async () => {
    const db = makeDb()
    const metered: Array<MeteringContext> = []
    const challenge = await cryptoChallenge('pi_paid_1', 100)

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
      fetch: settledCryptoFetch('pi_paid_1', 100),
      nowIso: () => '2026-06-22T12:00:00.000Z',
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }

    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          headers: {
            authorization: credentialHeader(challenge),
            'content-type': 'application/json',
          },
        }),
        deps,
      ),
    )

    expect(response.status).toBe(200)
    // Payment-Receipt header (standards-shaped, base64url JCS).
    expect(response.headers.get('payment-receipt')).toBe(
      jcsBase64Url({
        method: 'base',
        reference: 'pi_paid_1',
        status: 'success',
        timestamp: '2026-06-22T12:00:00.000Z',
      }),
    )
    expect(response.headers.get('cache-control')).toBe('private')

    const completion = (await response.json()) as {
      object: string
      choices: Array<{ message: { content: string } }>
      openagents?: unknown
    }
    expect(completion.object).toBe('chat.completion')
    expect(completion.choices[0]?.message.content).toBe('hello')
    expect(completion.openagents).toBeDefined()

    expect(metered.length).toBe(1)
    expect(metered[0]?.requestedModel).toBe(KHALA_MINI_MODEL_ID)

    const balance = await readAgentBalance(db, 'agent:mpp:pi_paid_1')
    expect(balance).not.toBeNull()
    expect(balance!.usdCreditMsat).toBeGreaterThan(0)
  })

  test('credit mint is idempotent across two settled retries (one payment = one grant)', async () => {
    const db = makeDb()
    const challenge = await cryptoChallenge('pi_idem_1', 100)
    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db),
      db,
      enabled: true,
      fetch: settledCryptoFetch('pi_idem_1', 100),
      nowIso: () => '2026-06-22T12:00:00.000Z',
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }
    const header = credentialHeader(challenge)
    const first = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: header } }),
        deps,
      ),
    )
    expect(first.status).toBe(200)
    const balance1 = await readAgentBalance(db, 'agent:mpp:pi_idem_1')
    const second = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: header } }),
        deps,
      ),
    )
    expect(second.status).toBe(200)
    const balance2 = await readAgentBalance(db, 'agent:mpp:pi_idem_1')
    // The second retry did not double-grant credit (idempotent by pi).
    expect(balance2!.usdCreditMsat).toBe(balance1!.usdCreditMsat)
  })

  test('a tampered credential is rejected (fresh 402, never serves)', async () => {
    const db = makeDb()
    let completionRan = false
    const challenge = await cryptoChallenge('pi_tamper_1', 100)
    const tampered: MppChallenge = { ...challenge, id: `${challenge.id}x` }
    const spyMetering = () =>
      Effect.sync(() => {
        completionRan = true
        return { metered: false, receiptRef: null }
      })
    const response = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: credentialHeader(tampered) } }),
        {
          completionDeps: { ...completionDeps(db), meteringHook: spyMetering },
          db,
          enabled: true,
          fetch: settledCryptoFetch('pi_tamper_1'),
          signingSecret: SIGNING_SECRET,
          stripeSecretKey: 'sk_test_x',
        },
      ),
    )
    expect(response.status).toBe(402)
    expect(response.headers.get('payment-receipt')).toBeNull()
    expect(completionRan).toBe(false)
  })

  test('an unsettled crypto payment returns 402 and serves nothing', async () => {
    const db = makeDb()
    let completionRan = false
    const challenge = await cryptoChallenge('pi_pending', 100)
    const fetch: StripeFetch = async (url, init) => {
      if ((init.method ?? 'GET') === 'GET' && url.includes('/payment_intents/')) {
        return new Response(
          JSON.stringify({ amount: 100, id: 'pi_pending', status: 'processing' }),
          { status: 200 },
        )
      }
      return new Response('{}', { status: 200 })
    }
    const spyMetering = () =>
      Effect.sync(() => {
        completionRan = true
        return { metered: false, receiptRef: null }
      })
    const response = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: credentialHeader(challenge) } }),
        {
          completionDeps: { ...completionDeps(db), meteringHook: spyMetering },
          db,
          enabled: true,
          fetch,
          signingSecret: SIGNING_SECRET,
          stripeSecretKey: 'sk_test_x',
        },
      ),
    )
    expect(response.status).toBe(402)
    expect(((await response.json()) as { error: string }).error).toBe(
      'payment_not_settled',
    )
    expect(completionRan).toBe(false)
  })
})

describe('MPP endpoint — card/SPT settlement + replay', () => {
  const PROFILE = 'profile_test'
  // A stripe challenge (card rail). Currency usd, networkId in methodDetails.
  const stripeChallenge = (): Promise<MppChallenge> =>
    buildChallenge(SIGNING_SECRET, {
      amountCents: 100,
      currency: 'usd',
      expires: '2099-01-15T12:05:00.000Z',
      method: 'stripe',
      realm: REALM,
      request: {
        amount: '100',
        currency: 'usd',
        methodDetails: { networkId: PROFILE, paymentMethodTypes: ['card'] },
      },
    })

  // Stripe fetch: POST /payment_intents with an SPT body succeeds; re-challenge
  // quote returns a deposit address.
  const sptFetch = (pi: string): StripeFetch => async (url, init) => {
    if ((init.method ?? 'GET') === 'POST' && url.endsWith('/payment_intents')) {
      const body = String(init.body ?? '')
      if (body.includes('shared_payment_granted_token')) {
        return new Response(
          JSON.stringify({ amount: 100, currency: 'usd', id: pi, status: 'succeeded' }),
          { status: 200 },
        )
      }
      // quote (deposit-mode crypto create)
      return new Response(
        JSON.stringify({
          amount: 1,
          id: 'pi_requote',
          next_action: {
            crypto_display_details: {
              deposit_addresses: { base: { address: '0xabc' } },
            },
          },
          status: 'requires_payment',
        }),
        { status: 200 },
      )
    }
    return new Response('{}', { status: 200 })
  }

  test('settles a fresh SPT, mints credit, serves + Payment-Receipt method=stripe', async () => {
    const db = makeDb()
    const challenge = await stripeChallenge()
    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          headers: {
            authorization: credentialHeader(challenge, { spt: 'spt_abc123' }),
          },
        }),
        {
          completionDeps: completionDeps(db),
          db,
          enabled: true,
          fetch: sptFetch('pi_card_1'),
          nowIso: () => '2026-06-22T12:00:00.000Z',
          signingSecret: SIGNING_SECRET,
          stripeNetworkProfileId: PROFILE,
          stripeSecretKey: 'sk_test_x',
        },
      ),
    )
    expect(response.status).toBe(200)
    const receipt = response.headers.get('payment-receipt')!
    expect(receipt).toBe(
      jcsBase64Url({
        method: 'stripe',
        reference: 'pi_card_1',
        status: 'success',
        timestamp: '2026-06-22T12:00:00.000Z',
      }),
    )
    const balance = await readAgentBalance(db, 'agent:mpp:pi_card_1')
    expect(balance!.usdCreditMsat).toBeGreaterThan(0)
  })

  test('a replayed SPT is rejected (402), never double-charges', async () => {
    const db = makeDb()
    const challenge = await stripeChallenge()
    let chargeCount = 0
    const fetch: StripeFetch = async (url, init) => {
      if (
        (init.method ?? 'GET') === 'POST' &&
        url.endsWith('/payment_intents') &&
        String(init.body ?? '').includes('shared_payment_granted_token')
      ) {
        chargeCount += 1
        return new Response(
          JSON.stringify({ amount: 100, currency: 'usd', id: 'pi_card_2', status: 'succeeded' }),
          { status: 200 },
        )
      }
      return new Response(
        JSON.stringify({
          amount: 1,
          id: 'pi_requote',
          next_action: {
            crypto_display_details: {
              deposit_addresses: { base: { address: '0xabc' } },
            },
          },
          status: 'requires_payment',
        }),
        { status: 200 },
      )
    }
    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db),
      db,
      enabled: true,
      fetch,
      nowIso: () => '2026-06-22T12:00:00.000Z',
      signingSecret: SIGNING_SECRET,
      stripeNetworkProfileId: PROFILE,
      stripeSecretKey: 'sk_test_x',
    }
    const header = credentialHeader(challenge, { spt: 'spt_replay_me' })
    const first = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: header } }),
        deps,
      ),
    )
    expect(first.status).toBe(200)
    const second = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: header } }),
        deps,
      ),
    )
    expect(second.status).toBe(402)
    expect(((await second.json()) as { reason?: string }).reason).toBe(
      'spt_replayed',
    )
    // Only the FIRST attempt hit Stripe with a charge.
    expect(chargeCount).toBe(1)
  })
})
