import type {
  AutopilotWorkModelCapabilities,
  AutopilotWorkModelProviderResolution,
  AutopilotWorkModelProviderState,
  AutopilotWorkModelResolutionSource,
  AutopilotWorkModelValidationState,
  AutopilotWorkProjection,
} from '../model'

export type ForgeModelProviderStatus =
  | 'blocked'
  | 'fallback_selected'
  | 'selected'
  | 'unavailable'
  | 'unknown'

export type ForgeModelProviderAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  credentialAuthority: false
  deploymentAuthority: false
  modelCallAuthority: false
  modelSwitchAuthority: false
  pricingWriteAuthority: false
  providerRetryAuthority: false
  publicClaimAuthority: false
  settingsWriteAuthority: false
  settlementAuthority: false
  streamParsingAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeModelCapabilities = Readonly<{
  cacheSupport: boolean | null
  contextWindowTokens: number | null
  documentSupport: boolean | null
  maxOutputTokens: number | null
  parallelToolCallSupport: boolean | null
  reasoningSupport: boolean | null
  serverToolSupport: boolean | null
  structuredOutputSupport: boolean | null
  toolCallSupport: boolean | null
  visionSupport: boolean | null
}>

export type ForgeModelProviderInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  capabilityRefs?: ReadonlyArray<string>
  capabilities?: AutopilotWorkModelCapabilities
  entitlementRefs?: ReadonlyArray<string>
  fallbackRefs?: ReadonlyArray<string>
  generatedAt: string
  modelRef?: string | null
  policyRefs?: ReadonlyArray<string>
  pricingRefs?: ReadonlyArray<string>
  privacyRefs?: ReadonlyArray<string>
  providerFacingModelRef?: string | null
  providerRef?: string | null
  requestedAliasRef?: string | null
  resolutionRef?: string
  resolutionSource?: AutopilotWorkModelResolutionSource
  state?: AutopilotWorkModelProviderState
  telemetryRefs?: ReadonlyArray<string>
  validationRefs?: ReadonlyArray<string>
  validationState?: AutopilotWorkModelValidationState
  workOrderRef: string
}>

