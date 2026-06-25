import { describe, expect, it } from 'vitest'

import { buildModelCatalog } from './model-catalog'
import {
  ALL_LANES_UNARMED,
  KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH,
  KHALA_FIREWORKS_DEEPSEEK_V4_FLASH_PRICE_MODEL,
  type SupplyLaneArming,
  filterServableCatalog,
  isKhalaBackingArmed,
  isLaneArmed,
  isModelServable,
  khalaBackingPriceModel,
  khalaBackingSupplyLane,
  projectKhalaCatalogForArming,
  resolveHydraliskGlm52Reap504bArming,
  resolveHydraliskGptOss20bArming,
  resolveHydraliskGptOss120bArming,
  resolveKhalaBackingModel,
  resolveNamedModelServability,
  resolveOpenAgentsNetworkGatewayArming,
  resolveSupplyLaneArming,
} from './model-serving-policy'
import {
  HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  KHALA_MODEL_ID,
  type SupplyLane,
} from './pricing'

const ALL_ARMED: SupplyLaneArming = {
  fireworks: true,
  hydralisk: true,
  'openagents-network': true,
  'vertex-anthropic': true,
  'vertex-gemini': true,
}

const HYDRALISK_READY_ENV = {
  HYDRALISK_BASE_URL: 'https://hydralisk-gpt-oss-20b.example.test',
  HYDRALISK_BEARER_TOKEN: 'secret-hydralisk-token',
  HYDRALISK_GPT_OSS_20B_ENABLED: 'ready',
  HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF: 'preflight.hydralisk.gpt_oss_20b.l4.v1',
  HYDRALISK_GPT_OSS_20B_RECEIPT_REF:
    'receipt.hydralisk.gpt_oss_20b.l4.smoke.v1',
} as const

const HYDRALISK_GLM_52_REAP_READY_ENV = {
  HYDRALISK_GLM_52_REAP_504B_BASE_URL:
    'https://hydralisk-glm-52-reap-504b.example.test',
  HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN: 'secret-hydralisk-glm-token',
  HYDRALISK_GLM_52_REAP_504B_ENABLED: 'ready',
  HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF:
    'preflight.hydralisk.glm_52_reap_504b.g4.mtp2.v1',
  HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF:
    'receipt.hydralisk.glm_52_reap_504b.g4.mtp2_smoke.v1',
} as const

const HYDRALISK_120B_READY_ENV = {
  HYDRALISK_GPT_OSS_120B_BASE_URL:
    'https://hydralisk-gpt-oss-120b.example.test',
  HYDRALISK_GPT_OSS_120B_BEARER_TOKEN: 'secret-hydralisk-120b-token',
  HYDRALISK_GPT_OSS_120B_ENABLED: 'ready',
  HYDRALISK_GPT_OSS_120B_PREFLIGHT_REF:
    'preflight.hydralisk.gpt_oss_120b.h100.v1',
  HYDRALISK_GPT_OSS_120B_RECEIPT_REF:
    'receipt.hydralisk.gpt_oss_120b.h100.smoke.v1',
} as const

