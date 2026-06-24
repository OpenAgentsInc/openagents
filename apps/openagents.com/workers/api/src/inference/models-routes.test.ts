import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { buildModelCatalog } from './model-catalog'
import { DEFAULT_FREE_TIER_QUOTA } from './inference-free-tier-key'
import { resolveSupplyLaneArming } from './model-serving-policy'
import {
  handleModelRetrieve,
  handleModelsList,
  routeModelRetrieveRequest,
} from './models-routes'
import {
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  KHALA_CODE_MODEL_ID,
  KHALA_MODEL_ID,
  KHALA_PYLON_MINI_MODEL_ID,
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
      data: ReadonlyArray<{
        id: string
        created: number
        object: string
        oa_free_tier_eligible: boolean
        oa_free_tier: { eligible: boolean; maxRequestsPerDay: number | null }
      }>
    }
    expect(body.object).toBe('list')
    expect(body.data.length).toBe(1)
    expect(body.data.every(m => m.object === 'model')).toBe(true)
    expect(body.data.every(m => m.created === 1_700_000_000)).toBe(true)
    expect(body.data.map(m => m.id)).toEqual([KHALA_MODEL_ID])
    expect(body.data[0]!.oa_free_tier_eligible).toBe(false)
    expect(body.data[0]!.oa_free_tier.eligible).toBe(false)
    expect(body.data[0]!.oa_free_tier.maxRequestsPerDay).toBeNull()
  })

  it('advertises the Khala free-key quota when free API mode is armed', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      {
        enabled: true,
        freeTierEnabled: true,
        nowEpochSeconds: () => 1_700_000_000,
      },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      data: ReadonlyArray<{
        id: string
        oa_free_tier_eligible: boolean
        oa_free_tier: {
          eligible: boolean
          maxRequestsPerDay: number
          maxTokensPerDay: number
          window: string
        }
      }>
    }
    expect(body.data).toEqual([
      expect.objectContaining({
        id: KHALA_MODEL_ID,
        oa_free_tier_eligible: true,
        oa_free_tier: {
          eligible: true,
          maxRequestsPerDay: DEFAULT_FREE_TIER_QUOTA.maxRequestsPerDay,
          maxTokensPerDay: DEFAULT_FREE_TIER_QUOTA.maxTokensPerDay,
          reasonRef: 'reason.inference_free_tier.eligible',
          window: 'utc_day',
        },
      }),
    ])
  })
})