export type ForgeModelProviderView = Readonly<{
  authority: ForgeModelProviderAuthority
  blockerRefs: ReadonlyArray<string>
  capabilityRefs: ReadonlyArray<string>
  capabilities: ForgeModelCapabilities
  entitlementRefs: ReadonlyArray<string>
  fallbackRefs: ReadonlyArray<string>
  generatedAt: string
  modelRef: string | null
  omittedUnsafeRefCount: number
  policyRefs: ReadonlyArray<string>
  pricingRefs: ReadonlyArray<string>
  privacyRefs: ReadonlyArray<string>
  providerFacingModelRef: string | null
  providerRef: string | null
  publicSafe: true
  requestedAliasRef: string | null
  resolutionRef: string | null
  resolutionSource: AutopilotWorkModelResolutionSource | null
  state: AutopilotWorkModelProviderState
  status: ForgeModelProviderStatus
  telemetryRefs: ReadonlyArray<string>
  validationRefs: ReadonlyArray<string>
  validationState: AutopilotWorkModelValidationState
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
const PRIVATE_MODEL_PROVIDER_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /internal[-_ ](?:codename|model|deployment)/i,
  /private[-_ ](?:content|deployment|diagnostic|model|provider|repo|request|source|workspace)/i,
  /provider[-_ ]payload/i,
  /raw[-_ ](?:body|content|diagnostic|file|log|payload|prompt|provider|request|response|shell|source|trace|transcript)/i,
  /sdk[-_ ]payload/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeModelProviderAuthority = {
  acceptedOutcomeAuthority: false,
  credentialAuthority: false,
  deploymentAuthority: false,
  modelCallAuthority: false,
  modelSwitchAuthority: false,
  pricingWriteAuthority: false,
  providerRetryAuthority: false,
  publicClaimAuthority: false,
  settingsWriteAuthority: false,
  settlementAuthority: false,
  streamParsingAuthority: false,
  workerPayoutAuthority: false,
}

const emptyCapabilities: ForgeModelCapabilities = {
  cacheSupport: null,
  contextWindowTokens: null,
  documentSupport: null,
  maxOutputTokens: null,
  parallelToolCallSupport: null,
  reasoningSupport: null,
  serverToolSupport: null,
  structuredOutputSupport: null,
  toolCallSupport: null,
  visionSupport: null,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_MODEL_PROVIDER_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-model-provider-blocker:${workOrderRef}:${suffix}`

const normalizeCapabilities = (
  capabilities: AutopilotWorkModelCapabilities | undefined,
): ForgeModelCapabilities =>
  capabilities === undefined
    ? emptyCapabilities
    : {
        cacheSupport: capabilities.cacheSupport ?? null,
        contextWindowTokens: capabilities.contextWindowTokens ?? null,
        documentSupport: capabilities.documentSupport ?? null,
        maxOutputTokens: capabilities.maxOutputTokens ?? null,
        parallelToolCallSupport: capabilities.parallelToolCallSupport ?? null,
        reasoningSupport: capabilities.reasoningSupport ?? null,
        serverToolSupport: capabilities.serverToolSupport ?? null,
        structuredOutputSupport: capabilities.structuredOutputSupport ?? null,
        toolCallSupport: capabilities.toolCallSupport ?? null,
        visionSupport: capabilities.visionSupport ?? null,
      }

const missingCapabilityBlockers = (
  workOrderRef: string,
  state: AutopilotWorkModelProviderState,
  capabilityRefs: ReadonlyArray<string>,
  capabilities: ForgeModelCapabilities,
): ReadonlyArray<string> =>
  state === 'selected' || state === 'fallback_selected'
    ? capabilityRefs.length === 0 &&
      capabilities.contextWindowTokens === null &&
      capabilities.maxOutputTokens === null
      ? [blockerRef(workOrderRef, 'missing-capability-evidence')]
      : []
    : []

const missingEntitlementBlockers = (
  workOrderRef: string,
  state: AutopilotWorkModelProviderState,
  entitlementRefs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  (state === 'selected' || state === 'fallback_selected') &&
  entitlementRefs.length === 0
    ? [blockerRef(workOrderRef, 'missing-entitlement-evidence')]
    : []

const fallbackBlockers = (
  workOrderRef: string,
  state: AutopilotWorkModelProviderState,
  fallbackRefs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  state === 'fallback_selected' && fallbackRefs.length === 0
    ? [blockerRef(workOrderRef, 'fallback-selected-without-evidence')]
    : []

const unavailableBlockers = (
  workOrderRef: string,
  state: AutopilotWorkModelProviderState,
  validationRefs: ReadonlyArray<string>,
  validationState: AutopilotWorkModelValidationState,
): ReadonlyArray<string> =>
  state === 'unavailable' &&
  validationState !== 'failed' &&
  validationRefs.length === 0
    ? [blockerRef(workOrderRef, 'provider-discovery-failure-not-unavailable-proof')]
    : []

const statusForState = (
  state: AutopilotWorkModelProviderState,
  blockerRefs: ReadonlyArray<string>,
): ForgeModelProviderStatus =>
  blockerRefs.length > 0 ? 'blocked' : state

export const projectForgeModelProvider = (
  input: ForgeModelProviderInput,
): ForgeModelProviderView => {
  const requestedAliasRef = safeOptionalRef(input.requestedAliasRef)
  const resolutionRef = safeOptionalRef(input.resolutionRef)
  const providerRef = safeOptionalRef(input.providerRef)
  const modelRef = safeOptionalRef(input.modelRef)
  const providerFacingModelRef = safeOptionalRef(input.providerFacingModelRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const capabilityRefs = safeRefs(input.capabilityRefs)
  const entitlementRefs = safeRefs(input.entitlementRefs)
  const fallbackRefs = safeRefs(input.fallbackRefs)
  const policyRefs = safeRefs(input.policyRefs)
  const pricingRefs = safeRefs(input.pricingRefs)
  const privacyRefs = safeRefs(input.privacyRefs)
  const telemetryRefs = safeRefs(input.telemetryRefs)
  const validationRefs = safeRefs(input.validationRefs)
  const capabilities = normalizeCapabilities(input.capabilities)
  const state = input.state ?? 'unknown'
  const validationState = input.validationState ?? 'unknown'
  const hasSource =
    input.resolutionRef !== undefined ||
    input.providerRef !== undefined ||
    input.modelRef !== undefined ||
    input.requestedAliasRef !== undefined
  const omittedUnsafeRefCount =
    requestedAliasRef.omittedUnsafeRefCount +
    resolutionRef.omittedUnsafeRefCount +
    providerRef.omittedUnsafeRefCount +
    modelRef.omittedUnsafeRefCount +
    providerFacingModelRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    capabilityRefs.omittedUnsafeRefCount +
    entitlementRefs.omittedUnsafeRefCount +
    fallbackRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    pricingRefs.omittedUnsafeRefCount +
    privacyRefs.omittedUnsafeRefCount +
    telemetryRefs.omittedUnsafeRefCount +
    validationRefs.omittedUnsafeRefCount
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...missingCapabilityBlockers(
        input.workOrderRef,
        state,
        capabilityRefs.refs,
        capabilities,
      ),
      ...missingEntitlementBlockers(input.workOrderRef, state, entitlementRefs.refs),
      ...fallbackBlockers(input.workOrderRef, state, fallbackRefs.refs),
      ...unavailableBlockers(
        input.workOrderRef,
        state,
        validationRefs.refs,
        validationState,
      ),
      ...(hasSource && resolutionRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-resolution-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-model-provider-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    capabilityRefs: capabilityRefs.refs,
    capabilities,
    entitlementRefs: entitlementRefs.refs,
    fallbackRefs: fallbackRefs.refs,
    generatedAt: input.generatedAt,
    modelRef: modelRef.ref,
    omittedUnsafeRefCount,
    policyRefs: policyRefs.refs,
    pricingRefs: pricingRefs.refs,
    privacyRefs: privacyRefs.refs,
    providerFacingModelRef: providerFacingModelRef.ref,
    providerRef: providerRef.ref,
    publicSafe: true,
    requestedAliasRef: requestedAliasRef.ref,
    resolutionRef: resolutionRef.ref,
    resolutionSource: input.resolutionSource ?? null,
    state,
    status: statusForState(state, blockerRefs),
    telemetryRefs: telemetryRefs.refs,
    validationRefs: validationRefs.refs,
    validationState,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeModelProviderInput = (
  work: AutopilotWorkProjection,
): ForgeModelProviderInput => {
  const source: AutopilotWorkModelProviderResolution | undefined =
    work.modelProvider

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
    ...(source.capabilityRefs === undefined
      ? {}
      : { capabilityRefs: source.capabilityRefs }),
    ...(source.capabilities === undefined ? {} : { capabilities: source.capabilities }),
    ...(source.entitlementRefs === undefined
      ? {}
      : { entitlementRefs: source.entitlementRefs }),
    ...(source.fallbackRefs === undefined ? {} : { fallbackRefs: source.fallbackRefs }),
    ...(source.modelRef === undefined ? {} : { modelRef: source.modelRef }),
    ...(source.policyRefs === undefined ? {} : { policyRefs: source.policyRefs }),
    ...(source.pricingRefs === undefined ? {} : { pricingRefs: source.pricingRefs }),
    ...(source.privacyRefs === undefined ? {} : { privacyRefs: source.privacyRefs }),
    ...(source.providerFacingModelRef === undefined
      ? {}
      : { providerFacingModelRef: source.providerFacingModelRef }),
    ...(source.providerRef === undefined ? {} : { providerRef: source.providerRef }),
    ...(source.requestedAliasRef === undefined
      ? {}
      : { requestedAliasRef: source.requestedAliasRef }),
    resolutionRef: source.resolutionRef,
    ...(source.resolutionSource === undefined
      ? {}
      : { resolutionSource: source.resolutionSource }),
    state: source.state,
    ...(source.telemetryRefs === undefined
      ? {}
      : { telemetryRefs: source.telemetryRefs }),
    ...(source.validationRefs === undefined
      ? {}
      : { validationRefs: source.validationRefs }),
    ...(source.validationState === undefined
      ? {}
      : { validationState: source.validationState }),
  }
}
