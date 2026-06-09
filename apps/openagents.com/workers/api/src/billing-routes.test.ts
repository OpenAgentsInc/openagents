import { describe, expect, test } from 'vitest'

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
  fulfillCheckoutSession: () => Promise.resolve({ ok: true }),
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
