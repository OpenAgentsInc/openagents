import {
  type ForgeRepositoryProfileFreshness,
  type ForgeRepositoryProfileKind,
  type ForgeRepositoryProfileRefreshInput,
  type ForgeRepositoryProfileRefreshReceipt,
  projectForgeRepositoryProfileRefreshReceipt,
} from './repository-profile-refresh'

export type ForgeRepositoryMemoryDirtyState = 'clean' | 'dirty' | 'unknown'

export type ForgeRepositoryMemoryProfileStatus =
  | 'blocked'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeRepositoryMemoryProfileInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  changedProfileKinds?: ReadonlyArray<ForgeRepositoryProfileKind>
  commandProfileRefs?: ReadonlyArray<string>
  currentInstructionRefs?: ReadonlyArray<string>
  devDoctorRefs?: ReadonlyArray<string>
  dirtyState?: ForgeRepositoryMemoryDirtyState
  freshness?: ForgeRepositoryProfileFreshness
  generatedAt: string
  instructionRefs?: ReadonlyArray<string>
  invariantRefs?: ReadonlyArray<string>
  profileRef: string
  refreshedAt?: string | null
  refreshEvents?: ReadonlyArray<ForgeRepositoryProfileRefreshInput>
  refreshReceiptRefs?: ReadonlyArray<string>
  repoIdentityRefs?: ReadonlyArray<string>
  testProfileRefs?: ReadonlyArray<string>
  workOrderRef: string
}>

