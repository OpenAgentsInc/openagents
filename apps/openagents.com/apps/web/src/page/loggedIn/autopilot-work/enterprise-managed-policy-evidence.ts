import type {
  AutopilotWorkEnterpriseManagedPolicyDecision,
  AutopilotWorkEnterpriseManagedPolicyEntry,
  AutopilotWorkEnterpriseManagedPolicyFreshness,
  AutopilotWorkEnterpriseManagedPolicyStatus,
  AutopilotWorkProjection,
} from '../model'

export type ForgeEnterpriseManagedPolicyStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeEnterpriseManagedPolicyAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  budgetMutationAuthority: false
  capabilityGrantAuthority: false
  emergencyOverrideAuthority: false
  integrationGateMutationAuthority: false
  policyEnforcementAuthority: false
  policyExportAuthority: false
  policyInstallAuthority: false
  policyLoadAuthority: false
  policyMutationAuthority: false
  providerMutationAuthority: false
  publicProjectionMutationAuthority: false
  retentionMutationAuthority: false
  settlementAuthority: false
  telemetryMutationAuthority: false
  updateChannelMutationAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeEnterpriseManagedPolicyItem = Readonly<{
  allowRefs: ReadonlyArray<string>
  askRefs: ReadonlyArray<string>
  auditRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  budgetPolicyRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  changeRefs: ReadonlyArray<string>
  conflictPriorityRefs: ReadonlyArray<string>
  conflictRefs: ReadonlyArray<string>
  conflictResolutionRefs: ReadonlyArray<string>
  decision: AutopilotWorkEnterpriseManagedPolicyDecision
  denialRefs: ReadonlyArray<string>
  devicePolicyRefs: ReadonlyArray<string>
  effectiveAtRefs: ReadonlyArray<string>
  effectivePolicyRefs: ReadonlyArray<string>
  emergencyOverrideReceiptRefs: ReadonlyArray<string>
  enforcementModeRefs: ReadonlyArray<string>
  expirationRefs: ReadonlyArray<string>
  freshness: AutopilotWorkEnterpriseManagedPolicyFreshness
  hookPolicyRefs: ReadonlyArray<string>
  mcpPolicyRefs: ReadonlyArray<string>
  organizationPolicyRefs: ReadonlyArray<string>
  ownerAdminRefs: ReadonlyArray<string>
  pluginPolicyRefs: ReadonlyArray<string>
  policyRef: string
  projectPolicyRefs: ReadonlyArray<string>
  providerPolicyRefs: ReadonlyArray<string>
  publicSummaryRefs: ReadonlyArray<string>
  remoteBridgePolicyRefs: ReadonlyArray<string>
  repositoryPolicyRefs: ReadonlyArray<string>
  restrictRefs: ReadonlyArray<string>
  retentionPolicyRefs: ReadonlyArray<string>
  ruleKindRefs: ReadonlyArray<string>
  runtimeCapabilityBoundaryRefs: ReadonlyArray<string>
  scopeRefs: ReadonlyArray<string>
  sessionPolicyRefs: ReadonlyArray<string>
  status: AutopilotWorkEnterpriseManagedPolicyStatus
  teamPolicyRefs: ReadonlyArray<string>
  telemetryPolicyRefs: ReadonlyArray<string>
  updatePolicyRefs: ReadonlyArray<string>
  userPolicyRefs: ReadonlyArray<string>
  userSafeReasonRefs: ReadonlyArray<string>
  versionRefs: ReadonlyArray<string>
}>

export type ForgeEnterpriseManagedPolicyInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkEnterpriseManagedPolicyEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeEnterpriseManagedPolicyCounts = Readonly<{
  askRestrict: number
  denied: number
  emergencyOverrides: number
  ready: number
  stale: number
  total: number
}>

export type ForgeEnterpriseManagedPolicyView = Readonly<{
  authority: ForgeEnterpriseManagedPolicyAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeEnterpriseManagedPolicyCounts
  entries: ReadonlyArray<ForgeEnterpriseManagedPolicyItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeEnterpriseManagedPolicyStatus
  versionRef: string | null
  workOrderRef: string
}>

type RefBundle = Readonly<{ omittedUnsafeRefCount: number; refs: ReadonlyArray<string> }>
type OptionalRefBundle = Readonly<{ omittedUnsafeRefCount: number; ref: string | null }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_POLICY_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:org|payload|policy|prompt|provider|rule|secret)/i,
  /private[-_ ](?:customer|data|org|policy|repo|repository|rule|workspace)/i,
  /credential[-_ ](?:payload|secret)/i,
  /customer[-_ ](?:data|payload|private)/i,
  /grant[-_ ]runtime[-_ ]authority/i,
  /policy[-_ ]internals/i,
  /provider[-_ ]payload/i,
  /prompt[-_ ]text/i,
  /silent[-_ ](?:authority[-_ ])?broadening/i,
  /wallet[-_ ](?:material|mnemonic|payload)/i,
  /payment[-_ ](?:material|payload)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret)\b/i,
  /\b(?:admin|auth|bearer|operator|private|refresh|secret|service|session)[_-]?token\b/i,
]

