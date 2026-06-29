import type {
  AutopilotWorkProjection,
  AutopilotWorkSecurityReviewDomain,
  AutopilotWorkSecurityReviewEntry,
  AutopilotWorkSecurityReviewFreshness,
  AutopilotWorkSecurityReviewRisk,
  AutopilotWorkSecurityReviewStatus,
} from '../model'

export type ForgeSecurityReviewEvidenceStatus =
  | 'approved'
  | 'blocked'
  | 'denied'
  | 'empty'
  | 'needs_review'
  | 'unknown'

export type ForgeSecurityReviewEvidenceAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  approvalGrantAuthority: false
  capabilityMutationAuthority: false
  credentialReadAuthority: false
  diagnosticBundleAuthority: false
  exceptionMutationAuthority: false
  productPromiseMutationAuthority: false
  publicProjectionMutationAuthority: false
  redactionScanAuthority: false
  releaseVerificationAuthority: false
  securityGateExecutionAuthority: false
  settlementAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeSecurityReviewEvidenceItem = Readonly<{
  approvalGateRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  denialReceiptRefs: ReadonlyArray<string>
  diagnosticBundleRefs: ReadonlyArray<string>
  domain: AutopilotWorkSecurityReviewDomain
  domainRef: string
  exceptionExpiryRefs: ReadonlyArray<string>
  exceptionRefs: ReadonlyArray<string>
  freshness: AutopilotWorkSecurityReviewFreshness
  ownerPolicyRefs: ReadonlyArray<string>
  providerCredentialPolicyRefs: ReadonlyArray<string>
  publicProjectionScanRefs: ReadonlyArray<string>
  redactionScanRefs: ReadonlyArray<string>
  regressionFixtureRefs: ReadonlyArray<string>
  releaseIntegrityRefs: ReadonlyArray<string>
  risk: AutopilotWorkSecurityReviewRisk
  status: AutopilotWorkSecurityReviewStatus
  threatModelRefs: ReadonlyArray<string>
}>

export type ForgeSecurityReviewEvidenceInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkSecurityReviewEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeSecurityReviewEvidenceCounts = Readonly<{
  approved: number
  denied: number
  domains: number
  exceptions: number
  highRisk: number
  needsReview: number
  stale: number
}>

export type ForgeSecurityReviewEvidenceView = Readonly<{
  authority: ForgeSecurityReviewEvidenceAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeSecurityReviewEvidenceCounts
  entries: ReadonlyArray<ForgeSecurityReviewEvidenceItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeSecurityReviewEvidenceStatus
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

const HIGH_RISK_DOMAINS: ReadonlySet<AutopilotWorkSecurityReviewDomain> =
  new Set([
    'browser_desktop_integration',
    'filesystem_workspace',
    'mcp_plugins_hooks_skills',
    'payment_wallet_settlement',
    'provider_credentials',
    'public_projection_claims',
    'release_artifacts',
    'remote_session_bridge',
    'shell_execution',
  ])

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_SECURITY_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|command|content|credential|data|detail|diagnostic|file|log|output|payload|provider|secret|shell|token|trace)/i,
  /private[-_ ](?:artifact|code|content|credential|data|diagnostic|file|log|payload|provider|repo|secret|source|workspace)/i,
  /artifact[-_ ]content/i,
  /credential[-_ ]value/i,
  /diagnostic[-_ ]content/i,
  /provider[-_ ]payload/i,
  /shell[-_ ]log/i,
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

