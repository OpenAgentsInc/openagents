import { describe, expect, it } from 'vitest'

import { buildModelCatalog } from './model-catalog'
import {
  ALL_LANES_UNARMED,
  filterServableCatalog,
  isLaneArmed,
  isModelServable,
  resolveHydraliskGptOss20bArming,
  resolveOpenAgentsNetworkGatewayArming,
  resolveNamedModelServability,
  resolveSupplyLaneArming,
  type SupplyLaneArming,
} from './model-serving-policy'
import type { SupplyLane } from './pricing'

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
  HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF:
    'preflight.hydralisk.gpt_oss_20b.l4.v1',
  HYDRALISK_GPT_OSS_20B_RECEIPT_REF:
    'receipt.hydralisk.gpt_oss_20b.l4.smoke.v1',
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

  it('arms Hydralisk only from ready flag, transport presence, and public-safe refs', () => {
    const arming = resolveSupplyLaneArming(HYDRALISK_READY_ENV)
    expect(arming.hydralisk).toBe(true)
    expect(arming.fireworks).toBe(false)
    expect(arming['openagents-network']).toBe(false)
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

describe('resolveNamedModelServability', () => {
  it('returns true for a known model on an armed lane', () => {
    expect(resolveNamedModelServability('gemini-3.5-flash', ALL_ARMED)).toBe(
      true,
    )
  })

  it('returns false for a known model on an unarmed lane', () => {
    expect(
      resolveNamedModelServability('gemini-3.5-flash', ALL_LANES_UNARMED),
    ).toBe(false)
  })

  it('returns undefined for an unknown model id (not gated)', () => {
    expect(
      resolveNamedModelServability('not-a-real-model', ALL_ARMED),
    ).toBeUndefined()
  })

  it('resolves case-insensitively (lookup keys on the canonical id)', () => {
    expect(resolveNamedModelServability('GEMINI-3.5-FLASH', ALL_ARMED)).toBe(
      true,
    )
    expect(
      resolveNamedModelServability('GEMINI-3.5-FLASH', ALL_LANES_UNARMED),
    ).toBe(false)
  })
})