const OPENAGENTS_NETWORK_READY_ENV = {
  OPENAGENTS_NETWORK_ADMITTED_PYLON_REF:
    'gcloud.gswarm508-clean2-20260325044551-contrib',
  OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN: 'secret-route-token',
  OPENAGENTS_NETWORK_FABRIC_SERVE_URL: 'https://pylon-route.example.test/serve',
  OPENAGENTS_NETWORK_GATEWAY_APPROVAL_REF:
    'approval.owner.khala.6089.gateway_route.2026_06_23',
  OPENAGENTS_NETWORK_GATEWAY_ROUTE_READY: 'ready',
  OPENAGENTS_NETWORK_REPLAY_CHALLENGE_REF:
    'challenge.pylon.serving.GuUBPkgNgLRtTCgkkO-s',
  OPENAGENTS_NETWORK_SERVING_PREFLIGHT_REF:
    'preflight.pylon.real_serving.ready.v0_1',
  OPENAGENTS_NETWORK_SERVING_RECEIPT_REF:
    'receipt.pylon.serving.OWtQlHDIdRmCvGpoOUt8',
} as const

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

  it('selects Fireworks DeepSeek V4 Flash as the Khala backing only from the explicit operator value', () => {
    const arming = resolveSupplyLaneArming({
      FIREWORKS_API_KEY: 'fw-secret',
      KHALA_BACKING_MODEL: 'deepseek-v4-flash',
    })
    expect(arming.khalaBacking).toBe(KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH)
    expect(
      resolveKhalaBackingModel('accounts/fireworks/models/deepseek-v4-flash'),
    ).toBe(KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH)
    expect(khalaBackingSupplyLane(arming)).toBe('fireworks')
    expect(khalaBackingPriceModel(arming)).toBe(
      KHALA_FIREWORKS_DEEPSEEK_V4_FLASH_PRICE_MODEL,
    )
    expect(isKhalaBackingArmed(arming)).toBe(true)
  })

  it('arms Hydralisk only from ready flag, transport presence, and public-safe refs', () => {
    const arming = resolveSupplyLaneArming(HYDRALISK_READY_ENV)
    expect(arming.hydralisk).toBe(true)
    expect(arming.hydraliskModels?.[HYDRALISK_GPT_OSS_20B_MODEL_ID]).toBe(true)
    expect(arming.hydraliskModels?.[HYDRALISK_GPT_OSS_120B_MODEL_ID]).toBe(
      false,
    )
    expect(arming.fireworks).toBe(false)
    expect(arming['openagents-network']).toBe(false)
  })

  it('arms GLM-5.2 REAP independently as a Hydralisk Khala backing lane', () => {
    const arming = resolveSupplyLaneArming(HYDRALISK_GLM_52_REAP_READY_ENV)
    expect(arming.hydralisk).toBe(true)
    expect(arming.hydraliskModels?.[HYDRALISK_GLM_52_REAP_504B_MODEL_ID]).toBe(
      true,
    )
    expect(arming.hydraliskModels?.[HYDRALISK_GPT_OSS_120B_MODEL_ID]).toBe(
      false,
    )
    expect(arming.hydraliskModels?.[HYDRALISK_GPT_OSS_20B_MODEL_ID]).toBe(false)
    expect(arming.fireworks).toBe(false)
  })

  it('arms GPT-OSS 120B Hydralisk independently from the 20B L4 lane', () => {
    const arming = resolveSupplyLaneArming(HYDRALISK_120B_READY_ENV)
    expect(arming.hydralisk).toBe(true)
    expect(arming.hydraliskModels?.[HYDRALISK_GPT_OSS_120B_MODEL_ID]).toBe(true)
    expect(arming.hydraliskModels?.[HYDRALISK_GPT_OSS_20B_MODEL_ID]).toBe(false)
    expect(arming.fireworks).toBe(false)
  })

  it('treats a blank/whitespace credential as absent', () => {
    expect(resolveSupplyLaneArming({ VERTEX_SA_KEY: '   ' })).toEqual(
      ALL_LANES_UNARMED,
    )
  })

  it('keeps the openagents-network lane unarmed without its route evidence', () => {
    const arming = resolveSupplyLaneArming({
      FIREWORKS_API_KEY: 'fw',
      VERTEX_SA_KEY: 'sa',
    })
    expect(arming['openagents-network']).toBe(false)
  })

  it('arms openagents-network only from route-ready plus public-safe evidence refs', () => {
    const arming = resolveSupplyLaneArming(OPENAGENTS_NETWORK_READY_ENV)
    expect(arming['openagents-network']).toBe(true)
    expect(arming.fireworks).toBe(false)
    expect(arming['vertex-gemini']).toBe(false)
  })
})

