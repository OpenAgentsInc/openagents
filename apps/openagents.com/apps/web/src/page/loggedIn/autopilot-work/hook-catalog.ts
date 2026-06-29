export type ForgeHookCatalogState =
  | 'configured'
  | 'disabled'
  | 'failed'
  | 'needs_trust'
  | 'pending'

export type ForgeHookCatalogFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeHookCatalogStatus =
  | 'blocked'
  | 'empty'
  | 'needs_attention'
  | 'ready'
  | 'stale'

export type ForgeHookDescriptorInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  descriptorRef: string
  doctorRefs?: ReadonlyArray<string>
  eventRefs?: ReadonlyArray<string>
  freshness?: ForgeHookCatalogFreshness
  hookRef: string
  policyRefs?: ReadonlyArray<string>
  state?: ForgeHookCatalogState
  workspaceTrustRefs?: ReadonlyArray<string>
}>

export type ForgeHookCatalogInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  catalogRef: string
  entries?: ReadonlyArray<ForgeHookDescriptorInput>
  freshness?: ForgeHookCatalogFreshness
  generatedAt: string
  workOrderRef: string
}>

export type ForgeHookExecutionGate = Readonly<{
  disabledByDefault: true
  hookExecutionAuthority: false
  policyRefsRequired: true
  shellExecutionAuthority: false
  workspaceTrustRequired: true
}>

export type ForgeHookEntryExecution = Readonly<{
  executionAllowed: false
  policySatisfied: boolean
  workspaceTrustSatisfied: boolean
}>

export type ForgeHookCatalogEntry = Readonly<{
  blockerRefs: ReadonlyArray<string>
  descriptorRef: string
  doctorRefs: ReadonlyArray<string>
  eventRefs: ReadonlyArray<string>
  execution: ForgeHookEntryExecution
  freshness: ForgeHookCatalogFreshness
  hookRef: string
  policyRefs: ReadonlyArray<string>
  state: ForgeHookCatalogState
  workspaceTrustRefs: ReadonlyArray<string>
}>

export type ForgeHookCatalogCounts = Readonly<{
  configured: number
  disabled: number
  failed: number
  needsTrust: number
  pending: number
  total: number
}>

export type ForgeHookCatalogAuthority = Readonly<{
  hookExecutionAuthority: false
  providerAccountAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  workspaceWriteAuthority: false
}>

export type ForgeHookCatalogView = Readonly<{
  authority: ForgeHookCatalogAuthority
  blockerRefs: ReadonlyArray<string>
  catalogRef: string
  counts: ForgeHookCatalogCounts
  entries: ReadonlyArray<ForgeHookCatalogEntry>
  executionGate: ForgeHookExecutionGate
  freshness: ForgeHookCatalogFreshness
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  status: ForgeHookCatalogStatus
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

const SAFE_HOOK_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_HOOK_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:hook|script|file|source|shell|command|prompt|log|transcript|test)/i,
  /hook[-_ ](?:script|body|command|source)/i,
  /shell[-_ ](?:command|script|payload)/i,
  /private[-_ ](?:repo|content|source|transcript|instructions?|hook|script)/i,
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

  return SAFE_HOOK_REF_PATTERN.test(trimmed) &&
    !PRIVATE_HOOK_MARKERS.some(marker => marker.test(trimmed))
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

const blockerRef = (catalogRef: string, suffix: string): string =>
  `forge-hook-catalog-blocker:${catalogRef}:${suffix}`

const STATE_PRIORITY: Readonly<Record<ForgeHookCatalogState, number>> = {
  needs_trust: 0,
  failed: 1,
  pending: 2,
  configured: 3,
  disabled: 4,
}

const effectiveHookState = (
  state: ForgeHookCatalogState,
  policyRefs: ReadonlyArray<string>,
  workspaceTrustRefs: ReadonlyArray<string>,
): ForgeHookCatalogState =>
  state === 'configured' && (policyRefs.length === 0 || workspaceTrustRefs.length === 0)
    ? 'needs_trust'
    : state

const entrySort = (
  left: ForgeHookCatalogEntry,
  right: ForgeHookCatalogEntry,
): number =>
  STATE_PRIORITY[left.state] - STATE_PRIORITY[right.state] ||
  left.hookRef.localeCompare(right.hookRef) ||
  left.descriptorRef.localeCompare(right.descriptorRef)

