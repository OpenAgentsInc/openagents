import {
  type ForgeExtensibilityDomain,
  type ForgeExtensibilityEffectiveConfigInput,
  type ForgeExtensibilityEffectiveConfigView,
  type ForgeExtensibilityEffectiveState,
  projectForgeExtensibilityEffectiveConfig,
} from './extensibility-effective-config'

export type ForgeExtensibilityExecutionRequestKind =
  | 'hook_enablement'
  | 'mcp_resource_read'
  | 'mcp_tool_call'
  | 'plugin_activation'
  | 'settings_activation'
  | 'skill_body_disclosure'

export type ForgeExtensibilityExecutionOutcome =
  | 'blocked'
  | 'callable'
  | 'disabled'
  | 'failed'
  | 'needs_auth'
  | 'needs_trust'
  | 'pending'

export type ForgeExtensibilityExecutionRequestInput = Readonly<{
  actorRef?: string | null
  authRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  catalogRefs?: ReadonlyArray<string>
  configRefs?: ReadonlyArray<string>
  domain: ForgeExtensibilityDomain
  explicitDisclosure?: boolean
  failureRefs?: ReadonlyArray<string>
  generatedAt?: string
  observedState?: 'failed' | null
  policyRefs?: ReadonlyArray<string>
  providerAccountRefs?: ReadonlyArray<string>
  requestKind: ForgeExtensibilityExecutionRequestKind
  requestRef: string
  sourceRefs?: ReadonlyArray<string>
  targetRef: string
  workspaceTrustRefs?: ReadonlyArray<string>
}>

export type ForgeExtensibilityExecutionReceiptsInput = Readonly<{
  config: ForgeExtensibilityEffectiveConfigInput
  generatedAt: string
  requests?: ReadonlyArray<ForgeExtensibilityExecutionRequestInput>
  workOrderRef: string
}>

export type ForgeExtensibilityExecutionReceipt = Readonly<{
  actorRef: string | null
  authority: Readonly<{
    contextInjectionAuthority: false
    hookExecutionAuthority: false
    mcpNetworkCallAuthority: false
    pluginActivationAuthority: false
    settingsWriteAuthority: false
    skillBodyLoaded: false
    workspaceWriteAuthority: false
  }>
  authRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  catalogRefs: ReadonlyArray<string>
  configRefs: ReadonlyArray<string>
  domain: ForgeExtensibilityDomain
  failureRefs: ReadonlyArray<string>
  generatedAt: string
  omittedUnsafeRefCount: number
  outcome: ForgeExtensibilityExecutionOutcome
  policyRefs: ReadonlyArray<string>
  provenance: 'refs_only_extensibility_execution_request'
  providerAccountRefs: ReadonlyArray<string>
  publicSafe: true
  receiptKind: 'forge_extensibility_execution_request.v1'
  receiptRef: string
  requestKind: ForgeExtensibilityExecutionRequestKind
  requestRef: string
  sourceRefs: ReadonlyArray<string>
  targetRef: string
  workspaceTrustRefs: ReadonlyArray<string>
  workOrderRef: string
}>

export type ForgeExtensibilityExecutionReceiptsView = Readonly<{
  blockerRefs: ReadonlyArray<string>
  config: ForgeExtensibilityEffectiveConfigView
  generatedAt: string
  omittedUnsafeRefCount: number
  receipts: ReadonlyArray<ForgeExtensibilityExecutionReceipt>
  status: 'blocked' | 'empty' | 'ready'
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

const SAFE_EXEC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_EXEC_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:config|plugin|tool|hook|skill|body|file|source|shell|command|prompt|log|transcript|test)/i,
  /skill[-_ ]body[-_ ](?:raw|content|text|payload)/i,
  /plugin[-_ ](?:body|code|payload|source[-_ ](?:content|file|raw))/i,
  /shell[-_ ](?:command|script|payload)/i,
  /private[-_ ](?:repo|content|source|transcript|instructions?|config|plugin|tool|skill)/i,
  /provider[-_ ]payload/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:api[-_ ]?key|bearer|token|secret|mnemonic|password)\b/i,
]

const unique = <T>(values: ReadonlyArray<T>): ReadonlyArray<T> =>
  Array.from(new Set(values))

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_EXEC_REF_PATTERN.test(trimmed) &&
    !PRIVATE_EXEC_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

const safeOptionalRef = (value: string | null | undefined): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const safeRefs = (refs: ReadonlyArray<string> | undefined): RefBundle => {
  const sanitized = (refs ?? []).reduce<Readonly<{ omitted: number; refs: string[] }>>(
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
    refs: unique(sanitized.refs),
  }
}