describe('resolveOpenAgentsNetworkGatewayArming', () => {
  it('fails closed with typed blockers when route evidence is absent', () => {
    const arming = resolveOpenAgentsNetworkGatewayArming({})
    expect(arming.armed).toBe(false)
    expect(arming.evidenceRefs).toEqual([])
    expect(arming.blockerRefs).toEqual([
      'blocker.openagents_network_gateway.route_not_ready',
      'blocker.openagents_network_gateway.transport_url_missing',
      'blocker.openagents_network_gateway.transport_bearer_missing',
      'blocker.openagents_network_gateway.approval_ref_missing',
      'blocker.openagents_network_gateway.serving_preflight_ref_missing',
      'blocker.openagents_network_gateway.serving_receipt_ref_missing',
      'blocker.openagents_network_gateway.replay_challenge_ref_missing',
      'blocker.openagents_network_gateway.admitted_pylon_ref_missing',
    ])
  })

  it('requires the exact route-ready token, not a truthy string', () => {
    const arming = resolveOpenAgentsNetworkGatewayArming({
      ...OPENAGENTS_NETWORK_READY_ENV,
      OPENAGENTS_NETWORK_GATEWAY_ROUTE_READY: 'true',
    })
    expect(arming.armed).toBe(false)
    expect(arming.blockerRefs).toContain(
      'blocker.openagents_network_gateway.route_not_ready',
    )
  })

  it('requires the secret-backed transport URL and bearer token by presence only', () => {
    const noUrl = resolveOpenAgentsNetworkGatewayArming({
      ...OPENAGENTS_NETWORK_READY_ENV,
      OPENAGENTS_NETWORK_FABRIC_SERVE_URL: '   ',
    })
    expect(noUrl.armed).toBe(false)
    expect(noUrl.blockerRefs).toContain(
      'blocker.openagents_network_gateway.transport_url_missing',
    )

    const noBearer = resolveOpenAgentsNetworkGatewayArming({
      ...OPENAGENTS_NETWORK_READY_ENV,
      OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN: '',
    })
    expect(noBearer.armed).toBe(false)
    expect(noBearer.blockerRefs).toContain(
      'blocker.openagents_network_gateway.transport_bearer_missing',
    )
  })

  it('rejects endpoint-shaped or secret-shaped values as evidence refs', () => {
    const endpoint = resolveOpenAgentsNetworkGatewayArming({
      ...OPENAGENTS_NETWORK_READY_ENV,
      OPENAGENTS_NETWORK_SERVING_RECEIPT_REF:
        'https://10.42.11.3:8000/v1/chat/completions',
    })
    expect(endpoint.armed).toBe(false)
    expect(endpoint.blockerRefs).toContain(
      'blocker.openagents_network_gateway.serving_receipt_ref_missing',
    )

    const secret = resolveOpenAgentsNetworkGatewayArming({
      ...OPENAGENTS_NETWORK_READY_ENV,
      OPENAGENTS_NETWORK_GATEWAY_APPROVAL_REF: 'sk-not-a-public-ref',
    })
    expect(secret.armed).toBe(false)
    expect(secret.blockerRefs).toContain(
      'blocker.openagents_network_gateway.approval_ref_missing',
    )
  })

  it('returns only public-safe refs when the gateway route evidence is complete', () => {
    const arming = resolveOpenAgentsNetworkGatewayArming(
      OPENAGENTS_NETWORK_READY_ENV,
    )
    expect(arming.armed).toBe(true)
    expect(arming.blockerRefs).toEqual([])
    expect(arming.evidenceRefs).toEqual([
      'approval.owner.khala.6089.gateway_route.2026_06_23',
      'preflight.pylon.real_serving.ready.v0_1',
      'receipt.pylon.serving.OWtQlHDIdRmCvGpoOUt8',
      'challenge.pylon.serving.GuUBPkgNgLRtTCgkkO-s',
      'gcloud.gswarm508-clean2-20260325044551-contrib',
    ])
  })
})

describe('resolveHydraliskGptOss20bArming', () => {
  it('fails closed with typed blockers when route evidence is absent', () => {
    const arming = resolveHydraliskGptOss20bArming({})
    expect(arming.armed).toBe(false)
    expect(arming.evidenceRefs).toEqual([])
    expect(arming.blockerRefs).toEqual([
      'blocker.hydralisk_gpt_oss_20b.route_not_ready',
      'blocker.hydralisk_gpt_oss_20b.base_url_missing',
      'blocker.hydralisk_gpt_oss_20b.bearer_missing',
      'blocker.hydralisk_gpt_oss_20b.preflight_ref_missing',
      'blocker.hydralisk_gpt_oss_20b.receipt_ref_missing',
    ])
  })

  it('requires the exact ready token and public-safe evidence refs', () => {
    const wrongFlag = resolveHydraliskGptOss20bArming({
      ...HYDRALISK_READY_ENV,
      HYDRALISK_GPT_OSS_20B_ENABLED: 'true',
    })
    expect(wrongFlag.armed).toBe(false)
    expect(wrongFlag.blockerRefs).toContain(
      'blocker.hydralisk_gpt_oss_20b.route_not_ready',
    )

    const endpointRef = resolveHydraliskGptOss20bArming({
      ...HYDRALISK_READY_ENV,
      HYDRALISK_GPT_OSS_20B_RECEIPT_REF:
        'https://hydralisk.example.test/hydralisk/v1/receipts/x',
    })
    expect(endpointRef.armed).toBe(false)
    expect(endpointRef.blockerRefs).toContain(
      'blocker.hydralisk_gpt_oss_20b.receipt_ref_missing',
    )
  })

  it('returns only public-safe refs when the lane evidence is complete', () => {
    const arming = resolveHydraliskGptOss20bArming(HYDRALISK_READY_ENV)
    expect(arming.armed).toBe(true)
    expect(arming.blockerRefs).toEqual([])
    expect(arming.evidenceRefs).toEqual([
      'preflight.hydralisk.gpt_oss_20b.l4.v1',
      'receipt.hydralisk.gpt_oss_20b.l4.smoke.v1',
    ])
  })
})

