import type {
  AutopilotWorkProjection,
  AutopilotWorkTestingSmokeClassification,
  AutopilotWorkTestingSmokeEntry,
  AutopilotWorkTestingSmokeFreshness,
  AutopilotWorkTestingSmokeLayer,
  AutopilotWorkTestingSmokeStatus,
} from '../model'

export type ForgeTestingSmokeEvidenceStatus =
  | 'blocked'
  | 'empty'
  | 'failed'
  | 'pending'
  | 'ready'
  | 'unknown'

export type ForgeTestingSmokeEvidenceAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  artifactReadAuthority: false
  credentialReadAuthority: false
  deploymentAuthority: false
  fixtureReadAuthority: false
  liveSpendAuthority: false
  productPromiseMutationAuthority: false
  providerCallAuthority: false
  publicClaimMutationAuthority: false
  pushAuthority: false
  rawSmokeOutputReadAuthority: false
  settlementAuthority: false
  smokeExecutionAuthority: false
  testExecutionAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeTestingSmokeEvidenceItem = Readonly<{
  adapterAvailabilityRefs: ReadonlyArray<string>
  approvalRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  classifications: ReadonlyArray<AutopilotWorkTestingSmokeClassification>
  commandRefs: ReadonlyArray<string>
  credentialAvailabilityRefs: ReadonlyArray<string>
  environmentRefs: ReadonlyArray<string>
  failureRefs: ReadonlyArray<string>
  fixtureRefs: ReadonlyArray<string>
  freshness: AutopilotWorkTestingSmokeFreshness
  layer: AutopilotWorkTestingSmokeLayer
  policyRefs: ReadonlyArray<string>
  productClaimRefs: ReadonlyArray<string>
  proofBoundaryRefs: ReadonlyArray<string>
  providerAvailabilityRefs: ReadonlyArray<string>
  redactionScanRefs: ReadonlyArray<string>
  smokeReceiptRefs: ReadonlyArray<string>
  status: AutopilotWorkTestingSmokeStatus
  testRef: string
  versionRefs: ReadonlyArray<string>
  workspaceAvailabilityRefs: ReadonlyArray<string>
}>

export type ForgeTestingSmokeEvidenceInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkTestingSmokeEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeTestingSmokeEvidenceCounts = Readonly<{
  blocked: number
  entries: number
  failed: number
  live: number
  paid: number
  passed: number
  stale: number
}>

export type ForgeTestingSmokeEvidenceView = Readonly<{
  authority: ForgeTestingSmokeEvidenceAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeTestingSmokeEvidenceCounts
  entries: ReadonlyArray<ForgeTestingSmokeEvidenceItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeTestingSmokeEvidenceStatus
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
const PRIVATE_TESTING_SMOKE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|command|content|credential|data|detail|fixture|log|output|payload|provider|smoke|test|workspace)/i,
  /private[-_ ](?:artifact|code|content|credential|data|fixture|log|payload|provider|repo|smoke|source|workspace)/i,
  /artifact[-_ ]content/i,
  /fixture[-_ ]body/i,
  /provider[-_ ]payload/i,
  /smoke[-_ ]output/i,
  /test[-_ ]log/i,
  /workspace[-_ ]path/i,
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

const PRIVILEGED_CLASSIFICATIONS: ReadonlySet<AutopilotWorkTestingSmokeClassification> =
  new Set(['deploy', 'live', 'paid', 'settlement', 'write'])