export type ForgeRepositoryMemoryProfile = Readonly<{
  blockerRefs: ReadonlyArray<string>
  changedProfileKinds: ReadonlyArray<ForgeRepositoryProfileKind>
  commandProfileRefs: ReadonlyArray<string>
  currentInstructionRefs: ReadonlyArray<string>
  devDoctorRefs: ReadonlyArray<string>
  dirtyState: ForgeRepositoryMemoryDirtyState
  freshness: ForgeRepositoryProfileFreshness
  generatedAt: string
  instructionRefs: ReadonlyArray<string>
  invariantRefs: ReadonlyArray<string>
  omittedUnsafeRefCount: number
  profileRef: string
  refreshedAt: string | null
  refreshReceiptRefs: ReadonlyArray<string>
  refreshReceipts: ReadonlyArray<ForgeRepositoryProfileRefreshReceipt>
  repoIdentityRefs: ReadonlyArray<string>
  status: ForgeRepositoryMemoryProfileStatus
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

const blockerRef = (profileRef: string, suffix: string): string =>
  `forge-repository-memory-profile-blocker:${profileRef}:${suffix}`

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

const sameRefSet = (
  left: ReadonlyArray<string>,
  right: ReadonlyArray<string>,
): boolean =>
  left.length === right.length && left.every(ref => right.includes(ref))

const hasInstructionInvalidation = (
  profileInstructionRefs: ReadonlyArray<string>,
  currentInstructionRefs: ReadonlyArray<string>,
): boolean =>
  profileInstructionRefs.length > 0 &&
  currentInstructionRefs.length > 0 &&
  !sameRefSet(profileInstructionRefs, currentInstructionRefs)

const sortReceipts = (
  receipts: ReadonlyArray<ForgeRepositoryProfileRefreshReceipt>,
): ReadonlyArray<ForgeRepositoryProfileRefreshReceipt> =>
  Array.from(receipts).sort(
    (left, right) =>
      Date.parse(right.generatedAt) - Date.parse(left.generatedAt) ||
      left.receiptRef.localeCompare(right.receiptRef),
  )

const statusForProfile = (input: Readonly<{
  blockingRefs: ReadonlyArray<string>
  freshness: ForgeRepositoryProfileFreshness
  staleRefs: ReadonlyArray<string>
}>): ForgeRepositoryMemoryProfileStatus => {
  if (input.blockingRefs.length > 0) {
    return 'blocked'
  }

  if (input.staleRefs.length > 0 || input.freshness === 'stale') {
    return 'stale'
  }

  return input.freshness === 'fresh' ? 'ready' : 'unknown'
}

export const projectForgeRepositoryMemoryProfile = (
  input: ForgeRepositoryMemoryProfileInput,
): ForgeRepositoryMemoryProfile => {
  const safeWorkOrderRef = safeRef(input.workOrderRef) ?? 'unsafe-work-order-ref-omitted'
  const safeProfileRef = safeRef(input.profileRef) ?? 'unsafe-profile-ref-omitted'
  const refreshedAt = input.refreshedAt ?? null
  const repoIdentityRefs = safeRefs(input.repoIdentityRefs)
  const commandProfileRefs = safeRefs(input.commandProfileRefs)
  const testProfileRefs = safeRefs(input.testProfileRefs)
  const instructionRefs = safeRefs(input.instructionRefs)
  const currentInstructionRefs = safeRefs(input.currentInstructionRefs)
  const invariantRefs = safeRefs(input.invariantRefs)
  const devDoctorRefs = safeRefs(input.devDoctorRefs)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const explicitRefreshReceiptRefs = safeRefs(input.refreshReceiptRefs)
  const refreshReceipts = sortReceipts(
    (input.refreshEvents ?? []).map(projectForgeRepositoryProfileRefreshReceipt),
  )
  const changedProfileKinds = uniqueProfileKinds(input.changedProfileKinds)
  const freshness =
    input.freshness ?? freshnessFromRefresh(refreshedAt, input.generatedAt)
  const dirtyState = input.dirtyState ?? 'unknown'
  const profileEvidenceCount =
    repoIdentityRefs.refs.length +
    commandProfileRefs.refs.length +
    testProfileRefs.refs.length +
    instructionRefs.refs.length +
    invariantRefs.refs.length
  const unsafeProfileRefCount = safeProfileRef === input.profileRef ? 0 : 1
  const unsafeWorkOrderCount = safeWorkOrderRef === input.workOrderRef ? 0 : 1
  const refreshReceiptRefs = Array.from(
    new Set([
      ...explicitRefreshReceiptRefs.refs,
      ...refreshReceipts.map(receipt => receipt.receiptRef),
    ]),
  )
  const refreshReceiptOmittedUnsafeRefCount = refreshReceipts.reduce(
    (total, receipt) => total + receipt.omittedUnsafeRefCount,
    0,
  )
  const omittedUnsafeRefCount =
    repoIdentityRefs.omittedUnsafeRefCount +
    commandProfileRefs.omittedUnsafeRefCount +
    testProfileRefs.omittedUnsafeRefCount +
    instructionRefs.omittedUnsafeRefCount +
    currentInstructionRefs.omittedUnsafeRefCount +
    invariantRefs.omittedUnsafeRefCount +
    devDoctorRefs.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    explicitRefreshReceiptRefs.omittedUnsafeRefCount +
    refreshReceiptOmittedUnsafeRefCount +
    unsafeProfileRefCount +
    unsafeWorkOrderCount
  const instructionInvalidated = hasInstructionInvalidation(
    instructionRefs.refs,
    currentInstructionRefs.refs,
  )
  const staleRefs = [
    ...(dirtyState === 'dirty'
      ? [blockerRef(safeProfileRef, 'dirty-worktree-invalidates-profile')]
      : []),
    ...(instructionInvalidated
      ? [blockerRef(safeProfileRef, 'instruction-refs-changed')]
      : []),
  ]
  const blockingRefs = [
    ...inputBlockerRefs.refs,
    ...(profileEvidenceCount === 0
      ? [blockerRef(safeProfileRef, 'missing-repository-profile-evidence')]
      : []),
    ...(devDoctorRefs.refs.length === 0
      ? [blockerRef(safeProfileRef, 'missing-dev-doctor-evidence')]
      : []),
    ...(freshness === 'unknown'
      ? [blockerRef(safeProfileRef, 'unknown-profile-freshness')]
      : []),
    ...(omittedUnsafeRefCount === 0
      ? []
      : [blockerRef(safeProfileRef, 'unsafe-profile-material-omitted')]),
  ]
  const blockerRefs = Array.from(new Set([...blockingRefs, ...staleRefs]))

  return {
    blockerRefs,
    changedProfileKinds,
    commandProfileRefs: commandProfileRefs.refs,
    currentInstructionRefs: currentInstructionRefs.refs,
    devDoctorRefs: devDoctorRefs.refs,
    dirtyState,
    freshness,
    generatedAt: input.generatedAt,
    instructionRefs: instructionRefs.refs,
    invariantRefs: invariantRefs.refs,
    omittedUnsafeRefCount,
    profileRef: safeProfileRef,
    refreshedAt,
    refreshReceiptRefs,
    refreshReceipts,
    repoIdentityRefs: repoIdentityRefs.refs,
    status: statusForProfile({ blockingRefs, freshness, staleRefs }),
    testProfileRefs: testProfileRefs.refs,
    workOrderRef: safeWorkOrderRef,
  }
}