describe('resolveHydraliskGlm52Reap504bArming', () => {
  it('fails closed with typed blockers when GLM route evidence is absent', () => {
    const arming = resolveHydraliskGlm52Reap504bArming({})
    expect(arming.armed).toBe(false)
    expect(arming.evidenceRefs).toEqual([])
    expect(arming.blockerRefs).toEqual([
      'blocker.hydralisk_glm_52_reap_504b.route_not_ready',
      'blocker.hydralisk_glm_52_reap_504b.base_url_missing',
      'blocker.hydralisk_glm_52_reap_504b.bearer_missing',
      'blocker.hydralisk_glm_52_reap_504b.preflight_ref_missing',
      'blocker.hydralisk_glm_52_reap_504b.receipt_ref_missing',
    ])
  })

  it('returns only public-safe refs when the GLM route evidence is complete', () => {
    const arming = resolveHydraliskGlm52Reap504bArming(
      HYDRALISK_GLM_52_REAP_READY_ENV,
    )
    expect(arming.armed).toBe(true)
    expect(arming.blockerRefs).toEqual([])
    expect(arming.evidenceRefs).toEqual([
      'preflight.hydralisk.glm_52_reap_504b.g4.mtp2.v1',
      'receipt.hydralisk.glm_52_reap_504b.g4.mtp2_smoke.v1',
    ])
  })
})

describe('resolveHydraliskGptOss120bArming', () => {
  it('fails closed with typed blockers when high-memory route evidence is absent', () => {
    const arming = resolveHydraliskGptOss120bArming({})
    expect(arming.armed).toBe(false)
    expect(arming.evidenceRefs).toEqual([])
    expect(arming.blockerRefs).toEqual([
      'blocker.hydralisk_gpt_oss_120b.route_not_ready',
      'blocker.hydralisk_gpt_oss_120b.base_url_missing',
      'blocker.hydralisk_gpt_oss_120b.bearer_missing',
      'blocker.hydralisk_gpt_oss_120b.preflight_ref_missing',
      'blocker.hydralisk_gpt_oss_120b.receipt_ref_missing',
    ])
  })

  it('returns only public-safe refs when the high-memory lane evidence is complete', () => {
    const arming = resolveHydraliskGptOss120bArming(HYDRALISK_120B_READY_ENV)
    expect(arming.armed).toBe(true)
    expect(arming.blockerRefs).toEqual([])
    expect(arming.evidenceRefs).toEqual([
      'preflight.hydralisk.gpt_oss_120b.h100.v1',
      'receipt.hydralisk.gpt_oss_120b.h100.smoke.v1',
    ])
  })
})

