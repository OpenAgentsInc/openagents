import type {
  AutopilotWorkMultiAgentCoordination,
  AutopilotWorkMultiAgentCoordinationCriticality,
  AutopilotWorkMultiAgentCoordinationFreshness,
  AutopilotWorkMultiAgentCoordinationLane,
  AutopilotWorkMultiAgentCoordinationLaneKind,
  AutopilotWorkMultiAgentCoordinationState,
  AutopilotWorkProjection,
} from '../model'

export type ForgeMultiAgentCoordinationStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'running'
  | 'stale'
  | 'unknown'
  | 'waiting'

export type ForgeMultiAgentCoordinationAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  artifactMergeAuthority: false
  assignmentMutationAuthority: false
  deploymentAuthority: false
  laneCancelAuthority: false
  laneInboxAuthority: false
  lanePauseAuthority: false
  laneResumeAuthority: false
  laneStartAuthority: false
  marketProviderSelectionAuthority: false
  planningAuthority: false
  publicClaimAuthority: false
  settlementAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeMultiAgentCoordinationItem = Readonly<{
  acceptancePolicyRefs: ReadonlyArray<string>
  adapterRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  assignmentRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  budgetCapRefs: ReadonlyArray<string>
  capabilityRefs: ReadonlyArray<string>
  closeoutRefs: ReadonlyArray<string>
  conflictRefs: ReadonlyArray<string>
  criticality: AutopilotWorkMultiAgentCoordinationCriticality
  dependencyRefs: ReadonlyArray<string>
  freshness: AutopilotWorkMultiAgentCoordinationFreshness
  inboxRefs: ReadonlyArray<string>
  kind: AutopilotWorkMultiAgentCoordinationLaneKind
  laneRef: string
  mergeStrategyRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  providerRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
  state: AutopilotWorkMultiAgentCoordinationState
  steeringReceiptRefs: ReadonlyArray<string>
}>

export type ForgeMultiAgentCoordinationInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkMultiAgentCoordinationLane>
  generatedAt: string
  parentRunRef?: string | null
  planRef?: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeMultiAgentCoordinationCounts = Readonly<{
  blocked: number
  completed: number
  failedMandatory: number
  mandatory: number
  running: number
  total: number
}>

export type ForgeMultiAgentCoordinationView = Readonly<{
  authority: ForgeMultiAgentCoordinationAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeMultiAgentCoordinationCounts
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  parentRunRef: string | null
  planRef: string | null
  publicSafe: true
  snapshotRef: string | null
  status: ForgeMultiAgentCoordinationStatus
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
const PRIVATE_COORDINATION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:agent|artifact|assignment|command|content|file|inbox|lane|log|message|output|payload|plan|prompt|provider|session|shell|source|steering|workspace)/i,
  /private[-_ ](?:agent|artifact|assignment|content|file|inbox|lane|message|plan|prompt|provider|session|source|workspace)/i,
  /lane[-_ ](?:message|payload|private|prompt|transcript)/i,
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

