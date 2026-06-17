import type {
  AutopilotWorkEvaluationRegressionEntry,
  AutopilotWorkEvaluationRegressionFreshness,
  AutopilotWorkEvaluationRegressionStatus,
  AutopilotWorkProjection,
} from '../model'

export type ForgeEvaluationRegressionEvidenceStatus =
  | 'blocked'
  | 'empty'
  | 'failed'
  | 'pending'
  | 'passed'
  | 'regressed'
  | 'unknown'

export type ForgeEvaluationRegressionEvidenceAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  evalExecutionAuthority: false
  evalSuiteLoadAuthority: false
  fixturePromotionAuthority: false
  modelProviderCallAuthority: false
  productPromiseMutationAuthority: false
  publicClaimMutationAuthority: false
  regressionGateMutationAuthority: false
  releaseGateEnforcementAuthority: false
  reportGenerationAuthority: false
  settlementAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeEvaluationRegressionEvidenceItem = Readonly<{
  adapterRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  budgetPolicyRefs: ReadonlyArray<string>
  costSummaryRefs: ReadonlyArray<string>
  evaluationRef: string
  failureRefs: ReadonlyArray<string>
  firstDivergenceRefs: ReadonlyArray<string>
  fixturePromotionRefs: ReadonlyArray<string>
  fixtureProvenanceRefs: ReadonlyArray<string>
  fixtureRedactionRefs: ReadonlyArray<string>
  fixtureRefs: ReadonlyArray<string>
  freshness: AutopilotWorkEvaluationRegressionFreshness
  latencySummaryRefs: ReadonlyArray<string>
  modelRefs: ReadonlyArray<string>
  privateReportRefs: ReadonlyArray<string>
  productClaimRefs: ReadonlyArray<string>
  providerRefs: ReadonlyArray<string>
  publicReportRefs: ReadonlyArray<string>
  regressionGateRefs: ReadonlyArray<string>
  resultVerdictRefs: ReadonlyArray<string>
  reviewRefs: ReadonlyArray<string>
  safetyVerdictRefs: ReadonlyArray<string>
  status: AutopilotWorkEvaluationRegressionStatus
  suiteRefs: ReadonlyArray<string>
  thresholdRefs: ReadonlyArray<string>
  toolPolicyRefs: ReadonlyArray<string>
  versionRefs: ReadonlyArray<string>
}>

export type ForgeEvaluationRegressionEvidenceInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkEvaluationRegressionEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeEvaluationRegressionEvidenceCounts = Readonly<{
  entries: number
  failed: number
  passed: number
  pending: number
  publicReports: number
  regressed: number
  stale: number
}>