const authority: ForgeEnterpriseManagedPolicyAuthority = {
  acceptedOutcomeAuthority: false,
  budgetMutationAuthority: false,
  capabilityGrantAuthority: false,
  emergencyOverrideAuthority: false,
  integrationGateMutationAuthority: false,
  policyEnforcementAuthority: false,
  policyExportAuthority: false,
  policyInstallAuthority: false,
  policyLoadAuthority: false,
  policyMutationAuthority: false,
  providerMutationAuthority: false,
  publicProjectionMutationAuthority: false,
  retentionMutationAuthority: false,
  settlementAuthority: false,
  telemetryMutationAuthority: false,
  updateChannelMutationAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_POLICY_MARKERS.some(marker => marker.test(trimmed))
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

const safeOptionalRef = (value: string | null | undefined): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-enterprise-managed-policy-blocker:${workOrderRef}:${suffix}`

const hasRole = (refs: ReadonlyArray<string>, pattern: RegExp): boolean =>
  refs.some(ref => pattern.test(ref))

const normalizeItem = (
  item: AutopilotWorkEnterpriseManagedPolicyEntry,
): Readonly<{
  item: ForgeEnterpriseManagedPolicyItem | null
  omittedUnsafeRefCount: number
}> => {
  const allowRefs = safeRefs(item.allowRefs)
  const askRefs = safeRefs(item.askRefs)
  const auditRefs = safeRefs(item.auditRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const budgetPolicyRefs = safeRefs(item.budgetPolicyRefs)
  const caveatRefs = safeRefs(item.caveatRefs)
  const changeRefs = safeRefs(item.changeRefs)
  const conflictPriorityRefs = safeRefs(item.conflictPriorityRefs)
  const conflictRefs = safeRefs(item.conflictRefs)
  const conflictResolutionRefs = safeRefs(item.conflictResolutionRefs)
  const denialRefs = safeRefs(item.denialRefs)
  const devicePolicyRefs = safeRefs(item.devicePolicyRefs)
  const effectiveAtRefs = safeRefs(item.effectiveAtRefs)
  const effectivePolicyRefs = safeRefs(item.effectivePolicyRefs)
  const emergencyOverrideReceiptRefs = safeRefs(item.emergencyOverrideReceiptRefs)
  const enforcementModeRefs = safeRefs(item.enforcementModeRefs)
  const expirationRefs = safeRefs(item.expirationRefs)
  const hookPolicyRefs = safeRefs(item.hookPolicyRefs)
  const mcpPolicyRefs = safeRefs(item.mcpPolicyRefs)
  const organizationPolicyRefs = safeRefs(item.organizationPolicyRefs)
  const ownerAdminRefs = safeRefs(item.ownerAdminRefs)
  const pluginPolicyRefs = safeRefs(item.pluginPolicyRefs)
  const policyRef = safeOptionalRef(item.policyRef)
  const projectPolicyRefs = safeRefs(item.projectPolicyRefs)
  const providerPolicyRefs = safeRefs(item.providerPolicyRefs)
  const publicSummaryRefs = safeRefs(item.publicSummaryRefs)
  const remoteBridgePolicyRefs = safeRefs(item.remoteBridgePolicyRefs)
  const repositoryPolicyRefs = safeRefs(item.repositoryPolicyRefs)
  const restrictRefs = safeRefs(item.restrictRefs)
  const retentionPolicyRefs = safeRefs(item.retentionPolicyRefs)
  const ruleKindRefs = safeRefs(item.ruleKindRefs)
  const runtimeCapabilityBoundaryRefs = safeRefs(item.runtimeCapabilityBoundaryRefs)
  const scopeRefs = safeRefs(item.scopeRefs)
  const sessionPolicyRefs = safeRefs(item.sessionPolicyRefs)
  const teamPolicyRefs = safeRefs(item.teamPolicyRefs)
  const telemetryPolicyRefs = safeRefs(item.telemetryPolicyRefs)
  const updatePolicyRefs = safeRefs(item.updatePolicyRefs)
  const userPolicyRefs = safeRefs(item.userPolicyRefs)
  const userSafeReasonRefs = safeRefs(item.userSafeReasonRefs)
  const versionRefs = safeRefs(item.versionRefs)
  const omittedUnsafeRefCount =
    allowRefs.omittedUnsafeRefCount +
    askRefs.omittedUnsafeRefCount +
    auditRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    budgetPolicyRefs.omittedUnsafeRefCount +
    caveatRefs.omittedUnsafeRefCount +
    changeRefs.omittedUnsafeRefCount +
    conflictPriorityRefs.omittedUnsafeRefCount +
    conflictRefs.omittedUnsafeRefCount +
    conflictResolutionRefs.omittedUnsafeRefCount +
    denialRefs.omittedUnsafeRefCount +
    devicePolicyRefs.omittedUnsafeRefCount +
    effectiveAtRefs.omittedUnsafeRefCount +
    effectivePolicyRefs.omittedUnsafeRefCount +
    emergencyOverrideReceiptRefs.omittedUnsafeRefCount +
    enforcementModeRefs.omittedUnsafeRefCount +
    expirationRefs.omittedUnsafeRefCount +
    hookPolicyRefs.omittedUnsafeRefCount +
    mcpPolicyRefs.omittedUnsafeRefCount +
    organizationPolicyRefs.omittedUnsafeRefCount +
    ownerAdminRefs.omittedUnsafeRefCount +
    pluginPolicyRefs.omittedUnsafeRefCount +
    policyRef.omittedUnsafeRefCount +
    projectPolicyRefs.omittedUnsafeRefCount +
    providerPolicyRefs.omittedUnsafeRefCount +
    publicSummaryRefs.omittedUnsafeRefCount +
    remoteBridgePolicyRefs.omittedUnsafeRefCount +
    repositoryPolicyRefs.omittedUnsafeRefCount +
    restrictRefs.omittedUnsafeRefCount +
    retentionPolicyRefs.omittedUnsafeRefCount +
    ruleKindRefs.omittedUnsafeRefCount +
    runtimeCapabilityBoundaryRefs.omittedUnsafeRefCount +
    scopeRefs.omittedUnsafeRefCount +
    sessionPolicyRefs.omittedUnsafeRefCount +
    teamPolicyRefs.omittedUnsafeRefCount +
    telemetryPolicyRefs.omittedUnsafeRefCount +
    updatePolicyRefs.omittedUnsafeRefCount +
    userPolicyRefs.omittedUnsafeRefCount +
    userSafeReasonRefs.omittedUnsafeRefCount +
    versionRefs.omittedUnsafeRefCount

  return policyRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          allowRefs: allowRefs.refs,
          askRefs: askRefs.refs,
          auditRefs: auditRefs.refs,
          blockerRefs: blockerRefs.refs,
          budgetPolicyRefs: budgetPolicyRefs.refs,
          caveatRefs: caveatRefs.refs,
          changeRefs: changeRefs.refs,
          conflictPriorityRefs: conflictPriorityRefs.refs,
          conflictRefs: conflictRefs.refs,
          conflictResolutionRefs: conflictResolutionRefs.refs,
          decision: item.decision,
          denialRefs: denialRefs.refs,
          devicePolicyRefs: devicePolicyRefs.refs,
          effectiveAtRefs: effectiveAtRefs.refs,
          effectivePolicyRefs: effectivePolicyRefs.refs,
          emergencyOverrideReceiptRefs: emergencyOverrideReceiptRefs.refs,
          enforcementModeRefs: enforcementModeRefs.refs,
          expirationRefs: expirationRefs.refs,
          freshness: item.freshness ?? 'unknown',
          hookPolicyRefs: hookPolicyRefs.refs,
          mcpPolicyRefs: mcpPolicyRefs.refs,
          organizationPolicyRefs: organizationPolicyRefs.refs,
          ownerAdminRefs: ownerAdminRefs.refs,
          pluginPolicyRefs: pluginPolicyRefs.refs,
          policyRef: policyRef.ref,
          projectPolicyRefs: projectPolicyRefs.refs,
          providerPolicyRefs: providerPolicyRefs.refs,
          publicSummaryRefs: publicSummaryRefs.refs,
          remoteBridgePolicyRefs: remoteBridgePolicyRefs.refs,
          repositoryPolicyRefs: repositoryPolicyRefs.refs,
          restrictRefs: restrictRefs.refs,
          retentionPolicyRefs: retentionPolicyRefs.refs,
          ruleKindRefs: ruleKindRefs.refs,
          runtimeCapabilityBoundaryRefs: runtimeCapabilityBoundaryRefs.refs,
          scopeRefs: scopeRefs.refs,
          sessionPolicyRefs: sessionPolicyRefs.refs,
          status: item.status,
          teamPolicyRefs: teamPolicyRefs.refs,
          telemetryPolicyRefs: telemetryPolicyRefs.refs,
          updatePolicyRefs: updatePolicyRefs.refs,
          userPolicyRefs: userPolicyRefs.refs,
          userSafeReasonRefs: userSafeReasonRefs.refs,
          versionRefs: versionRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeEnterpriseManagedPolicyItem>,
): ForgeEnterpriseManagedPolicyCounts => ({
  askRestrict: entries.filter(
    entry => entry.decision === 'ask' || entry.decision === 'restrict',
  ).length,
  denied: entries.filter(entry => entry.decision === 'deny').length,
  emergencyOverrides: entries.filter(
    entry =>
      entry.emergencyOverrideReceiptRefs.length > 0 ||
      hasRole(entry.ruleKindRefs, /emergency|override/i),
  ).length,
  ready: entries.filter(entry => entry.status === 'ready').length,
  stale: entries.filter(entry => entry.freshness === 'stale' || entry.status === 'stale').length,
  total: entries.length,
})

const readyContractMissing = (item: ForgeEnterpriseManagedPolicyItem): boolean =>
  item.status === 'ready' &&
  (item.effectivePolicyRefs.length === 0 ||
    item.scopeRefs.length === 0 ||
    item.ownerAdminRefs.length === 0 ||
    item.versionRefs.length === 0 ||
    item.ruleKindRefs.length === 0 ||
    item.enforcementModeRefs.length === 0 ||
    item.auditRefs.length === 0 ||
    item.changeRefs.length === 0 ||
    item.publicSummaryRefs.length === 0 ||
    item.runtimeCapabilityBoundaryRefs.length === 0)

const emergencyOverrideDeclared = (item: ForgeEnterpriseManagedPolicyItem): boolean =>
  item.emergencyOverrideReceiptRefs.length > 0 ||
  hasRole(item.ruleKindRefs, /emergency|override/i)

const itemBlockers = (
  workOrderRef: string,
  item: ForgeEnterpriseManagedPolicyItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]

  if (item.freshness === 'stale' || item.status === 'stale') {
    blockers.push(
      blockerRef(workOrderRef, `stale-enterprise-managed-policy-evidence:${item.policyRef}`),
    )
  }

  if (readyContractMissing(item)) {
    blockers.push(
      blockerRef(workOrderRef, `ready-managed-policy-evidence-missing:${item.policyRef}`),
    )
  }

  if (
    item.decision === 'deny' &&
    (item.denialRefs.length === 0 || item.userSafeReasonRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `managed-policy-denial-reason-missing:${item.policyRef}`),
    )
  }

  if (item.decision === 'ask' && item.askRefs.length === 0) {
    blockers.push(blockerRef(workOrderRef, `managed-policy-ask-ref-missing:${item.policyRef}`))
  }

  if (item.decision === 'restrict' && item.restrictRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `managed-policy-restrict-ref-missing:${item.policyRef}`),
    )
  }

  if (
    item.conflictRefs.length > 0 &&
    (item.conflictResolutionRefs.length === 0 ||
      item.conflictPriorityRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `managed-policy-conflict-resolution-missing:${item.policyRef}`),
    )
  }

  if (
    emergencyOverrideDeclared(item) &&
    (item.emergencyOverrideReceiptRefs.length === 0 || item.expirationRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `emergency-override-expiration-receipt-missing:${item.policyRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeEnterpriseManagedPolicyItem>,
  blockers: ReadonlyArray<string>,
): ForgeEnterpriseManagedPolicyStatus => {
  if (blockers.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }
  if (entries.length === 0) {
    return 'empty'
  }
  if (entries.some(entry => entry.freshness === 'stale' || entry.status === 'stale')) {
    return 'stale'
  }
  if (entries.every(entry => entry.status === 'ready')) {
    return 'ready'
  }
  return 'unknown'
}

export const projectForgeEnterpriseManagedPolicyEvidence = (
  input: ForgeEnterpriseManagedPolicyInput,
): ForgeEnterpriseManagedPolicyView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalized = (input.entries ?? []).map(normalizeItem)
  const entries = normalized.flatMap(result => (result.item === null ? [] : [result.item]))
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
    blockers.push(blockerRef(input.workOrderRef, 'missing-enterprise-managed-policy-snapshot-ref'))
  }
  if (omittedUnsafeRefCount > 0) {
    blockers.push(blockerRef(input.workOrderRef, 'unsafe-managed-policy-material-omitted'))
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

export const buildForgeEnterpriseManagedPolicyInput = (
  work: AutopilotWorkProjection,
): ForgeEnterpriseManagedPolicyInput => {
  const evidence = work.enterpriseManagedPolicyEvidence

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
