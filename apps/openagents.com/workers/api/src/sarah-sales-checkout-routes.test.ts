import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOperatorSarahSalesCheckoutRoutes } from './sarah-sales-checkout-routes'

type TestEnv = Readonly<{
  OPENAGENTS_APP_URL: string
  OPENAGENTS_DB: D1Database
  SARAH_SALES_CHECKOUT_TEST_MODE?: string | undefined
}>

const packageCatalog = [
  {
    id: 'quick_win_credit_pack',
    paidAmountCents: 100_000,
  },
]

const makeRequest = (body: Record<string, unknown>, token = 'test-admin') =>
  new Request('https://openagents.com/api/operator/business/sarah-checkout-links', {
    body: JSON.stringify(body),
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })

const validBody = {
  amountUsdCents: 100_000,
  buyerUserId: 'github:buyer-1',
  contactEmail: 'buyer@example.com',
  dealRuleRefs: ['rule.credit_volume.usd_1000_2999.bonus_10pct'],
  packageId: 'quick_win_credit_pack',
  quoteRef: 'sarah_quote.1234567890abcdef12345678',
  signupId: 'business_signup_001',
  sourceRef: 'sarah.s6_checkout.test',
}

const runRoute = async (
  request: Request,
  overrides: Partial<TestEnv> = {},
  createCreditCheckout?: Parameters<
    typeof makeOperatorSarahSalesCheckoutRoutes<TestEnv>
  >[0]['createCreditCheckout'],
): Promise<Response> => {
  const routes = makeOperatorSarahSalesCheckoutRoutes<TestEnv>({
    appOrigin: () => 'https://openagents.com',
    ...(createCreditCheckout === undefined ? {} : { createCreditCheckout }),
    makeDb: env => env.OPENAGENTS_DB,
    nowIso: () => '2026-07-08T00:00:00.000Z',
    readCreditPackages: () => packageCatalog,
    requireAdminApiToken: async authRequest =>
      authRequest.headers.get('authorization') === 'Bearer test-admin',
  })
  const routed = routes.routeOperatorSarahSalesCheckoutRequest(
    request,
    {
      OPENAGENTS_APP_URL: 'https://openagents.com',
      OPENAGENTS_DB: {} as D1Database,
      ...overrides,
    },
    {} as ExecutionContext,
  )

  if (routed === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(routed)
}

describe('Sarah sales checkout operator route', () => {
  test('requires the operator admin bearer token', async () => {
    const response = await runRoute(makeRequest(validBody, 'wrong-token'))

    expect(response.status).toBe(401)
    expect(await response.json()).toEqual({ error: 'unauthorized' })
  })

  test('returns a no-money test-mode checkout receipt when armed', async () => {
    const response = await runRoute(makeRequest(validBody), {
      SARAH_SALES_CHECKOUT_TEST_MODE: '1',
    })
    const body = await response.json()

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      amountUsdCents: 100_000,
      checkoutUrl: expect.stringContaining(
        'https://openagents.com/business?sarah_checkout_ref=',
      ),
      mode: 'test',
      moneyMovement: 'none',
      packageId: 'quick_win_credit_pack',
      quoteRef: 'sarah_quote.1234567890abcdef12345678',
      status: 'checkout_test_mode',
    })
  })

  test('rejects an amount that does not match the configured checkout catalog', async () => {
    const response = await runRoute(
      makeRequest({
        ...validBody,
        amountUsdCents: 200_000,
      }),
      { SARAH_SALES_CHECKOUT_TEST_MODE: '1' },
    )

    expect(response.status).toBe(400)
    expect(await response.json()).toMatchObject({
      error: 'sarah_checkout_validation_error',
      reason:
        'amountUsdCents must match the OpenAgents checkout catalog package.',
    })
  })

  test('requires a buyer user id before live Stripe checkout', async () => {
    const response = await runRoute(
      makeRequest({
        ...validBody,
        buyerUserId: null,
      }),
    )

    expect(response.status).toBe(400)
    expect(await response.json()).toMatchObject({
      error: 'sarah_checkout_validation_error',
      reason: 'buyerUserId is required for live Stripe checkout.',
    })
  })

  test('delegates live checkout creation to the Stripe checkout service', async () => {
    const calls: Array<Record<string, unknown>> = []
    const response = await runRoute(makeRequest(validBody), {}, async input => {
      calls.push(input)

      return {
        checkoutUrl: 'https://checkout.stripe.test/cs_test_sarah',
        sessionId: 'cs_test_sarah',
      }
    })

    expect(response.status).toBe(201)
    expect(calls).toHaveLength(1)
    expect(calls[0]).toMatchObject({
      businessKickoff: { signupId: 'business_signup_001' },
      email: 'buyer@example.com',
      packageId: 'quick_win_credit_pack',
      userId: 'github:buyer-1',
    })
    expect(await response.json()).toMatchObject({
      checkoutUrl: 'https://checkout.stripe.test/cs_test_sarah',
      mode: 'live',
      status: 'checkout_created',
    })
  })
})
