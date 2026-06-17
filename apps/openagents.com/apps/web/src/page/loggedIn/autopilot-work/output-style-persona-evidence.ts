import type {
  AutopilotWorkOutputStyleDomainMode,
  AutopilotWorkOutputStyleEntry,
  AutopilotWorkOutputStyleFreshness,
  AutopilotWorkOutputStyleStatus,
  AutopilotWorkOutputStyleVerbosity,
  AutopilotWorkProjection,
} from '../model'

export type ForgeOutputStylePersonaStatus =
  | 'blocked'
  | 'empty'
  | 'planned'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeOutputStylePersonaAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  formatterExecutionAuthority: false
  hiddenChainAccessAuthority: false
  instructionMutationAuthority: false
  managedPolicyMutationAuthority: false
  outputRewriteAuthority: false
  personaInstallAuthority: false
  privateDataReadAuthority: false
  productClaimMutationAuthority: false
  promptMutationAuthority: false
  safetyPrivacyApprovalBypassAuthority: false
  settlementAuthority: false
  stylePreferenceWriteAuthority: false
  toolAuthorityChangeAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeOutputStylePersonaItem = Readonly<{
  accessibilityRefs: ReadonlyArray<string>
  audienceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  citationRequirementRefs: ReadonlyArray<string>
  claimReceiptRefs: ReadonlyArray<string>
  conflictResolutionRefs: ReadonlyArray<string>
  disallowedClaimRefs: ReadonlyArray<string>
  domainMode: AutopilotWorkOutputStyleDomainMode
  evidenceRequirementRefs: ReadonlyArray<string>
  finalAnswerExpectationRefs: ReadonlyArray<string>
  formattingRefs: ReadonlyArray<string>
  freshness: AutopilotWorkOutputStyleFreshness
  managedPolicyRefs: ReadonlyArray<string>
  overrideRefs: ReadonlyArray<string>
  personaConstraintRefs: ReadonlyArray<string>
  productDefaultRefs: ReadonlyArray<string>
  projectConstraintRefs: ReadonlyArray<string>
  safetyPolicyRefs: ReadonlyArray<string>
  status: AutopilotWorkOutputStyleStatus
  styleAuditRefs: ReadonlyArray<string>
  stylePolicyRef: string
  toolAuthorityBoundaryRefs: ReadonlyArray<string>
  userPreferenceRefs: ReadonlyArray<string>
  verbosity: AutopilotWorkOutputStyleVerbosity
}>

export type ForgeOutputStylePersonaInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkOutputStyleEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeOutputStylePersonaCounts = Readonly<{
  accessibilityPolicies: number
  conflicts: number
  overrides: number
  policies: number
  ready: number
  stale: number
}>

export type ForgeOutputStylePersonaView = Readonly<{
  authority: ForgeOutputStylePersonaAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeOutputStylePersonaCounts
  entries: ReadonlyArray<ForgeOutputStylePersonaItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeOutputStylePersonaStatus
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
const PRIVATE_OUTPUT_STYLE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:claim|content|data|instruction|output|payload|preference|prompt|secret|style|token|voice)/i,
  /private[-_ ](?:chain|content|data|instruction|preference|project|prompt|repo|source|style|workspace)/i,
  /capability[-_ ]claim/i,
  /hidden[-_ ](?:chain|reasoning|state|thought)/i,
  /instruction[-_ ]body/i,
  /output[-_ ]body/i,
  /persona[-_ ]text/i,
  /preference[-_ ]payload/i,
  /project[-_ ]instruction/i,
  /secret[-_ ]bearing[-_ ]override/i,
  /style[-_ ]override[-_ ]payload/i,
  /unsupported[-_ ]claim/i,
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

const SHAPED_MODES: ReadonlySet<AutopilotWorkOutputStyleDomainMode> = new Set([
  'implementation',
  'planning',
  'review',
  'status',
])