const normalizeEntry = (
  entry: ForgeHookDescriptorInput,
): Readonly<{ entry: ForgeHookCatalogEntry | null; omitted: number }> => {
  const descriptorRef = safeOptionalRef(entry.descriptorRef)
  const hookRef = safeOptionalRef(entry.hookRef)
  const eventRefs = safeRefs(entry.eventRefs)
  const doctorRefs = safeRefs(entry.doctorRefs)
  const workspaceTrustRefs = safeRefs(entry.workspaceTrustRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const omitted =
    descriptorRef.omittedUnsafeRefCount +
    hookRef.omittedUnsafeRefCount +
    eventRefs.omittedUnsafeRefCount +
    doctorRefs.omittedUnsafeRefCount +
    workspaceTrustRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount

  return {
    entry:
      descriptorRef.ref === null || hookRef.ref === null
        ? null
        : {
            blockerRefs: blockerRefs.refs,
            descriptorRef: descriptorRef.ref,
            doctorRefs: doctorRefs.refs,
            eventRefs: eventRefs.refs,
            execution: {
              executionAllowed: false,
              policySatisfied: policyRefs.refs.length > 0,
              workspaceTrustSatisfied: workspaceTrustRefs.refs.length > 0,
            },
            freshness: entry.freshness ?? 'unknown',
            hookRef: hookRef.ref,
            policyRefs: policyRefs.refs,
            state: effectiveHookState(
              entry.state ?? 'pending',
              policyRefs.refs,
              workspaceTrustRefs.refs,
            ),
            workspaceTrustRefs: workspaceTrustRefs.refs,
          },
    omitted,
  }
}

const catalogCounts = (
  entries: ReadonlyArray<ForgeHookCatalogEntry>,
): ForgeHookCatalogCounts => ({
  configured: entries.filter(entry => entry.state === 'configured').length,
  disabled: entries.filter(entry => entry.state === 'disabled').length,
  failed: entries.filter(entry => entry.state === 'failed').length,
  needsTrust: entries.filter(entry => entry.state === 'needs_trust').length,
  pending: entries.filter(entry => entry.state === 'pending').length,
  total: entries.length,
})

const catalogStatus = (
  counts: ForgeHookCatalogCounts,
  blockerRefs: ReadonlyArray<string>,
  freshness: ForgeHookCatalogFreshness,
): ForgeHookCatalogStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (freshness === 'stale') {
    return 'stale'
  }

  if (counts.total === 0) {
    return 'empty'
  }

  return counts.failed > 0 || counts.needsTrust > 0 || counts.pending > 0
    ? 'needs_attention'
    : 'ready'
}

export const projectForgeHookCatalog = (
  input: ForgeHookCatalogInput,
): ForgeHookCatalogView => {
  const catalogRef = safeOptionalRef(input.catalogRef)
  const workOrderRef = safeOptionalRef(input.workOrderRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(entrySort)
  const freshness = input.freshness ?? 'unknown'
  const omittedUnsafeRefCount =
    catalogRef.omittedUnsafeRefCount +
    workOrderRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omitted, 0)
  const safeCatalogRef = catalogRef.ref ?? 'unsafe-hook-catalog'
  const blockerRefs = Array.from(
    new Set([
      ...inputBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...(catalogRef.ref === null
        ? [blockerRef(safeCatalogRef, 'missing-catalog-ref')]
        : []),
      ...(workOrderRef.ref === null
        ? [blockerRef(safeCatalogRef, 'missing-work-order-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(safeCatalogRef, 'unsafe-hook-material-omitted')]),
    ]),
  )
  const counts = catalogCounts(entries)

  return {
    authority: {
      hookExecutionAuthority: false,
      providerAccountAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      workspaceWriteAuthority: false,
    },
    blockerRefs,
    catalogRef: catalogRef.ref ?? 'unsafe-catalog-ref-omitted',
    counts,
    entries,
    executionGate: {
      disabledByDefault: true,
      hookExecutionAuthority: false,
      policyRefsRequired: true,
      shellExecutionAuthority: false,
      workspaceTrustRequired: true,
    },
    freshness,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    status: catalogStatus(counts, blockerRefs, freshness),
    workOrderRef: workOrderRef.ref ?? 'unsafe-work-order-ref-omitted',
  }
}
