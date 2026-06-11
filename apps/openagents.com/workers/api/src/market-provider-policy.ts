export type MarketProviderCapacityState =
  | 'idle'
  | 'offline'
  | 'serving_market_work'
  | 'serving_owned_work'

export type MarketProviderModeConfig = Readonly<{
  allowedCapabilityRefs: ReadonlyArray<string>
  capacityState: MarketProviderCapacityState
  enabled: boolean
  earningsVisibilityRef?: string
  maxJobSats: number
  minQuoteSats: number
  ownerConsentRef?: string
  ownWorkPreemption: boolean
  pricingPolicyRef?: string
  publicProviderRef?: string
  settlementBridgeReady: boolean
}>

export type MarketProviderModeDecision = Readonly<{
  allowedCapabilityRefs: ReadonlyArray<string>
  marketProviderRef?: string
  reasonRefs: ReadonlyArray<string>
  state: 'blocked' | 'offline' | 'preempted' | 'ready'
}>

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => [
  ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
]

const add = (
  reasons: Set<string>,
  condition: boolean,
  reason: string,
): void => {
  if (condition) {
    reasons.add(reason)
  }
}

export const evaluateMarketProviderMode = (
  config: Partial<MarketProviderModeConfig> = {},
): MarketProviderModeDecision => {
  const reasons = new Set<string>()
  const allowedCapabilityRefs = uniqueRefs(config.allowedCapabilityRefs)
  const capacityState = config.capacityState ?? 'offline'
  const ownerConsentRef = config.ownerConsentRef?.trim() ?? ''
  const pricingPolicyRef = config.pricingPolicyRef?.trim() ?? ''
  const earningsVisibilityRef = config.earningsVisibilityRef?.trim() ?? ''
  const publicProviderRef = config.publicProviderRef?.trim() ?? ''
  const minQuoteSats = config.minQuoteSats ?? 0
  const maxJobSats = config.maxJobSats ?? 0

  if (config.enabled !== true) {
    return {
      allowedCapabilityRefs,
      reasonRefs: ['market_provider.default_off'],
      state: 'offline',
    }
  }

  add(reasons, ownerConsentRef === '', 'market_provider.owner_consent_missing')
  add(reasons, publicProviderRef === '', 'market_provider.public_ref_missing')
  add(
    reasons,
    pricingPolicyRef === '',
    'market_provider.pricing_policy_missing',
  )
  add(reasons, minQuoteSats <= 0, 'market_provider.min_quote_missing')
  add(reasons, maxJobSats <= 0, 'market_provider.max_job_budget_missing')
  add(
    reasons,
    minQuoteSats > 0 && maxJobSats > 0 && minQuoteSats > maxJobSats,
    'market_provider.min_quote_exceeds_max_job',
  )
  add(
    reasons,
    allowedCapabilityRefs.length === 0,
    'market_provider.capabilities_missing',
  )
  add(
    reasons,
    config.ownWorkPreemption !== true,
    'market_provider.own_work_preemption_required',
  )
  add(
    reasons,
    config.settlementBridgeReady !== true,
    'market_provider.settlement_bridge_missing',
  )
  add(
    reasons,
    earningsVisibilityRef === '',
    'market_provider.earnings_visibility_missing',
  )

  if (
    capacityState === 'serving_owned_work' &&
    config.ownWorkPreemption === true
  ) {
    return {
      allowedCapabilityRefs,
      ...(publicProviderRef === ''
        ? {}
        : { marketProviderRef: publicProviderRef }),
      reasonRefs: [
        ...new Set([...reasons, 'market_provider.owned_work_preempts_market']),
      ].sort(),
      state: 'preempted',
    }
  }

  add(
    reasons,
    capacityState !== 'idle',
    `market_provider.capacity_not_idle.${capacityState}`,
  )

  if (reasons.size > 0) {
    return {
      allowedCapabilityRefs,
      ...(publicProviderRef === ''
        ? {}
        : { marketProviderRef: publicProviderRef }),
      reasonRefs: [...reasons].sort(),
      state: 'blocked',
    }
  }

  return {
    allowedCapabilityRefs,
    marketProviderRef: publicProviderRef,
    reasonRefs: ['market_provider.ready_for_open_market_quotes'],
    state: 'ready',
  }
}
