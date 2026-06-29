import type {
  AutopilotWorkOnboardingCapabilityEntry,
  AutopilotWorkOnboardingCapabilityFreshness,
  AutopilotWorkOnboardingCapabilityMode,
  AutopilotWorkOnboardingCapabilityStatus,
  AutopilotWorkOnboardingCapabilityStepKind,
  AutopilotWorkProjection,
} from '../model'

export type ForgeOnboardingCapabilityStatus =
  | 'blocked'
  | 'empty'
  | 'in_progress'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeOnboardingCapabilityAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  capabilityEnablementAuthority: false
  credentialStorageAuthority: false
  dataScopeMutationAuthority: false
  firstRunSmokeExecutionAuthority: false
  integrationEnablementAuthority: false
  onboardingStepMutationAuthority: false
  paidWorkflowActivationAuthority: false
  permissionGrantAuthority: false
  providerConnectionAuthority: false
  repositoryScanAuthority: false
  repositoryWriteAuthority: false
  secretCollectionAuthority: false
  settingsMutationAuthority: false
  settlementAuthority: false
  teamInvitationAcceptanceAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeOnboardingCapabilityItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  capabilityProbeRefs: ReadonlyArray<string>
  completionReceiptRefs: ReadonlyArray<string>
  credentialPolicyRefs: ReadonlyArray<string>
  dataScopeRefs: ReadonlyArray<string>
  firstRunSmokeRefs: ReadonlyArray<string>
  freshness: AutopilotWorkOnboardingCapabilityFreshness
  instructionRefs: ReadonlyArray<string>
  integrationRefs: ReadonlyArray<string>
  invariantRefs: ReadonlyArray<string>
  mode: AutopilotWorkOnboardingCapabilityMode
  permissionDecisionRefs: ReadonlyArray<string>
  providerReadinessRefs: ReadonlyArray<string>
  repositoryProfileRefs: ReadonlyArray<string>
  skipReceiptRefs: ReadonlyArray<string>
  status: AutopilotWorkOnboardingCapabilityStatus
  stepKind: AutopilotWorkOnboardingCapabilityStepKind
  stepRef: string
  userDeviceRefs: ReadonlyArray<string>
  workspaceRefs: ReadonlyArray<string>
}>

export type ForgeOnboardingCapabilityInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkOnboardingCapabilityEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeOnboardingCapabilityCounts = Readonly<{
  blocked: number
  providerConnected: number
  ready: number
  skipped: number
  smokes: number
  stale: number
  steps: number
}>

export type ForgeOnboardingCapabilityView = Readonly<{
  authority: ForgeOnboardingCapabilityAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeOnboardingCapabilityCounts
  entries: ReadonlyArray<ForgeOnboardingCapabilityItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeOnboardingCapabilityStatus
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
const PRIVATE_ONBOARDING_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:credential|data|device|file|integration|instruction|log|provider|repo|secret|smoke|token|user|workspace)/i,
  /private[-_ ](?:content|credential|data|file|integration|payload|provider|repo|source|workspace)/i,
  /credential[-_ ]value/i,
  /instruction[-_ ]body/i,
  /integration[-_ ]payload/i,
  /provider[-_ ]payload/i,
  /repo(?:sitory)?[-_ ](?:data|payload|private)/i,
  /smoke[-_ ]log/i,
  /workspace[-_ ](?:path|payload|private)/i,
  /user[-_ ](?:email|id|identifier|private)/i,
  /device[-_ ](?:id|identifier|serial)/i,
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

const PROVIDER_CONNECTED_MODES: ReadonlySet<AutopilotWorkOnboardingCapabilityMode> =
  new Set(['api_connected', 'managed', 'pylon_provider'])

const REQUIRED_SETUP_STEPS: ReadonlySet<AutopilotWorkOnboardingCapabilityStepKind> =
  new Set([
    'capability_probe',
    'data_scope',
    'first_run_smoke',
    'instructions_invariants',
    'permission',
    'repository_profile',
    'workspace',
  ])

const REPOSITORY_WORKSPACE_STEPS: ReadonlySet<AutopilotWorkOnboardingCapabilityStepKind> =
  new Set(['repository_profile', 'workspace'])

const READY_STATUSES: ReadonlySet<AutopilotWorkOnboardingCapabilityStatus> =
  new Set(['completed', 'ready'])

