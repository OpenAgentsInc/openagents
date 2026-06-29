import type {
  AutopilotWorkBudgetAction,
  AutopilotWorkBudgetState,
  AutopilotWorkBudgetThreshold,
  AutopilotWorkProjection,
  AutopilotWorkUsageBudget,
  AutopilotWorkUsageCostEstimate,
  AutopilotWorkUsagePricingState,
  AutopilotWorkUsageTokenCounts,
  AutopilotWorkUsageTruth,
} from '../model'

export type ForgeUsageBudgetStatus =
  | 'blocked'
  | 'empty'
  | 'exceeded'
  | 'near_limit'
  | 'unknown'
  | 'within'

export type ForgeUsageBudgetAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  budgetEnforcementAuthority: false
  deploymentAuthority: false
  maxOutputEscalationAuthority: false
  pricingWriteAuthority: false
  providerRetryAuthority: false
  publicClaimAuthority: false
  settlementAuthority: false
  spendAuthorizationAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeUsageTokenCounts = Readonly<{
  cacheReadTokens: number | null
  cacheWriteTokens: number | null
  contextWindowTokens: number | null
  inputTokens: number | null
  outputTokens: number | null
  serverToolRequestCount: number | null
  totalTokens: number | null
}>

export type ForgeUsageCostEstimate = Readonly<{
  costRef: string
  currency: string | null
  estimatedCostCents: number | null
  pricingRef: string | null
  pricingState: AutopilotWorkUsagePricingState
}>

export type ForgeUsageBudgetThreshold = Readonly<{
  action: AutopilotWorkBudgetAction
  budgetRef: string
  limitCostCents: number | null
  limitTokens: number | null
  policyRefs: ReadonlyArray<string>
  state: AutopilotWorkBudgetState
}>

export type ForgeUsageBudgetInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  budgetThresholds?: ReadonlyArray<AutopilotWorkBudgetThreshold>
  contextEstimateRef?: string | null
  costEstimate?: AutopilotWorkUsageCostEstimate
  generatedAt: string
  modelRef?: string | null
  providerRef?: string | null
  quotaBlockerRefs?: ReadonlyArray<string>
  rateLimitRefs?: ReadonlyArray<string>
  tokenCounts?: AutopilotWorkUsageTokenCounts
  usageRef?: string
  usageTruth?: AutopilotWorkUsageTruth
  workOrderRef: string
}>

export type ForgeUsageBudgetView = Readonly<{
  authority: ForgeUsageBudgetAuthority
  blockerRefs: ReadonlyArray<string>
  budgetThresholds: ReadonlyArray<ForgeUsageBudgetThreshold>
  contextEstimateRef: string | null
  costEstimate: ForgeUsageCostEstimate | null
  generatedAt: string
  modelRef: string | null
  omittedUnsafeRefCount: number
  providerRef: string | null
  publicSafe: true
  quotaBlockerRefs: ReadonlyArray<string>
  rateLimitRefs: ReadonlyArray<string>
  status: ForgeUsageBudgetStatus
  tokenCounts: ForgeUsageTokenCounts
  usageRef: string | null
  usageTruth: AutopilotWorkUsageTruth
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_USAGE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:content|diagnostic|file|log|patch|payload|prompt|provider|shell|source|stderr|stdout|trace|transcript|usage)/i,
  /private[-_ ](?:content|diagnostic|prompt|repo|source|usage|workspace)/i,
  /provider[-_ ]payload/i,
  /shell[-_ ](?:log|output|transcript)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeUsageBudgetAuthority = {
  acceptedOutcomeAuthority: false,
  budgetEnforcementAuthority: false,
  deploymentAuthority: false,
  maxOutputEscalationAuthority: false,
  pricingWriteAuthority: false,
  providerRetryAuthority: false,
  publicClaimAuthority: false,
  settlementAuthority: false,
  spendAuthorizationAuthority: false,
  workerPayoutAuthority: false,
}

