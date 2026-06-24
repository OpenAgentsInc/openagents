import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { buildModelCatalog } from './model-catalog'
import { resolveSupplyLaneArming } from './model-serving-policy'
import {
  handleModelRetrieve,
  handleModelsList,
  routeModelRetrieveRequest,
} from './models-routes'
import {
  AUTOPILOT_CONCIERGE_MODEL_ID,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  KHALA_CODE_MODEL_ID,
  KHALA_MINI_MODEL_ID,
  MODEL_PRICING_TABLE,
} from './pricing'

const run = (request: Request, deps: Parameters<typeof handleModelsList>[1]) =>
  Effect.runPromise(handleModelsList(request, deps))

const runRetrieve = (
  request: Request,
  modelId: string,
  deps: Parameters<typeof handleModelRetrieve>[2],
) => Effect.runPromise(handleModelRetrieve(request, modelId, deps))

describe('handleModelsList', () => {
  it('404s when the gateway is disabled (inert posture)', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: false },
    )
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'inference_gateway_disabled',
    })
  })

  it('405s on non-GET methods', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'POST' }),
      { enabled: true },
    )
    expect(response.status).toBe(405)
  })

  it('serves the OpenAI-compatible catalog when enabled', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: true, nowEpochSeconds: () => 1_700_000_000 },
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as {
      object: string
      data: ReadonlyArray<{ id: string; created: number; object: string }>
    }
    expect(body.object).toBe('list')
    expect(body.data.length).toBe(MODEL_PRICING_TABLE.length)
    expect(body.data.every(m => m.object === 'model')).toBe(true)
    expect(body.data.every(m => m.created === 1_700_000_000)).toBe(true)
    expect(body.data.some(m => m.id === KHALA_MINI_MODEL_ID)).toBe(true)
    expect(body.data.some(m => m.id === KHALA_CODE_MODEL_ID)).toBe(true)
    expect(body.data.some(m => m.id === AUTOPILOT_CONCIERGE_MODEL_ID)).toBe(
      true,
    )
  })
})

describe('handleModelRetrieve', () => {
  const servedModelId = MODEL_PRICING_TABLE[0]!.model

  it('404s when the gateway is disabled (inert posture)', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${servedModelId}`, { method: 'GET' }),
      servedModelId,
      { enabled: false },
    )
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'inference_gateway_disabled',
    })
  })

  it('405s on non-GET methods', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${servedModelId}`, { method: 'DELETE' }),
      servedModelId,
      { enabled: true },
    )
    expect(response.status).toBe(405)
  })

  it('returns the OpenAI model object for a served model', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${servedModelId}`, { method: 'GET' }),
      servedModelId,
      { enabled: true, nowEpochSeconds: () => 1_700_000_000 },
    )
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const body = (await response.json()) as {
      id: string
      object: string
      created: number
      oa_price: { inputUsdPerMtok: number }
    }
    expect(body.id).toBe(servedModelId)
    expect(body.object).toBe('model')
    expect(body.created).toBe(1_700_000_000)
    expect(typeof body.oa_price.inputUsdPerMtok).toBe('number')
  })

  it('404s with the OpenAI model_not_found shape for an unknown model', async () => {
    const response = await runRetrieve(
      new Request('https://x/v1/models/nope-not-served', { method: 'GET' }),
      'nope-not-served',
      { enabled: true },
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as {
      error: { code: string; type: string; param: string }
    }
    expect(body.error.code).toBe('model_not_found')
    expect(body.error.type).toBe('invalid_request_error')
    expect(body.error.param).toBe('model')
  })

  it('404s (model_not_found) for a blank model id', async () => {
    const response = await runRetrieve(
      new Request('https://x/v1/models/', { method: 'GET' }),
      '',
      { enabled: true },
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: { code: string } }
    expect(body.error.code).toBe('model_not_found')
  })
})

describe('routeModelRetrieveRequest dispatcher', () => {
  const servedModelId = MODEL_PRICING_TABLE[0]!.model

  it('does NOT match the list path /v1/models (exact route owns it)', () => {
    const effect = routeModelRetrieveRequest(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: true },
    )
    expect(effect).toBeUndefined()
  })

  it('does NOT match a trailing-slash-only path (falls through)', () => {
    const effect = routeModelRetrieveRequest(
      new Request('https://x/v1/models/', { method: 'GET' }),
      { enabled: true },
    )
    expect(effect).toBeUndefined()
  })

  it('does NOT match a nested path (no served id has a slash)', () => {
    const effect = routeModelRetrieveRequest(
      new Request('https://x/v1/models/accounts/fireworks', { method: 'GET' }),
      { enabled: true },
    )
    expect(effect).toBeUndefined()
  })

  it('does NOT match an unrelated path', () => {
    const effect = routeModelRetrieveRequest(
      new Request('https://x/v1/chat/completions', { method: 'POST' }),
      { enabled: true },
    )
    expect(effect).toBeUndefined()
  })

  it('routes a served model id to the retrieve handler', async () => {
    const effect = routeModelRetrieveRequest(
      new Request(`https://x/v1/models/${servedModelId}`, { method: 'GET' }),
      { enabled: true, nowEpochSeconds: () => 1_700_000_000 },
    )
    expect(effect).toBeDefined()
    const response = await Effect.runPromise(effect!)
    expect(response.status).toBe(200)
    const body = (await response.json()) as { id: string; object: string }
    expect(body.id).toBe(servedModelId)
    expect(body.object).toBe('model')
  })

  it('decodes a percent-encoded model id before lookup', async () => {
    const effect = routeModelRetrieveRequest(
      new Request(`https://x/v1/models/${encodeURIComponent(servedModelId)}`, {
        method: 'GET',
      }),
      { enabled: true },
    )
    expect(effect).toBeDefined()
    const response = await Effect.runPromise(effect!)
    expect(response.status).toBe(200)
    const body = (await response.json()) as { id: string }
    expect(body.id).toBe(servedModelId)
  })

  it('routes an unknown model to the handler (model_not_found, not fall-through)', async () => {
    const effect = routeModelRetrieveRequest(
      new Request('https://x/v1/models/nope-not-served', { method: 'GET' }),
      { enabled: true },
    )
    expect(effect).toBeDefined()
    const response = await Effect.runPromise(effect!)
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: { code: string } }
    expect(body.error.code).toBe('model_not_found')
  })

  it('routes a matching path to the handler even when the gateway is off (inert 404)', async () => {
    const effect = routeModelRetrieveRequest(
      new Request(`https://x/v1/models/${servedModelId}`, { method: 'GET' }),
      { enabled: false },
    )
    expect(effect).toBeDefined()
    const response = await Effect.runPromise(effect!)
    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({
      error: 'inference_gateway_disabled',
    })
  })
})

