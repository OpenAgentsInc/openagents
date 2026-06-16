export type ForgeMcpCapabilityState =
  | 'configured'
  | 'disabled'
  | 'failed'
  | 'needs_auth'
  | 'pending'

export type ForgeMcpCapabilityFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeMcpCapabilityCatalogStatus =
  | 'blocked'
  | 'empty'
  | 'needs_attention'
  | 'ready'
  | 'stale'

export type ForgeMcpCapabilityInput = Readonly<{
  authRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  capabilityRefs?: ReadonlyArray<string>
  freshness?: ForgeMcpCapabilityFreshness
  policyRefs?: ReadonlyArray<string>
  serverRef: string
  state?: ForgeMcpCapabilityState
}>

export type ForgeMcpCapabilityCatalogInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  catalogRef: string
  entries?: ReadonlyArray<ForgeMcpCapabilityInput>
  freshness?: ForgeMcpCapabilityFreshness
  generatedAt: string
  workOrderRef: string
}>

export type ForgeMcpCapabilityEntry = Readonly<{
  authRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  capabilityRefs: ReadonlyArray<string>
  freshness: ForgeMcpCapabilityFreshness
  policyRefs: ReadonlyArray<string>
  serverRef: string
  state: ForgeMcpCapabilityState
}>

export type ForgeMcpCapabilityCatalogCounts = Readonly<{
  configured: number
  disabled: number
  failed: number
  needsAuth: number
  pending: number
  total: number
}>

export type ForgeMcpCapabilityCatalogAuthority = Readonly<{
  approvalBypassAuthority: false
  providerAccountAuthority: false
  settlementAuthority: false
  toolCallAuthority: false
  workspaceWriteAuthority: false
}>

export type ForgeMcpCapabilityCatalogView = Readonly<{
  authority: ForgeMcpCapabilityCatalogAuthority
  blockerRefs: ReadonlyArray<string>
  catalogRef: string
  counts: ForgeMcpCapabilityCatalogCounts
  entries: ReadonlyArray<ForgeMcpCapabilityEntry>
  freshness: ForgeMcpCapabilityFreshness
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  status: ForgeMcpCapabilityCatalogStatus
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

const SAFE_MCP_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_MCP_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:tool|schema|config|file|source|shell|command|prompt|log|transcript|test)/i,
  /private[-_ ](?:repo|content|source|transcript|instructions?|server|config)/i,
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

  return SAFE_MCP_REF_PATTERN.test(trimmed) &&
    !PRIVATE_MCP_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-mcp-capability-catalog-blocker:${catalogRef}:${suffix}`

const STATE_PRIORITY: Readonly<Record<ForgeMcpCapabilityState, number>> = {
  needs_auth: 0,
  failed: 1,
  pending: 2,
  configured: 3,
  disabled: 4,
}

const entrySort = (
  left: ForgeMcpCapabilityEntry,
  right: ForgeMcpCapabilityEntry,
): number =>
  STATE_PRIORITY[left.state] - STATE_PRIORITY[right.state] ||
  left.serverRef.localeCompare(right.serverRef) ||
  left.capabilityRefs.join('|').localeCompare(right.capabilityRefs.join('|'))

const normalizeEntry = (
  entry: ForgeMcpCapabilityInput,
): Readonly<{ entry: ForgeMcpCapabilityEntry | null; omitted: number }> => {
  const serverRef = safeOptionalRef(entry.serverRef)
  const capabilityRefs = safeRefs(entry.capabilityRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const authRefs = safeRefs(entry.authRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const omitted =
    serverRef.omittedUnsafeRefCount +
    capabilityRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    authRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount

  return {
    entry:
      serverRef.ref === null
        ? null
        : {
            authRefs: authRefs.refs,
            blockerRefs: blockerRefs.refs,
            capabilityRefs: capabilityRefs.refs,
            freshness: entry.freshness ?? 'unknown',
            policyRefs: policyRefs.refs,
            serverRef: serverRef.ref,
            state: entry.state ?? 'pending',
          },
    omitted,
  }
}

const catalogCounts = (
  entries: ReadonlyArray<ForgeMcpCapabilityEntry>,
): ForgeMcpCapabilityCatalogCounts => ({
  configured: entries.filter(entry => entry.state === 'configured').length,
  disabled: entries.filter(entry => entry.state === 'disabled').length,
  failed: entries.filter(entry => entry.state === 'failed').length,
  needsAuth: entries.filter(entry => entry.state === 'needs_auth').length,
  pending: entries.filter(entry => entry.state === 'pending').length,
  total: entries.length,
})

const catalogStatus = (
  counts: ForgeMcpCapabilityCatalogCounts,
  blockerRefs: ReadonlyArray<string>,
  freshness: ForgeMcpCapabilityFreshness,
): ForgeMcpCapabilityCatalogStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (freshness === 'stale') {
    return 'stale'
  }

  if (counts.total === 0) {
    return 'empty'
  }

  return counts.failed > 0 || counts.needsAuth > 0 || counts.pending > 0
    ? 'needs_attention'
    : 'ready'
}

export const projectForgeMcpCapabilityCatalog = (
  input: ForgeMcpCapabilityCatalogInput,
): ForgeMcpCapabilityCatalogView => {
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
  const safeCatalogRef = catalogRef.ref ?? 'unsafe-mcp-capability-catalog'
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
        : [blockerRef(safeCatalogRef, 'unsafe-mcp-material-omitted')]),
    ]),
  )
  const counts = catalogCounts(entries)

  return {
    authority: {
      approvalBypassAuthority: false,
      providerAccountAuthority: false,
      settlementAuthority: false,
      toolCallAuthority: false,
      workspaceWriteAuthority: false,
    },
    blockerRefs,
    catalogRef: catalogRef.ref ?? 'unsafe-catalog-ref-omitted',
    counts,
    entries,
    freshness,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    status: catalogStatus(counts, blockerRefs, freshness),
    workOrderRef: workOrderRef.ref ?? 'unsafe-work-order-ref-omitted',
  }
}