const authority: ForgeSecurityReviewEvidenceAuthority = {
  acceptedOutcomeAuthority: false,
  approvalGrantAuthority: false,
  capabilityMutationAuthority: false,
  credentialReadAuthority: false,
  diagnosticBundleAuthority: false,
  exceptionMutationAuthority: false,
  productPromiseMutationAuthority: false,
  publicProjectionMutationAuthority: false,
  redactionScanAuthority: false,
  releaseVerificationAuthority: false,
  securityGateExecutionAuthority: false,
  settlementAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_SECURITY_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-security-review-evidence-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkSecurityReviewEntry,
): Readonly<{
  item: ForgeSecurityReviewEvidenceItem | null
  omittedUnsafeRefCount: number
}> => {
  const approvalGateRefs = safeRefs(item.approvalGateRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const denialReceiptRefs = safeRefs(item.denialReceiptRefs)
  const diagnosticBundleRefs = safeRefs(item.diagnosticBundleRefs)
  const domainRef = safeOptionalRef(item.domainRef)
  const exceptionExpiryRefs = safeRefs(item.exceptionExpiryRefs)
  const exceptionRefs = safeRefs(item.exceptionRefs)
  const ownerPolicyRefs = safeRefs(item.ownerPolicyRefs)
  const providerCredentialPolicyRefs = safeRefs(item.providerCredentialPolicyRefs)
  const publicProjectionScanRefs = safeRefs(item.publicProjectionScanRefs)
  const redactionScanRefs = safeRefs(item.redactionScanRefs)
  const regressionFixtureRefs = safeRefs(item.regressionFixtureRefs)
  const releaseIntegrityRefs = safeRefs(item.releaseIntegrityRefs)
  const threatModelRefs = safeRefs(item.threatModelRefs)
  const omittedUnsafeRefCount =
    approvalGateRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    denialReceiptRefs.omittedUnsafeRefCount +
    diagnosticBundleRefs.omittedUnsafeRefCount +
    domainRef.omittedUnsafeRefCount +
    exceptionExpiryRefs.omittedUnsafeRefCount +
    exceptionRefs.omittedUnsafeRefCount +
    ownerPolicyRefs.omittedUnsafeRefCount +
    providerCredentialPolicyRefs.omittedUnsafeRefCount +
    publicProjectionScanRefs.omittedUnsafeRefCount +
    redactionScanRefs.omittedUnsafeRefCount +
    regressionFixtureRefs.omittedUnsafeRefCount +
    releaseIntegrityRefs.omittedUnsafeRefCount +
    threatModelRefs.omittedUnsafeRefCount

  return domainRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          approvalGateRefs: approvalGateRefs.refs,
          blockerRefs: blockerRefs.refs,
          denialReceiptRefs: denialReceiptRefs.refs,
          diagnosticBundleRefs: diagnosticBundleRefs.refs,
          domain: item.domain,
          domainRef: domainRef.ref,
          exceptionExpiryRefs: exceptionExpiryRefs.refs,
          exceptionRefs: exceptionRefs.refs,
          freshness: item.freshness ?? 'unknown',
          ownerPolicyRefs: ownerPolicyRefs.refs,
          providerCredentialPolicyRefs: providerCredentialPolicyRefs.refs,
          publicProjectionScanRefs: publicProjectionScanRefs.refs,
          redactionScanRefs: redactionScanRefs.refs,
          regressionFixtureRefs: regressionFixtureRefs.refs,
          releaseIntegrityRefs: releaseIntegrityRefs.refs,
          risk: item.risk,
          status: item.status,
          threatModelRefs: threatModelRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeSecurityReviewEvidenceItem>,
): ForgeSecurityReviewEvidenceCounts => ({
  approved: entries.filter(entry => entry.status === 'approved').length,
  denied: entries.filter(entry => entry.status === 'denied').length,
  domains: entries.length,
  exceptions: entries.filter(entry => entry.exceptionRefs.length > 0).length,
  highRisk: entries.filter(
    entry =>
      entry.risk === 'high' ||
      entry.risk === 'critical' ||
      HIGH_RISK_DOMAINS.has(entry.domain),
  ).length,
  needsReview: entries.filter(entry => entry.status === 'needs_review').length,
  stale: entries.filter(
    entry => entry.freshness === 'stale' || entry.status === 'expired',
  ).length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeSecurityReviewEvidenceItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]
  const highRisk =
    item.risk === 'high' ||
    item.risk === 'critical' ||
    HIGH_RISK_DOMAINS.has(item.domain)

  if (item.freshness === 'stale' || item.status === 'expired') {
    blockers.push(
      blockerRef(workOrderRef, `stale-security-review-evidence:${item.domainRef}`),
    )
  }

  if (
    highRisk &&
    (item.threatModelRefs.length === 0 ||
      item.ownerPolicyRefs.length === 0 ||
      item.approvalGateRefs.length === 0 ||
      item.redactionScanRefs.length === 0 ||
      item.regressionFixtureRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `high-risk-review-missing-evidence:${item.domainRef}`),
    )
  }

  if (
    item.exceptionRefs.length > 0 &&
    (item.exceptionExpiryRefs.length === 0 || item.denialReceiptRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `security-exception-missing-expiry-receipt:${item.domainRef}`),
    )
  }

  if (
    item.domain === 'provider_credentials' &&
    item.providerCredentialPolicyRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `provider-credential-policy-missing:${item.domainRef}`),
    )
  }

  if (item.domain === 'release_artifacts' && item.releaseIntegrityRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `release-integrity-missing:${item.domainRef}`),
    )
  }

  if (
    item.domain === 'public_projection_claims' &&
    (item.publicProjectionScanRefs.length === 0 ||
      item.redactionScanRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `public-projection-scan-missing:${item.domainRef}`),
    )
  }

  if (item.status === 'denied' && item.denialReceiptRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `security-denial-receipt-missing:${item.domainRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeSecurityReviewEvidenceItem>,
  blockers: ReadonlyArray<string>,
): ForgeSecurityReviewEvidenceStatus => {
  if (blockers.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.status === 'denied')) {
    return 'denied'
  }

  if (entries.some(entry => entry.status === 'needs_review')) {
    return 'needs_review'
  }

  if (entries.every(entry => entry.status === 'approved')) {
    return 'approved'
  }

  return 'unknown'
}

export const projectForgeSecurityReviewEvidence = (
  input: ForgeSecurityReviewEvidenceInput,
): ForgeSecurityReviewEvidenceView => {
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
      blockerRef(input.workOrderRef, 'missing-security-review-evidence-snapshot-ref'),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-security-review-material-omitted'),
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

export const buildForgeSecurityReviewEvidenceInput = (
  work: AutopilotWorkProjection,
): ForgeSecurityReviewEvidenceInput => {
  const evidence = work.securityReviewEvidence

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
