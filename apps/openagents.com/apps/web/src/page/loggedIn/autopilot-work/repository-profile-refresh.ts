export type ForgeRepositoryProfileKind =
  | 'command'
  | 'instruction'
  | 'invariant'
  | 'test'

export type ForgeRepositoryProfileFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeRepositoryProfileRefreshStatus =
  | 'blocked'
  | 'changed'
  | 'fresh'
  | 'stale'
  | 'unknown'

export type ForgeRepositoryProfileRefreshInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  changedProfileKinds?: ReadonlyArray<ForgeRepositoryProfileKind>
  commandProfileRefs?: ReadonlyArray<string>
  freshness?: ForgeRepositoryProfileFreshness
  generatedAt: string
  instructionRefs?: ReadonlyArray<string>
  invariantRefs?: ReadonlyArray<string>
  refreshedAt?: string | null
  repoIdentityRefs?: ReadonlyArray<string>
  testProfileRefs?: ReadonlyArray<string>
  workOrderRef: string
}>

export type ForgeRepositoryProfileRefreshReceipt = Readonly<{
  authority: Readonly<{
    commandExecutionAuthority: false
    invariantPolicyAuthority: false
    repositoryScanProof: false
    testExecutionAuthority: false
  }>
  blockerRefs: ReadonlyArray<string>
  changedProfileKinds: ReadonlyArray<ForgeRepositoryProfileKind>
  commandProfileRefs: ReadonlyArray<string>
  freshness: ForgeRepositoryProfileFreshness
  generatedAt: string
  instructionRefs: ReadonlyArray<string>
  invariantRefs: ReadonlyArray<string>
  omittedUnsafeRefCount: number
  provenance: 'refs_only_repository_profile_refresh'
  publicSafe: true
  receiptKind: 'forge_repository_profile_refresh.v1'
  receiptRef: string
  refreshedAt: string | null
  repoIdentityRefs: ReadonlyArray<string>
  status: ForgeRepositoryProfileRefreshStatus
  testProfileRefs: ReadonlyArray<string>
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

const SAFE_PROFILE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_PROFILE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:patch|file|source|shell|command|prompt|log|transcript|test)/i,
  /private[-_ ](?:repo|content|source|transcript|instructions?)/i,
  /provider[-_ ]payload/i,
  /wallet|payment[-_ ](?:material|preimage|hash)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:bearer|token|secret|mnemonic|preimage|invoice)\b/i,
]

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_PROFILE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_PROFILE_MARKERS.some(marker => marker.test(trimmed))
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

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-repository-profile-refresh-blocker:${workOrderRef}:${suffix}`

const slugRefPart = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '') || 'unknown'

const freshnessFromRefresh = (
  refreshedAt: string | null,
  generatedAt: string,
): ForgeRepositoryProfileFreshness => {
  if (refreshedAt === null) {
    return 'unknown'
  }

  const refreshed = Date.parse(refreshedAt)
  const generated = Date.parse(generatedAt)

  if (!Number.isFinite(refreshed) || !Number.isFinite(generated)) {
    return 'unknown'
  }

  return Math.max(0, generated - refreshed) > 24 * 60 * 60_000 ? 'stale' : 'fresh'
}

const uniqueProfileKinds = (
  kinds: ReadonlyArray<ForgeRepositoryProfileKind> | undefined,
): ReadonlyArray<ForgeRepositoryProfileKind> =>
  Array.from(new Set(kinds ?? [])).sort()

const statusForReceipt = (input: Readonly<{
  blockerRefs: ReadonlyArray<string>
  changedProfileKinds: ReadonlyArray<ForgeRepositoryProfileKind>
  freshness: ForgeRepositoryProfileFreshness
}>): ForgeRepositoryProfileRefreshStatus => {
  if (input.blockerRefs.length > 0) {
    return 'blocked'
  }

  if (input.freshness === 'stale') {
    return 'stale'
  }

  if (input.changedProfileKinds.length > 0) {
    return 'changed'
  }

  return input.freshness === 'fresh' ? 'fresh' : 'unknown'
}

export const projectForgeRepositoryProfileRefreshReceipt = (
  input: ForgeRepositoryProfileRefreshInput,
): ForgeRepositoryProfileRefreshReceipt => {
  const safeWorkOrderRef = safeRef(input.workOrderRef) ?? 'unsafe-work-order-ref-omitted'
  const refreshedAt = input.refreshedAt ?? null
  const repoIdentityRefs = safeRefs(input.repoIdentityRefs)
  const commandProfileRefs = safeRefs(input.commandProfileRefs)
  const testProfileRefs = safeRefs(input.testProfileRefs)
  const instructionRefs = safeRefs(input.instructionRefs)
  const invariantRefs = safeRefs(input.invariantRefs)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const changedProfileKinds = uniqueProfileKinds(input.changedProfileKinds)
  const freshness =
    input.freshness ?? freshnessFromRefresh(refreshedAt, input.generatedAt)
  const profileEvidenceCount =
    repoIdentityRefs.refs.length +
    commandProfileRefs.refs.length +
    testProfileRefs.refs.length +
    instructionRefs.refs.length +
    invariantRefs.refs.length
  const unsafeWorkOrderCount = safeWorkOrderRef === input.workOrderRef ? 0 : 1
  const omittedUnsafeRefCount =
    repoIdentityRefs.omittedUnsafeRefCount +
    commandProfileRefs.omittedUnsafeRefCount +
    testProfileRefs.omittedUnsafeRefCount +
    instructionRefs.omittedUnsafeRefCount +
    invariantRefs.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    unsafeWorkOrderCount
  const blockerRefs = Array.from(
    new Set([
      ...inputBlockerRefs.refs,
      ...(profileEvidenceCount === 0
        ? [blockerRef(safeWorkOrderRef, 'missing-repository-profile-evidence')]
        : []),
      ...(freshness === 'unknown'
        ? [blockerRef(safeWorkOrderRef, 'unknown-profile-refresh-freshness')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(safeWorkOrderRef, 'unsafe-profile-material-omitted')]),
    ]),
  )

  return {
    authority: {
      commandExecutionAuthority: false,
      invariantPolicyAuthority: false,
      repositoryScanProof: false,
      testExecutionAuthority: false,
    },
    blockerRefs,
    changedProfileKinds,
    commandProfileRefs: commandProfileRefs.refs,
    freshness,
    generatedAt: input.generatedAt,
    instructionRefs: instructionRefs.refs,
    invariantRefs: invariantRefs.refs,
    omittedUnsafeRefCount,
    provenance: 'refs_only_repository_profile_refresh',
    publicSafe: true,
    receiptKind: 'forge_repository_profile_refresh.v1',
    receiptRef: `forge.repository_profile_refresh.${slugRefPart(safeWorkOrderRef)}.${slugRefPart(input.generatedAt)}`,
    refreshedAt,
    repoIdentityRefs: repoIdentityRefs.refs,
    status: statusForReceipt({ blockerRefs, changedProfileKinds, freshness }),
    testProfileRefs: testProfileRefs.refs,
    workOrderRef: safeWorkOrderRef,
  }
}