const emptyTokenCounts: ForgeUsageTokenCounts = {
  cacheReadTokens: null,
  cacheWriteTokens: null,
  contextWindowTokens: null,
  inputTokens: null,
  outputTokens: null,
  serverToolRequestCount: null,
  totalTokens: null,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_USAGE_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

const safeRefs = (
  ...groups: ReadonlyArray<ReadonlyArray<string> | undefined>
): RefBundle => {
  const refs = groups.flatMap(group => group ?? [])
  const sanitized = refs.reduce<Readonly<{ omitted: number; refs: string[] }>>(
    (state, ref) => {
      const safe = safeRef(ref)

      return safe === null
        ? { omitted: state.omitted + 1, refs: state.refs }
        : { omitted: state.omitted, refs: [...state.refs, safe] }
    },
    { omitted: 0, refs: [] },
  )

  return {
    omittedUnsafeRefCount: sanitized.omitted,
    refs: Array.from(new Set(sanitized.refs)),
  }
}

const safeOptionalRef = (
  value: string | null | undefined,
): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-usage-budget-blocker:${workOrderRef}:${suffix}`

const normalizeTokenCounts = (
  counts: AutopilotWorkUsageTokenCounts | undefined,
): ForgeUsageTokenCounts =>
  counts === undefined
    ? emptyTokenCounts
    : {
        cacheReadTokens: counts.cacheReadTokens ?? null,
        cacheWriteTokens: counts.cacheWriteTokens ?? null,
        contextWindowTokens: counts.contextWindowTokens ?? null,
        inputTokens: counts.inputTokens ?? null,
        outputTokens: counts.outputTokens ?? null,
        serverToolRequestCount: counts.serverToolRequestCount ?? null,
        totalTokens: counts.totalTokens ?? null,
      }

const normalizeCostEstimate = (
  estimate: AutopilotWorkUsageCostEstimate | undefined,
): Readonly<{
  costEstimate: ForgeUsageCostEstimate | null
  omittedUnsafeRefCount: number
}> => {
  if (estimate === undefined) {
    return { costEstimate: null, omittedUnsafeRefCount: 0 }
  }

  const costRef = safeOptionalRef(estimate.costRef)
  const pricingRef = safeOptionalRef(estimate.pricingRef)
  const currency = safeOptionalRef(estimate.currency)
  const omittedUnsafeRefCount =
    costRef.omittedUnsafeRefCount +
    pricingRef.omittedUnsafeRefCount +
    currency.omittedUnsafeRefCount

  return costRef.ref === null
    ? { costEstimate: null, omittedUnsafeRefCount }
    : {
        costEstimate: {
          costRef: costRef.ref,
          currency: currency.ref,
          estimatedCostCents: estimate.estimatedCostCents ?? null,
          pricingRef: pricingRef.ref,
          pricingState: estimate.pricingState,
        },
        omittedUnsafeRefCount,
      }
}

const normalizeThreshold = (
  threshold: AutopilotWorkBudgetThreshold,
): Readonly<{
  omittedUnsafeRefCount: number
  threshold: ForgeUsageBudgetThreshold | null
}> => {
  const budgetRef = safeOptionalRef(threshold.budgetRef)
  const policyRefs = safeRefs(threshold.policyRefs)
  const omittedUnsafeRefCount =
    budgetRef.omittedUnsafeRefCount + policyRefs.omittedUnsafeRefCount

  return budgetRef.ref === null
    ? { omittedUnsafeRefCount, threshold: null }
    : {
        omittedUnsafeRefCount,
        threshold: {
          action: threshold.action,
          budgetRef: budgetRef.ref,
          limitCostCents: threshold.limitCostCents ?? null,
          limitTokens: threshold.limitTokens ?? null,
          policyRefs: policyRefs.refs,
          state: threshold.state,
        },
      }
}

const outputOnlyContextBlocker = (
  workOrderRef: string,
  tokenCounts: ForgeUsageTokenCounts,
): ReadonlyArray<string> =>
  tokenCounts.contextWindowTokens === null &&
  tokenCounts.outputTokens !== null &&
  tokenCounts.inputTokens === null
    ? [blockerRef(workOrderRef, 'output-only-usage-not-context-headroom')]
    : []

const unknownPricingBlocker = (
  workOrderRef: string,
  costEstimate: ForgeUsageCostEstimate | null,
): ReadonlyArray<string> =>
  costEstimate?.pricingState === 'unknown'
    ? [blockerRef(workOrderRef, 'unknown-pricing-cost-not-exact')]
    : []

const syntheticUsageBlocker = (
  workOrderRef: string,
  usageTruth: AutopilotWorkUsageTruth,
): ReadonlyArray<string> =>
  usageTruth === 'synthetic'
    ? [blockerRef(workOrderRef, 'synthetic-usage-not-provider-usage')]
    : []

const contextCostSeparationBlockers = (
  workOrderRef: string,
  thresholds: ReadonlyArray<ForgeUsageBudgetThreshold>,
): ReadonlyArray<string> =>
  thresholds
    .filter(
      threshold =>
        threshold.limitTokens !== null &&
        threshold.limitCostCents !== null,
    )
    .map(threshold =>
      blockerRef(workOrderRef, `mixed-token-cost-threshold:${threshold.budgetRef}`)
    )

const statusForView = (
  hasSource: boolean,
  thresholds: ReadonlyArray<ForgeUsageBudgetThreshold>,
  blockerRefs: ReadonlyArray<string>,
  usageTruth: AutopilotWorkUsageTruth,
): ForgeUsageBudgetStatus => {
  if (!hasSource) {
    return 'empty'
  }

  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (thresholds.some(threshold => threshold.state === 'exceeded')) {
    return 'exceeded'
  }

  if (thresholds.some(threshold => threshold.state === 'near_limit')) {
    return 'near_limit'
  }

  if (
    usageTruth === 'unknown' ||
    thresholds.length === 0 ||
    thresholds.some(threshold => threshold.state === 'unknown')
  ) {
    return 'unknown'
  }

  return 'within'
}

export const projectForgeUsageBudget = (
  input: ForgeUsageBudgetInput,
): ForgeUsageBudgetView => {
  const usageRef = safeOptionalRef(input.usageRef)
  const contextEstimateRef = safeOptionalRef(input.contextEstimateRef)
  const modelRef = safeOptionalRef(input.modelRef)
  const providerRef = safeOptionalRef(input.providerRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const quotaBlockerRefs = safeRefs(input.quotaBlockerRefs)
  const rateLimitRefs = safeRefs(input.rateLimitRefs)
  const tokenCounts = normalizeTokenCounts(input.tokenCounts)
  const costEstimate = normalizeCostEstimate(input.costEstimate)
  const normalizedThresholds = (input.budgetThresholds ?? []).map(
    normalizeThreshold,
  )
  const budgetThresholds = normalizedThresholds.flatMap(result =>
    result.threshold === null ? [] : [result.threshold]
  )
  const usageTruth = input.usageTruth ?? 'unknown'
  const omittedUnsafeRefCount =
    usageRef.omittedUnsafeRefCount +
    contextEstimateRef.omittedUnsafeRefCount +
    modelRef.omittedUnsafeRefCount +
    providerRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    quotaBlockerRefs.omittedUnsafeRefCount +
    rateLimitRefs.omittedUnsafeRefCount +
    costEstimate.omittedUnsafeRefCount +
    normalizedThresholds.reduce(
      (sum, result) => sum + result.omittedUnsafeRefCount,
      0,
    )
  const hasSource =
    input.usageRef !== undefined ||
    input.tokenCounts !== undefined ||
    input.costEstimate !== undefined ||
    input.budgetThresholds !== undefined ||
    input.quotaBlockerRefs !== undefined ||
    input.rateLimitRefs !== undefined
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...quotaBlockerRefs.refs,
      ...rateLimitRefs.refs,
      ...outputOnlyContextBlocker(input.workOrderRef, tokenCounts),
      ...unknownPricingBlocker(input.workOrderRef, costEstimate.costEstimate),
      ...syntheticUsageBlocker(input.workOrderRef, usageTruth),
      ...contextCostSeparationBlockers(input.workOrderRef, budgetThresholds),
      ...(hasSource && usageRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-usage-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-usage-budget-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    budgetThresholds,
    contextEstimateRef: contextEstimateRef.ref,
    costEstimate: costEstimate.costEstimate,
    generatedAt: input.generatedAt,
    modelRef: modelRef.ref,
    omittedUnsafeRefCount,
    providerRef: providerRef.ref,
    publicSafe: true,
    quotaBlockerRefs: quotaBlockerRefs.refs,
    rateLimitRefs: rateLimitRefs.refs,
    status: statusForView(hasSource, budgetThresholds, blockerRefs, usageTruth),
    tokenCounts,
    usageRef: usageRef.ref,
    usageTruth,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeUsageBudgetInput = (
  work: AutopilotWorkProjection,
): ForgeUsageBudgetInput => {
  const source: AutopilotWorkUsageBudget | undefined = work.usageBudget

  if (source === undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: source.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source.budgetThresholds === undefined
      ? {}
      : { budgetThresholds: source.budgetThresholds }),
    ...(source.contextEstimateRef === undefined
      ? {}
      : { contextEstimateRef: source.contextEstimateRef }),
    ...(source.costEstimate === undefined
      ? {}
      : { costEstimate: source.costEstimate }),
    ...(source.modelRef === undefined ? {} : { modelRef: source.modelRef }),
    ...(source.providerRef === undefined ? {} : { providerRef: source.providerRef }),
    ...(source.quotaBlockerRefs === undefined
      ? {}
      : { quotaBlockerRefs: source.quotaBlockerRefs }),
    ...(source.rateLimitRefs === undefined ? {} : { rateLimitRefs: source.rateLimitRefs }),
    ...(source.tokenCounts === undefined ? {} : { tokenCounts: source.tokenCounts }),
    usageRef: source.usageRef,
    ...(source.usageTruth === undefined ? {} : { usageTruth: source.usageTruth }),
  }
}
