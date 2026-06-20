import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { makeEcommerceCampaignReceiptRoutes } from './ecommerce-campaign-receipt-routes'
import { makeInMemoryEcommerceCampaignReceiptStore } from './ecommerce-campaign-receipt-store'

describe('ecommerce campaign receipt routes', () => {
  it('returns 404 for unknown receipt', async () => {
    const store = makeInMemoryEcommerceCampaignReceiptStore()
    const routes = makeEcommerceCampaignReceiptRoutes({ makeStore: () => store })

    const request = new Request('https://openagents.com/api/public/ecommerce-campaign/receipts/unknown_ref')

    const responseEffect = routes.routeEcommerceCampaignReceiptRequest(request, {})
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(404)
  })
})
