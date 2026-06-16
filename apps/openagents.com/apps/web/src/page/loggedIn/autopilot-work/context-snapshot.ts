export type ForgeContextSnapshotStatus = 'blocked' | 'ready' | 'stale' | 'unknown'

export type ForgeContextFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeContextDirtyState = 'clean' | 'dirty' | 'unknown'

export type ForgeContextRefGroupInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  refs?: ReadonlyArray<string>
}>

export type ForgeContextCurrentJobInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  capabilityRefs?: ReadonlyArray<string>
  jobRefs?: ReadonlyArray<string>
  verificationRefs?: ReadonlyArray<string>
}>

export type ForgeContextRepoInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  changedCount?: number | null
  dirtyState?: ForgeContextDirtyState
  dirtyStateRefs?: ReadonlyArray<string>
  identityRefs?: ReadonlyArray<string>
}>

export type ForgeContextSnapshotInput = Readonly<{
  adapters?: ForgeContextRefGroupInput & Readonly<{
    capabilityRefs?: ReadonlyArray<string>
  }>
  blockerRefs?: ReadonlyArray<string>
  currentJob?: ForgeContextCurrentJobInput
  devDoctor?: ForgeContextRefGroupInput
  freshness?: ForgeContextFreshness
  generatedAt: string
  instructions?: ForgeContextRefGroupInput & Readonly<{
    configRefs?: ReadonlyArray<string>
  }>
  observedAt?: string | null
  repo?: ForgeContextRepoInput
  workOrderRef: string
}>