const blockerRef = (scopeRef: string, suffix: string): string =>
  `forge-extensibility-execution-blocker:${scopeRef}:${suffix}`

const slugRefPart = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '') || 'unknown'

const effectiveStateOutcome = (
  state: ForgeExtensibilityEffectiveState,
): ForgeExtensibilityExecutionOutcome =>
  state === 'enabled'
    ? 'callable'
    : state === 'needs_auth'
      ? 'needs_auth'
      : state === 'needs_trust'
        ? 'needs_trust'
        : state === 'disabled'
          ? 'disabled'
          : state

const requestNeedsWorkspaceTrust = (
  requestKind: ForgeExtensibilityExecutionRequestKind,
): boolean =>
  requestKind === 'hook_enablement' ||
  requestKind === 'plugin_activation' ||
  requestKind === 'settings_activation'

const requestNeedsProviderAccount = (
  requestKind: ForgeExtensibilityExecutionRequestKind,
): boolean => requestKind === 'mcp_tool_call'

const matchingEntry = (
  config: ForgeExtensibilityEffectiveConfigView,
  request: Readonly<{
    configRefs: ReadonlyArray<string>
    domain: ForgeExtensibilityDomain
    targetRef: string | null
  }>,
) =>
  config.entries.find(
    entry =>
      entry.domain === request.domain &&
      (request.configRefs.some(ref => entry.configRefs.includes(ref)) ||
        (request.targetRef !== null &&
          [
            ...entry.configRefs,
            ...entry.catalogRefs,
            ...entry.sourceRefs,
          ].includes(request.targetRef))),
  )

const receiptSort = (
  left: ForgeExtensibilityExecutionReceipt,
  right: ForgeExtensibilityExecutionReceipt,
): number =>
  left.outcome.localeCompare(right.outcome) ||
  left.domain.localeCompare(right.domain) ||
  left.requestRef.localeCompare(right.requestRef)

