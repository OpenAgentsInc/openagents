import { describe, expect, test } from 'vitest'

import {
  type MarketProviderModeConfig,
  evaluateMarketProviderMode,
} from './market-provider-policy'

const readyConfig = {
  allowedCapabilityRefs: ['capability.pylon.local_claude_agent'],
  capacityState: 'idle',
  earningsVisibilityRef: 'projection.public.pylon_earnings.provider_1',
  enabled: true,
  maxJobSats: 5_000,
  minQuoteSats: 1_000,
  ownerConsentRef: 'receipt.provider_mode.owner_consent.provider_1',
  ownWorkPreemption: true,
  pricingPolicyRef: 'policy.provider_pricing.provider_1',
  publicProviderRef: 'provider.public.pylon.provider_1',
  settlementBridgeReady: true,
} satisfies MarketProviderModeConfig

describe('market provider mode', () => {
  test('is default-off until the owner explicitly goes online', () => {
    expect(evaluateMarketProviderMode()).toEqual({
      allowedCapabilityRefs: [],
      reasonRefs: ['market_provider.default_off'],
      state: 'offline',
    })
  })

  test('requires consent, pricing, capability, settlement, and earnings visibility refs', () => {
    const decision = evaluateMarketProviderMode({
      ...readyConfig,
      allowedCapabilityRefs: [],
      earningsVisibilityRef: '',
      maxJobSats: 0,
      ownerConsentRef: '',
      pricingPolicyRef: '',
      publicProviderRef: '',
      settlementBridgeReady: false,
    })

    expect(decision.state).toBe('blocked')
    expect(decision.reasonRefs).toEqual([
      'market_provider.capabilities_missing',
      'market_provider.earnings_visibility_missing',
      'market_provider.max_job_budget_missing',
      'market_provider.owner_consent_missing',
      'market_provider.pricing_policy_missing',
      'market_provider.public_ref_missing',
      'market_provider.settlement_bridge_missing',
    ])
  })

  test('preempts open-market serving when owned work needs the same pylon', () => {
    const decision = evaluateMarketProviderMode({
      ...readyConfig,
      capacityState: 'serving_owned_work',
    })

    expect(decision).toMatchObject({
      marketProviderRef: 'provider.public.pylon.provider_1',
      reasonRefs: ['market_provider.owned_work_preempts_market'],
      state: 'preempted',
    })
  })

  test('admits idle pylons only after every paid-provider gate is satisfied', () => {
    expect(evaluateMarketProviderMode(readyConfig)).toEqual({
      allowedCapabilityRefs: ['capability.pylon.local_claude_agent'],
      marketProviderRef: 'provider.public.pylon.provider_1',
      reasonRefs: ['market_provider.ready_for_open_market_quotes'],
      state: 'ready',
    })
  })
})