export type ForgeContextSnapshot = Readonly<{
  adapters: Readonly<{
    blockerRefs: ReadonlyArray<string>
    capabilityRefs: ReadonlyArray<string>
    readinessRefs: ReadonlyArray<string>
  }>
  blockerRefs: ReadonlyArray<string>
  currentJob: Readonly<{
    blockerRefs: ReadonlyArray<string>
    capabilityRefs: ReadonlyArray<string>
    jobRefs: ReadonlyArray<string>
    verificationRefs: ReadonlyArray<string>
  }>
  devDoctor: Readonly<{
    blockerRefs: ReadonlyArray<string>
    doctorRefs: ReadonlyArray<string>
  }>
  freshness: ForgeContextFreshness
  generatedAt: string
  instructions: Readonly<{
    blockerRefs: ReadonlyArray<string>
    configRefs: ReadonlyArray<string>
    instructionRefs: ReadonlyArray<string>
  }>
  observedAt: string | null
  omittedUnsafeRefCount: number
  repo: Readonly<{
    blockerRefs: ReadonlyArray<string>
    changedCount: number | null
    dirtyState: ForgeContextDirtyState
    dirtyStateRefs: ReadonlyArray<string>
    identityRefs: ReadonlyArray<string>
  }>
  status: ForgeContextSnapshotStatus
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

const SAFE_CONTEXT_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_CONTEXT_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:patch|file|source|shell|command|prompt|log|transcript)/i,
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

  return SAFE_CONTEXT_REF_PATTERN.test(trimmed) &&
    !PRIVATE_CONTEXT_MARKERS.some(marker => marker.test(trimmed))
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

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-context-snapshot-blocker:${workOrderRef}:${suffix}`

const nonNegativeCount = (value: number | null | undefined): number | null =>
  value === null || value === undefined || !Number.isSafeInteger(value) || value < 0
    ? null
    : value

const freshnessFromObservation = (
  observedAt: string | null,
  generatedAt: string,
): ForgeContextFreshness => {
  if (observedAt === null) {
    return 'unknown'
  }

  const observed = Date.parse(observedAt)
  const generated = Date.parse(generatedAt)

  if (!Number.isFinite(observed) || !Number.isFinite(generated)) {
    return 'unknown'
  }

  return Math.max(0, generated - observed) > 30 * 60_000 ? 'stale' : 'fresh'
}

const hasAnyContextEvidence = (snapshot: Omit<
  ForgeContextSnapshot,
  'blockerRefs' | 'freshness' | 'omittedUnsafeRefCount' | 'status'
>): boolean =>
  snapshot.repo.identityRefs.length > 0 ||
  snapshot.repo.dirtyStateRefs.length > 0 ||
  snapshot.instructions.instructionRefs.length > 0 ||
  snapshot.instructions.configRefs.length > 0 ||
  snapshot.adapters.readinessRefs.length > 0 ||
  snapshot.adapters.capabilityRefs.length > 0 ||
  snapshot.devDoctor.doctorRefs.length > 0 ||
  snapshot.currentJob.jobRefs.length > 0 ||
  snapshot.currentJob.verificationRefs.length > 0 ||
  snapshot.currentJob.capabilityRefs.length > 0

const statusForSnapshot = (
  snapshot: Omit<ForgeContextSnapshot, 'status'>,
): ForgeContextSnapshotStatus => {
  if (snapshot.blockerRefs.length > 0) {
    return 'blocked'
  }

  if (
    snapshot.freshness === 'stale' ||
    snapshot.repo.dirtyState === 'dirty' ||
    snapshot.repo.dirtyStateRefs.length > 0
  ) {
    return 'stale'
  }

  return snapshot.freshness === 'fresh' ? 'ready' : 'unknown'
}

export const projectForgeContextSnapshot = (
  input: ForgeContextSnapshotInput,
): ForgeContextSnapshot => {
  const safeWorkOrderRef = safeRef(input.workOrderRef) ?? 'unsafe-work-order-ref-omitted'
  const observedAt = input.observedAt ?? null
  const repoIdentityRefs = safeRefs(input.repo?.identityRefs)
  const repoDirtyStateRefs = safeRefs(input.repo?.dirtyStateRefs)
  const repoBlockerRefs = safeRefs(input.repo?.blockerRefs)
  const instructionRefs = safeRefs(input.instructions?.refs)
  const instructionConfigRefs = safeRefs(input.instructions?.configRefs)
  const instructionBlockerRefs = safeRefs(input.instructions?.blockerRefs)
  const adapterReadinessRefs = safeRefs(input.adapters?.refs)
  const adapterCapabilityRefs = safeRefs(input.adapters?.capabilityRefs)
  const adapterBlockerRefs = safeRefs(input.adapters?.blockerRefs)
  const devDoctorRefs = safeRefs(input.devDoctor?.refs)
  const devDoctorBlockerRefs = safeRefs(input.devDoctor?.blockerRefs)
  const currentJobRefs = safeRefs(input.currentJob?.jobRefs)
  const currentJobVerificationRefs = safeRefs(input.currentJob?.verificationRefs)
  const currentJobCapabilityRefs = safeRefs(input.currentJob?.capabilityRefs)
  const currentJobBlockerRefs = safeRefs(input.currentJob?.blockerRefs)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const unsafeWorkOrderCount = safeWorkOrderRef === input.workOrderRef ? 0 : 1
  const freshness =
    input.freshness ?? freshnessFromObservation(observedAt, input.generatedAt)
  const baseSnapshot = {
    adapters: {
      blockerRefs: adapterBlockerRefs.refs,
      capabilityRefs: adapterCapabilityRefs.refs,
      readinessRefs: adapterReadinessRefs.refs,
    },
    currentJob: {
      blockerRefs: currentJobBlockerRefs.refs,
      capabilityRefs: currentJobCapabilityRefs.refs,
      jobRefs: currentJobRefs.refs,
      verificationRefs: currentJobVerificationRefs.refs,
    },
    devDoctor: {
      blockerRefs: devDoctorBlockerRefs.refs,
      doctorRefs: devDoctorRefs.refs,
    },
    generatedAt: input.generatedAt,
    instructions: {
      blockerRefs: instructionBlockerRefs.refs,
      configRefs: instructionConfigRefs.refs,
      instructionRefs: instructionRefs.refs,
    },
    observedAt,
    repo: {
      blockerRefs: repoBlockerRefs.refs,
      changedCount: nonNegativeCount(input.repo?.changedCount),
      dirtyState: input.repo?.dirtyState ?? 'unknown',
      dirtyStateRefs: repoDirtyStateRefs.refs,
      identityRefs: repoIdentityRefs.refs,
    },
    workOrderRef: safeWorkOrderRef,
  }
  const evidenceBlockerRefs = hasAnyContextEvidence(baseSnapshot)
    ? []
    : [blockerRef(safeWorkOrderRef, 'missing-context-evidence')]
  const freshnessBlockerRefs =
    freshness === 'unknown'
      ? [blockerRef(safeWorkOrderRef, 'unknown-context-freshness')]
      : []
  const unsafeRefCount =
    repoIdentityRefs.omittedUnsafeRefCount +
    repoDirtyStateRefs.omittedUnsafeRefCount +
    repoBlockerRefs.omittedUnsafeRefCount +
    instructionRefs.omittedUnsafeRefCount +
    instructionConfigRefs.omittedUnsafeRefCount +
    instructionBlockerRefs.omittedUnsafeRefCount +
    adapterReadinessRefs.omittedUnsafeRefCount +
    adapterCapabilityRefs.omittedUnsafeRefCount +
    adapterBlockerRefs.omittedUnsafeRefCount +
    devDoctorRefs.omittedUnsafeRefCount +
    devDoctorBlockerRefs.omittedUnsafeRefCount +
    currentJobRefs.omittedUnsafeRefCount +
    currentJobVerificationRefs.omittedUnsafeRefCount +
    currentJobCapabilityRefs.omittedUnsafeRefCount +
    currentJobBlockerRefs.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    unsafeWorkOrderCount
  const unsafeBlockerRefs =
    unsafeRefCount === 0
      ? []
      : [blockerRef(safeWorkOrderRef, 'unsafe-context-material-omitted')]
  const blockerRefs = Array.from(
    new Set([
      ...repoBlockerRefs.refs,
      ...instructionBlockerRefs.refs,
      ...adapterBlockerRefs.refs,
      ...devDoctorBlockerRefs.refs,
      ...currentJobBlockerRefs.refs,
      ...inputBlockerRefs.refs,
      ...evidenceBlockerRefs,
      ...freshnessBlockerRefs,
      ...unsafeBlockerRefs,
    ]),
  )
  const snapshotWithoutStatus = {
    ...baseSnapshot,
    blockerRefs,
    freshness,
    omittedUnsafeRefCount: unsafeRefCount,
  }

  return {
    ...snapshotWithoutStatus,
    status: statusForSnapshot(snapshotWithoutStatus),
  }
}
