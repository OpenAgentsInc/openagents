export type ForgeExtensibilityDomain = 'hooks' | 'mcp' | 'plugins' | 'skills'

export type ForgeExtensibilityEffectiveState =
  | 'blocked'
  | 'disabled'
  | 'enabled'
  | 'needs_auth'
  | 'needs_trust'
  | 'pending'

export type ForgeExtensibilityFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeExtensibilityConfigStatus =
  | 'blocked'
  | 'empty'
  | 'needs_attention'
  | 'ready'
  | 'stale'

export type ForgeExtensibilityConfigInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  catalogRefs?: ReadonlyArray<string>
  configRefs?: ReadonlyArray<string>
  domain: ForgeExtensibilityDomain
  effectiveState?: ForgeExtensibilityEffectiveState
  freshness?: ForgeExtensibilityFreshness
  policyRefs?: ReadonlyArray<string>
  sourceRefs?: ReadonlyArray<string>
}>

export type ForgeExtensibilityEffectiveConfigInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  configRef: string
  entries?: ReadonlyArray<ForgeExtensibilityConfigInput>
  freshness?: ForgeExtensibilityFreshness
  generatedAt: string
  workOrderRef: string
}>

export type ForgeExtensibilityConfigEntry = Readonly<{
  blockerRefs: ReadonlyArray<string>
  catalogRefs: ReadonlyArray<string>
  configRefs: ReadonlyArray<string>
  domain: ForgeExtensibilityDomain
  effectiveState: ForgeExtensibilityEffectiveState
  freshness: ForgeExtensibilityFreshness
  policyRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

export type ForgeExtensibilityStateCounts = Readonly<{
  blocked: number
  disabled: number
  enabled: number
  needsAuth: number
  needsTrust: number
  pending: number
  total: number
}>

export type ForgeExtensibilityDomainCounts = Readonly<
  Record<ForgeExtensibilityDomain, number>
>

export type ForgeExtensibilityConfigAuthority = Readonly<{
  contextInjectionAuthority: false
  hookExecutionAuthority: false
  providerAccountAuthority: false
  settlementAuthority: false
  toolCallAuthority: false
  workspaceWriteAuthority: false
}>

export type ForgeExtensibilityEffectiveConfigView = Readonly<{
  authority: ForgeExtensibilityConfigAuthority
  blockerRefs: ReadonlyArray<string>
  configRef: string
  domainCounts: ForgeExtensibilityDomainCounts
  entries: ReadonlyArray<ForgeExtensibilityConfigEntry>
  freshness: ForgeExtensibilityFreshness
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  stateCounts: ForgeExtensibilityStateCounts
  status: ForgeExtensibilityConfigStatus
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

const SAFE_CONFIG_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_CONFIG_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:config|plugin|tool|hook|skill|file|source|shell|command|prompt|log|transcript|test)/i,
  /plugin[-_ ](?:body|code|payload|source[-_ ](?:content|file|raw))/i,
  /shell[-_ ](?:command|script|payload)/i,
  /private[-_ ](?:repo|content|source|transcript|instructions?|config|plugin|tool)/i,
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

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_CONFIG_REF_PATTERN.test(trimmed) &&
    !PRIVATE_CONFIG_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
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
    refs: Array.from(new Set(sanitized.refs)),
  }
}