export const projectForgeExtensibilityExecutionReceipts = (
  input: ForgeExtensibilityExecutionReceiptsInput,
): ForgeExtensibilityExecutionReceiptsView => {
  const config = projectForgeExtensibilityEffectiveConfig(input.config)
  const workOrderRef = safeOptionalRef(input.workOrderRef)
  const receipts = (input.requests ?? []).map(request => {
    const requestRef = safeOptionalRef(request.requestRef)
    const targetRef = safeOptionalRef(request.targetRef)
    const actorRef = safeOptionalRef(request.actorRef)
    const requestConfigRefs = safeRefs(request.configRefs)
    const requestCatalogRefs = safeRefs(request.catalogRefs)
    const requestPolicyRefs = safeRefs(request.policyRefs)
    const requestSourceRefs = safeRefs(request.sourceRefs)
    const requestAuthRefs = safeRefs(request.authRefs)
    const requestProviderAccountRefs = safeRefs(request.providerAccountRefs)
    const requestWorkspaceTrustRefs = safeRefs(request.workspaceTrustRefs)
    const requestFailureRefs = safeRefs(request.failureRefs)
    const requestBlockerRefs = safeRefs(request.blockerRefs)
    const entry = matchingEntry(config, {
      configRefs: requestConfigRefs.refs,
      domain: request.domain,
      targetRef: targetRef.ref,
    })
    const configRefs = unique([
      ...requestConfigRefs.refs,
      ...(entry?.configRefs ?? []),
    ])
    const catalogRefs = unique([
      ...requestCatalogRefs.refs,
      ...(entry?.catalogRefs ?? []),
    ])
    const policyRefs = unique([
      ...requestPolicyRefs.refs,
      ...(entry?.policyRefs ?? []),
    ])
    const sourceRefs = unique([
      ...requestSourceRefs.refs,
      ...(entry?.sourceRefs ?? []),
    ])
    const authRefs = unique([
      ...requestAuthRefs.refs,
      ...((entry?.effectiveState ?? 'pending') === 'needs_auth'
        ? entry?.policyRefs ?? []
        : []),
    ])
    const omittedUnsafeRefCount =
      requestRef.omittedUnsafeRefCount +
      targetRef.omittedUnsafeRefCount +
      actorRef.omittedUnsafeRefCount +
      requestConfigRefs.omittedUnsafeRefCount +
      requestCatalogRefs.omittedUnsafeRefCount +
      requestPolicyRefs.omittedUnsafeRefCount +
      requestSourceRefs.omittedUnsafeRefCount +
      requestAuthRefs.omittedUnsafeRefCount +
      requestProviderAccountRefs.omittedUnsafeRefCount +
      requestWorkspaceTrustRefs.omittedUnsafeRefCount +
      requestFailureRefs.omittedUnsafeRefCount +
      requestBlockerRefs.omittedUnsafeRefCount
    const safeRequestRef = requestRef.ref ?? 'unsafe-request-ref-omitted'
    const scopeRef = requestRef.ref ?? targetRef.ref ?? config.configRef
    const generatedAt = request.generatedAt ?? input.generatedAt
    const baseBlockers = [
      ...requestBlockerRefs.refs,
      ...(entry?.blockerRefs ?? []),
      ...(requestRef.ref === null ? [blockerRef(scopeRef, 'missing-request-ref')] : []),
      ...(targetRef.ref === null ? [blockerRef(scopeRef, 'missing-target-ref')] : []),
      ...(workOrderRef.ref === null
        ? [blockerRef(scopeRef, 'missing-work-order-ref')]
        : []),
      ...(entry === undefined
        ? [blockerRef(scopeRef, 'missing-effective-config-entry')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(scopeRef, 'unsafe-extensibility-request-material-omitted')]),
    ]
    const effectiveOutcome =
      request.observedState === 'failed'
        ? 'failed'
        : effectiveStateOutcome(entry?.effectiveState ?? 'pending')
    const guardBlockers =
      effectiveOutcome === 'callable'
        ? [
            ...(policyRefs.length === 0
              ? [blockerRef(scopeRef, 'missing-policy-ref')]
              : []),
            ...(requestNeedsWorkspaceTrust(request.requestKind) &&
            requestWorkspaceTrustRefs.refs.length === 0
              ? [blockerRef(scopeRef, 'missing-workspace-trust-ref')]
              : []),
            ...(requestNeedsProviderAccount(request.requestKind) &&
            requestProviderAccountRefs.refs.length === 0
              ? [blockerRef(scopeRef, 'missing-provider-account-ref')]
              : []),
            ...(request.requestKind === 'skill_body_disclosure' &&
            !request.explicitDisclosure
              ? [blockerRef(scopeRef, 'skill-body-disclosure-not-explicit')]
              : []),
          ]
        : []
    const blockerRefs = unique([...baseBlockers, ...guardBlockers])
    const outcome =
      request.observedState === 'failed'
        ? 'failed'
        : blockerRefs.length > 0 && effectiveOutcome === 'callable'
          ? 'blocked'
          : effectiveOutcome

    return {
      actorRef: actorRef.ref,
      authority: {
        contextInjectionAuthority: false,
        hookExecutionAuthority: false,
        mcpNetworkCallAuthority: false,
        pluginActivationAuthority: false,
        settingsWriteAuthority: false,
        skillBodyLoaded: false,
        workspaceWriteAuthority: false,
      } as const,
      authRefs,
      blockerRefs,
      catalogRefs,
      configRefs,
      domain: request.domain,
      failureRefs: requestFailureRefs.refs,
      generatedAt,
      omittedUnsafeRefCount,
      outcome,
      policyRefs,
      provenance: 'refs_only_extensibility_execution_request' as const,
      providerAccountRefs: requestProviderAccountRefs.refs,
      publicSafe: true as const,
      receiptKind: 'forge_extensibility_execution_request.v1' as const,
      receiptRef: `forge.extensibility_execution.${slugRefPart(safeRequestRef)}.${slugRefPart(generatedAt)}`,
      requestKind: request.requestKind,
      requestRef: safeRequestRef,
      sourceRefs,
      targetRef: targetRef.ref ?? 'unsafe-target-ref-omitted',
      workspaceTrustRefs: requestWorkspaceTrustRefs.refs,
      workOrderRef: workOrderRef.ref ?? 'unsafe-work-order-ref-omitted',
    }
  })
  const sortedReceipts = receipts.sort(receiptSort)
  const omittedUnsafeRefCount =
    config.omittedUnsafeRefCount +
    workOrderRef.omittedUnsafeRefCount +
    sortedReceipts.reduce((sum, receipt) => sum + receipt.omittedUnsafeRefCount, 0)
  const blockerRefs = unique([
    ...config.blockerRefs,
    ...sortedReceipts.flatMap(receipt => receipt.blockerRefs),
  ])

  return {
    blockerRefs,
    config,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    receipts: sortedReceipts,
    status:
      blockerRefs.length > 0
        ? 'blocked'
        : sortedReceipts.length === 0
          ? 'empty'
          : 'ready',
    workOrderRef: workOrderRef.ref ?? 'unsafe-work-order-ref-omitted',
  }
}
