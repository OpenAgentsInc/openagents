import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { handleGatewayReadiness } from './gateway-readiness-routes'
import type { ModelCatalogEntry } from './model-catalog'
import {
  ALL_LANES_UNARMED,
  type SupplyLaneArming,
} from './model-serving-policy'
import { KHALA_MODEL_ID, type SupplyLane } from './pricing'

const ALL_ARMED: SupplyLaneArming = {
  fireworks: true,
  hydralisk: true,
  'openagents-network': true,
  'vertex-anthropic': true,
  'vertex-gemini': true,
}

const entry = (id: string, lane: SupplyLane): ModelCatalogEntry => ({
  costBasis: 'verified',
  freeTierEligible: false,
  id,
  lane,
  multiplier: 1,
  ownedBy: `openagents/${lane}`,
  price: {
    cachedInputCreditsPerMtok: 0,
    cachedInputUsdPerMtok: 0,
    inputCreditsPerMtok: 0,
    inputUsdPerMtok: 0,
    outputCreditsPerMtok: 0,
    outputUsdPerMtok: 0,
  },
})

const FIXTURE_CATALOG: ReadonlyArray<ModelCatalogEntry> = [
  entry(KHALA_MODEL_ID, 'hydralisk'),
  entry('gemini-flash', 'vertex-gemini'),
  entry('claude-sonnet', 'vertex-anthropic'),
]

const get = (url = 'https://x/v1/gateway/readiness') =>
  new Request(url, { method: 'GET' })

describe('handleGatewayReadiness', () => {
  it('404s with inference_gateway_disabled when the gateway is off (INERT)', async () => {
    const response = await Effect.runPromise(
      handleGatewayReadiness(get(), {
        enabled: false,
        laneArming: ALL_ARMED,
        catalog: FIXTURE_CATALOG,
      }),
    )
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({ error: 'inference_gateway_disabled' })
  })

  it('405s on a non-GET method', async () => {
    const response = await Effect.runPromise(
      handleGatewayReadiness(new Request('https://x/v1/gateway/readiness', { method: 'POST' }), {
        enabled: true,
        laneArming: ALL_ARMED,
        catalog: FIXTURE_CATALOG,
      }),
    )
    expect(response.status).toBe(405)
    expect(await response.json()).toEqual({ error: 'method_not_allowed' })
  })

  it('reports ready when every model is servable', async () => {
    const response = await Effect.runPromise(
      handleGatewayReadiness(get(), {
        enabled: true,
        laneArming: ALL_ARMED,
        catalog: FIXTURE_CATALOG,
      }),
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as Record<string, unknown>
    expect(body.status).toBe('ready')
    expect(body.servableModelCount).toBe(1)
    expect(body.hiddenModelCount).toBe(0)
  })

  it('reports unavailable when no lane is armed', async () => {
    const response = await Effect.runPromise(
      handleGatewayReadiness(get(), {
        enabled: true,
        laneArming: ALL_LANES_UNARMED,
        catalog: FIXTURE_CATALOG,
      }),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.status).toBe('unavailable')
    expect(body.servableModelCount).toBe(0)
    expect(body.reasonRefs).toContain(
      'gateway.readiness.unavailable.no_servable_models',
    )
  })

  it('reports unavailable and names the unarmed public backing lane when Khala is hidden', async () => {
    const response = await Effect.runPromise(
      handleGatewayReadiness(get(), {
        enabled: true,
        laneArming: { ...ALL_LANES_UNARMED, 'vertex-gemini': true },
        catalog: FIXTURE_CATALOG,
      }),
    )
    const body = (await response.json()) as {
      status: string
      reasonRefs: ReadonlyArray<string>
    }
    expect(body.status).toBe('unavailable')
    expect(body.reasonRefs).toContain(
      'gateway.readiness.lane_unarmed.hydralisk',
    )
  })

  it('uses the live published catalog when none is injected', async () => {
    const response = await Effect.runPromise(
      handleGatewayReadiness(get(), {
        enabled: true,
        laneArming: ALL_LANES_UNARMED,
      }),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { totalModelCount: number }
    expect(body.totalModelCount).toBeGreaterThan(0)
  })

  it('leaks no secret-shaped material in the body (presence-only)', async () => {
    const response = await Effect.runPromise(
      handleGatewayReadiness(get(), {
        enabled: true,
        laneArming: ALL_ARMED,
        catalog: FIXTURE_CATALOG,
      }),
    )
    const text = await response.text()
    expect(text).not.toMatch(/sk-[A-Za-z0-9]{16,}/u)
    expect(text).not.toContain('private')
  })
})