const authority: ForgeOnboardingCapabilityAuthority = {
  acceptedOutcomeAuthority: false,
  capabilityEnablementAuthority: false,
  credentialStorageAuthority: false,
  dataScopeMutationAuthority: false,
  firstRunSmokeExecutionAuthority: false,
  integrationEnablementAuthority: false,
  onboardingStepMutationAuthority: false,
  paidWorkflowActivationAuthority: false,
  permissionGrantAuthority: false,
  providerConnectionAuthority: false,
  repositoryScanAuthority: false,
  repositoryWriteAuthority: false,
  secretCollectionAuthority: false,
  settingsMutationAuthority: false,
  settlementAuthority: false,
  teamInvitationAcceptanceAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_ONBOARDING_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-onboarding-capability-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkOnboardingCapabilityEntry,
): Readonly<{
  item: ForgeOnboardingCapabilityItem | null
  omittedUnsafeRefCount: number
}> => {
  const blockerRefs = safeRefs(item.blockerRefs)
  const capabilityProbeRefs = safeRefs(item.capabilityProbeRefs)
  const completionReceiptRefs = safeRefs(item.completionReceiptRefs)
  const credentialPolicyRefs = safeRefs(item.credentialPolicyRefs)
  const dataScopeRefs = safeRefs(item.dataScopeRefs)
  const firstRunSmokeRefs = safeRefs(item.firstRunSmokeRefs)
  const instructionRefs = safeRefs(item.instructionRefs)
  const integrationRefs = safeRefs(item.integrationRefs)
  const invariantRefs = safeRefs(item.invariantRefs)
  const permissionDecisionRefs = safeRefs(item.permissionDecisionRefs)
  const providerReadinessRefs = safeRefs(item.providerReadinessRefs)
  const repositoryProfileRefs = safeRefs(item.repositoryProfileRefs)
  const skipReceiptRefs = safeRefs(item.skipReceiptRefs)
  const stepRef = safeOptionalRef(item.stepRef)
  const userDeviceRefs = safeRefs(item.userDeviceRefs)
  const workspaceRefs = safeRefs(item.workspaceRefs)
  const omittedUnsafeRefCount =
    blockerRefs.omittedUnsafeRefCount +
    capabilityProbeRefs.omittedUnsafeRefCount +
    completionReceiptRefs.omittedUnsafeRefCount +
    credentialPolicyRefs.omittedUnsafeRefCount +
    dataScopeRefs.omittedUnsafeRefCount +
    firstRunSmokeRefs.omittedUnsafeRefCount +
    instructionRefs.omittedUnsafeRefCount +
    integrationRefs.omittedUnsafeRefCount +
    invariantRefs.omittedUnsafeRefCount +
    permissionDecisionRefs.omittedUnsafeRefCount +
    providerReadinessRefs.omittedUnsafeRefCount +
    repositoryProfileRefs.omittedUnsafeRefCount +
    skipReceiptRefs.omittedUnsafeRefCount +
    stepRef.omittedUnsafeRefCount +
    userDeviceRefs.omittedUnsafeRefCount +
    workspaceRefs.omittedUnsafeRefCount

  return stepRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          blockerRefs: blockerRefs.refs,
          capabilityProbeRefs: capabilityProbeRefs.refs,
          completionReceiptRefs: completionReceiptRefs.refs,
          credentialPolicyRefs: credentialPolicyRefs.refs,
          dataScopeRefs: dataScopeRefs.refs,
          firstRunSmokeRefs: firstRunSmokeRefs.refs,
          freshness: item.freshness ?? 'unknown',
          instructionRefs: instructionRefs.refs,
          integrationRefs: integrationRefs.refs,
          invariantRefs: invariantRefs.refs,
          mode: item.mode,
          permissionDecisionRefs: permissionDecisionRefs.refs,
          providerReadinessRefs: providerReadinessRefs.refs,
          repositoryProfileRefs: repositoryProfileRefs.refs,
          skipReceiptRefs: skipReceiptRefs.refs,
          status: item.status,
          stepKind: item.stepKind,
          stepRef: stepRef.ref,
          userDeviceRefs: userDeviceRefs.refs,
          workspaceRefs: workspaceRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeOnboardingCapabilityItem>,
): ForgeOnboardingCapabilityCounts => ({
  blocked: entries.filter(entry => entry.status === 'blocked').length,
  providerConnected: entries.filter(entry =>
    PROVIDER_CONNECTED_MODES.has(entry.mode),
  ).length,
  ready: entries.filter(
    entry => entry.status === 'ready' || entry.status === 'completed',
  ).length,
  skipped: entries.filter(entry => entry.status === 'skipped').length,
  smokes: entries.filter(entry => entry.firstRunSmokeRefs.length > 0).length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
  steps: entries.length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeOnboardingCapabilityItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]
  const ready = READY_STATUSES.has(item.status)

  if (item.freshness === 'stale') {
    blockers.push(
      blockerRef(workOrderRef, `stale-onboarding-capability-evidence:${item.stepRef}`),
    )
  }

  if (
    ready &&
    REQUIRED_SETUP_STEPS.has(item.stepKind) &&
    item.capabilityProbeRefs.length === 0 &&
    item.completionReceiptRefs.length === 0 &&
    item.firstRunSmokeRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `required-onboarding-step-evidence-missing:${item.stepRef}`),
    )
  }

  if (
    ready &&
    PROVIDER_CONNECTED_MODES.has(item.mode) &&
    (item.providerReadinessRefs.length === 0 ||
      item.credentialPolicyRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `provider-connected-mode-evidence-missing:${item.stepRef}`),
    )
  }

  if (
    ready &&
    REPOSITORY_WORKSPACE_STEPS.has(item.stepKind) &&
    (item.repositoryProfileRefs.length === 0 ||
      item.instructionRefs.length === 0 ||
      item.invariantRefs.length === 0 ||
      item.permissionDecisionRefs.length === 0 ||
      item.dataScopeRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `repository-workspace-setup-evidence-missing:${item.stepRef}`),
    )
  }

  if (
    item.status === 'skipped' &&
    item.skipReceiptRefs.length === 0 &&
    (item.stepKind === 'integration' || item.stepKind === 'provider')
  ) {
    blockers.push(
      blockerRef(workOrderRef, `optional-onboarding-skip-receipt-missing:${item.stepRef}`),
    )
  }

  if (
    ready &&
    item.stepKind === 'first_run_smoke' &&
    item.firstRunSmokeRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `first-run-smoke-receipt-missing:${item.stepRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeOnboardingCapabilityItem>,
  blockers: ReadonlyArray<string>,
): ForgeOnboardingCapabilityStatus => {
  if (blockers.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (
    entries.some(
      entry => entry.status === 'in_progress' || entry.status === 'planned',
    )
  ) {
    return 'in_progress'
  }

  if (
    entries.every(entry =>
      ['completed', 'ready', 'skipped'].includes(entry.status),
    )
  ) {
    return 'ready'
  }

  return 'unknown'
}

export const projectForgeOnboardingCapabilityEvidence = (
  input: ForgeOnboardingCapabilityInput,
): ForgeOnboardingCapabilityView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalized = (input.entries ?? []).map(normalizeItem)
  const entries = normalized.flatMap(result =>
    result.item === null ? [] : [result.item],
  )
  const normalizedOmissions = normalized.reduce(
    (total, result) => total + result.omittedUnsafeRefCount,
    0,
  )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedOmissions
  const blockers = [
    ...inputBlockerRefs.refs,
    ...entries.flatMap(entry => itemBlockers(input.workOrderRef, entry)),
  ]

  if (input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null) {
    blockers.push(
      blockerRef(input.workOrderRef, 'missing-onboarding-capability-snapshot-ref'),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-onboarding-capability-material-omitted'),
    )
  }

  const uniqueBlockers = Array.from(new Set(blockers))

  return {
    authority,
    blockerRefs: uniqueBlockers,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusFrom(entries, uniqueBlockers),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeOnboardingCapabilityInput = (
  work: AutopilotWorkProjection,
): ForgeOnboardingCapabilityInput => {
  const evidence = work.onboardingCapabilityEvidence

  return {
    generatedAt: evidence?.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(evidence?.blockerRefs === undefined
      ? {}
      : { blockerRefs: evidence.blockerRefs }),
    ...(evidence?.entries === undefined ? {} : { entries: evidence.entries }),
    ...(evidence?.snapshotRef === undefined
      ? {}
      : { snapshotRef: evidence.snapshotRef }),
    ...(evidence?.versionRef === undefined
      ? {}
      : { versionRef: evidence.versionRef }),
  }
}
