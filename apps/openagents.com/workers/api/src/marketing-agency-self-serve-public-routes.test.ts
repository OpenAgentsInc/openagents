import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeMarketingAgencySelfServePublicRoutes } from './marketing-agency-self-serve-public-routes'
import { selfServeDeliverabilityFixture } from './marketing-agency-self-serve-fixture'

describe('marketing-agency-self-serve-public-routes', () => {
  const routes = makeMarketingAgencySelfServePublicRoutes()

  test('returns 404 for unknown workspace', async () => {
    const request = new Request('https://api.example.com/api/public/marketing-agency/self-serve/deliverability/unknown-ws')
    const response = await Effect.runPromise(routes.routeMarketingAgencySelfServeRequest(request)!)
    
    expect(response.status).toBe(404)
    const body = await response.json()
    expect(body).toEqual({ error: 'not_found', reason: 'Deliverability record not found.' })
  })

  test('returns deliverability fixture for known workspace', async () => {
    const request = new Request(`https://api.example.com/api/public/marketing-agency/self-serve/deliverability/${selfServeDeliverabilityFixture.workspaceId}`)
    const response = await Effect.runPromise(routes.routeMarketingAgencySelfServeRequest(request)!)
    
    expect(response.status).toBe(200)
    const body = await response.json() as any
    expect(body.deliverability).toEqual(selfServeDeliverabilityFixture)
    expect(body.staleness).toBeDefined()
    expect(body.generatedAt).toBeDefined()
  })

  test('returns 405 for non-GET requests', async () => {
    const request = new Request(`https://api.example.com/api/public/marketing-agency/self-serve/deliverability/${selfServeDeliverabilityFixture.workspaceId}`, {
      method: 'POST'
    })
    const response = await Effect.runPromise(routes.routeMarketingAgencySelfServeRequest(request)!)
    
    expect(response.status).toBe(405)
    expect(response.headers.get('Allow')).toBe('GET')
  })

  test('returns undefined for non-matching paths', () => {
    const request = new Request('https://api.example.com/api/public/other')
    expect(routes.routeMarketingAgencySelfServeRequest(request)).toBeUndefined()
  })
})
