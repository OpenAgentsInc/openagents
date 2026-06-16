export type ForgeTerminalAgentSystemFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeTerminalAgentSystemsReadinessStatus =
  | 'blocked'
  | 'empty'
  | 'partial'
  | 'ready'
  | 'stale'

export type ForgeTerminalAgentSystemReadinessInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  evidenceRefs?: ReadonlyArray<string>
  freshness?: ForgeTerminalAgentSystemFreshness
  groupRef: string
  publicSafe?: boolean
  publicSafetyRefs?: ReadonlyArray<string>
  surfaced?: boolean
  systemRef: string
  tested?: boolean
  testRefs?: ReadonlyArray<string>
}>

export type ForgeTerminalAgentSystemsReadinessInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  readinessRef: string
  systems?: ReadonlyArray<ForgeTerminalAgentSystemReadinessInput>
}>

export type ForgeTerminalAgentSystemReadinessEntry = Readonly<{
  blockerRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  freshness: ForgeTerminalAgentSystemFreshness
  groupRef: string
  publicSafe: boolean
  publicSafetyRefs: ReadonlyArray<string>
  surfaced: boolean
  systemRef: string
  tested: boolean
  testRefs: ReadonlyArray<string>
}>

export type ForgeTerminalAgentSystemsReadinessCounts = Readonly<{
  blocked: number
  publicSafe: number
  stale: number
  surfaced: number
  tested: number
  total: number
}>

export type ForgeTerminalAgentSystemsReadinessView = Readonly<{
  blockerRefs: ReadonlyArray<string>
  counts: ForgeTerminalAgentSystemsReadinessCounts
  entries: ReadonlyArray<ForgeTerminalAgentSystemReadinessEntry>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  readinessRef: string
  status: ForgeTerminalAgentSystemsReadinessStatus
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_READINESS_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_READINESS_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:trace|transcript|file|source|shell|command|prompt|log|test|provider|payload)/i,
  /private[-_ ](?:repo|content|source|trace|transcript|instructions?|customer|workspace)/i,
  /provider[-_ ]payload/i,
  /customer[-_ ]private/i,
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

  return SAFE_READINESS_REF_PATTERN.test(trimmed) &&
    !PRIVATE_READINESS_MARKERS.some(marker => marker.test(trimmed))
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

const blockerRef = (readinessRef: string, suffix: string): string =>
  `forge-terminal-agent-systems-readiness-blocker:${readinessRef}:${suffix}`

const normalizeEntry = (
  input: ForgeTerminalAgentSystemReadinessInput,
): Readonly<{ entry: ForgeTerminalAgentSystemReadinessEntry | null; omitted: number }> => {
  const systemRef = safeOptionalRef(input.systemRef)
  const groupRef = safeOptionalRef(input.groupRef)
  const evidenceRefs = safeRefs(input.evidenceRefs)
  const testRefs = safeRefs(input.testRefs)
  const publicSafetyRefs = safeRefs(input.publicSafetyRefs)
  const blockerRefs = safeRefs(input.blockerRefs)
  const omitted =
    systemRef.omittedUnsafeRefCount +
    groupRef.omittedUnsafeRefCount +
    evidenceRefs.omittedUnsafeRefCount +
    testRefs.omittedUnsafeRefCount +
    publicSafetyRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount

  return {
    entry:
      systemRef.ref === null || groupRef.ref === null
        ? null
        : {
            blockerRefs: blockerRefs.refs,
            evidenceRefs: evidenceRefs.refs,
            freshness: input.freshness ?? 'unknown',
            groupRef: groupRef.ref,
            publicSafe: input.publicSafe ?? false,
            publicSafetyRefs: publicSafetyRefs.refs,
            surfaced: input.surfaced ?? false,
            systemRef: systemRef.ref,
            tested: input.tested ?? false,
            testRefs: testRefs.refs,
          },
    omitted,
  }
}

const entrySort = (
  left: ForgeTerminalAgentSystemReadinessEntry,
  right: ForgeTerminalAgentSystemReadinessEntry,
): number =>
  left.groupRef.localeCompare(right.groupRef) ||
  left.systemRef.localeCompare(right.systemRef)

const readinessCounts = (
  entries: ReadonlyArray<ForgeTerminalAgentSystemReadinessEntry>,
): ForgeTerminalAgentSystemsReadinessCounts => ({
  blocked: entries.filter(entry => entry.blockerRefs.length > 0).length,
  publicSafe: entries.filter(entry => entry.publicSafe).length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
  surfaced: entries.filter(entry => entry.surfaced).length,
  tested: entries.filter(entry => entry.tested).length,
  total: entries.length,
})

const readinessStatus = (
  counts: ForgeTerminalAgentSystemsReadinessCounts,
  blockerRefs: ReadonlyArray<string>,
): ForgeTerminalAgentSystemsReadinessStatus => {
  if (blockerRefs.length > 0 || counts.blocked > 0) {
    return 'blocked'
  }

  if (counts.total === 0) {
    return 'empty'
  }

  if (counts.stale > 0) {
    return 'stale'
  }

  return counts.surfaced === counts.total &&
    counts.tested === counts.total &&
    counts.publicSafe === counts.total
    ? 'ready'
    : 'partial'
}

export const projectForgeTerminalAgentSystemsReadiness = (
  input: ForgeTerminalAgentSystemsReadinessInput,
): ForgeTerminalAgentSystemsReadinessView => {
  const readinessRef = safeOptionalRef(input.readinessRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.systems ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(entrySort)
  const omittedUnsafeRefCount =
    readinessRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omitted, 0)
  const safeReadinessRef = readinessRef.ref ?? 'unsafe-terminal-agent-readiness'
  const blockerRefs = Array.from(
    new Set([
      ...inputBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...(readinessRef.ref === null
        ? [blockerRef(safeReadinessRef, 'missing-readiness-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(safeReadinessRef, 'unsafe-readiness-material-omitted')]),
    ]),
  )
  const counts = readinessCounts(entries)

  return {
    blockerRefs,
    counts,
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    readinessRef: readinessRef.ref ?? 'unsafe-readiness-ref-omitted',
    status: readinessStatus(counts, blockerRefs),
  }
}
