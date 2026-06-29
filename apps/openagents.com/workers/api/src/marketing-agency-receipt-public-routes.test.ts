import { describe, expect, test } from 'vitest'
import { Effect } from 'effect'
import { makeMarketingAgencyReceiptPublicRoutes } from './marketing-agency-receipt-public-routes'
import { firstPaidMarketingAgencyDeliveryReceiptFixture } from './marketing-agency-delivery-receipt-fixture'

describe('marketing-agency-receipt-public-routes', () => {
  const routes = makeMarketingAgencyReceiptPublicRoutes<any>({ makeClaimStore: () => ({ list: () => [] }) })

  test('returns the fixture when requested by its work item ref', async () => {
    const request = new Request(`https://openagents.com/api/public/marketing-agency/receipts/${encodeURIComponent(firstPaidMarketingAgencyDeliveryReceiptFixture.workItemRef)}`)
    const handler = routes.routeMarketingAgencyReceiptRequest(request, {})
    
    expect(handler).toBeDefined()
    if (!handler) return

    const response = await Effect.runPromise(handler)
    expect(response.status).toBe(200)

    const json = (await response.json()) as {
      generatedAt: string
      staleness: { composition: string; contractVersion: string; maxStalenessSeconds: number; rebuildsOn: ReadonlyArray<string> }
      receipt: typeof firstPaidMarketingAgencyDeliveryReceiptFixture
    }
    expect(json.receipt).toEqual(firstPaidMarketingAgencyDeliveryReceiptFixture)
    expect(typeof json.generatedAt).toBe('string')
    expect(json.staleness).toEqual({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
      rebuildsOn: ['fixture_only'],
    })
  })

  test('returns 404 for unknown receipt refs', async () => {
    const request = new Request('https://openagents.com/api/public/marketing-agency/receipts/unknown_ref')
    const handler = routes.routeMarketingAgencyReceiptRequest(request, {})
    
    expect(handler).toBeDefined()
    if (!handler) return

    const response = await Effect.runPromise(handler)
    expect(response.status).toBe(404)
  })

  test('returns undefined for non-matching paths', () => {
    const request = new Request('https://openagents.com/api/public/other/path')
    expect(routes.routeMarketingAgencyReceiptRequest(request, {})).toBeUndefined()
  })

  test('returns 405 for non-GET methods on a matched path', async () => {
    const request = new Request(`https://openagents.com/api/public/marketing-agency/receipts/${encodeURIComponent(firstPaidMarketingAgencyDeliveryReceiptFixture.workItemRef)}`, { method: 'POST' })
    const handler = routes.routeMarketingAgencyReceiptRequest(request, {})
    
    expect(handler).toBeDefined()
    if (!handler) return

    const response = await Effect.runPromise(handler)
    expect(response.status).toBe(405)
  })
})
