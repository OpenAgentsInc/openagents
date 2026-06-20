import { describe, expect, it } from 'vitest'

import type { ModelCatalogEntry } from './model-catalog'
import { buildModelCatalog } from './model-catalog'
import {
  projectGatewayReadiness,
  type GatewayReadiness,
} from './gateway-readiness'
import {
  ALL_LANES_UNARMED,
  resolveSupplyLaneArming,
} from './model-serving-policy'
import type { SupplyLane } from './pricing'
import type { SupplyLaneArming } from './model-serving-policy'

const ALL_ARMED: SupplyLaneArming = {
  fireworks: true,
  'openagents-network': true,
  'vertex-anthropic': true,
  'vertex-gemini': true,
}

// A tiny deterministic catalog spanning two lanes, so counts are unambiguous.
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
  entry('gemini-flash', 'vertex-gemini'),
  entry('gemini-pro', 'vertex-gemini'),
  entry('claude-sonnet', 'vertex-anthropic'),
]

describe('projectGatewayReadiness', () => {
  it('reports unavailable when no lane is armed', () => {
    const readiness = projectGatewayReadiness(
      ALL_LANES_UNARMED,
      FIXTURE_CATALOG,
    )
    expect(readiness.status).toBe('unavailable')
    expect(readiness.servableModelCount).toBe(0)
    expect(readiness.hiddenModelCount).toBe(3)
    expect(readiness.totalModelCount).toBe(3)
    expect(readiness.reasonRefs).toContain(
      'gateway.readiness.unavailable.no_servable_models',
    )
  })

  it('reports ready when every published model is servable', () => {
    const readiness = projectGatewayReadiness(ALL_ARMED, FIXTURE_CATALOG)
    expect(readiness.status).toBe('ready')
    expect(readiness.servableModelCount).toBe(3)
    expect(readiness.hiddenModelCount).toBe(0)
    expect(readiness.reasonRefs).toEqual([
      'gateway.readiness.ready.all_models_servable',
    ])
  })

  it('reports degraded when some lanes are armed and others are not', () => {
    const arming: SupplyLaneArming = {
      ...ALL_LANES_UNARMED,
      'vertex-gemini': true,
    }
    const readiness = projectGatewayReadiness(arming, FIXTURE_CATALOG)
    expect(readiness.status).toBe('degraded')
    expect(readiness.servableModelCount).toBe(2)
    expect(readiness.hiddenModelCount).toBe(1)
    expect(readiness.reasonRefs).toContain(
      'gateway.readiness.degraded.some_lanes_unarmed',
    )
    // The unarmed lane that hides a published model is named in reason refs.
    expect(readiness.reasonRefs).toContain(
      'gateway.readiness.lane_unarmed.vertex-anthropic',
    )
    // The armed lane is not named as unarmed.
    expect(readiness.reasonRefs).not.toContain(
      'gateway.readiness.lane_unarmed.vertex-gemini',
    )
  })

  it('breaks counts down per lane in a stable order', () => {
    const arming: SupplyLaneArming = {
      ...ALL_LANES_UNARMED,
      'vertex-gemini': true,
    }
    const readiness = projectGatewayReadiness(arming, FIXTURE_CATALOG)
    expect(readiness.lanes.map(lane => lane.lane)).toEqual([
      'vertex-gemini',
      'vertex-anthropic',
      'fireworks',
      'openagents-network',
    ])
    const gemini = readiness.lanes.find(l => l.lane === 'vertex-gemini')
    expect(gemini).toEqual({
      armed: true,
      hiddenModelCount: 0,
      lane: 'vertex-gemini',
      servableModelCount: 2,
    })
    const anthropic = readiness.lanes.find(l => l.lane === 'vertex-anthropic')
    expect(anthropic).toEqual({
      armed: false,
      hiddenModelCount: 1,
      lane: 'vertex-anthropic',
      servableModelCount: 0,
    })
  })

  it('reports unavailable for an empty catalog', () => {
    const readiness = projectGatewayReadiness(ALL_ARMED, [])
    expect(readiness.status).toBe('unavailable')
    expect(readiness.totalModelCount).toBe(0)
    expect(readiness.servableModelCount).toBe(0)
  })

  it('lane servable counts always sum to the total servable count', () => {
    const arming: SupplyLaneArming = {
      ...ALL_LANES_UNARMED,
      'vertex-gemini': true,
    }
    const readiness = projectGatewayReadiness(arming, FIXTURE_CATALOG)
    const summed = readiness.lanes.reduce(
      (acc, lane) => acc + lane.servableModelCount,
      0,
    )
    expect(summed).toBe(readiness.servableModelCount)
  })

  it('contains no secret-shaped material in its reason refs (presence-only)', () => {
    const arming = resolveSupplyLaneArming({ VERTEX_SA_KEY: '{"private":1}' })
    const readiness: GatewayReadiness = projectGatewayReadiness(arming)
    const serialized = JSON.stringify(readiness)
    expect(serialized).not.toContain('private')
    expect(serialized).not.toMatch(/sk-[A-Za-z0-9]{16,}/u)
  })

  it('works against the live published catalog with a Vertex-armed env', () => {
    const arming = resolveSupplyLaneArming({ VERTEX_SA_KEY: '{"k":1}' })
    const readiness = projectGatewayReadiness(arming)
    // The live catalog has both Vertex and non-Vertex lanes, so a Vertex-only
    // env is servable-but-degraded (some published models are hidden).
    expect(readiness.totalModelCount).toBeGreaterThan(0)
    expect(readiness.servableModelCount).toBeGreaterThan(0)
    expect(['degraded', 'ready']).toContain(readiness.status)
    // Servable + hidden partition the whole catalog.
    expect(readiness.servableModelCount + readiness.hiddenModelCount).toBe(
      readiness.totalModelCount,
    )
    // Consistent with the live catalog size.
    expect(readiness.totalModelCount).toBe(buildModelCatalog().length)
  })
})
