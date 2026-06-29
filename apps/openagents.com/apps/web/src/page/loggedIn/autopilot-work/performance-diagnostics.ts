import type {
  AutopilotWorkPerformanceDiagnostics,
  AutopilotWorkPerformanceEntry,
  AutopilotWorkPerformanceFreshness,
  AutopilotWorkPerformanceLatencyClass,
  AutopilotWorkPerformanceResourceClass,
  AutopilotWorkPerformanceStatus,
  AutopilotWorkProjection,
} from '../model'

export type ForgePerformanceDiagnosticsStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'slow'
  | 'stale'
  | 'unknown'

export type ForgePerformanceDiagnosticsAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  backpressureControlAuthority: false
  budgetProviderMutationAuthority: false
  deploymentAuthority: false
  metricsRecordAuthority: false
  profileExportAuthority: false
  publicClaimAuthority: false
  rawOutputReadAuthority: false
  runPauseCancelAuthority: false
  settlementAuthority: false
  timeoutEnforcementAuthority: false
  workerPayoutAuthority: false
}>

export type ForgePerformanceDiagnosticsItem = Readonly<{
  artifactRefs: ReadonlyArray<string>
  backpressureRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  budgetStopRefs: ReadonlyArray<string>
  counterRefs: ReadonlyArray<string>
  freshness: AutopilotWorkPerformanceFreshness
  latencyClass: AutopilotWorkPerformanceLatencyClass
  localResourcePressureRefs: ReadonlyArray<string>
  outputVolumeRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  profileRefs: ReadonlyArray<string>
  providerRateLimitRefs: ReadonlyArray<string>
  redactionRefs: ReadonlyArray<string>
  resourceClass: AutopilotWorkPerformanceResourceClass
  runRefs: ReadonlyArray<string>
  spanRef: string
  status: AutopilotWorkPerformanceStatus
  timeoutRefs: ReadonlyArray<string>
  truncationRefs: ReadonlyArray<string>
}>

export type ForgePerformanceDiagnosticsInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkPerformanceEntry>
  generatedAt: string
  profileRefs?: ReadonlyArray<string>
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgePerformanceDiagnosticsCounts = Readonly<{
  blocked: number
  entries: number
  slow: number
  stale: number
  truncated: number
}>

export type ForgePerformanceDiagnosticsView = Readonly<{
  authority: ForgePerformanceDiagnosticsAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgePerformanceDiagnosticsCounts
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  profileRefs: ReadonlyArray<string>
  publicSafe: true
  snapshotRef: string | null
  status: ForgePerformanceDiagnosticsStatus
  versionRef: string | null
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
const PRIVATE_PERFORMANCE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:command|content|detail|file|log|output|payload|profile|prompt|provider|shell|trace|transcript)/i,
  /private[-_ ](?:code|command|content|detail|file|log|payload|profile|prompt|repo|source|trace|workspace)/i,
  /command[-_ ]detail/i,
  /profile[-_ ]detail/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /customer[-_ ](?:data|private|payload|record)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgePerformanceDiagnosticsAuthority = {
  acceptedOutcomeAuthority: false,
  backpressureControlAuthority: false,
  budgetProviderMutationAuthority: false,
  deploymentAuthority: false,
  metricsRecordAuthority: false,
  profileExportAuthority: false,
  publicClaimAuthority: false,
  rawOutputReadAuthority: false,
  runPauseCancelAuthority: false,
  settlementAuthority: false,
  timeoutEnforcementAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_PERFORMANCE_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-performance-diagnostics-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkPerformanceEntry,
): Readonly<{
  item: ForgePerformanceDiagnosticsItem | null
  omittedUnsafeRefCount: number
}> => {
  const artifactRefs = safeRefs(item.artifactRefs)
  const backpressureRefs = safeRefs(item.backpressureRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const budgetStopRefs = safeRefs(item.budgetStopRefs)
  const counterRefs = safeRefs(item.counterRefs)
  const localResourcePressureRefs = safeRefs(item.localResourcePressureRefs)
  const outputVolumeRefs = safeRefs(item.outputVolumeRefs)
  const policyRefs = safeRefs(item.policyRefs)
  const profileRefs = safeRefs(item.profileRefs)
  const providerRateLimitRefs = safeRefs(item.providerRateLimitRefs)
  const redactionRefs = safeRefs(item.redactionRefs)
  const runRefs = safeRefs(item.runRefs)
  const spanRef = safeOptionalRef(item.spanRef)
  const timeoutRefs = safeRefs(item.timeoutRefs)
  const truncationRefs = safeRefs(item.truncationRefs)
  const omittedUnsafeRefCount =
    artifactRefs.omittedUnsafeRefCount +
    backpressureRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    budgetStopRefs.omittedUnsafeRefCount +
    counterRefs.omittedUnsafeRefCount +
    localResourcePressureRefs.omittedUnsafeRefCount +
    outputVolumeRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    profileRefs.omittedUnsafeRefCount +
    providerRateLimitRefs.omittedUnsafeRefCount +
    redactionRefs.omittedUnsafeRefCount +
    runRefs.omittedUnsafeRefCount +
    spanRef.omittedUnsafeRefCount +
    timeoutRefs.omittedUnsafeRefCount +
    truncationRefs.omittedUnsafeRefCount

  return spanRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          artifactRefs: artifactRefs.refs,
          backpressureRefs: backpressureRefs.refs,
          blockerRefs: blockerRefs.refs,
          budgetStopRefs: budgetStopRefs.refs,
          counterRefs: counterRefs.refs,
          freshness: item.freshness ?? 'unknown',
          latencyClass: item.latencyClass,
          localResourcePressureRefs: localResourcePressureRefs.refs,
          outputVolumeRefs: outputVolumeRefs.refs,
          policyRefs: policyRefs.refs,
          profileRefs: profileRefs.refs,
          providerRateLimitRefs: providerRateLimitRefs.refs,
          redactionRefs: redactionRefs.refs,
          resourceClass: item.resourceClass,
          runRefs: runRefs.refs,
          spanRef: spanRef.ref,
          status: item.status,
          timeoutRefs: timeoutRefs.refs,
          truncationRefs: truncationRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>,
): ForgePerformanceDiagnosticsCounts => ({
  blocked: entries.filter(entry => entry.status === 'blocked').length,
  entries: entries.length,
  slow: entries.filter(
    entry => entry.status === 'slow' || entry.latencyClass === 'slow',
  ).length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
  truncated: entries.filter(entry => entry.status === 'truncated').length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry =>
      blockerRef(workOrderRef, `stale-performance-evidence:${entry.spanRef}`),
    )

const slowBlockedEvidenceBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.status === 'blocked' || entry.latencyClass === 'blocked') &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `blocked-performance-without-blocker:${entry.spanRef}`),
    )

const truncationArtifactBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.status === 'truncated' || entry.truncationRefs.length > 0) &&
        entry.artifactRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `truncation-artifact-ref-missing:${entry.spanRef}`),
    )

const profilePolicyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.profileRefs.length > 0 &&
        (entry.redactionRefs.length === 0 || entry.policyRefs.length === 0) &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `profile-redaction-policy-missing:${entry.spanRef}`),
    )

const localPressurePolicyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.localResourcePressureRefs.length > 0 &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `local-pressure-policy-missing:${entry.spanRef}`),
    )

const rateLimitMisclassificationBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.providerRateLimitRefs.length > 0 &&
        entry.resourceClass === 'local_resource' &&
        entry.blockerRefs.length === 0,
    )
    .map(entry =>
      blockerRef(workOrderRef, `provider-rate-limit-not-local-pressure:${entry.spanRef}`),
    )

const statusForView = (
  entries: ReadonlyArray<ForgePerformanceDiagnosticsItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgePerformanceDiagnosticsStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.status === 'slow' || entry.latencyClass === 'slow')) {
    return 'slow'
  }

  return entries.every(entry => entry.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgePerformanceDiagnostics = (
  input: ForgePerformanceDiagnosticsInput,
): ForgePerformanceDiagnosticsView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const profileRefs = safeRefs(input.profileRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeItem)
  const entries = normalizedEntries
    .flatMap(result => (result.item === null ? [] : [result.item]))
    .sort(
      (left, right) =>
        left.resourceClass.localeCompare(right.resourceClass) ||
        left.spanRef.localeCompare(right.spanRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    profileRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const hasEntries = (input.entries ?? []).length > 0
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleBlockers(input.workOrderRef, entries),
      ...slowBlockedEvidenceBlockers(input.workOrderRef, entries),
      ...truncationArtifactBlockers(input.workOrderRef, entries),
      ...profilePolicyBlockers(input.workOrderRef, entries),
      ...localPressurePolicyBlockers(input.workOrderRef, entries),
      ...rateLimitMisclassificationBlockers(input.workOrderRef, entries),
      ...(hasEntries && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-performance-diagnostics-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-performance-diagnostics-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    profileRefs: profileRefs.refs,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgePerformanceDiagnosticsInput = (
  work: AutopilotWorkProjection,
): ForgePerformanceDiagnosticsInput => {
  const source: AutopilotWorkPerformanceDiagnostics | undefined =
    work.performanceDiagnostics

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
    ...(source.entries === undefined ? {} : { entries: source.entries }),
    ...(source.profileRefs === undefined ? {} : { profileRefs: source.profileRefs }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