describe('isLaneArmed / isModelServable', () => {
  const lanes: ReadonlyArray<SupplyLane> = [
    'fireworks',
    'hydralisk',
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

  it('only the single public Khala model is servable when its backing lane is armed', () => {
    const catalog = buildModelCatalog()
    const khala = catalog.find(e => e.id === KHALA_MODEL_ID)!
    const gemini = catalog.find(e => e.id === 'gemini-3.5-flash')!
    expect(isModelServable(khala, ALL_ARMED)).toBe(true)
    expect(isModelServable(khala, ALL_LANES_UNARMED)).toBe(false)
    expect(isModelServable(gemini, ALL_ARMED)).toBe(false)
    expect(
      isModelServable(gemini, resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' })),
    ).toBe(false)
    expect(
      isModelServable(
        gemini,
        resolveSupplyLaneArming({ FIREWORKS_API_KEY: 'fw' }),
      ),
    ).toBe(false)
  })

  it('does not publish raw Hydralisk model ids even when their lanes are armed', () => {
    const catalog = buildModelCatalog()
    const twentyB = catalog.find(e => e.id === HYDRALISK_GPT_OSS_20B_MODEL_ID)!
    const glmReap = catalog.find(
      e => e.id === HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
    )
    const oneTwentyB = catalog.find(
      e => e.id === HYDRALISK_GPT_OSS_120B_MODEL_ID,
    )!
    const twentyBArming = resolveSupplyLaneArming(HYDRALISK_READY_ENV)
    const glmArming = resolveSupplyLaneArming(HYDRALISK_GLM_52_REAP_READY_ENV)
    expect(glmReap).toBeUndefined()
    expect(isModelServable(twentyB, twentyBArming)).toBe(false)
    expect(isModelServable(oneTwentyB, twentyBArming)).toBe(false)
    expect(
      resolveNamedModelServability(
        HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
        glmArming,
      ),
    ).toBe(false)
  })
})

describe('filterServableCatalog', () => {
  const catalog = buildModelCatalog()

  it('keeps only the single public model when every backing lane is armed', () => {
    expect(
      filterServableCatalog(catalog, ALL_ARMED).map(entry => entry.id),
    ).toEqual([KHALA_MODEL_ID])
  })

  it('is empty when no lane is armed', () => {
    expect(filterServableCatalog(catalog, ALL_LANES_UNARMED)).toEqual([])
  })

  it('keeps no public model when only VERTEX_SA_KEY is present', () => {
    const armed = resolveSupplyLaneArming({ VERTEX_SA_KEY: 'sa' })
    const filtered = filterServableCatalog(catalog, armed)
    expect(filtered).toEqual([])
  })

  it('keeps no public model when only FIREWORKS_API_KEY is present', () => {
    const armed = resolveSupplyLaneArming({ FIREWORKS_API_KEY: 'fw' })
    const filtered = filterServableCatalog(catalog, armed)
    expect(filtered).toEqual([])
  })

  it('projects the single Khala public model onto the Fireworks DeepSeek backing when explicitly armed', () => {
    const armed = resolveSupplyLaneArming({
      FIREWORKS_API_KEY: 'fw',
      KHALA_BACKING_MODEL: 'deepseek-v4-flash',
    })
    const filtered = filterServableCatalog(catalog, armed)
    expect(filtered).toEqual([
      expect.objectContaining({
        id: KHALA_MODEL_ID,
        lane: 'fireworks',
        ownedBy: 'openagents/fireworks',
      }),
    ])
    expect(
      projectKhalaCatalogForArming(catalog, armed).find(
        entry => entry.id === KHALA_MODEL_ID,
      )?.price,
    ).toEqual(
      catalog.find(
        entry => entry.id === KHALA_FIREWORKS_DEEPSEEK_V4_FLASH_PRICE_MODEL,
      )?.price,
    )
  })

  it('preserves public catalog order', () => {
    const filtered = filterServableCatalog(catalog, ALL_ARMED)
    expect(filtered.map(e => e.id)).toEqual([KHALA_MODEL_ID])
  })
})

describe('resolveNamedModelServability', () => {
  it('returns true for the public Khala model on an armed backing lane', () => {
    expect(resolveNamedModelServability(KHALA_MODEL_ID, ALL_ARMED)).toBe(true)
  })

  it('returns false for the public Khala model on an unarmed backing lane', () => {
    expect(
      resolveNamedModelServability(KHALA_MODEL_ID, ALL_LANES_UNARMED),
    ).toBe(false)
  })

  it('returns false for an unknown model id (no public model selection)', () => {
    expect(resolveNamedModelServability('not-a-real-model', ALL_ARMED)).toBe(
      false,
    )
  })

  it('resolves the public id case-insensitively', () => {
    expect(resolveNamedModelServability('OPENAGENTS/KHALA', ALL_ARMED)).toBe(
      true,
    )
    expect(
      resolveNamedModelServability('OPENAGENTS/KHALA', ALL_LANES_UNARMED),
    ).toBe(false)
  })

  it('rejects raw Hydralisk model ids even when their backing routes are armed', () => {
    const twentyBArming = resolveSupplyLaneArming(HYDRALISK_READY_ENV)
    const glmArming = resolveSupplyLaneArming(HYDRALISK_GLM_52_REAP_READY_ENV)
    const oneTwentyBArming = resolveSupplyLaneArming(HYDRALISK_120B_READY_ENV)
    expect(
      resolveNamedModelServability(
        HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
        glmArming,
      ),
    ).toBe(false)
    expect(
      resolveNamedModelServability(
        HYDRALISK_GPT_OSS_20B_MODEL_ID,
        twentyBArming,
      ),
    ).toBe(false)
    expect(
      resolveNamedModelServability(
        HYDRALISK_GPT_OSS_120B_MODEL_ID,
        twentyBArming,
      ),
    ).toBe(false)
    expect(
      resolveNamedModelServability(
        HYDRALISK_GPT_OSS_120B_MODEL_ID,
        oneTwentyBArming,
      ),
    ).toBe(false)
  })
})
