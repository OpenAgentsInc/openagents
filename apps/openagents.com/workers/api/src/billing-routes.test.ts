import { describe, expect, test } from 'vitest'

import { applyStripeCheckoutCredit } from './billing'
import { makeBillingApiHandlers } from './billing-routes'

type TestSession = Readonly<{ user: Readonly<{ userId: string }> }>
type QueryBinding = Readonly<{
  query: string
  values: ReadonlyArray<unknown>
}>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 0,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 0,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const jsonFixture = <T>(value: unknown): T => JSON.parse(JSON.stringify(value))

const makeBillingD1 = (): Readonly<{
  bindings: Array<QueryBinding>
  db: D1Database
}> => {
  const bindings: Array<QueryBinding> = []
  const db: D1Database = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare: (query: string) => {
      let values: ReadonlyArray<unknown> = []

      function raw<T = unknown[]>(options: {
        columnNames: true
      }): Promise<[Array<string>, ...Array<T>]>
      function raw<T = unknown[]>(options?: {
        columnNames?: false
      }): Promise<Array<T>>
      function raw<T = unknown[]>(options?: {
        columnNames?: boolean
      }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
        return options?.columnNames === true
          ? Promise.resolve([[]])
          : Promise.resolve([])
      }

      const statement: D1PreparedStatement = {
        all: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })

          return Promise.resolve(makeResult<T>())
        },
        bind: (...nextValues: ReadonlyArray<unknown>) => {
          values = nextValues

          return statement
        },
        first: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })

          if (query.includes('SELECT status FROM billing_accounts')) {
            return Promise.resolve(jsonFixture<T>({ status: 'active' }))
          }

          if (query.includes('SUM(amount_cents) AS balance_cents')) {
            return Promise.resolve(jsonFixture<T>({ balance_cents: 0 }))
          }

          return Promise.resolve(null)
        },
        raw,
        run: <T = Record<string, unknown>>() => {
          bindings.push({ query, values })

          return Promise.resolve(makeResult<T>())
        },
      }

      return statement
    },
    withSession: () => ({
      batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
        Promise.all(statements.map(statement => statement.run<T>())),
      getBookmark: () => null,
      prepare: query => db.prepare(query),
    }),
  }

  return { bindings, db }
}

type TestStripe = NonNullable<
  Parameters<typeof makeBillingApiHandlers<TestSession, never>>[0]['stripe']
>

const defaultStripe: TestStripe = {
  createCreditCheckout: () =>
    Promise.resolve({ checkoutUrl: 'https://checkout.stripe.test/session' }),
  createSetupIntent: () =>
    Promise.resolve({
      clientSecret: 'seti_secret_test',
      setupIntentId: 'seti_test',
      status: 'requires_payment_method',
    }),
  fulfillCheckoutSession: () => Promise.resolve({ ok: true }),
  saveSetupIntentPaymentMethod: () => Promise.resolve({ ok: true }),
  chargeAutoTopUp: input =>
    Promise.resolve({
      billing: {
        activeRuns: [],
        autoTopUp: {
          events: [],
          policy: {
            amountCents: 2500,
            amountFormatted: '$25.00',
            enabled: true,
            monthlyCapCents: 10000,
            monthlyCapFormatted: '$100.00',
            pauseReason: null,
            spentThisMonthCents: 2500,
            spentThisMonthFormatted: '$25.00',
            status: 'active',
            thresholdCents: 500,
            thresholdFormatted: '$5.00',
            updatedAt: '2026-06-11T00:00:00.000Z',
          },
          savedPaymentMethod: null,
        },
        balanceCents: 2500,
        balanceFormatted: '$25.00',
        currency: 'USD',
        minimumRunCreditCents: 5,
        minimumRunCreditFormatted: '$0.05',
        rates: {
          codexCentsPerThousandTokens: 2,
          containerCentsPerMinute: 5,
        },
        recentEntries: [],
        status: 'active',
      },
      status: input.idempotencyKey === 'skip' ? 'skipped' : 'succeeded',
    }),
  processWebhook: () =>
    Promise.resolve({
      eventId: 'evt_test',
      status: 'processed',
      type: 'checkout.session.completed',
    }),
}

