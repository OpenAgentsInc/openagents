import { DatabaseSync } from 'node:sqlite'

import { Duration, Effect, Fiber } from 'effect'
import { TestClock } from 'effect/testing'
import { describe, expect, test } from 'vitest'

import { readAgentBalance } from '../../payments-ledger'
import { makeLedgerMeteringHook, type MeteringContext } from '../metering-hook'
import {
  HYDRALISK_ADAPTER_ID,
  HYDRALISK_GPT_OSS_120B_ADAPTER_ID,
  selectAdapterPlan,
} from '../model-router'
import { resolveSupplyLaneArming } from '../model-serving-policy'
import {
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  KHALA_MODEL_ID,
} from '../pricing'
import {
  type InferenceProviderAdapter,
  InferenceProviderRegistry,
} from '../provider-adapter'
import { stubEchoAdapter } from '../stub-echo-adapter'
import {
  base64UrlEncode,
  decodeJcsBase64UrlRecord,
  jcsBase64Url,
} from './mpp-canonical'
import {
  type MppChatCompletionsDeps,
  handleMppChatCompletions,
  isKhalaMppEnabled,
  isKhalaMppLightningEnabled,
} from './mpp-chat-completions-routes'
import {
  type LightningInvoice,
  type MintLightningInvoice,
  LightningInvoiceError,
  makeFallbackLightningInvoiceIssuer,
} from './mpp-lightning-invoice'
import { sha256Hex } from './mpp-lightning-verify'
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
CREATE TABLE mpp_spt_replay (
  spt TEXT PRIMARY KEY,
  challenge_id TEXT NOT NULL,
  payment_intent_id TEXT,
  consumed_at TEXT NOT NULL
);
CREATE TABLE mpp_lightning_replay (
  payment_hash TEXT PRIMARY KEY,
  challenge_id TEXT NOT NULL,
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
  registry.register(echoAdapter(HYDRALISK_GPT_OSS_120B_ADAPTER_ID))
  registry.register(echoAdapter(HYDRALISK_ADAPTER_ID))
  return registry
}

const hydraliskRegistry = (): InferenceProviderRegistry => {
  const registry = new InferenceProviderRegistry()
  registry.register(echoAdapter(HYDRALISK_ADAPTER_ID))
  return registry
}

const hydralisk120bRegistry = (): InferenceProviderRegistry => {
  const registry = new InferenceProviderRegistry()
  registry.register(echoAdapter(HYDRALISK_GPT_OSS_120B_ADAPTER_ID))
  return registry
}

const HYDRALISK_READY_ENV = {
  HYDRALISK_BASE_URL: 'https://hydralisk.example.test',
  HYDRALISK_BEARER_TOKEN: 'secret-route-token',
  HYDRALISK_GPT_OSS_20B_ENABLED: 'ready',
  HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF: 'preflight.hydralisk.gpt_oss_20b.l4.v1',
  HYDRALISK_GPT_OSS_20B_RECEIPT_REF:
    'receipt.hydralisk.gpt_oss_20b.l4.smoke.v1',
}

const HYDRALISK_120B_READY_ENV = {
  HYDRALISK_GPT_OSS_120B_BASE_URL: 'https://hydralisk-120b.example.test',
  HYDRALISK_GPT_OSS_120B_BEARER_TOKEN: 'secret-route-token',
  HYDRALISK_GPT_OSS_120B_ENABLED: 'ready',
  HYDRALISK_GPT_OSS_120B_PREFLIGHT_REF:
    'preflight.hydralisk.gpt_oss_120b.h100.v1',
  HYDRALISK_GPT_OSS_120B_RECEIPT_REF:
    'receipt.hydralisk.gpt_oss_120b.h100.smoke.v1',
}

const completionDeps = (
  db: D1Database,
  overrides: Partial<MppChatCompletionsDeps['completionDeps']> = {},
): MppChatCompletionsDeps['completionDeps'] => ({
  enabled: true,
  lanePlan: selectAdapterPlan,
  meteringHook: makeLedgerMeteringHook({ db }),
  registry: khalaRegistry(),
  ...overrides,
})