export type ForgeEvaluationRegressionEvidenceView = Readonly<{
  authority: ForgeEvaluationRegressionEvidenceAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeEvaluationRegressionEvidenceCounts
  entries: ReadonlyArray<ForgeEvaluationRegressionEvidenceItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeEvaluationRegressionEvidenceStatus
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
const PRIVATE_EVAL_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|command|content|customer|data|detail|fixture|log|output|payload|provider|report|task|transcript)/i,
  /private[-_ ](?:artifact|code|content|customer|data|fixture|log|payload|provider|repo|report|source|task|transcript|workspace)/i,
  /artifact[-_ ]content/i,
  /customer[-_ ](?:data|private|payload|record)/i,
  /fixture[-_ ]body/i,
  /provider[-_ ]payload/i,
  /raw[-_ ]transcript/i,
  /task[-_ ]body/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeEvaluationRegressionEvidenceAuthority = {
  acceptedOutcomeAuthority: false,
  evalExecutionAuthority: false,
  evalSuiteLoadAuthority: false,
  fixturePromotionAuthority: false,
  modelProviderCallAuthority: false,
  productPromiseMutationAuthority: false,
  publicClaimMutationAuthority: false,
  regressionGateMutationAuthority: false,
  releaseGateEnforcementAuthority: false,
  reportGenerationAuthority: false,
  settlementAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_EVAL_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-evaluation-regression-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkEvaluationRegressionEntry,
): Readonly<{
  item: ForgeEvaluationRegressionEvidenceItem | null
  omittedUnsafeRefCount: number
}> => {
  const adapterRefs = safeRefs(item.adapterRefs)
  const artifactRefs = safeRefs(item.artifactRefs)
  const blockerRefs = safeRefs(item.blockerRefs)
  const budgetPolicyRefs = safeRefs(item.budgetPolicyRefs)
  const costSummaryRefs = safeRefs(item.costSummaryRefs)
  const evaluationRef = safeOptionalRef(item.evaluationRef)
  const failureRefs = safeRefs(item.failureRefs)
  const firstDivergenceRefs = safeRefs(item.firstDivergenceRefs)
  const fixturePromotionRefs = safeRefs(item.fixturePromotionRefs)
  const fixtureProvenanceRefs = safeRefs(item.fixtureProvenanceRefs)
  const fixtureRedactionRefs = safeRefs(item.fixtureRedactionRefs)
  const fixtureRefs = safeRefs(item.fixtureRefs)
  const latencySummaryRefs = safeRefs(item.latencySummaryRefs)
  const modelRefs = safeRefs(item.modelRefs)
  const privateReportRefs = safeRefs(item.privateReportRefs)
  const productClaimRefs = safeRefs(item.productClaimRefs)
  const providerRefs = safeRefs(item.providerRefs)
  const publicReportRefs = safeRefs(item.publicReportRefs)
  const regressionGateRefs = safeRefs(item.regressionGateRefs)
  const resultVerdictRefs = safeRefs(item.resultVerdictRefs)
  const reviewRefs = safeRefs(item.reviewRefs)
  const safetyVerdictRefs = safeRefs(item.safetyVerdictRefs)
  const suiteRefs = safeRefs(item.suiteRefs)
  const thresholdRefs = safeRefs(item.thresholdRefs)
  const toolPolicyRefs = safeRefs(item.toolPolicyRefs)
  const versionRefs = safeRefs(item.versionRefs)
  const omittedUnsafeRefCount =
    adapterRefs.omittedUnsafeRefCount +
    artifactRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    budgetPolicyRefs.omittedUnsafeRefCount +
    costSummaryRefs.omittedUnsafeRefCount +
    evaluationRef.omittedUnsafeRefCount +
    failureRefs.omittedUnsafeRefCount +
    firstDivergenceRefs.omittedUnsafeRefCount +
    fixturePromotionRefs.omittedUnsafeRefCount +
    fixtureProvenanceRefs.omittedUnsafeRefCount +
    fixtureRedactionRefs.omittedUnsafeRefCount +
    fixtureRefs.omittedUnsafeRefCount +
    latencySummaryRefs.omittedUnsafeRefCount +
    modelRefs.omittedUnsafeRefCount +
    privateReportRefs.omittedUnsafeRefCount +
    productClaimRefs.omittedUnsafeRefCount +
    providerRefs.omittedUnsafeRefCount +
    publicReportRefs.omittedUnsafeRefCount +
    regressionGateRefs.omittedUnsafeRefCount +
    resultVerdictRefs.omittedUnsafeRefCount +
    reviewRefs.omittedUnsafeRefCount +
    safetyVerdictRefs.omittedUnsafeRefCount +
    suiteRefs.omittedUnsafeRefCount +
    thresholdRefs.omittedUnsafeRefCount +
    toolPolicyRefs.omittedUnsafeRefCount +
    versionRefs.omittedUnsafeRefCount

  return evaluationRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          adapterRefs: adapterRefs.refs,
          artifactRefs: artifactRefs.refs,
          blockerRefs: blockerRefs.refs,
          budgetPolicyRefs: budgetPolicyRefs.refs,
          costSummaryRefs: costSummaryRefs.refs,
          evaluationRef: evaluationRef.ref,
          failureRefs: failureRefs.refs,
          firstDivergenceRefs: firstDivergenceRefs.refs,
          fixturePromotionRefs: fixturePromotionRefs.refs,
          fixtureProvenanceRefs: fixtureProvenanceRefs.refs,
          fixtureRedactionRefs: fixtureRedactionRefs.refs,
          fixtureRefs: fixtureRefs.refs,
          freshness: item.freshness ?? 'unknown',
          latencySummaryRefs: latencySummaryRefs.refs,
          modelRefs: modelRefs.refs,
          privateReportRefs: privateReportRefs.refs,
          productClaimRefs: productClaimRefs.refs,
          providerRefs: providerRefs.refs,
          publicReportRefs: publicReportRefs.refs,
          regressionGateRefs: regressionGateRefs.refs,
          resultVerdictRefs: resultVerdictRefs.refs,
          reviewRefs: reviewRefs.refs,
          safetyVerdictRefs: safetyVerdictRefs.refs,
          status: item.status,
          suiteRefs: suiteRefs.refs,
          thresholdRefs: thresholdRefs.refs,
          toolPolicyRefs: toolPolicyRefs.refs,
          versionRefs: versionRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeEvaluationRegressionEvidenceItem>,
): ForgeEvaluationRegressionEvidenceCounts => ({
  entries: entries.length,
  failed: entries.filter(entry => entry.status === 'failed').length,
  passed: entries.filter(entry => entry.status === 'passed').length,
  pending: entries.filter(entry => entry.status === 'pending').length,
  publicReports: entries.filter(entry => entry.publicReportRefs.length > 0).length,
  regressed: entries.filter(entry => entry.status === 'regressed').length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeEvaluationRegressionEvidenceItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]
  const hasPublicClaim = item.productClaimRefs.length > 0
  const isComparison =
    item.adapterRefs.length > 1 ||
    item.providerRefs.length > 1 ||
    item.modelRefs.length > 1

  if (item.freshness === 'stale') {
    blockers.push(
      blockerRef(workOrderRef, `stale-evaluation-evidence:${item.evaluationRef}`),
    )
  }

  if (
    hasPublicClaim &&
    (item.suiteRefs.length === 0 ||
      item.fixtureRefs.length === 0 ||
      item.fixtureProvenanceRefs.length === 0 ||
      item.fixtureRedactionRefs.length === 0 ||
      item.resultVerdictRefs.length === 0 ||
      item.firstDivergenceRefs.length === 0 ||
      item.safetyVerdictRefs.length === 0 ||
      item.publicReportRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `public-eval-claim-missing-evidence:${item.evaluationRef}`),
    )
  }

  if (
    isComparison &&
    (item.budgetPolicyRefs.length === 0 || item.toolPolicyRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(
        workOrderRef,
        `comparison-missing-policy-equivalence:${item.evaluationRef}`,
      ),
    )
  }

  if (
    (item.status === 'regressed' || item.status === 'failed') &&
    item.regressionGateRefs.length > 0 &&
    (item.thresholdRefs.length === 0 ||
      (item.blockerRefs.length === 0 && item.failureRefs.length === 0))
  ) {
    blockers.push(
      blockerRef(workOrderRef, `regression-gate-missing-threshold-blocker:${item.evaluationRef}`),
    )
  }

  if (
    item.fixturePromotionRefs.length > 0 &&
    (item.reviewRefs.length === 0 || item.fixtureRedactionRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `fixture-promotion-missing-review-redaction:${item.evaluationRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeEvaluationRegressionEvidenceItem>,
  blockers: ReadonlyArray<string>,
): ForgeEvaluationRegressionEvidenceStatus => {
  if (blockers.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.status === 'regressed')) {
    return 'regressed'
  }

  if (entries.some(entry => entry.status === 'failed')) {
    return 'failed'
  }

  if (entries.some(entry => entry.status === 'pending')) {
    return 'pending'
  }

  if (entries.every(entry => entry.status === 'passed')) {
    return 'passed'
  }

  return 'unknown'
}

export const projectForgeEvaluationRegressionEvidence = (
  input: ForgeEvaluationRegressionEvidenceInput,
): ForgeEvaluationRegressionEvidenceView => {
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
      blockerRef(
        input.workOrderRef,
        'missing-evaluation-regression-evidence-snapshot-ref',
      ),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-evaluation-regression-material-omitted'),
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

export const buildForgeEvaluationRegressionEvidenceInput = (
  work: AutopilotWorkProjection,
): ForgeEvaluationRegressionEvidenceInput => {
  const evidence = work.evaluationRegressionEvidence

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