const authority: ForgeTestingSmokeEvidenceAuthority = {
  acceptedOutcomeAuthority: false,
  artifactReadAuthority: false,
  credentialReadAuthority: false,
  deploymentAuthority: false,
  fixtureReadAuthority: false,
  liveSpendAuthority: false,
  productPromiseMutationAuthority: false,
  providerCallAuthority: false,
  publicClaimMutationAuthority: false,
  pushAuthority: false,
  rawSmokeOutputReadAuthority: false,
  settlementAuthority: false,
  smokeExecutionAuthority: false,
  testExecutionAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_TESTING_SMOKE_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-testing-smoke-evidence-blocker:${workOrderRef}:${suffix}`

const normalizeClassifications = (
  classifications: ReadonlyArray<AutopilotWorkTestingSmokeClassification> | undefined,
): ReadonlyArray<AutopilotWorkTestingSmokeClassification> =>
  Array.from(new Set(classifications ?? ['unknown']))

const normalizeItem = (
  item: AutopilotWorkTestingSmokeEntry,
): Readonly<{
  item: ForgeTestingSmokeEvidenceItem | null
  omittedUnsafeRefCount: number
}> => {
  const adapterAvailabilityRefs = safeRefs(item.adapterAvailabilityRefs)
  const approvalRefs = safeRefs(item.approvalRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const commandRefs = safeRefs(item.commandRefs)
  const credentialAvailabilityRefs = safeRefs(item.credentialAvailabilityRefs)
  const environmentRefs = safeRefs(item.environmentRefs)
  const failureRefs = safeRefs(item.failureRefs)
  const fixtureRefs = safeRefs(item.fixtureRefs)
  const policyRefs = safeRefs(item.policyRefs)
  const productClaimRefs = safeRefs(item.productClaimRefs)
  const proofBoundaryRefs = safeRefs(item.proofBoundaryRefs)
  const providerAvailabilityRefs = safeRefs(item.providerAvailabilityRefs)
  const redactionScanRefs = safeRefs(item.redactionScanRefs)
  const smokeReceiptRefs = safeRefs(item.smokeReceiptRefs)
  const testRef = safeOptionalRef(item.testRef)
  const versionRefs = safeRefs(item.versionRefs)
  const workspaceAvailabilityRefs = safeRefs(item.workspaceAvailabilityRefs)
  const omittedUnsafeRefCount =
    adapterAvailabilityRefs.omittedUnsafeRefCount +
    approvalRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    commandRefs.omittedUnsafeRefCount +
    credentialAvailabilityRefs.omittedUnsafeRefCount +
    environmentRefs.omittedUnsafeRefCount +
    failureRefs.omittedUnsafeRefCount +
    fixtureRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    productClaimRefs.omittedUnsafeRefCount +
    proofBoundaryRefs.omittedUnsafeRefCount +
    providerAvailabilityRefs.omittedUnsafeRefCount +
    redactionScanRefs.omittedUnsafeRefCount +
    smokeReceiptRefs.omittedUnsafeRefCount +
    testRef.omittedUnsafeRefCount +
    versionRefs.omittedUnsafeRefCount +
    workspaceAvailabilityRefs.omittedUnsafeRefCount

  return testRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          adapterAvailabilityRefs: adapterAvailabilityRefs.refs,
          approvalRefs: approvalRefs.refs,
          blockerRefs: blockerRefs.refs,
          classifications: normalizeClassifications(item.classifications),
          commandRefs: commandRefs.refs,
          credentialAvailabilityRefs: credentialAvailabilityRefs.refs,
          environmentRefs: environmentRefs.refs,
          failureRefs: failureRefs.refs,
          fixtureRefs: fixtureRefs.refs,
          freshness: item.freshness ?? 'unknown',
          layer: item.layer,
          policyRefs: policyRefs.refs,
          productClaimRefs: productClaimRefs.refs,
          proofBoundaryRefs: proofBoundaryRefs.refs,
          providerAvailabilityRefs: providerAvailabilityRefs.refs,
          redactionScanRefs: redactionScanRefs.refs,
          smokeReceiptRefs: smokeReceiptRefs.refs,
          status: item.status,
          testRef: testRef.ref,
          versionRefs: versionRefs.refs,
          workspaceAvailabilityRefs: workspaceAvailabilityRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeTestingSmokeEvidenceItem>,
): ForgeTestingSmokeEvidenceCounts => ({
  blocked: entries.filter(entry => entry.status === 'blocked').length,
  entries: entries.length,
  failed: entries.filter(entry => entry.status === 'failed').length,
  live: entries.filter(entry => entry.classifications.includes('live')).length,
  paid: entries.filter(entry => entry.classifications.includes('paid')).length,
  passed: entries.filter(entry => entry.status === 'passed').length,
  stale: entries.filter(
    entry => entry.freshness === 'stale' || entry.status === 'stale',
  ).length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeTestingSmokeEvidenceItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]
  const hasPrivilegedClassification = item.classifications.some(classification =>
    PRIVILEGED_CLASSIFICATIONS.has(classification),
  )
  const hasClaimBoundary =
    item.productClaimRefs.length > 0 || item.proofBoundaryRefs.length > 0

  if (item.freshness === 'stale' || item.status === 'stale') {
    blockers.push(
      blockerRef(workOrderRef, `stale-testing-smoke-evidence:${item.testRef}`),
    )
  }

  if (
    hasPrivilegedClassification &&
    (item.approvalRefs.length === 0 || item.policyRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(
        workOrderRef,
        `privileged-smoke-missing-approval-policy:${item.testRef}`,
      ),
    )
  }

  if (hasClaimBoundary && item.smokeReceiptRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `proof-boundary-missing-smoke-receipt:${item.testRef}`),
    )
  }

  if (hasClaimBoundary && item.redactionScanRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `proof-boundary-missing-redaction-scan:${item.testRef}`),
    )
  }

  if (
    item.classifications.includes('ci_safe') &&
    (item.credentialAvailabilityRefs.length > 0 ||
      item.providerAvailabilityRefs.length > 0 ||
      item.workspaceAvailabilityRefs.length > 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `ci-safe-private-dependency:${item.testRef}`),
    )
  }

  if (
    item.status === 'failed' &&
    item.blockerRefs.length === 0 &&
    item.failureRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `failed-smoke-missing-blocker:${item.testRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeTestingSmokeEvidenceItem>,
  blockers: ReadonlyArray<string>,
): ForgeTestingSmokeEvidenceStatus => {
  if (blockers.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.status === 'failed')) {
    return 'failed'
  }

  if (entries.some(entry => entry.status === 'pending')) {
    return 'pending'
  }

  if (
    entries.every(entry => entry.status === 'passed' || entry.status === 'skipped')
  ) {
    return 'ready'
  }

  return 'unknown'
}

export const projectForgeTestingSmokeEvidence = (
  input: ForgeTestingSmokeEvidenceInput,
): ForgeTestingSmokeEvidenceView => {
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
      blockerRef(input.workOrderRef, 'missing-testing-smoke-evidence-snapshot-ref'),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-testing-smoke-material-omitted'),
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

export const buildForgeTestingSmokeEvidenceInput = (
  work: AutopilotWorkProjection,
): ForgeTestingSmokeEvidenceInput => {
  const testingSmokeEvidence = work.testingSmokeEvidence

  return {
    generatedAt: testingSmokeEvidence?.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(testingSmokeEvidence?.blockerRefs === undefined
      ? {}
      : { blockerRefs: testingSmokeEvidence.blockerRefs }),
    ...(testingSmokeEvidence?.entries === undefined
      ? {}
      : { entries: testingSmokeEvidence.entries }),
    ...(testingSmokeEvidence?.snapshotRef === undefined
      ? {}
      : { snapshotRef: testingSmokeEvidence.snapshotRef }),
    ...(testingSmokeEvidence?.versionRef === undefined
      ? {}
      : { versionRef: testingSmokeEvidence.versionRef }),
  }
}
