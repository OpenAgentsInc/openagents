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
import { KHALA_MODEL_ID, type SupplyLane } from './pricing'
import type { SupplyLaneArming } from './model-serving-policy'

const ALL_ARMED: SupplyLaneArming = {
  fireworks: true,
  hydralisk: true,
  'openagents-network': true,
  'vertex-anthropic': true,
  'vertex-gemini': true,
}

const HYDRALISK_ARMED_ENV = {
  HYDRALISK_BASE_URL: 'https://hydralisk.example.test',
  HYDRALISK_BEARER_TOKEN: 'secret-route-token',
  HYDRALISK_GPT_OSS_20B_ENABLED: 'ready',
  HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF:
    'preflight.hydralisk.gpt_oss_20b.l4.v1',
  HYDRALISK_GPT_OSS_20B_RECEIPT_REF:
    'receipt.hydralisk.gpt_oss_20b.l4.smoke.v1',
}

// A tiny deterministic catalog with one public model and two internal rows. The
// readiness projection counts only the public Khala row.
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

describe('projectGatewayReadiness', () => {
  it('reports unavailable when no lane is armed', () => {
    const readiness = projectGatewayReadiness(
      ALL_LANES_UNARMED,
      FIXTURE_CATALOG,
    )
    expect(readiness.status).toBe('unavailable')
    expect(readiness.servableModelCount).toBe(0)
    expect(readiness.hiddenModelCount).toBe(1)
    expect(readiness.totalModelCount).toBe(1)
    expect(readiness.reasonRefs).toContain(
      'gateway.readiness.unavailable.no_servable_models',
    )
  })

  it('reports ready when the public Khala model is servable', () => {
    const readiness = projectGatewayReadiness(ALL_ARMED, FIXTURE_CATALOG)
    expect(readiness.status).toBe('ready')
    expect(readiness.servableModelCount).toBe(1)
    expect(readiness.hiddenModelCount).toBe(0)
    expect(readiness.reasonRefs).toEqual([
      'gateway.readiness.ready.all_models_servable',
    ])
  })

  it('ignores armed internal lanes when Khala is not servable', () => {
    const arming: SupplyLaneArming = {
      ...ALL_LANES_UNARMED,
      'vertex-gemini': true,
    }
    const readiness = projectGatewayReadiness(arming, FIXTURE_CATALOG)
    expect(readiness.status).toBe('unavailable')
    expect(readiness.servableModelCount).toBe(0)
    expect(readiness.hiddenModelCount).toBe(1)
    expect(readiness.reasonRefs).toContain(
      'gateway.readiness.unavailable.no_servable_models',
    )
    // The unarmed public Khala lane is named in reason refs.
    expect(readiness.reasonRefs).toContain(
      'gateway.readiness.lane_unarmed.hydralisk',
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
      'hydralisk',
      'openagents-network',
    ])
    const gemini = readiness.lanes.find(l => l.lane === 'vertex-gemini')
    expect(gemini).toEqual({
      armed: true,
      hiddenModelCount: 0,
      lane: 'vertex-gemini',
      servableModelCount: 0,
    })
    const hydralisk = readiness.lanes.find(l => l.lane === 'hydralisk')
    expect(hydralisk).toEqual({
      armed: false,
      hiddenModelCount: 1,
      lane: 'hydralisk',
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

  it('works against the live published catalog with a Hydralisk-armed env', () => {
    const arming = resolveSupplyLaneArming(HYDRALISK_ARMED_ENV)
    const readiness = projectGatewayReadiness(arming)
    expect(readiness.totalModelCount).toBe(1)
    expect(readiness.servableModelCount).toBe(1)
    expect(readiness.status).toBe('ready')
    // Servable + hidden partition the whole catalog.
    expect(readiness.servableModelCount + readiness.hiddenModelCount).toBe(
      readiness.totalModelCount,
    )
    // Consistent with the public subset of the live catalog.
    expect(readiness.totalModelCount).toBe(
      buildModelCatalog().filter(entry => entry.id === KHALA_MODEL_ID).length,
    )
  })
})
