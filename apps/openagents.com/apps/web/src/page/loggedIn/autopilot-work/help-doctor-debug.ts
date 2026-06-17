import type {
  AutopilotWorkHelpDoctorDebug,
  AutopilotWorkHelpDoctorEntry,
  AutopilotWorkHelpDoctorFreshness,
  AutopilotWorkHelpDoctorSeverity,
  AutopilotWorkHelpDoctorState,
  AutopilotWorkProjection,
} from '../model'

export type ForgeHelpDoctorDebugStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'warning'
  | 'unknown'

export type ForgeHelpDoctorDebugAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  debugBundleCollectionAuthority: false
  deploymentAuthority: false
  diagnosticsExecutionAuthority: false
  doctorExecutionAuthority: false
  fileReadAuthority: false
  logCollectionAuthority: false
  providerAuthority: false
  publicClaimAuthority: false
  runStateMutationAuthority: false
  settingsWriteAuthority: false
  settlementAuthority: false
  shellExecutionAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeHelpDoctorDebugItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  debugBundleRefs: ReadonlyArray<string>
  diagnosticRefs: ReadonlyArray<string>
  doctorCheckRefs: ReadonlyArray<string>
  freshness: AutopilotWorkHelpDoctorFreshness
  helpTopicRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  remediationRefs: ReadonlyArray<string>
  severity: AutopilotWorkHelpDoctorSeverity
  sourceRefs: ReadonlyArray<string>
  state: AutopilotWorkHelpDoctorState
  surfaceRef: string
}>

export type ForgeHelpDoctorDebugInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkHelpDoctorEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeHelpDoctorDebugCounts = Readonly<{
  blocked: number
  failed: number
  passed: number
  total: number
  warnings: number
}>

export type ForgeHelpDoctorDebugView = Readonly<{
  authority: ForgeHelpDoctorDebugAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeHelpDoctorDebugCounts
  entries: ReadonlyArray<ForgeHelpDoctorDebugItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeHelpDoctorDebugStatus
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
const PRIVATE_DEBUG_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /doctor[-_ ](?:body|content|log|output|text)/i,
  /raw[-_ ](?:body|command|content|debug|diagnostic|file|input|key|log|memory|notification|output|payload|prompt|provider|request|shell|source|stderr|stdout|terminal|trace|transcript)/i,
  /private[-_ ](?:command|content|debug|diagnostic|input|notification|prompt|repo|source|terminal|transcript|workspace)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeHelpDoctorDebugAuthority = {
  acceptedOutcomeAuthority: false,
  debugBundleCollectionAuthority: false,
  deploymentAuthority: false,
  diagnosticsExecutionAuthority: false,
  doctorExecutionAuthority: false,
  fileReadAuthority: false,
  logCollectionAuthority: false,
  providerAuthority: false,
  publicClaimAuthority: false,
  runStateMutationAuthority: false,
  settingsWriteAuthority: false,
  settlementAuthority: false,
  shellExecutionAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_DEBUG_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-help-doctor-debug-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkHelpDoctorEntry,
): Readonly<{
  entry: ForgeHelpDoctorDebugItem | null
  omittedUnsafeRefCount: number
}> => {
  const surfaceRef = safeOptionalRef(entry.surfaceRef)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const debugBundleRefs = safeRefs(entry.debugBundleRefs)
  const diagnosticRefs = safeRefs(entry.diagnosticRefs)
  const doctorCheckRefs = safeRefs(entry.doctorCheckRefs)
  const helpTopicRefs = safeRefs(entry.helpTopicRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const remediationRefs = safeRefs(entry.remediationRefs)
  const sourceRefs = safeRefs(entry.sourceRefs)
  const omittedUnsafeRefCount =
    surfaceRef.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    debugBundleRefs.omittedUnsafeRefCount +
    diagnosticRefs.omittedUnsafeRefCount +
    doctorCheckRefs.omittedUnsafeRefCount +
    helpTopicRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    remediationRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount

  return surfaceRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          blockerRefs: blockerRefs.refs,
          debugBundleRefs: debugBundleRefs.refs,
          diagnosticRefs: diagnosticRefs.refs,
          doctorCheckRefs: doctorCheckRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          helpTopicRefs: helpTopicRefs.refs,
          policyRefs: policyRefs.refs,
          remediationRefs: remediationRefs.refs,
          severity: entry.severity,
          sourceRefs: sourceRefs.refs,
          state: entry.state,
          surfaceRef: surfaceRef.ref,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeHelpDoctorDebugItem>,
): ForgeHelpDoctorDebugCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  failed: entries.filter(entry => entry.state === 'failed').length,
  passed: entries.filter(entry => entry.state === 'passed').length,
  total: entries.length,
  warnings: entries.filter(entry => entry.state === 'warning').length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeHelpDoctorDebugItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-doctor-evidence:${entry.surfaceRef}`))

const remediationBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeHelpDoctorDebugItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.state === 'failed' || entry.state === 'blocked') &&
        entry.remediationRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `remediation-ref-missing:${entry.surfaceRef}`))

const debugPolicyBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeHelpDoctorDebugItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.debugBundleRefs.length > 0 &&
        entry.policyRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `debug-bundle-policy-missing:${entry.surfaceRef}`))

const sourceEvidenceBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeHelpDoctorDebugItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.helpTopicRefs.length === 0 &&
        entry.doctorCheckRefs.length === 0 &&
        entry.diagnosticRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `help-doctor-source-missing:${entry.surfaceRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeHelpDoctorDebugItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeHelpDoctorDebugStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.state === 'warning')) {
    return 'warning'
  }

  return entries.every(entry => entry.state === 'passed') ? 'ready' : 'unknown'
}

export const projectForgeHelpDoctorDebug = (
  input: ForgeHelpDoctorDebugInput,
): ForgeHelpDoctorDebugView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.state.localeCompare(right.state) ||
        left.severity.localeCompare(right.severity) ||
        left.surfaceRef.localeCompare(right.surfaceRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleBlockers(input.workOrderRef, entries),
      ...remediationBlockers(input.workOrderRef, entries),
      ...debugPolicyBlockers(input.workOrderRef, entries),
      ...sourceEvidenceBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-help-doctor-debug-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-help-doctor-debug-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeHelpDoctorDebugInput = (
  work: AutopilotWorkProjection,
): ForgeHelpDoctorDebugInput => {
  const source: AutopilotWorkHelpDoctorDebug | undefined = work.helpDoctorDebug

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
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
