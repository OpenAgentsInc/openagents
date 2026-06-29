import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { makeEcommerceCampaignReceiptRoutes } from './ecommerce-campaign-receipt-routes'
import { makeInMemoryEcommerceCampaignReceiptStore } from './ecommerce-campaign-receipt-store'
import { makeInMemoryEcommerceCampaignPaidDeliveryClaimStore } from './ecommerce-campaign-claim-upgrade'

describe('ecommerce campaign receipt routes', () => {
  it('returns 404 for unknown receipt', async () => {
    const store = makeInMemoryEcommerceCampaignReceiptStore()
    const routes = makeEcommerceCampaignReceiptRoutes({
      makeStore: () => store,
      makeClaimStore: () => makeInMemoryEcommerceCampaignPaidDeliveryClaimStore([]),
    })

    const request = new Request('https://openagents.com/api/public/ecommerce-campaign/receipts/unknown_ref')

    const responseEffect = routes.routeEcommerceCampaignReceiptRequest(request, {})
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(404)
  })

  it('projects paid delivery claims on ?view=paid-delivery-claims', async () => {
    const store = makeInMemoryEcommerceCampaignReceiptStore()
    const claimStore = makeInMemoryEcommerceCampaignPaidDeliveryClaimStore([])
    const routes = makeEcommerceCampaignReceiptRoutes({
      makeStore: () => store,
      makeClaimStore: () => claimStore,
    })

    const request = new Request('https://openagents.com/api/public/ecommerce-campaign/receipts?view=paid-delivery-claims')

    const responseEffect = routes.routeEcommerceCampaignReceiptRequest(request, {})
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(200)

    const json = await response.json()
    expect(json).toMatchObject({
      schema: 'openagents.ecommerce_campaign.paid_delivery_claim.v1',
      promiseIds: ['business.ecommerce_workspace_pack.v1'],
      promiseState: 'yellow',
      totals: {
        assessedCount: 0,
        substantiatedCount: 0,
        withheldCount: 0,
      },
      paidDeliveryClaimSubstantiated: false,
    })
  })
})