const authority: ForgeOutputStylePersonaAuthority = {
  acceptedOutcomeAuthority: false,
  formatterExecutionAuthority: false,
  hiddenChainAccessAuthority: false,
  instructionMutationAuthority: false,
  managedPolicyMutationAuthority: false,
  outputRewriteAuthority: false,
  personaInstallAuthority: false,
  privateDataReadAuthority: false,
  productClaimMutationAuthority: false,
  promptMutationAuthority: false,
  safetyPrivacyApprovalBypassAuthority: false,
  settlementAuthority: false,
  stylePreferenceWriteAuthority: false,
  toolAuthorityChangeAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_OUTPUT_STYLE_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-output-style-persona-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkOutputStyleEntry,
): Readonly<{
  item: ForgeOutputStylePersonaItem | null
  omittedUnsafeRefCount: number
}> => {
  const accessibilityRefs = safeRefs(item.accessibilityRefs)
  const audienceRefs = safeRefs(item.audienceRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const citationRequirementRefs = safeRefs(item.citationRequirementRefs)
  const claimReceiptRefs = safeRefs(item.claimReceiptRefs)
  const conflictResolutionRefs = safeRefs(item.conflictResolutionRefs)
  const disallowedClaimRefs = safeRefs(item.disallowedClaimRefs)
  const evidenceRequirementRefs = safeRefs(item.evidenceRequirementRefs)
  const finalAnswerExpectationRefs = safeRefs(item.finalAnswerExpectationRefs)
  const formattingRefs = safeRefs(item.formattingRefs)
  const managedPolicyRefs = safeRefs(item.managedPolicyRefs)
  const overrideRefs = safeRefs(item.overrideRefs)
  const personaConstraintRefs = safeRefs(item.personaConstraintRefs)
  const productDefaultRefs = safeRefs(item.productDefaultRefs)
  const projectConstraintRefs = safeRefs(item.projectConstraintRefs)
  const safetyPolicyRefs = safeRefs(item.safetyPolicyRefs)
  const styleAuditRefs = safeRefs(item.styleAuditRefs)
  const stylePolicyRef = safeOptionalRef(item.stylePolicyRef)
  const toolAuthorityBoundaryRefs = safeRefs(item.toolAuthorityBoundaryRefs)
  const userPreferenceRefs = safeRefs(item.userPreferenceRefs)
  const omittedUnsafeRefCount =
    accessibilityRefs.omittedUnsafeRefCount +
    audienceRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    citationRequirementRefs.omittedUnsafeRefCount +
    claimReceiptRefs.omittedUnsafeRefCount +
    conflictResolutionRefs.omittedUnsafeRefCount +
    disallowedClaimRefs.omittedUnsafeRefCount +
    evidenceRequirementRefs.omittedUnsafeRefCount +
    finalAnswerExpectationRefs.omittedUnsafeRefCount +
    formattingRefs.omittedUnsafeRefCount +
    managedPolicyRefs.omittedUnsafeRefCount +
    overrideRefs.omittedUnsafeRefCount +
    personaConstraintRefs.omittedUnsafeRefCount +
    productDefaultRefs.omittedUnsafeRefCount +
    projectConstraintRefs.omittedUnsafeRefCount +
    safetyPolicyRefs.omittedUnsafeRefCount +
    styleAuditRefs.omittedUnsafeRefCount +
    stylePolicyRef.omittedUnsafeRefCount +
    toolAuthorityBoundaryRefs.omittedUnsafeRefCount +
    userPreferenceRefs.omittedUnsafeRefCount

  return stylePolicyRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          accessibilityRefs: accessibilityRefs.refs,
          audienceRefs: audienceRefs.refs,
          blockerRefs: blockerRefs.refs,
          citationRequirementRefs: citationRequirementRefs.refs,
          claimReceiptRefs: claimReceiptRefs.refs,
          conflictResolutionRefs: conflictResolutionRefs.refs,
          disallowedClaimRefs: disallowedClaimRefs.refs,
          domainMode: item.domainMode,
          evidenceRequirementRefs: evidenceRequirementRefs.refs,
          finalAnswerExpectationRefs: finalAnswerExpectationRefs.refs,
          formattingRefs: formattingRefs.refs,
          freshness: item.freshness ?? 'unknown',
          managedPolicyRefs: managedPolicyRefs.refs,
          overrideRefs: overrideRefs.refs,
          personaConstraintRefs: personaConstraintRefs.refs,
          productDefaultRefs: productDefaultRefs.refs,
          projectConstraintRefs: projectConstraintRefs.refs,
          safetyPolicyRefs: safetyPolicyRefs.refs,
          status: item.status,
          styleAuditRefs: styleAuditRefs.refs,
          stylePolicyRef: stylePolicyRef.ref,
          toolAuthorityBoundaryRefs: toolAuthorityBoundaryRefs.refs,
          userPreferenceRefs: userPreferenceRefs.refs,
          verbosity: item.verbosity,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeOutputStylePersonaItem>,
): ForgeOutputStylePersonaCounts => ({
  accessibilityPolicies: entries.filter(entry => entry.accessibilityRefs.length > 0)
    .length,
  conflicts: entries.filter(entry => entry.status === 'conflicted').length,
  overrides: entries.filter(entry => entry.overrideRefs.length > 0).length,
  policies: entries.length,
  ready: entries.filter(entry => entry.status === 'ready').length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeOutputStylePersonaItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]
  const hasManagedOrProject =
    item.managedPolicyRefs.length > 0 || item.projectConstraintRefs.length > 0

  if (item.freshness === 'stale') {
    blockers.push(
      blockerRef(workOrderRef, `stale-output-style-evidence:${item.stylePolicyRef}`),
    )
  }

  if (
    item.status === 'ready' &&
    (item.userPreferenceRefs.length === 0 ||
      item.productDefaultRefs.length === 0 ||
      !hasManagedOrProject)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `style-policy-resolution-evidence-missing:${item.stylePolicyRef}`),
    )
  }

  if (
    item.status === 'ready' &&
    SHAPED_MODES.has(item.domainMode) &&
    (item.finalAnswerExpectationRefs.length === 0 ||
      item.formattingRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `mode-output-shape-evidence-missing:${item.stylePolicyRef}`),
    )
  }

  if (
    item.accessibilityRefs.length > 0 &&
    item.personaConstraintRefs.length > 0 &&
    item.conflictResolutionRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `accessibility-persona-precedence-missing:${item.stylePolicyRef}`),
    )
  }

  if (
    item.status === 'conflicted' &&
    item.overrideRefs.length > 0 &&
    item.conflictResolutionRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `style-conflict-resolution-missing:${item.stylePolicyRef}`),
    )
  }

  if (
    item.disallowedClaimRefs.length > 0 &&
    item.evidenceRequirementRefs.length === 0 &&
    item.claimReceiptRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `disallowed-claim-evidence-missing:${item.stylePolicyRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeOutputStylePersonaItem>,
  blockers: ReadonlyArray<string>,
): ForgeOutputStylePersonaStatus => {
  if (blockers.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.status === 'planned')) {
    return 'planned'
  }

  if (entries.every(entry => entry.status === 'ready')) {
    return 'ready'
  }

  return 'unknown'
}

export const projectForgeOutputStylePersonaEvidence = (
  input: ForgeOutputStylePersonaInput,
): ForgeOutputStylePersonaView => {
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
      blockerRef(input.workOrderRef, 'missing-output-style-persona-snapshot-ref'),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-output-style-persona-material-omitted'),
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

export const buildForgeOutputStylePersonaInput = (
  work: AutopilotWorkProjection,
): ForgeOutputStylePersonaInput => {
  const evidence = work.outputStylePersonaEvidence

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