const authority: ForgeMultiAgentCoordinationAuthority = {
  acceptedOutcomeAuthority: false,
  artifactMergeAuthority: false,
  assignmentMutationAuthority: false,
  deploymentAuthority: false,
  laneCancelAuthority: false,
  laneInboxAuthority: false,
  lanePauseAuthority: false,
  laneResumeAuthority: false,
  laneStartAuthority: false,
  marketProviderSelectionAuthority: false,
  planningAuthority: false,
  publicClaimAuthority: false,
  settlementAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_COORDINATION_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-multi-agent-coordination-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkMultiAgentCoordinationLane,
): Readonly<{
  entry: ForgeMultiAgentCoordinationItem | null
  omittedUnsafeRefCount: number
}> => {
  const acceptancePolicyRefs = safeRefs(entry.acceptancePolicyRefs)
  const adapterRefs = safeRefs(entry.adapterRefs)
  const artifactRefs = safeRefs(entry.artifactRefs)
  const assignmentRefs = safeRefs(entry.assignmentRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const budgetCapRefs = safeRefs(entry.budgetCapRefs)
  const capabilityRefs = safeRefs(entry.capabilityRefs)
  const closeoutRefs = safeRefs(entry.closeoutRefs)
  const conflictRefs = safeRefs(entry.conflictRefs)
  const dependencyRefs = safeRefs(entry.dependencyRefs)
  const inboxRefs = safeRefs(entry.inboxRefs)
  const laneRef = safeOptionalRef(entry.laneRef)
  const mergeStrategyRefs = safeRefs(entry.mergeStrategyRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const providerRefs = safeRefs(entry.providerRefs)
  const receiptRefs = safeRefs(entry.receiptRefs)
  const steeringReceiptRefs = safeRefs(entry.steeringReceiptRefs)
  const omittedUnsafeRefCount =
    acceptancePolicyRefs.omittedUnsafeRefCount +
    adapterRefs.omittedUnsafeRefCount +
    artifactRefs.omittedUnsafeRefCount +
    assignmentRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    budgetCapRefs.omittedUnsafeRefCount +
    capabilityRefs.omittedUnsafeRefCount +
    closeoutRefs.omittedUnsafeRefCount +
    conflictRefs.omittedUnsafeRefCount +
    dependencyRefs.omittedUnsafeRefCount +
    inboxRefs.omittedUnsafeRefCount +
    laneRef.omittedUnsafeRefCount +
    mergeStrategyRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    providerRefs.omittedUnsafeRefCount +
    receiptRefs.omittedUnsafeRefCount +
    steeringReceiptRefs.omittedUnsafeRefCount

  return laneRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          acceptancePolicyRefs: acceptancePolicyRefs.refs,
          adapterRefs: adapterRefs.refs,
          artifactRefs: artifactRefs.refs,
          assignmentRefs: assignmentRefs.refs,
          blockerRefs: blockerRefs.refs,
          budgetCapRefs: budgetCapRefs.refs,
          capabilityRefs: capabilityRefs.refs,
          closeoutRefs: closeoutRefs.refs,
          conflictRefs: conflictRefs.refs,
          criticality: entry.criticality,
          dependencyRefs: dependencyRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          inboxRefs: inboxRefs.refs,
          kind: entry.kind,
          laneRef: laneRef.ref,
          mergeStrategyRefs: mergeStrategyRefs.refs,
          policyRefs: policyRefs.refs,
          providerRefs: providerRefs.refs,
          receiptRefs: receiptRefs.refs,
          state: entry.state,
          steeringReceiptRefs: steeringReceiptRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>,
): ForgeMultiAgentCoordinationCounts => ({
  blocked: entries.filter(entry => entry.state === 'blocked').length,
  completed: entries.filter(
    entry => entry.state === 'completed' || entry.state === 'merged',
  ).length,
  failedMandatory: entries.filter(
    entry => entry.criticality === 'mandatory' && entry.state === 'failed',
  ).length,
  mandatory: entries.filter(entry => entry.criticality === 'mandatory').length,
  running: entries.filter(entry => entry.state === 'running').length,
  total: entries.length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-lane-evidence:${entry.laneRef}`))

const activeLaneReadinessBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.state === 'running' ||
          entry.state === 'waiting' ||
          entry.state === 'planned') &&
        (entry.assignmentRefs.length === 0 ||
          entry.capabilityRefs.length === 0 ||
          entry.policyRefs.length === 0) &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `active-lane-readiness-missing:${entry.laneRef}`))

const mandatoryFailureBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.criticality === 'mandatory' &&
        entry.state === 'failed' &&
        entry.closeoutRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `mandatory-lane-failed-without-closeout:${entry.laneRef}`))

const providerReceiptBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.kind === 'market' || entry.kind === 'external' || entry.kind === 'hosted') &&
        (entry.providerRefs.length === 0 ||
          entry.policyRefs.length === 0 ||
          entry.receiptRefs.length === 0) &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `provider-lane-receipt-missing:${entry.laneRef}`))

const conflictMergeBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.conflictRefs.length > 0 &&
        (entry.mergeStrategyRefs.length === 0 || entry.policyRefs.length === 0) &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `conflict-merge-policy-missing:${entry.laneRef}`))

const inboxReceiptBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.inboxRefs.length > 0 &&
        entry.steeringReceiptRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `lane-inbox-steering-receipt-missing:${entry.laneRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeMultiAgentCoordinationItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeMultiAgentCoordinationStatus => {
  if (blockerRefs.length > 0 || entries.some(entry => entry.state === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.state === 'running')) {
    return 'running'
  }

  if (entries.some(entry => entry.state === 'waiting' || entry.state === 'planned')) {
    return 'waiting'
  }

  return entries.every(entry => entry.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeMultiAgentCoordination = (
  input: ForgeMultiAgentCoordinationInput,
): ForgeMultiAgentCoordinationView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const planRef = safeOptionalRef(input.planRef)
  const parentRunRef = safeOptionalRef(input.parentRunRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.criticality.localeCompare(right.criticality) ||
        left.state.localeCompare(right.state) ||
        left.laneRef.localeCompare(right.laneRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    planRef.omittedUnsafeRefCount +
    parentRunRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleBlockers(input.workOrderRef, entries),
      ...activeLaneReadinessBlockers(input.workOrderRef, entries),
      ...mandatoryFailureBlockers(input.workOrderRef, entries),
      ...providerReceiptBlockers(input.workOrderRef, entries),
      ...conflictMergeBlockers(input.workOrderRef, entries),
      ...inboxReceiptBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-multi-agent-coordination-snapshot-ref')]
        : []),
      ...(input.entries !== undefined && input.entries.length > 0 && planRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-coordination-plan-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-multi-agent-coordination-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    parentRunRef: parentRunRef.ref,
    planRef: planRef.ref,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeMultiAgentCoordinationInput = (
  work: AutopilotWorkProjection,
): ForgeMultiAgentCoordinationInput => {
  const source: AutopilotWorkMultiAgentCoordination | undefined =
    work.multiAgentCoordination

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
    ...(source.parentRunRef === undefined ? {} : { parentRunRef: source.parentRunRef }),
    planRef: source.planRef,
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
