import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeCheckoutPageRoutes } from './checkout-page-routes'
import { makeFakeOpenAgentsHostedMdkClient } from './hosted-mdk-client'

const fakeClient = () =>
  makeFakeOpenAgentsHostedMdkClient(
    {
      configRef: 'config.checkout.page.test',
      credentialBindingRef: 'binding.checkout.page.test',
      environment: 'sandbox',
      providerRef: 'provider.checkout.page.test',
      webhookBindingRef: null,
    },
    { nowIso: '2026-06-09T20:00:00.000Z' },
  )

describe('checkout page routes', () => {
  test('renders a payable invoice page for pending checkouts', async () => {
    const routes = makeCheckoutPageRoutes<Record<string, unknown>>({
      hostedMdkClient: () => fakeClient(),
    })
    const effect = routes.routeCheckoutPageRequest(
      new Request('https://openagents.com/checkout/cmtestcheckout001'),
      {},
    )

    expect(effect).toBeDefined()

    const response = await Effect.runPromise(effect!)
    const html = await response.text()

    expect(response.status).toBe(200)
    expect(html).toContain('Pay with Lightning')
    expect(html).toContain('lntbs')
    expect(html).toContain('lightning:')
    expect(html).toContain('<svg')
    expect(html).toContain('class="qr"')
    expect(html).not.toContain('oa_agent_')
  })

  test('shows paid state for received checkouts', async () => {
    const base = fakeClient()
    const paid = {
      ...base,
      getCheckoutStatus: (
        request: Parameters<typeof base.getCheckoutStatus>[0],
      ) =>
        Effect.map(base.getCheckoutStatus(request), status => ({
          ...status,
          status: 'payment_received' as const,
        })),
    }
    const routes = makeCheckoutPageRoutes<Record<string, unknown>>({
      hostedMdkClient: () => paid,
    })
    const response = await Effect.runPromise(
      routes.routeCheckoutPageRequest(
        new Request('https://openagents.com/checkout/cmtestcheckout001'),
        {},
      )!,
    )

    expect(response.status).toBe(200)
    expect(await response.text()).toContain('Payment received')
  })

  test('ignores non-checkout paths and denies non-GET', async () => {
    const routes = makeCheckoutPageRoutes<Record<string, unknown>>({
      hostedMdkClient: () => fakeClient(),
    })

    expect(
      routes.routeCheckoutPageRequest(
        new Request('https://openagents.com/forum'),
        {},
      ),
    ).toBeUndefined()

    const denied = await Effect.runPromise(
      routes.routeCheckoutPageRequest(
        new Request('https://openagents.com/checkout/cmtestcheckout001', {
          method: 'POST',
        }),
        {},
      )!,
    )

    expect(denied.status).toBe(405)
  })
})