const mppBody = (model = KHALA_MODEL_ID): unknown => ({
  messages: [{ content: 'hello', role: 'user' }],
  model,
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

const firstChallengeOpaque = (response: Response): Record<string, unknown> | undefined => {
  const header = response.headers.get('www-authenticate') ?? ''
  const opaque = header.match(/opaque="([^"]+)"/u)?.[1]
  return opaque === undefined ? undefined : decodeJcsBase64UrlRecord(opaque)
}

// A valid crypto challenge whose opaque carries the deposit PaymentIntent id.
const cryptoChallenge = (
  pi: string,
  amountCents = 100,
  model = KHALA_MODEL_ID,
): Promise<MppChallenge> =>
  buildChallenge(SIGNING_SECRET, {
    amountCents,
    currency: 'usdc',
    expires: '2099-01-15T12:05:00.000Z',
    method: 'base',
    network: 'base',
    opaque: { amount: String(amountCents), model, network: 'base', pi },
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

  test('omitted model defaults to the external Khala id', async () => {
    const db = makeDb()
    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          body: JSON.stringify({
            messages: [{ content: 'hello', role: 'user' }],
          }),
        }),
        {
          completionDeps: completionDeps(db),
          db,
          enabled: true,
          fetch: quoteFetch('pi_quote_khala_default'),
          newId: () => 'fixed',
          signingSecret: SIGNING_SECRET,
          stripeSecretKey: 'sk_test_x',
        },
      ),
    )
    expect(response.status).toBe(402)
    expect(firstChallengeOpaque(response)).toMatchObject({
      model: KHALA_MODEL_ID,
      pi: 'pi_quote_khala_default',
    })
  })

  test('explicit unsupported models fail before a Stripe quote is created', async () => {
    const db = makeDb()
    let quoteCalls = 0
    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          body: JSON.stringify(mppBody('anthropic/claude-opus-4')),
        }),
        {
          completionDeps: completionDeps(db),
          db,
          enabled: true,
          fetch: async () => {
            quoteCalls += 1
            return new Response('{}', { status: 200 })
          },
          signingSecret: SIGNING_SECRET,
          stripeSecretKey: 'sk_test_x',
        },
      ),
    )
    expect(response.status).toBe(400)
    const body = (await response.json()) as {
      error: string
      supported_models?: ReadonlyArray<string>
    }
    expect(body.error).toBe('mpp_model_not_supported')
    expect(body.supported_models).toEqual([KHALA_MODEL_ID])
    expect(quoteCalls).toBe(0)
  })

  test('explicit raw GPT-OSS 120B fails before a Stripe quote even when Hydralisk is armed', async () => {
    const db = makeDb()
    let quoteCalls = 0
    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          body: JSON.stringify(mppBody(HYDRALISK_GPT_OSS_120B_MODEL_ID)),
        }),
        {
          completionDeps: completionDeps(db, {
            laneArming: resolveSupplyLaneArming(HYDRALISK_READY_ENV),
          }),
          db,
          enabled: true,
          fetch: async () => {
            quoteCalls += 1
            return new Response('{}', { status: 200 })
          },
          signingSecret: SIGNING_SECRET,
          stripeSecretKey: 'sk_test_x',
        },
      ),
    )
    expect(response.status).toBe(403)
    expect(await response.json()).toMatchObject({
      error: 'model_not_public',
      model: HYDRALISK_GPT_OSS_120B_MODEL_ID,
      supported_models: [KHALA_MODEL_ID],
    })
    expect(response.headers.get('www-authenticate')).toBeNull()
    expect(quoteCalls).toBe(0)
  })

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
          metadata: { model: KHALA_MODEL_ID },
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
    expect(metered[0]?.requestedModel).toBe(KHALA_MODEL_ID)

    const balance = await readAgentBalance(db, 'agent:mpp:pi_paid_1')
    expect(balance).not.toBeNull()
    expect(balance!.usdCreditMsat).toBeGreaterThan(0)
  })

  test('refuses a paid raw GPT-OSS 20B request before settlement or completion', async () => {
    const db = makeDb()
    const challenge = await cryptoChallenge(
      'pi_gptoss_paid_1',
      100,
      HYDRALISK_GPT_OSS_20B_MODEL_ID,
    )

    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db, {
        laneArming: resolveSupplyLaneArming(HYDRALISK_READY_ENV),
        registry: hydraliskRegistry(),
      }),
      db,
      enabled: true,
      fetch: settledCryptoFetch('pi_gptoss_paid_1', 100),
      nowIso: () => '2026-06-22T12:00:00.000Z',
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }

    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          body: JSON.stringify(mppBody(HYDRALISK_GPT_OSS_20B_MODEL_ID)),
          headers: {
            authorization: credentialHeader(challenge),
            'content-type': 'application/json',
          },
        }),
        deps,
      ),
    )

    expect(response.status).toBe(403)
    expect(response.headers.get('payment-receipt')).toBeNull()
    expect(await response.json()).toMatchObject({
      error: 'model_not_public',
      model: HYDRALISK_GPT_OSS_20B_MODEL_ID,
      supported_models: [KHALA_MODEL_ID],
    })
    const balance = await readAgentBalance(db, 'agent:mpp:pi_gptoss_paid_1')
    expect(balance).toBeNull()
  })

  test('refuses a paid raw GPT-OSS 120B request before settlement or completion', async () => {
    const db = makeDb()
    const challenge = await cryptoChallenge(
      'pi_gptoss_120b_paid_1',
      100,
      HYDRALISK_GPT_OSS_120B_MODEL_ID,
    )

    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db, {
        laneArming: resolveSupplyLaneArming(HYDRALISK_120B_READY_ENV),
        registry: hydralisk120bRegistry(),
      }),
      db,
      enabled: true,
      fetch: settledCryptoFetch('pi_gptoss_120b_paid_1', 100),
      nowIso: () => '2026-06-22T12:00:00.000Z',
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }

    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          body: JSON.stringify(mppBody(HYDRALISK_GPT_OSS_120B_MODEL_ID)),
          headers: {
            authorization: credentialHeader(challenge),
            'content-type': 'application/json',
          },
        }),
        deps,
      ),
    )

    expect(response.status).toBe(403)
    expect(response.headers.get('payment-receipt')).toBeNull()
    expect(await response.json()).toMatchObject({
      error: 'model_not_public',
      model: HYDRALISK_GPT_OSS_120B_MODEL_ID,
      supported_models: [KHALA_MODEL_ID],
    })
    const balance = await readAgentBalance(db, 'agent:mpp:pi_gptoss_120b_paid_1')
    expect(balance).toBeNull()
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

  test('a credential cannot be replayed into a raw GPT-OSS model request', async () => {
    const db = makeDb()
    let completionRan = false
    const challenge = await cryptoChallenge('pi_model_mismatch_1', 100)
    const spyMetering = () =>
      Effect.sync(() => {
        completionRan = true
        return { metered: false, receiptRef: null }
      })
    const response = await run(
      handleMppChatCompletions(
        mppRequest({
          body: JSON.stringify(mppBody(HYDRALISK_GPT_OSS_20B_MODEL_ID)),
          headers: { authorization: credentialHeader(challenge) },
        }),
        {
          completionDeps: { ...completionDeps(db), meteringHook: spyMetering },
          db,
          enabled: true,
          fetch: settledCryptoFetch('pi_model_mismatch_1'),
          signingSecret: SIGNING_SECRET,
          stripeSecretKey: 'sk_test_x',
        },
      ),
    )
    expect(response.status).toBe(403)
    expect(response.headers.get('payment-receipt')).toBeNull()
    expect(await response.json()).toMatchObject({
      error: 'model_not_public',
      model: HYDRALISK_GPT_OSS_20B_MODEL_ID,
    })
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

// ---- Lightning rail (draft-lightning-charge-00) ----

// A known preimage and the deterministic invoice/paymentHash it pays. The fake
// issuer returns this invoice; the test pays with this preimage.
const LIGHTNING_PREIMAGE =
  '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff'
const LIGHTNING_INVOICE = `lnbc100n1p${'a'.repeat(48)}`

const lightningPaymentHash = (): Promise<string> =>
  sha256Hex(
    Uint8Array.from(
      (LIGHTNING_PREIMAGE.match(/.{2}/g) ?? []).map(b => Number.parseInt(b, 16)),
    ),
  )

// A fake invoice issuer returning a fixed invoice whose paymentHash is the
// sha256 of LIGHTNING_PREIMAGE. `invoiceExpiresAt` is overridable for the
// expiry test; `fail` makes the issuer fail (honesty-gate test).
const fakeLightningIssuer = (
  opts: { invoiceExpiresAt?: string; fail?: boolean } = {},
): { mint: MintLightningInvoice; calls: () => number } => {
  let calls = 0
  return {
    calls: () => calls,
    mint: () =>
      Effect.gen(function* () {
        calls += 1
        if (opts.fail === true) {
          return yield* Effect.fail(
            new LightningInvoiceError('provider_unavailable'),
          )
        }
        const paymentHash = yield* Effect.promise(lightningPaymentHash)
        const invoice: LightningInvoice = {
          bolt11: LIGHTNING_INVOICE,
          network: 'mainnet',
          paymentHash,
          ...(opts.invoiceExpiresAt === undefined
            ? {}
            : { invoiceExpiresAt: opts.invoiceExpiresAt }),
        }
        return invoice
      }),
  }
}

// Parse one `WWW-Authenticate: Payment ...` header for `method` from a 402 and
// reconstruct the issued challenge shape so a retry can echo it.
const lightningChallengeFrom402 = (response: Response): MppChallenge | undefined => {
  const headers = response.headers.get('www-authenticate') ?? ''
  // getSetCookie-style: multiple headers are comma-joined; split on the scheme.
  const parts = headers.split(/,\s*(?=Payment\s)/i).map(h => h.trim())
  for (const part of parts) {
    const params: Record<string, string> = {}
    for (const m of part.matchAll(/(\w+)="((?:[^"\\]|\\.)*)"/g)) {
      params[m[1]!] = m[2]!.replace(/\\"/g, '"')
    }
    if (params.method === 'lightning') {
      return {
        amountCents: 0,
        currency: 'sat',
        expires: params.expires ?? '',
        id: params.id!,
        intent: 'charge',
        method: 'lightning',
        opaque: params.opaque,
        realm: params.realm!,
        request: params.request!,
      }
    }
  }
  return undefined
}

const lightningQuoteFetch = (id = 'pi_quote_ln'): StripeFetch => async () =>
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

describe('isKhalaMppLightningEnabled flag', () => {
  test('default OFF; on for explicit truthy tokens', () => {
    expect(isKhalaMppLightningEnabled(undefined)).toBe(false)
    expect(isKhalaMppLightningEnabled('false')).toBe(false)
    expect(isKhalaMppLightningEnabled('1')).toBe(true)
    expect(isKhalaMppLightningEnabled('on')).toBe(true)
  })
})

describe('MPP endpoint — Lightning rail (Bitcoin-first)', () => {
  test('OFFER ORDERING: armed Lightning challenge is emitted FIRST', async () => {
    const db = makeDb()
    const issuer = fakeLightningIssuer()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: lightningQuoteFetch(),
        lightningEnabled: true,
        mintLightningInvoice: issuer.mint,
        newId: () => 'fixed',
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(402)
    // The FIRST www-authenticate header is the lightning challenge.
    const wwwAuth = response.headers.get('www-authenticate') ?? ''
    const firstChallenge = wwwAuth.split(/,\s*(?=Payment\s)/i)[0] ?? ''
    expect(firstChallenge).toContain('method="lightning"')
    // The problem body lists lightning FIRST too.
    const problem = (await response.json()) as {
      challenges: Array<{ method: string }>
    }
    expect(problem.challenges[0]?.method).toBe('lightning')
    expect(issuer.calls()).toBe(1)
  })

  test('INERT: flag OFF => no lightning challenge even with an issuer present', async () => {
    const db = makeDb()
    const issuer = fakeLightningIssuer()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: lightningQuoteFetch(),
        lightningEnabled: false,
        mintLightningInvoice: issuer.mint,
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(402)
    expect(response.headers.get('www-authenticate') ?? '').not.toContain(
      'method="lightning"',
    )
    expect(issuer.calls()).toBe(0)
  })

  test('HONESTY GATE: flag ON but no issuer => no lightning challenge', async () => {
    const db = makeDb()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: lightningQuoteFetch(),
        lightningEnabled: true,
        // mintLightningInvoice omitted => rail not advertised
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(402)
    expect(response.headers.get('www-authenticate') ?? '').not.toContain(
      'method="lightning"',
    )
  })

  test('PAY LOOP: a valid preimage settles, mints Bitcoin-origin credit, serves 200 + receipt', async () => {
    const db = makeDb()
    const issuer = fakeLightningIssuer()
    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db),
      db,
      enabled: true,
      fetch: lightningQuoteFetch(),
      lightningEnabled: true,
      mintLightningInvoice: issuer.mint,
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }
    // 1) Drive a 402 and grab the issued lightning challenge.
    const challengeResp = await run(handleMppChatCompletions(mppRequest(), deps))
    const challenge = lightningChallengeFrom402(challengeResp)!
    expect(challenge).toBeDefined()
    // 2) Retry with the preimage credential.
    const header = credentialHeader(challenge, { preimage: LIGHTNING_PREIMAGE })
    const served = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: header } }),
        deps,
      ),
    )
    expect(served.status).toBe(200)
    // Receipt: method lightning, reference = paymentHash (NEVER the preimage).
    const receipt = served.headers.get('payment-receipt')
    expect(receipt).toBeTruthy()
    const decoded = JSON.parse(
      Buffer.from(receipt!.replace(/-/g, '+').replace(/_/g, '/'), 'base64').toString(),
    ) as { method: string; reference: string }
    expect(decoded.method).toBe('lightning')
    const paymentHash = await lightningPaymentHash()
    expect(decoded.reference).toBe(paymentHash)
    expect(decoded.reference).not.toBe(LIGHTNING_PREIMAGE)
    // Bitcoin-origin credit: balance_msat credited, usd_credit_msat NOT bumped.
    const row = (await (
      db.prepare(
        'SELECT balance_msat, usd_credit_msat FROM agent_balances WHERE actor_ref = ?',
      ).bind(`agent:mpp-lightning:${paymentHash}`) as unknown as {
        first: () => Promise<{ balance_msat: number; usd_credit_msat: number } | null>
      }
    ).first())
    expect(row).not.toBeNull()
    expect(Number(row?.usd_credit_msat)).toBe(0)
    expect(Number(row?.balance_msat)).toBeGreaterThanOrEqual(0)
    // The pay-in was recorded as lightning_charge (Bitcoin-origin), not usd_credit_grant.
    const payIn = (await (
      db.prepare(
        'SELECT pay_in_type FROM pay_ins WHERE payer_ref = ?',
      ).bind(`agent:mpp-lightning:${paymentHash}`) as unknown as {
        first: () => Promise<{ pay_in_type: string } | null>
      }
    ).first())
    expect(payIn?.pay_in_type).toBe('lightning_charge')
  })

  test('REPLAY: a second use of the same preimage is refused (consume-once)', async () => {
    const db = makeDb()
    const issuer = fakeLightningIssuer()
    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db),
      db,
      enabled: true,
      fetch: lightningQuoteFetch(),
      lightningEnabled: true,
      mintLightningInvoice: issuer.mint,
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }
    const challengeResp = await run(handleMppChatCompletions(mppRequest(), deps))
    const challenge = lightningChallengeFrom402(challengeResp)!
    const header = credentialHeader(challenge, { preimage: LIGHTNING_PREIMAGE })
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
      'preimage_replayed',
    )
  })

  test('INVALID PREIMAGE: a wrong preimage is rejected with invalid_preimage', async () => {
    const db = makeDb()
    const issuer = fakeLightningIssuer()
    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db),
      db,
      enabled: true,
      fetch: lightningQuoteFetch(),
      lightningEnabled: true,
      mintLightningInvoice: issuer.mint,
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }
    const challengeResp = await run(handleMppChatCompletions(mppRequest(), deps))
    const challenge = lightningChallengeFrom402(challengeResp)!
    const wrong =
      'ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100'
    const header = credentialHeader(challenge, { preimage: wrong })
    const served = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: header } }),
        deps,
      ),
    )
    expect(served.status).toBe(402)
    expect(((await served.json()) as { reason?: string }).reason).toBe(
      'invalid_preimage',
    )
  })

  test('TAMPERED CHALLENGE: a forged HMAC id is rejected (re-issues 402, no serve)', async () => {
    const db = makeDb()
    const issuer = fakeLightningIssuer()
    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db),
      db,
      enabled: true,
      fetch: lightningQuoteFetch(),
      lightningEnabled: true,
      mintLightningInvoice: issuer.mint,
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }
    const challengeResp = await run(handleMppChatCompletions(mppRequest(), deps))
    const challenge = lightningChallengeFrom402(challengeResp)!
    const tampered: MppChallenge = { ...challenge, id: 'forged-id' }
    const header = credentialHeader(tampered, { preimage: LIGHTNING_PREIMAGE })
    const served = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: header } }),
        deps,
      ),
    )
    // Verification fails => fresh 402 (never served).
    expect(served.status).toBe(402)
  })

  test('EXPIRED INVOICE: a past invoice expiry is rejected (earlier-of expiry)', async () => {
    const db = makeDb()
    // Issue an invoice that has ALREADY expired per its BOLT11 expiry.
    const issuer = fakeLightningIssuer({
      invoiceExpiresAt: '2000-01-01T00:00:00.000Z',
    })
    const deps: MppChatCompletionsDeps = {
      completionDeps: completionDeps(db),
      db,
      enabled: true,
      fetch: lightningQuoteFetch(),
      lightningEnabled: true,
      mintLightningInvoice: issuer.mint,
      // now is well past the invoice expiry but the challenge TTL would be future
      nowMs: () => Date.parse('2026-06-22T12:00:00.000Z'),
      signingSecret: SIGNING_SECRET,
      stripeSecretKey: 'sk_test_x',
    }
    const challengeResp = await run(handleMppChatCompletions(mppRequest(), deps))
    const challenge = lightningChallengeFrom402(challengeResp)!
    // The challenge `expires` is clamped to the (past) invoice expiry, so the
    // generic verify already rejects on challenge expiry => fresh 402.
    const header = credentialHeader(challenge, { preimage: LIGHTNING_PREIMAGE })
    const served = await run(
      handleMppChatCompletions(
        mppRequest({ headers: { authorization: header } }),
        deps,
      ),
    )
    expect(served.status).toBe(402)
  })

  // ---- PER-RAIL ISOLATION / NO-HANG (root-cause regression) ----
  //
  // The observed prod hang: arming KHALA_MPP_LIGHTNING_ENABLED made a slow/cold
  // MDK-sidecar mint block the ENTIRE 402 handler (crypto + card timed out too)
  // because the Lightning leg ran first and synchronously. These tests prove the
  // Lightning leg is now fully isolated: a hung/slow mint is bounded and dropped,
  // and the endpoint still returns a 402 carrying the crypto (+ card) challenges.

  test('HANGING MINT: a never-resolving mint is bounded => 402 still carries crypto + card, lightning DROPPED', async () => {
    const db = makeDb()
    let mintStarted = false
    // A mint that NEVER resolves — models a cold/blocked MDK sidecar container.
    const hangingMint: MintLightningInvoice = () =>
      Effect.callback<LightningInvoice, LightningInvoiceError>(() => {
        mintStarted = true
        // never resume => the effect hangs until interrupted by the timeout.
      })
    const response = await run(
      Effect.gen(function* () {
        const fiber = yield* Effect.forkChild(
          handleMppChatCompletions(mppRequest(), {
            completionDeps: completionDeps(db),
            db,
            enabled: true,
            fetch: lightningQuoteFetch(),
            lightningEnabled: true,
            mintLightningInvoice: hangingMint,
            newId: () => 'fixed',
            signingSecret: SIGNING_SECRET,
            // card rail armed too, to prove BOTH non-lightning rails survive.
            stripeNetworkProfileId: 'profile_test',
            stripeSecretKey: 'sk_test_x',
          }),
        )
        // Advance well past BOTH the inner mint timeout and the outer leg guard.
        yield* TestClock.adjust(10_000)
        return yield* Fiber.join(fiber)
      }).pipe(Effect.provide(TestClock.layer())),
    )
    expect(mintStarted).toBe(true)
    // The endpoint still returns a 402 (NOT a hang, NOT a 503).
    expect(response.status).toBe(402)
    const wwwAuth = response.headers.get('www-authenticate') ?? ''
    // Lightning is DROPPED (honesty gate) but the other rails are present.
    expect(wwwAuth).not.toContain('method="lightning"')
    expect(wwwAuth).toContain('method="base"')
    expect(wwwAuth).toContain('method="stripe"')
    const problem = (await response.json()) as {
      challenges: Array<{ method: string }>
    }
    expect(problem.challenges.some(c => c.method === 'lightning')).toBe(false)
    expect(problem.challenges.some(c => c.method === 'base')).toBe(true)
    expect(problem.challenges.some(c => c.method === 'stripe')).toBe(true)
  })

  test('FAILING MINT: a fast typed failure drops ONLY lightning; crypto still present', async () => {
    const db = makeDb()
    const issuer = fakeLightningIssuer({ fail: true })
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: lightningQuoteFetch(),
        lightningEnabled: true,
        mintLightningInvoice: issuer.mint,
        newId: () => 'fixed',
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(402)
    expect(issuer.calls()).toBe(1)
    const wwwAuth = response.headers.get('www-authenticate') ?? ''
    expect(wwwAuth).not.toContain('method="lightning"')
    expect(wwwAuth).toContain('method="base"')
  })

  test('FAST SUCCESS: a fast mint => lightning challenge present and FIRST, crypto still present', async () => {
    const db = makeDb()
    const issuer = fakeLightningIssuer()
    const response = await run(
      handleMppChatCompletions(mppRequest(), {
        completionDeps: completionDeps(db),
        db,
        enabled: true,
        fetch: lightningQuoteFetch(),
        lightningEnabled: true,
        mintLightningInvoice: issuer.mint,
        newId: () => 'fixed',
        signingSecret: SIGNING_SECRET,
        stripeSecretKey: 'sk_test_x',
      }),
    )
    expect(response.status).toBe(402)
    expect(issuer.calls()).toBe(1)
    const wwwAuth = response.headers.get('www-authenticate') ?? ''
    // Lightning is the FIRST presented challenge (Bitcoin-first).
    const firstChallenge = wwwAuth.split(/,\s*(?=Payment\s)/i)[0] ?? ''
    expect(firstChallenge).toContain('method="lightning"')
    // ...and the crypto rail is still present.
    expect(wwwAuth).toContain('method="base"')
    const problem = (await response.json()) as {
      challenges: Array<{ method: string }>
    }
    expect(problem.challenges[0]?.method).toBe('lightning')
    expect(problem.challenges.some(c => c.method === 'base')).toBe(true)
  })

  // ---- BUDGET RAISE (#6049): cover REAL warm Spark mint latency ----
  //
  // The real warm Spark `/spark/funding-invoice` mint subrequest returns 200 in
  // ~3.76–3.95s (observed in prod `wrangler tail`), and the worker-side
  // round-trip lands at/just over 4s — so the old 1.2s issuer cap + 2.5s leg
  // guard (and even a first-raise 4s/4.5s) tripped and the honesty gate dropped
  // Lightning from every prod 402. The budgets are now
  // `SPARK_LIGHTNING_MINT_TIMEOUT_MS = 6000` and `LIGHTNING_LEG_GUARD_MS = 6500`.
  // These two tests pin the new semantics: a mint that completes within the
  // budget SUCCEEDS and surfaces Lightning FIRST; a mint that exceeds the outer
  // guard still DROPS only Lightning (crypto + card stay fast, no hang).

  test('SLOW-BUT-IN-BUDGET MINT (~4s, real warm Spark): lightning SURFACES first, crypto + card present', async () => {
    const db = makeDb()
    let mintStarted = false
    // Models a real warm Spark mint: resolves after ~4s, under the 6.5s outer
    // leg guard. The honesty gate must now ACCEPT it (old 1.2s/4s caps would
    // drop this real-world latency).
    const slowMint: MintLightningInvoice = () =>
      Effect.gen(function* () {
        mintStarted = true
        yield* Effect.sleep(Duration.millis(4_000))
        const paymentHash = yield* Effect.promise(lightningPaymentHash)
        return {
          bolt11: LIGHTNING_INVOICE,
          network: 'mainnet' as const,
          paymentHash,
        }
      })
    const response = await run(
      Effect.gen(function* () {
        const fiber = yield* Effect.forkChild(
          handleMppChatCompletions(mppRequest(), {
            completionDeps: completionDeps(db),
            db,
            enabled: true,
            fetch: lightningQuoteFetch(),
            lightningEnabled: true,
            mintLightningInvoice: slowMint,
            newId: () => 'fixed',
            signingSecret: SIGNING_SECRET,
            stripeNetworkProfileId: 'profile_test',
            stripeSecretKey: 'sk_test_x',
          }),
        )
        // Advance past the ~4s mint but still under the 6.5s guard.
        yield* TestClock.adjust(5_000)
        return yield* Fiber.join(fiber)
      }).pipe(Effect.provide(TestClock.layer())),
    )
    expect(mintStarted).toBe(true)
    expect(response.status).toBe(402)
    const wwwAuth = response.headers.get('www-authenticate') ?? ''
    // Lightning is the FIRST presented challenge (Bitcoin-first) — it SURVIVED.
    const firstChallenge = wwwAuth.split(/,\s*(?=Payment\s)/i)[0] ?? ''
    expect(firstChallenge).toContain('method="lightning"')
    expect(wwwAuth).toContain('method="base"')
    expect(wwwAuth).toContain('method="stripe"')
    const problem = (await response.json()) as {
      challenges: Array<{ method: string }>
    }
    expect(problem.challenges[0]?.method).toBe('lightning')
    expect(problem.challenges.some(c => c.method === 'base')).toBe(true)
    expect(problem.challenges.some(c => c.method === 'stripe')).toBe(true)
  })

  test('OVER-GUARD MINT (~8s, exceeds 6.5s guard): lightning DROPPED, crypto + card still present, no hang', async () => {
    const db = makeDb()
    let mintStarted = false
    // Models a mint that exceeds even the raised 6.5s outer leg guard. Per-rail
    // isolation (#6149) must still hold: the leg is interrupted and ONLY Lightning
    // is dropped — crypto + card are unaffected and the 402 never hangs.
    const tooSlowMint: MintLightningInvoice = () =>
      Effect.gen(function* () {
        mintStarted = true
        yield* Effect.sleep(Duration.millis(8_000))
        const paymentHash = yield* Effect.promise(lightningPaymentHash)
        return {
          bolt11: LIGHTNING_INVOICE,
          network: 'mainnet' as const,
          paymentHash,
        }
      })
    const response = await run(
      Effect.gen(function* () {
        const fiber = yield* Effect.forkChild(
          handleMppChatCompletions(mppRequest(), {
            completionDeps: completionDeps(db),
            db,
            enabled: true,
            fetch: lightningQuoteFetch(),
            lightningEnabled: true,
            mintLightningInvoice: tooSlowMint,
            newId: () => 'fixed',
            signingSecret: SIGNING_SECRET,
            stripeNetworkProfileId: 'profile_test',
            stripeSecretKey: 'sk_test_x',
          }),
        )
        // Advance well past the 6.5s guard AND the 8s mint.
        yield* TestClock.adjust(10_000)
        return yield* Fiber.join(fiber)
      }).pipe(Effect.provide(TestClock.layer())),
    )
    expect(mintStarted).toBe(true)
    expect(response.status).toBe(402)
    const wwwAuth = response.headers.get('www-authenticate') ?? ''
    expect(wwwAuth).not.toContain('method="lightning"')
    expect(wwwAuth).toContain('method="base"')
    expect(wwwAuth).toContain('method="stripe"')
    const problem = (await response.json()) as {
      challenges: Array<{ method: string }>
    }
    expect(problem.challenges.some(c => c.method === 'lightning')).toBe(false)
    expect(problem.challenges.some(c => c.method === 'base')).toBe(true)
    expect(problem.challenges.some(c => c.method === 'stripe')).toBe(true)
  })

  test('OUTER GUARD aborts the caller signal when MDK fallback starts after a slow primary', async () => {
    const db = makeDb()
    let fallbackSignal: AbortSignal | undefined
    const primary: MintLightningInvoice = () =>
      Effect.sleep(Duration.millis(6_000)).pipe(
        Effect.andThen(
          Effect.fail(new LightningInvoiceError('provider_unavailable')),
        ),
      )
    const fallback: MintLightningInvoice = input => {
      fallbackSignal = input.abortSignal
      return Effect.never
    }
    const response = await run(
      Effect.gen(function* () {
        const fiber = yield* Effect.forkChild(
          handleMppChatCompletions(mppRequest(), {
            completionDeps: completionDeps(db),
            db,
            enabled: true,
            fetch: lightningQuoteFetch(),
            lightningEnabled: true,
            mintLightningInvoice: makeFallbackLightningInvoiceIssuer(
              primary,
              fallback,
            ),
            newId: () => 'fixed',
            signingSecret: SIGNING_SECRET,
            stripeNetworkProfileId: 'profile_test',
            stripeSecretKey: 'sk_test_x',
          }),
        )
        yield* TestClock.adjust(7_000)
        return yield* Fiber.join(fiber)
      }).pipe(Effect.provide(TestClock.layer())),
    )
    expect(response.status).toBe(402)
    expect(fallbackSignal).toBeInstanceOf(AbortSignal)
    expect(fallbackSignal?.aborted).toBe(true)
    const wwwAuth = response.headers.get('www-authenticate') ?? ''
    expect(wwwAuth).not.toContain('method="lightning"')
    expect(wwwAuth).toContain('method="base"')
    expect(wwwAuth).toContain('method="stripe"')
  })
})