function makeHandlers(
  session: TestSession | null = {
    user: { userId: 'github:1' },
  },
  stripe: TestStripe | null = defaultStripe,
) {
  return makeBillingApiHandlers({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
    ...(stripe === null ? {} : { stripe }),
  })
}

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

describe('billing API handlers', () => {
  test('returns unauthorized without a browser session', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers(null).handleBillingSummaryApi(
      new Request('https://openagents.com/api/billing/summary'),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
    expect(response.status).toBe(401)
  })

  test('summary renders the server credit catalog from STRIPE_CREDIT_PACKAGES_JSON', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers().handleBillingSummaryApi(
      new Request('https://openagents.com/api/billing/summary'),
      {
        OPENAGENTS_DB: db,
        STRIPE_CREDIT_PACKAGES_JSON: JSON.stringify([
          {
            amountCents: 1000,
            id: 'credits_10',
            label: '$10 credits',
            priceId: 'price_10',
          },
          {
            amountCents: 5000,
            bonusCents: 500,
            id: 'credits_50',
            label: '$50 credits',
            priceId: 'price_50',
          },
        ]),
      },
      executionContext(),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      billing: {
        packages: ReadonlyArray<{
          amountCents: number
          amountFormatted: string
          bonusCents: number
          bonusFormatted: string
          creditsExpire: false
          currency: string
          id: string
          label: string
          paidAmountCents: number
          paidAmountFormatted: string
        }>
      }
    }
    // The UI buys against these exact ids, so they must equal the server
    // catalog ids the checkout endpoint accepts (closes the hardcoded gap).
    expect(body.billing.packages).toEqual([
      {
        amountCents: 1000,
        amountFormatted: '$10.00',
        bonusCents: 0,
        bonusFormatted: '$0.00',
        creditsExpire: false,
        currency: 'USD',
        id: 'credits_10',
        label: '$10 credits',
        paidAmountCents: 1000,
        paidAmountFormatted: '$10.00',
      },
      {
        amountCents: 5500,
        amountFormatted: '$55.00',
        bonusCents: 500,
        bonusFormatted: '$5.00',
        creditsExpire: false,
        currency: 'USD',
        id: 'credits_50',
        label: '$50 credits',
        paidAmountCents: 5000,
        paidAmountFormatted: '$50.00',
      },
    ])
  })

  test('summary returns an empty catalog when Stripe is unconfigured', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers().handleBillingSummaryApi(
      new Request('https://openagents.com/api/billing/summary'),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      billing: { packages: ReadonlyArray<unknown> }
    }
    expect(body.billing.packages).toEqual([])
  })

  test('Stripe volume prepay grants paid and bonus credits atomically', async () => {
    const { bindings, db } = makeBillingD1()

    await applyStripeCheckoutCredit(
      db,
      {
        amountCents: 50_000,
        bonusCents: 5_000,
        packageId: 'prepay_500',
        sessionId: 'cs_test_volume',
        userId: 'github:1',
      },
      {
        nowIso: () => '2026-07-02T00:00:00.000Z',
        randomId: prefix => `${prefix}_fixed`,
      },
    )

    const stripeCreditInsert = bindings.find(
      binding =>
        binding.query.includes("'stripe_checkout'") &&
        binding.query.includes('billing_ledger_entries'),
    )

    expect(stripeCreditInsert?.values[2]).toBe(
      'Stripe volume prepay credit purchase',
    )
    expect(stripeCreditInsert?.values[3]).toBe(55_000)
    expect(stripeCreditInsert?.values[5]).toBe(55_000)
    expect(JSON.parse(String(stripeCreditInsert?.values[7]))).toEqual({
      bonusCents: 5_000,
      creditsExpire: false,
      packageId: 'prepay_500',
      paidAmountCents: 50_000,
      sessionId: 'cs_test_volume',
      totalCreditCents: 55_000,
    })
  })

  test('checkout rejects a missing packageId instead of defaulting', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers().handleBillingCheckoutApi(
      new Request('https://openagents.com/api/billing/checkout', {
        body: JSON.stringify({}),
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toMatchObject({
      error: 'package_required',
    })
  })

  test('requires a coupon code and refreshes session cookies', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers().handleBillingCouponRedeemApi(
      new Request('https://openagents.com/api/billing/coupons/redeem', {
        body: JSON.stringify({}),
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(400)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    await expect(response.json()).resolves.toMatchObject({
      error: 'coupon_code_required',
      message: 'Enter a coupon code.',
    })
  })

  test('returns checkout URL payload', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers().handleBillingCheckoutApi(
      new Request('https://openagents.com/api/billing/checkout', {
        body: JSON.stringify({ packageId: 'pro' }),
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      checkoutUrl: 'https://checkout.stripe.test/session',
      packageId: 'pro',
      status: 'checkout_created',
    })
  })

  test('passes business signup id into checkout creation when provided', async () => {
    const { db } = makeBillingD1()
    const calls: Array<Parameters<TestStripe['createCreditCheckout']>[0]> = []
    const response = await makeHandlers({
      user: { userId: 'github:1' },
    }, {
      ...defaultStripe,
      createCreditCheckout: input => {
        calls.push(input)

        return Promise.resolve({
          checkoutUrl: 'https://checkout.stripe.test/session',
        })
      },
    }).handleBillingCheckoutApi(
      new Request('https://openagents.com/api/billing/checkout', {
        body: JSON.stringify({
          businessSignupId: 'business_signup_001',
          packageId: 'pro',
        }),
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(200)
    expect(calls[0]?.businessKickoff).toEqual({
      signupId: 'business_signup_001',
    })
  })

  test('creates Stripe SetupIntent payload for card-on-file setup', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers().handleBillingStripeSetupIntentApi(
      new Request('https://openagents.com/api/billing/stripe/setup-intents', {
        body: '{}',
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      clientSecret: 'seti_secret_test',
      setupIntentId: 'seti_test',
    })
  })

  test('saves bounded auto top-up policy', async () => {
    const { bindings, db } = makeBillingD1()
    const response = await makeHandlers().handleBillingAutoTopUpPolicyApi(
      new Request('https://openagents.com/api/billing/auto-top-up-policy', {
        body: JSON.stringify({
          amountCents: 2500,
          enabled: true,
          monthlyCapCents: 10000,
          thresholdCents: 500,
        }),
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      message: 'Auto top-up enabled.',
    })
    expect(
      bindings.some(binding =>
        binding.query.includes('billing_auto_top_up_policies'),
      ),
    ).toBe(true)
  })

  test('runs auto top-up trigger through Stripe service', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers().handleBillingAutoTopUpRunApi(
      new Request('https://openagents.com/api/billing/auto-top-up/run', {
        body: JSON.stringify({}),
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      message: 'Auto top-up completed.',
      status: 'succeeded',
    })
  })

  test('returns unavailable when Stripe config is missing', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers(
      undefined,
      null,
    ).handleBillingCheckoutApi(
      new Request('https://openagents.com/api/billing/checkout', {
        body: JSON.stringify({ packageId: 'starter' }),
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
      executionContext(),
    )

    expect(response.status).toBe(503)
    await expect(response.json()).resolves.toMatchObject({
      error: 'stripe_unconfigured',
    })
  })

  test('accepts Stripe webhook processing without a browser session', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers(null).handleBillingStripeWebhookApi(
      new Request('https://openagents.com/api/billing/stripe/webhook', {
        body: '{}',
        headers: { 'Stripe-Signature': 'sig_test' },
        method: 'POST',
      }),
      { OPENAGENTS_DB: db },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      result: { status: 'processed' },
    })
  })

  test('redirects checkout returns to clean billing URL', async () => {
    const { db } = makeBillingD1()
    const response = await makeHandlers(
      null,
    ).handleBillingStripeCheckoutReturnApi(
      new Request(
        'https://openagents.com/api/billing/stripe/checkout-return?session_id=cs_test',
      ),
      { OPENAGENTS_DB: db },
    )

    expect(response.status).toBe(303)
    expect(response.headers.get('location')).toBe(
      'https://openagents.com/billing',
    )
  })
})
