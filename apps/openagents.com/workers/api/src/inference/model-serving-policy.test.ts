import { describe, expect, it } from 'vitest'

import { buildModelCatalog } from './model-catalog'
import {
  ALL_LANES_UNARMED,
  filterServableCatalog,
  isLaneArmed,
  isModelServable,
  resolveSupplyLaneArming,
  type SupplyLaneArming,
} from './model-serving-policy'
import type { SupplyLane } from './pricing'

const ALL_ARMED: SupplyLaneArming = {
  fireworks: true,
  'openagents-network': true,
  'vertex-anthropic': true,
  'vertex-gemini': true,
}

describe('resolveSupplyLaneArming', () => {
  it('arms nothing for an empty env (safe default)', () => {
    expect(resolveSupplyLaneArming({})).toEqual(ALL_LANES_UNARMED)
  })

  it('arms both Vertex lanes from VERTEX_SA_KEY presence', () => {
    const arming = resolveSupplyLaneArming({ VERTEX_SA_KEY: '{"k":1}' })
    expect(arming['vertex-gemini']).toBe(true)
    expect(arming['vertex-anthropic']).toBe(true)
    expect(arming.fireworks).toBe(false)
  })

  it('arms the Fireworks lane from FIREWORKS_API_KEY presence', () => {
    const arming = resolveSupplyLaneArming({ FIREWORKS_API_KEY: 'fw-secret' })
    expect(arming.fireworks).toBe(true)
    expect(arming['vertex-gemini']).toBe(false)
    expect(arming['vertex-anthropic']).toBe(false)
  })

  it('treats a blank/whitespace credential as absent', () => {
    expect(resolveSupplyLaneArming({ VERTEX_SA_KEY: '   ' })).toEqual(
      ALL_LANES_UNARMED,
    )
  })

  it('never arms the openagents-network lane (serving fabric is roadmap)', () => {
    const arming = resolveSupplyLaneArming({
      FIREWORKS_API_KEY: 'fw',
      VERTEX_SA_KEY: 'sa',
    })
    expect(arming['openagents-network']).toBe(false)
  })
})

describe('isLaneArmed / isModelServable', () => {
  const lanes: ReadonlyArray<SupplyLane> = [
    'fireworks',
    'openagents-network',
    'vertex-anthropic',
    'vertex-gemini',
  ]

  it('reads arming per lane', () => {
    for (const lane of lanes) {
      expect(isLaneArmed(ALL_ARMED, lane)).toBe(true)
      expect(isLaneArmed(ALL_LANES_UNARMED, lane)).toBe(false)
    }
  })

  it('a model is servable iff its lane is armed', () => {
    const gemini = buildModelCatalog().find(
      e => e.lane === 'vertex-gemini',
    )!
    expect(isModelServable(gemini, ALL_ARMED)).toBe(true)
    expect(isModelServable(gemini, ALL_LANES_UNARMED)).toBe(false)
    expect(
      isModelServable(gemini, resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' })),
    ).toBe(true)
    expect(
      isModelServable(gemini, resolveSupplyLaneArming({ FIREWORKS_API_KEY: 'fw' })),
    ).toBe(false)
  })
})

describe('filterServableCatalog', () => {
  const catalog = buildModelCatalog()

  it('is the identity filter when every lane is armed', () => {
    expect(filterServableCatalog(catalog, ALL_ARMED)).toEqual(catalog)
  })

  it('is empty when no lane is armed', () => {
    expect(filterServableCatalog(catalog, ALL_LANES_UNARMED)).toEqual([])
  })

  it('keeps only Vertex models when only VERTEX_SA_KEY is present', () => {
    const armed = resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' })
    const filtered = filterServableCatalog(catalog, armed)
    expect(filtered.length).toBeGreaterThan(0)
    expect(
      filtered.every(
        e => e.lane === 'vertex-gemini' || e.lane === 'vertex-anthropic',
      ),
    ).toBe(true)
    // The Vertex Gemini lane (the api.hosted_gemini.v1 model) is published.
    expect(filtered.some(e => e.lane === 'vertex-gemini')).toBe(true)
  })

  it('keeps only Fireworks models when only FIREWORKS_API_KEY is present', () => {
    const armed = resolveSupplyLaneArming({ FIREWORKS_API_KEY: 'fw' })
    const filtered = filterServableCatalog(catalog, armed)
    expect(filtered.length).toBeGreaterThan(0)
    expect(filtered.every(e => e.lane === 'fireworks')).toBe(true)
  })

  it('preserves catalog order', () => {
    const filtered = filterServableCatalog(catalog, ALL_ARMED)
    expect(filtered.map(e => e.id)).toEqual(catalog.map(e => e.id))
  })
})