describe('provider serving policy (laneArming)', () => {
  const catalog = buildModelCatalog()
  const geminiId = catalog.find(e => e.lane === 'vertex-gemini')!.id
  const fireworksId = catalog.find(e => e.lane === 'fireworks')!.id

  it('lists every model when laneArming is omitted (prior behaviour)', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: true },
    )
    const body = (await response.json()) as { data: ReadonlyArray<unknown> }
    expect(body.data.length).toBe(MODEL_PRICING_TABLE.length)
  })

  it('advertises only servable models when arming is supplied', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' }),
      },
    )
    const body = (await response.json()) as {
      data: ReadonlyArray<{ id: string; oa_lane: string }>
    }
    expect(body.data.length).toBeGreaterThan(0)
    expect(body.data.length).toBeLessThan(MODEL_PRICING_TABLE.length)
    expect(body.data.some(m => m.id === geminiId)).toBe(true)
    expect(body.data.some(m => m.id === KHALA_MINI_MODEL_ID)).toBe(true)
    expect(body.data.some(m => m.id === KHALA_CODE_MODEL_ID)).toBe(false)
    expect(body.data.some(m => m.id === HYDRALISK_GPT_OSS_20B_MODEL_ID)).toBe(
      false,
    )
    expect(body.data.some(m => m.id === fireworksId)).toBe(false)
    expect(body.data.every(m => m.oa_lane.startsWith('vertex-'))).toBe(true)
  })

  it('advertises no models when no lane credential is provisioned', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: true, laneArming: resolveSupplyLaneArming({}) },
    )
    const body = (await response.json()) as { data: ReadonlyArray<unknown> }
    expect(body.data).toEqual([])
  })

  it('retrieves a model on an armed lane', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${geminiId}`, { method: 'GET' }),
      geminiId,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' }),
      },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { id: string }
    expect(body.id).toBe(geminiId)
  })

  it('retrieves the Khala mini virtual model on its armed backing lane', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${KHALA_MINI_MODEL_ID}`, {
        method: 'GET',
      }),
      KHALA_MINI_MODEL_ID,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' }),
      },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { id: string; oa_lane: string }
    expect(body.id).toBe(KHALA_MINI_MODEL_ID)
    expect(body.oa_lane).toBe('vertex-gemini')
  })

  it('retrieves the Khala code virtual model on its armed coding lane', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${KHALA_CODE_MODEL_ID}`, {
        method: 'GET',
      }),
      KHALA_CODE_MODEL_ID,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming({ FIREWORKS_API_KEY: 'fw' }),
      },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { id: string; oa_lane: string }
    expect(body.id).toBe(KHALA_CODE_MODEL_ID)
    expect(body.oa_lane).toBe('fireworks')
  })

  it('lists and retrieves the OpenAI GPT-OSS model id only when Hydralisk is armed', async () => {
    const hydraliskEnv = {
      HYDRALISK_BASE_URL: 'https://hydralisk.example.test',
      HYDRALISK_BEARER_TOKEN: 'secret-route-token',
      HYDRALISK_GPT_OSS_20B_ENABLED: 'ready',
      HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF:
        'preflight.hydralisk.gpt_oss_20b.l4.v1',
      HYDRALISK_GPT_OSS_20B_RECEIPT_REF:
        'receipt.hydralisk.gpt_oss_20b.l4.smoke.v1',
    }
    const list = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming(hydraliskEnv),
      },
    )
    const listBody = (await list.json()) as {
      data: ReadonlyArray<{ id: string; oa_lane: string }>
    }
    expect(listBody.data).toEqual([
      expect.objectContaining({
        id: HYDRALISK_GPT_OSS_20B_MODEL_ID,
        oa_lane: 'hydralisk',
      }),
    ])

    const retrieved = await runRetrieve(
      new Request(`https://x/v1/models/${HYDRALISK_GPT_OSS_20B_MODEL_ID}`, {
        method: 'GET',
      }),
      HYDRALISK_GPT_OSS_20B_MODEL_ID,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming(hydraliskEnv),
      },
    )
    expect(retrieved.status).toBe(200)
    const retrieveBody = (await retrieved.json()) as {
      id: string
      oa_lane: string
    }
    expect(retrieveBody.id).toBe(HYDRALISK_GPT_OSS_20B_MODEL_ID)
    expect(retrieveBody.oa_lane).toBe('hydralisk')
  })

  it('reports model_not_found for a known model on an UNARMED lane', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${fireworksId}`, { method: 'GET' }),
      fireworksId,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' }),
      },
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: { code: string } }
    expect(body.error.code).toBe('model_not_found')
  })
})
