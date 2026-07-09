import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCrmSalesCheckoutRoutes } from './crm-sales-checkout-routes'

type Env = Readonly<{
  OPENAGENTS_DB: D1Database
  OPENAGENTS_APP_URL?: string
  STRIPE_API_KEY?: string
  STRIPE_WEBHOOK_SIGNING_SECRET?: string
  STRIPE_CREDIT_PACKAGES_JSON?: string
}>

const ctx = {} as ExecutionContext
const base = 'https://openagents.com'

const makeDb = (): D1Database =>
  ({
    prepare: () =>
      ({
        bind: () => ({
          first: () => Promise.resolve(null),
          all: () => Promise.resolve({ meta: {}, results: [], success: true }),
          run: () => Promise.resolve({ meta: {}, results: [], success: true }),
        }),
      }) as unknown as D1PreparedStatement,
  }) as unknown as D1Database

const routesFor = (admin: boolean, env: Env) => {
  const routes = makeCrmSalesCheckoutRoutes<Env>({
    requireAdminApiToken: () => Promise.resolve(admin),
  })
  return (request: Request): Promise<Response> => {
    const effect = routes.routeCrmSalesCheckoutRequest(request, env, ctx)
    if (effect === undefined) throw new Error(`route did not match: ${request.url}`)
    return Effect.runPromise(effect)
  }
}

const post = (path: string, body: unknown): Request =>
  new Request(`${base}${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

describe('CRM sales checkout routes (OB-5, #8562)', () => {
  test('401 without admin token', async () => {
    const run = routesFor(false, { OPENAGENTS_DB: makeDb() })
    const res = await run(
      post('/api/operator/crm/sales/checkout-link', {
        contactId: 'crm_contact_1',
        packageId: 'starter_10',
      }),
    )
    expect(res.status).toBe(401)
  })

  test('400 when contactId/packageId are missing (checked before any Stripe call)', async () => {
    const run = routesFor(true, { OPENAGENTS_DB: makeDb() })
    const res = await run(post('/api/operator/crm/sales/checkout-link', {}))
    expect(res.status).toBe(400)
  })

  test('400 on a non-JSON body', async () => {
    const run = routesFor(true, { OPENAGENTS_DB: makeDb() })
    const res = await run(
      new Request(`${base}/api/operator/crm/sales/checkout-link`, {
        method: 'POST',
        body: 'not json',
      }),
    )
    expect(res.status).toBe(400)
  })

  test('405 on GET', async () => {
    const run = routesFor(true, { OPENAGENTS_DB: makeDb() })
    const res = await run(
      new Request(`${base}/api/operator/crm/sales/checkout-link`, { method: 'GET' }),
    )
    expect(res.status).toBe(405)
  })

  test('non-matching path passes through', () => {
    const routes = makeCrmSalesCheckoutRoutes<Env>({
      requireAdminApiToken: () => Promise.resolve(true),
    })
    const effect = routes.routeCrmSalesCheckoutRequest(
      new Request(`${base}/api/operator/crm/replies`),
      { OPENAGENTS_DB: makeDb() },
      ctx,
    )
    expect(effect).toBeUndefined()
  })

  test('surfaces a Stripe-config error as 422 once past the bad-request checks', async () => {
    // Deliberately omit STRIPE_API_KEY/STRIPE_CREDIT_PACKAGES_JSON/etc so
    // `makeStripeCheckoutServiceForRoutes` throws a StripeConfigError — this
    // proves the route's error boundary catches non-CrmSalesCheckoutError
    // failures (e.g. missing config) as a generic 500/422 rather than
    // leaking an uncaught exception past the guard.
    const run = routesFor(true, { OPENAGENTS_DB: makeDb() })
    const res = await run(
      post('/api/operator/crm/sales/checkout-link', {
        contactId: 'crm_contact_1',
        packageId: 'starter_10',
      }),
    )
    expect([422, 500]).toContain(res.status)
  })
})
