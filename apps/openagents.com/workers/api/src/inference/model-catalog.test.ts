import { describe, expect, it } from 'vitest'

import {
  buildModelCatalog,
  findModelCatalogEntry,
  toOpenAiModelObject,
  toOpenAiModelsResponse,
} from './model-catalog'
import {
  DEFAULT_MARGIN,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  MODEL_PRICING_TABLE,
  sellPricePerMtok,
} from './pricing'

describe('buildModelCatalog', () => {
  it('lists exactly the models in the pricing table, in table order', () => {
    const catalog = buildModelCatalog()
    expect(catalog.map(m => m.id)).toEqual(
      MODEL_PRICING_TABLE.map(e => e.model),
    )
  })

  it('publishes prices that match the metering sell rate (no drift)', () => {
    for (const model of buildModelCatalog()) {
      // The catalog input/output prices must equal the price the metering hook
      // would charge against (sellPricePerMtok), so the published price can
      // never drift from the billed price.
      expect(model.price.inputUsdPerMtok).toBeCloseTo(
        sellPricePerMtok(model.id, 'input')!,
        6,
      )
      expect(model.price.outputUsdPerMtok).toBeCloseTo(
        sellPricePerMtok(model.id, 'output')!,
        6,
      )
      expect(model.price.cachedInputUsdPerMtok).toBeCloseTo(
        sellPricePerMtok(model.id, 'cached')!,
        6,
      )
    }
  })

  it('expresses credits per Mtok as USD per Mtok / $0.01', () => {
    for (const model of buildModelCatalog()) {
      expect(model.price.inputCreditsPerMtok).toBeCloseTo(
        model.price.inputUsdPerMtok / 0.01,
        4,
      )
      expect(model.price.outputCreditsPerMtok).toBeCloseTo(
        model.price.outputUsdPerMtok / 0.01,
        4,
      )
    }
  })

  it('marks only the Gemini free-tier lane as free-tier eligible', () => {
    const free = buildModelCatalog().filter(m => m.freeTierEligible)
    expect(free.map(m => m.id)).toEqual(['gemini-3.5-flash'])
  })

  it('marks Fireworks open models as verified cost basis and Vertex as list placeholder', () => {
    const catalog = buildModelCatalog()
    const fireworks = catalog.find(m => m.id === 'glm-5p2')
    const claude = catalog.find(m => m.id === 'opus')
    expect(fireworks?.costBasis).toBe('verified')
    expect(claude?.costBasis).toBe('list_placeholder')
  })

  it('re-solves prices when the margin changes', () => {
    const base = buildModelCatalog(0)
    const marked = buildModelCatalog(1)
    const baseGlm = base.find(m => m.id === 'glm-5p2')!
    const markedGlm = marked.find(m => m.id === 'glm-5p2')!
    // At margin 0 the price equals cost; at margin 1 it doubles.
    expect(markedGlm.price.inputUsdPerMtok).toBeCloseTo(
      baseGlm.price.inputUsdPerMtok * 2,
      6,
    )
  })

  it('attaches a legible owned_by label per lane', () => {
    const catalog = buildModelCatalog()
    expect(catalog.find(m => m.id === 'gemini-3.5-flash')?.ownedBy).toBe(
      'openagents/vertex-gemini',
    )
    expect(catalog.find(m => m.id === 'glm-5p2')?.ownedBy).toBe(
      'openagents/fireworks',
    )
    expect(
      catalog.find(m => m.id === HYDRALISK_GPT_OSS_20B_MODEL_ID)?.ownedBy,
    ).toBe('openagents/hydralisk')
  })

  it('uses the launch margin by default', () => {
    const def = buildModelCatalog()
    const explicit = buildModelCatalog(DEFAULT_MARGIN)
    expect(def).toEqual(explicit)
  })
})

describe('toOpenAiModelsResponse', () => {
  it('projects an OpenAI list payload with the injected created timestamp', () => {
    const response = toOpenAiModelsResponse(buildModelCatalog(), 1_700_000_000)
    expect(response.object).toBe('list')
    expect(response.data.length).toBe(MODEL_PRICING_TABLE.length)
    const first = response.data[0]!
    expect(first.object).toBe('model')
    expect(first.created).toBe(1_700_000_000)
    expect(typeof first.id).toBe('string')
    expect(first.oa_price.inputUsdPerMtok).toBeGreaterThanOrEqual(0)
  })

  it('carries the OpenAgents price/policy extension fields', () => {
    const response = toOpenAiModelsResponse(buildModelCatalog(), 1)
    const gemini = response.data.find(m => m.id === 'gemini-3.5-flash')!
    expect(gemini.oa_free_tier_eligible).toBe(true)
    expect(gemini.oa_lane).toBe('vertex-gemini')
    expect(gemini.oa_cost_basis).toBe('list_placeholder')
  })

  it('publishes sell price >= the underlying cost basis (margin is non-negative)', () => {
    // The public payload exposes the SELL price, which is always >= our cost.
    const response = toOpenAiModelsResponse(buildModelCatalog(), 1)
    for (const model of response.data) {
      const entry = MODEL_PRICING_TABLE.find(e => e.model === model.id)!
      expect(model.oa_price.inputUsdPerMtok).toBeGreaterThanOrEqual(
        entry.cost.inputUsdPerMtok,
      )
      expect(model.oa_price.outputUsdPerMtok).toBeGreaterThanOrEqual(
        entry.cost.outputUsdPerMtok,
      )
    }
  })
})

describe('findModelCatalogEntry', () => {
  it('resolves a served model to its catalog entry', () => {
    const id = MODEL_PRICING_TABLE[0]!.model
    const entry = findModelCatalogEntry(id)
    expect(entry?.id).toBe(id)
  })

  it('returns the SAME entry the list catalog publishes (no divergence)', () => {
    const id = MODEL_PRICING_TABLE[0]!.model
    const fromList = buildModelCatalog().find(m => m.id === id)
    expect(findModelCatalogEntry(id)).toEqual(fromList)
  })

  it('returns undefined for an unknown model', () => {
    expect(findModelCatalogEntry('nope-not-served')).toBeUndefined()
  })

  it('returns undefined for a blank id', () => {
    expect(findModelCatalogEntry('')).toBeUndefined()
    expect(findModelCatalogEntry('   ')).toBeUndefined()
  })

  it('honours the margin override like the list builder', () => {
    const id = 'glm-5p2'
    expect(findModelCatalogEntry(id, 1)).toEqual(
      buildModelCatalog(1).find(m => m.id === id),
    )
  })
})

describe('toOpenAiModelObject', () => {
  it('projects a single model object matching the list projection', () => {
    const entry = findModelCatalogEntry('gemini-3.5-flash')!
    const single = toOpenAiModelObject(entry, 1_700_000_000)
    const fromList = toOpenAiModelsResponse(
      buildModelCatalog(),
      1_700_000_000,
    ).data.find(m => m.id === 'gemini-3.5-flash')!
    expect(single).toEqual(fromList)
  })
})