const safeOptionalRef = (value: string): OptionalRefBundle => {
  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (configRef: string, suffix: string): string =>
  `forge-extensibility-config-blocker:${configRef}:${suffix}`

const DOMAIN_PRIORITY: Readonly<Record<ForgeExtensibilityDomain, number>> = {
  mcp: 0,
  skills: 1,
  hooks: 2,
  plugins: 3,
}

const STATE_PRIORITY: Readonly<Record<ForgeExtensibilityEffectiveState, number>> = {
  blocked: 0,
  needs_auth: 1,
  needs_trust: 2,
  pending: 3,
  enabled: 4,
  disabled: 5,
}

const entrySort = (
  left: ForgeExtensibilityConfigEntry,
  right: ForgeExtensibilityConfigEntry,
): number =>
  DOMAIN_PRIORITY[left.domain] - DOMAIN_PRIORITY[right.domain] ||
  STATE_PRIORITY[left.effectiveState] - STATE_PRIORITY[right.effectiveState] ||
  left.configRefs.join('|').localeCompare(right.configRefs.join('|'))

const normalizeEntry = (
  entry: ForgeExtensibilityConfigInput,
): Readonly<{ entry: ForgeExtensibilityConfigEntry; omitted: number }> => {
  const configRefs = safeRefs(entry.configRefs)
  const catalogRefs = safeRefs(entry.catalogRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const sourceRefs = safeRefs(entry.sourceRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)

  return {
    entry: {
      blockerRefs: blockerRefs.refs,
      catalogRefs: catalogRefs.refs,
      configRefs: configRefs.refs,
      domain: entry.domain,
      effectiveState: entry.effectiveState ?? 'pending',
      freshness: entry.freshness ?? 'unknown',
      policyRefs: policyRefs.refs,
      sourceRefs: sourceRefs.refs,
    },
    omitted:
      configRefs.omittedUnsafeRefCount +
      catalogRefs.omittedUnsafeRefCount +
      policyRefs.omittedUnsafeRefCount +
      sourceRefs.omittedUnsafeRefCount +
      blockerRefs.omittedUnsafeRefCount,
  }
}

const stateCounts = (
  entries: ReadonlyArray<ForgeExtensibilityConfigEntry>,
): ForgeExtensibilityStateCounts => ({
  blocked: entries.filter(entry => entry.effectiveState === 'blocked').length,
  disabled: entries.filter(entry => entry.effectiveState === 'disabled').length,
  enabled: entries.filter(entry => entry.effectiveState === 'enabled').length,
  needsAuth: entries.filter(entry => entry.effectiveState === 'needs_auth').length,
  needsTrust: entries.filter(entry => entry.effectiveState === 'needs_trust').length,
  pending: entries.filter(entry => entry.effectiveState === 'pending').length,
  total: entries.length,
})

const domainCounts = (
  entries: ReadonlyArray<ForgeExtensibilityConfigEntry>,
): ForgeExtensibilityDomainCounts => ({
  hooks: entries.filter(entry => entry.domain === 'hooks').length,
  mcp: entries.filter(entry => entry.domain === 'mcp').length,
  plugins: entries.filter(entry => entry.domain === 'plugins').length,
  skills: entries.filter(entry => entry.domain === 'skills').length,
})

const configStatus = (
  counts: ForgeExtensibilityStateCounts,
  blockerRefs: ReadonlyArray<string>,
  freshness: ForgeExtensibilityFreshness,
): ForgeExtensibilityConfigStatus => {
  if (blockerRefs.length > 0 || counts.blocked > 0) {
    return 'blocked'
  }

  if (freshness === 'stale') {
    return 'stale'
  }

  if (counts.total === 0) {
    return 'empty'
  }

  return counts.needsAuth > 0 || counts.needsTrust > 0 || counts.pending > 0
    ? 'needs_attention'
    : 'ready'
}

export const projectForgeExtensibilityEffectiveConfig = (
  input: ForgeExtensibilityEffectiveConfigInput,
): ForgeExtensibilityEffectiveConfigView => {
  const configRef = safeOptionalRef(input.configRef)
  const workOrderRef = safeOptionalRef(input.workOrderRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .map(result => result.entry)
    .sort(entrySort)
  const freshness = input.freshness ?? 'unknown'
  const omittedUnsafeRefCount =
    configRef.omittedUnsafeRefCount +
    workOrderRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omitted, 0)
  const safeConfigRef = configRef.ref ?? 'unsafe-extensibility-config'
  const blockerRefs = Array.from(
    new Set([
      ...inputBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...(configRef.ref === null
        ? [blockerRef(safeConfigRef, 'missing-config-ref')]
        : []),
      ...(workOrderRef.ref === null
        ? [blockerRef(safeConfigRef, 'missing-work-order-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(safeConfigRef, 'unsafe-extensibility-material-omitted')]),
    ]),
  )
  const counts = stateCounts(entries)

  return {
    authority: {
      contextInjectionAuthority: false,
      hookExecutionAuthority: false,
      providerAccountAuthority: false,
      settlementAuthority: false,
      toolCallAuthority: false,
      workspaceWriteAuthority: false,
    },
    blockerRefs,
    configRef: configRef.ref ?? 'unsafe-config-ref-omitted',
    domainCounts: domainCounts(entries),
    entries,
    freshness,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    stateCounts: counts,
    status: configStatus(counts, blockerRefs, freshness),
    workOrderRef: workOrderRef.ref ?? 'unsafe-work-order-ref-omitted',
  }
}