describe('handleModelRetrieve', () => {
  const servedModelId = KHALA_MODEL_ID

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
      oa_free_tier_eligible: boolean
      oa_free_tier: { eligible: boolean }
      oa_price: { inputUsdPerMtok: number }
    }
    expect(body.id).toBe(servedModelId)
    expect(body.object).toBe('model')
    expect(body.created).toBe(1_700_000_000)
    expect(body.oa_free_tier_eligible).toBe(false)
    expect(body.oa_free_tier.eligible).toBe(false)
    expect(typeof body.oa_price.inputUsdPerMtok).toBe('number')
  })

  it('retrieves Khala with free-key quota when free API mode is armed', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${servedModelId}`, { method: 'GET' }),
      servedModelId,
      {
        enabled: true,
        freeTierEnabled: true,
        nowEpochSeconds: () => 1_700_000_000,
      },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      id: string
      oa_free_tier_eligible: boolean
      oa_free_tier: {
        eligible: boolean
        maxRequestsPerDay: number
        maxTokensPerDay: number
        window: string
      }
    }
    expect(body.id).toBe(servedModelId)
    expect(body.oa_free_tier_eligible).toBe(true)
    expect(body.oa_free_tier).toEqual({
      eligible: true,
      maxRequestsPerDay: DEFAULT_FREE_TIER_QUOTA.maxRequestsPerDay,
      maxTokensPerDay: DEFAULT_FREE_TIER_QUOTA.maxTokensPerDay,
      reasonRef: 'reason.inference_free_tier.eligible',
      window: 'utc_day',
    })
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
  const servedModelId = KHALA_MODEL_ID

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
      new Request(`https://x/v1/models/${encodeURIComponent(servedModelId)}`, {
        method: 'GET',
      }),
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
      new Request(`https://x/v1/models/${encodeURIComponent(servedModelId)}`, {
        method: 'GET',
      }),
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
  const pylonEnv = {
    OPENAGENTS_NETWORK_ADMITTED_PYLON_REF:
      'gcloud.gswarm508-clean2-20260325044551-contrib',
    OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN: 'secret-route-token',
    OPENAGENTS_NETWORK_FABRIC_SERVE_URL: 'https://pylon-route.example.test',
    OPENAGENTS_NETWORK_GATEWAY_APPROVAL_REF:
      'approval.owner.khala.6089.gateway_route.2026_06_24',
    OPENAGENTS_NETWORK_GATEWAY_ROUTE_READY: 'ready',
    OPENAGENTS_NETWORK_REPLAY_CHALLENGE_REF:
      'challenge.pylon.serving.GuUBPkgNgLRtTCgkkO-s',
    OPENAGENTS_NETWORK_SERVING_PREFLIGHT_REF:
      'preflight.pylon.real_serving.ready.v0_1',
    OPENAGENTS_NETWORK_SERVING_RECEIPT_REF:
      'serve.pylon.gateway_proxy.cAR4xZXQagyw7yBsjeO6IG',
  }

  it('lists only the public Khala model when laneArming is omitted', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: true },
    )
    const body = (await response.json()) as {
      data: ReadonlyArray<{ id: string }>
    }
    expect(body.data.map(m => m.id)).toEqual([KHALA_MODEL_ID])
  })

  it('advertises no public model when only Vertex is armed', async () => {
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
    expect(body.data).toEqual([])
  })

  it('advertises no models when no lane credential is provisioned', async () => {
    const response = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: true, laneArming: resolveSupplyLaneArming({}) },
    )
    const body = (await response.json()) as { data: ReadonlyArray<unknown> }
    expect(body.data).toEqual([])
  })

  it('does not retrieve a non-Khala model even when its lane is armed', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${geminiId}`, { method: 'GET' }),
      geminiId,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' }),
      },
    )
    expect(response.status).toBe(404)
    const body = (await response.json()) as { error: { code: string } }
    expect(body.error.code).toBe('model_not_found')
  })

  it('retrieves the single Khala model on its armed Hydralisk backing lane', async () => {
    const response = await runRetrieve(
      new Request(`https://x/v1/models/${KHALA_MODEL_ID}`, {
        method: 'GET',
      }),
      KHALA_MODEL_ID,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming({
          HYDRALISK_BASE_URL: 'https://hydralisk.example.test',
          HYDRALISK_BEARER_TOKEN: 'secret-route-token',
          HYDRALISK_GPT_OSS_20B_ENABLED: 'ready',
          HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF:
            'preflight.hydralisk.gpt_oss_20b.l4.v1',
          HYDRALISK_GPT_OSS_20B_RECEIPT_REF:
            'receipt.hydralisk.gpt_oss_20b.l4.smoke.v1',
        }),
      },
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as { id: string; oa_lane: string }
    expect(body.id).toBe(KHALA_MODEL_ID)
    expect(body.oa_lane).toBe('hydralisk')
  })

  it('lists and retrieves only Khala while projecting the Fireworks DeepSeek backing price', async () => {
    const laneArming = resolveSupplyLaneArming({
      FIREWORKS_API_KEY: 'fw',
      KHALA_BACKING_MODEL: 'deepseek-v4-flash',
    })
    const list = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      { enabled: true, freeTierEnabled: true, laneArming },
    )
    expect(list.status).toBe(200)
    const listBody = (await list.json()) as {
      data: ReadonlyArray<{
        id: string
        oa_lane: string
        oa_free_tier_eligible: boolean
        oa_free_tier: { eligible: boolean; maxRequestsPerDay: number }
        owned_by: string
        oa_price: { inputUsdPerMtok: number }
      }>
    }
    expect(listBody.data).toEqual([
      expect.objectContaining({
        id: KHALA_MODEL_ID,
        oa_lane: 'fireworks',
        oa_free_tier_eligible: true,
        oa_free_tier: expect.objectContaining({
          eligible: true,
          maxRequestsPerDay: DEFAULT_FREE_TIER_QUOTA.maxRequestsPerDay,
        }),
        owned_by: 'openagents/fireworks',
      }),
    ])
    expect(listBody.data.map(model => model.id)).not.toContain(
      'deepseek-v4-flash',
    )

    const retrieved = await runRetrieve(
      new Request(`https://x/v1/models/${KHALA_MODEL_ID}`, { method: 'GET' }),
      KHALA_MODEL_ID,
      { enabled: true, freeTierEnabled: true, laneArming },
    )
    expect(retrieved.status).toBe(200)
    const body = (await retrieved.json()) as {
      id: string
      oa_lane: string
      oa_free_tier_eligible: boolean
      oa_free_tier: { eligible: boolean; maxRequestsPerDay: number }
      owned_by: string
      oa_price: { inputUsdPerMtok: number }
    }
    expect(body.id).toBe(KHALA_MODEL_ID)
    expect(body.oa_lane).toBe('fireworks')
    expect(body.oa_free_tier_eligible).toBe(true)
    expect(body.oa_free_tier.maxRequestsPerDay).toBe(
      DEFAULT_FREE_TIER_QUOTA.maxRequestsPerDay,
    )
    expect(body.owned_by).toBe('openagents/fireworks')
    expect(body.oa_price.inputUsdPerMtok).toBe(0.196)
  })

  it('does not retrieve the old Khala code split model', async () => {
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
    expect(response.status).toBe(404)
  })

  it('does not list or retrieve the raw OpenAI GPT-OSS model id even when Hydralisk is armed', async () => {
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
        id: KHALA_MODEL_ID,
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
    expect(retrieved.status).toBe(404)
  })

  it('does not list or retrieve raw GPT-OSS 120B even when its high-memory Hydralisk lane is armed', async () => {
    const hydralisk120bEnv = {
      HYDRALISK_GPT_OSS_120B_BASE_URL: 'https://hydralisk-120b.example.test',
      HYDRALISK_GPT_OSS_120B_BEARER_TOKEN: 'secret-route-token',
      HYDRALISK_GPT_OSS_120B_ENABLED: 'ready',
      HYDRALISK_GPT_OSS_120B_PREFLIGHT_REF:
        'preflight.hydralisk.gpt_oss_120b.h100.v1',
      HYDRALISK_GPT_OSS_120B_RECEIPT_REF:
        'receipt.hydralisk.gpt_oss_120b.h100.smoke.v1',
    }
    const list = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming(hydralisk120bEnv),
      },
    )
    const listBody = (await list.json()) as {
      data: ReadonlyArray<{ id: string; oa_lane: string }>
    }
    expect(listBody.data).toEqual([
      expect.objectContaining({
        id: KHALA_MODEL_ID,
        oa_lane: 'hydralisk',
      }),
    ])

    const retrieved = await runRetrieve(
      new Request(`https://x/v1/models/${HYDRALISK_GPT_OSS_120B_MODEL_ID}`, {
        method: 'GET',
      }),
      HYDRALISK_GPT_OSS_120B_MODEL_ID,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming(hydralisk120bEnv),
      },
    )
    expect(retrieved.status).toBe(404)
  })

  it('does not list or retrieve the old Khala Pylon canary split alias', async () => {
    const list = await run(
      new Request('https://x/v1/models', { method: 'GET' }),
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming(pylonEnv),
      },
    )
    const listBody = (await list.json()) as {
      data: ReadonlyArray<{ id: string; oa_lane: string }>
    }
    expect(listBody.data).toEqual([])

    const retrieved = await runRetrieve(
      new Request(`https://x/v1/models/${KHALA_PYLON_MINI_MODEL_ID}`, {
        method: 'GET',
      }),
      KHALA_PYLON_MINI_MODEL_ID,
      {
        enabled: true,
        laneArming: resolveSupplyLaneArming(pylonEnv),
      },
    )
    expect(retrieved.status).toBe(404)
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
