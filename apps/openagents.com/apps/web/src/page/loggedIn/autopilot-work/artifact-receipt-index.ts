import type {
  AutopilotWorkArtifactReceiptArtifact,
  AutopilotWorkArtifactReceiptArtifactKind,
  AutopilotWorkArtifactReceiptFreshness,
  AutopilotWorkArtifactReceiptIndex,
  AutopilotWorkArtifactReceiptReceipt,
  AutopilotWorkArtifactReceiptRedactionClass,
  AutopilotWorkArtifactReceiptTransitionKind,
  AutopilotWorkArtifactReceiptVisibility,
  AutopilotWorkProjection,
} from '../model'

export type ForgeArtifactReceiptIndexStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeArtifactReceiptIndexAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  artifactDeleteAuthority: false
  artifactDownloadAuthority: false
  artifactStoreAuthority: false
  claimSatisfactionAuthority: false
  deploymentAuthority: false
  publicClaimAuthority: false
  receiptAppendAuthority: false
  receiptRevokeAuthority: false
  settlementAuthority: false
  visibilityWidenAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeArtifactReceiptArtifactItem = Readonly<{
  artifactRef: string
  assignmentRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  digestRefs: ReadonlyArray<string>
  freshness: AutopilotWorkArtifactReceiptFreshness
  kind: AutopilotWorkArtifactReceiptArtifactKind
  laneRefs: ReadonlyArray<string>
  mediaTypeRefs: ReadonlyArray<string>
  missionRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  producerRefs: ReadonlyArray<string>
  redactionClass: AutopilotWorkArtifactReceiptRedactionClass
  relatedReceiptRefs: ReadonlyArray<string>
  retentionRefs: ReadonlyArray<string>
  runRefs: ReadonlyArray<string>
  sizeRefs: ReadonlyArray<string>
  subjectRefs: ReadonlyArray<string>
  summaryRefs: ReadonlyArray<string>
  visibility: AutopilotWorkArtifactReceiptVisibility
  workOrderRefs: ReadonlyArray<string>
}>

export type ForgeArtifactReceiptReceiptItem = Readonly<{
  actorRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  claimRequirementRefs: ReadonlyArray<string>
  freshness: AutopilotWorkArtifactReceiptFreshness
  idempotencyRefs: ReadonlyArray<string>
  inputRefs: ReadonlyArray<string>
  outputRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  receiptRef: string
  satisfyingReceiptRefs: ReadonlyArray<string>
  serviceRefs: ReadonlyArray<string>
  subjectRefs: ReadonlyArray<string>
  transitionKind: AutopilotWorkArtifactReceiptTransitionKind
  verificationRefs: ReadonlyArray<string>
}>

export type ForgeArtifactReceiptIndexInput = Readonly<{
  artifacts?: ReadonlyArray<AutopilotWorkArtifactReceiptArtifact>
  blockerRefs?: ReadonlyArray<string>
  generatedAt: string
  receipts?: ReadonlyArray<AutopilotWorkArtifactReceiptReceipt>
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeArtifactReceiptIndexCounts = Readonly<{
  artifacts: number
  publicArtifacts: number
  receipts: number
  stale: number
}>

export type ForgeArtifactReceiptIndexView = Readonly<{
  artifacts: ReadonlyArray<ForgeArtifactReceiptArtifactItem>
  authority: ForgeArtifactReceiptIndexAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeArtifactReceiptIndexCounts
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  receipts: ReadonlyArray<ForgeArtifactReceiptReceiptItem>
  snapshotRef: string | null
  status: ForgeArtifactReceiptIndexStatus
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
const PRIVATE_ARTIFACT_RECEIPT_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|body|build|command|content|diff|file|log|patch|payload|prompt|provider|receipt|settlement|shell|source|test|transcript|wallet)/i,
  /private[-_ ](?:artifact|content|diff|file|log|patch|payload|prompt|receipt|repo|source|transcript|workspace)/i,
  /artifact[-_ ](?:body|content|payload|raw)/i,
  /receipt[-_ ](?:body|content|payload|raw)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /wallet[-_ ](?:material|mnemonic|private)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeArtifactReceiptIndexAuthority = {
  acceptedOutcomeAuthority: false,
  artifactDeleteAuthority: false,
  artifactDownloadAuthority: false,
  artifactStoreAuthority: false,
  claimSatisfactionAuthority: false,
  deploymentAuthority: false,
  publicClaimAuthority: false,
  receiptAppendAuthority: false,
  receiptRevokeAuthority: false,
  settlementAuthority: false,
  visibilityWidenAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_ARTIFACT_RECEIPT_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-artifact-receipt-index-blocker:${workOrderRef}:${suffix}`

const normalizeArtifact = (
  artifact: AutopilotWorkArtifactReceiptArtifact,
): Readonly<{
  artifact: ForgeArtifactReceiptArtifactItem | null
  omittedUnsafeRefCount: number
}> => {
  const artifactRef = safeOptionalRef(artifact.artifactRef)
  const assignmentRefs = safeRefs(artifact.assignmentRefs)
  const blockerRefs = safeRefs(artifact.blockerRefs)
  const digestRefs = safeRefs(artifact.digestRefs)
  const laneRefs = safeRefs(artifact.laneRefs)
  const mediaTypeRefs = safeRefs(artifact.mediaTypeRefs)
  const missionRefs = safeRefs(artifact.missionRefs)
  const policyRefs = safeRefs(artifact.policyRefs)
  const producerRefs = safeRefs(artifact.producerRefs)
  const relatedReceiptRefs = safeRefs(artifact.relatedReceiptRefs)
  const retentionRefs = safeRefs(artifact.retentionRefs)
  const runRefs = safeRefs(artifact.runRefs)
  const sizeRefs = safeRefs(artifact.sizeRefs)
  const subjectRefs = safeRefs(artifact.subjectRefs)
  const summaryRefs = safeRefs(artifact.summaryRefs)
  const workOrderRefs = safeRefs(artifact.workOrderRefs)
  const omittedUnsafeRefCount =
    artifactRef.omittedUnsafeRefCount +
    assignmentRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    digestRefs.omittedUnsafeRefCount +
    laneRefs.omittedUnsafeRefCount +
    mediaTypeRefs.omittedUnsafeRefCount +
    missionRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    producerRefs.omittedUnsafeRefCount +
    relatedReceiptRefs.omittedUnsafeRefCount +
    retentionRefs.omittedUnsafeRefCount +
    runRefs.omittedUnsafeRefCount +
    sizeRefs.omittedUnsafeRefCount +
    subjectRefs.omittedUnsafeRefCount +
    summaryRefs.omittedUnsafeRefCount +
    workOrderRefs.omittedUnsafeRefCount

  return artifactRef.ref === null
    ? { artifact: null, omittedUnsafeRefCount }
    : {
        artifact: {
          artifactRef: artifactRef.ref,
          assignmentRefs: assignmentRefs.refs,
          blockerRefs: blockerRefs.refs,
          digestRefs: digestRefs.refs,
          freshness: artifact.freshness ?? 'unknown',
          kind: artifact.kind,
          laneRefs: laneRefs.refs,
          mediaTypeRefs: mediaTypeRefs.refs,
          missionRefs: missionRefs.refs,
          policyRefs: policyRefs.refs,
          producerRefs: producerRefs.refs,
          redactionClass: artifact.redactionClass ?? 'private_ref',
          relatedReceiptRefs: relatedReceiptRefs.refs,
          retentionRefs: retentionRefs.refs,
          runRefs: runRefs.refs,
          sizeRefs: sizeRefs.refs,
          subjectRefs: subjectRefs.refs,
          summaryRefs: summaryRefs.refs,
          visibility: artifact.visibility,
          workOrderRefs: workOrderRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const normalizeReceipt = (
  receipt: AutopilotWorkArtifactReceiptReceipt,
): Readonly<{
  omittedUnsafeRefCount: number
  receipt: ForgeArtifactReceiptReceiptItem | null
}> => {
  const actorRefs = safeRefs(receipt.actorRefs)
  const blockerRefs = safeRefs(receipt.blockerRefs)
  const caveatRefs = safeRefs(receipt.caveatRefs)
  const claimRequirementRefs = safeRefs(receipt.claimRequirementRefs)
  const idempotencyRefs = safeRefs(receipt.idempotencyRefs)
  const inputRefs = safeRefs(receipt.inputRefs)
  const outputRefs = safeRefs(receipt.outputRefs)
  const policyRefs = safeRefs(receipt.policyRefs)
  const receiptRef = safeOptionalRef(receipt.receiptRef)
  const satisfyingReceiptRefs = safeRefs(receipt.satisfyingReceiptRefs)
  const serviceRefs = safeRefs(receipt.serviceRefs)
  const subjectRefs = safeRefs(receipt.subjectRefs)
  const verificationRefs = safeRefs(receipt.verificationRefs)
  const omittedUnsafeRefCount =
    actorRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    caveatRefs.omittedUnsafeRefCount +
    claimRequirementRefs.omittedUnsafeRefCount +
    idempotencyRefs.omittedUnsafeRefCount +
    inputRefs.omittedUnsafeRefCount +
    outputRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    receiptRef.omittedUnsafeRefCount +
    satisfyingReceiptRefs.omittedUnsafeRefCount +
    serviceRefs.omittedUnsafeRefCount +
    subjectRefs.omittedUnsafeRefCount +
    verificationRefs.omittedUnsafeRefCount

  return receiptRef.ref === null
    ? { omittedUnsafeRefCount, receipt: null }
    : {
        omittedUnsafeRefCount,
        receipt: {
          actorRefs: actorRefs.refs,
          blockerRefs: blockerRefs.refs,
          caveatRefs: caveatRefs.refs,
          claimRequirementRefs: claimRequirementRefs.refs,
          freshness: receipt.freshness ?? 'unknown',
          idempotencyRefs: idempotencyRefs.refs,
          inputRefs: inputRefs.refs,
          outputRefs: outputRefs.refs,
          policyRefs: policyRefs.refs,
          receiptRef: receiptRef.ref,
          satisfyingReceiptRefs: satisfyingReceiptRefs.refs,
          serviceRefs: serviceRefs.refs,
          subjectRefs: subjectRefs.refs,
          transitionKind: receipt.transitionKind,
          verificationRefs: verificationRefs.refs,
        },
      }
}

const counts = (
  artifacts: ReadonlyArray<ForgeArtifactReceiptArtifactItem>,
  receipts: ReadonlyArray<ForgeArtifactReceiptReceiptItem>,
): ForgeArtifactReceiptIndexCounts => ({
  artifacts: artifacts.length,
  publicArtifacts: artifacts.filter(artifact => artifact.visibility === 'public').length,
  receipts: receipts.length,
  stale:
    artifacts.filter(artifact => artifact.freshness === 'stale').length +
    receipts.filter(receipt => receipt.freshness === 'stale').length,
})

const hasClaimRequirement = (
  receipt: ForgeArtifactReceiptReceiptItem,
  pattern: RegExp,
): boolean => receipt.claimRequirementRefs.some(ref => pattern.test(ref))

const staleBlockers = (
  workOrderRef: string,
  artifacts: ReadonlyArray<ForgeArtifactReceiptArtifactItem>,
  receipts: ReadonlyArray<ForgeArtifactReceiptReceiptItem>,
): ReadonlyArray<string> => [
  ...artifacts
    .filter(
      artifact => artifact.freshness === 'stale' && artifact.blockerRefs.length === 0,
    )
    .map(artifact =>
      blockerRef(workOrderRef, `stale-artifact-evidence:${artifact.artifactRef}`),
    ),
  ...receipts
    .filter(receipt => receipt.freshness === 'stale' && receipt.blockerRefs.length === 0)
    .map(receipt =>
      blockerRef(workOrderRef, `stale-receipt-evidence:${receipt.receiptRef}`),
    ),
]

const publicArtifactBlockers = (
  workOrderRef: string,
  artifacts: ReadonlyArray<ForgeArtifactReceiptArtifactItem>,
): ReadonlyArray<string> =>
  artifacts
    .filter(
      artifact =>
        artifact.visibility === 'public' &&
        (artifact.digestRefs.length === 0 ||
          artifact.redactionClass !== 'public_safe' ||
          artifact.policyRefs.length === 0) &&
        artifact.blockerRefs.length === 0,
    )
    .map(artifact =>
      blockerRef(workOrderRef, `public-artifact-evidence-missing:${artifact.artifactRef}`),
    )

const artifactReceiptBlockers = (
  workOrderRef: string,
  artifacts: ReadonlyArray<ForgeArtifactReceiptArtifactItem>,
): ReadonlyArray<string> =>
  artifacts
    .filter(
      artifact =>
        artifact.relatedReceiptRefs.length === 0 && artifact.blockerRefs.length === 0,
    )
    .map(artifact =>
      blockerRef(workOrderRef, `artifact-receipt-link-missing:${artifact.artifactRef}`),
    )

const receiptContractBlockers = (
  workOrderRef: string,
  receipts: ReadonlyArray<ForgeArtifactReceiptReceiptItem>,
): ReadonlyArray<string> =>
  receipts
    .filter(
      receipt =>
        (receipt.subjectRefs.length === 0 ||
          receipt.idempotencyRefs.length === 0 ||
          receipt.policyRefs.length === 0) &&
        receipt.blockerRefs.length === 0,
    )
    .map(receipt =>
      blockerRef(workOrderRef, `receipt-contract-incomplete:${receipt.receiptRef}`),
    )

const claimRequirementBlockers = (
  workOrderRef: string,
  receipts: ReadonlyArray<ForgeArtifactReceiptReceiptItem>,
): ReadonlyArray<string> =>
  receipts
    .filter(
      receipt =>
        receipt.claimRequirementRefs.length > 0 &&
        receipt.satisfyingReceiptRefs.length === 0 &&
        receipt.blockerRefs.length === 0,
    )
    .map(receipt =>
      blockerRef(workOrderRef, `claim-requirement-unsatisfied:${receipt.receiptRef}`),
    )

const paymentAcceptanceBlockers = (
  workOrderRef: string,
  receipts: ReadonlyArray<ForgeArtifactReceiptReceiptItem>,
): ReadonlyArray<string> =>
  receipts
    .filter(
      receipt =>
        receipt.transitionKind === 'payment' &&
        hasClaimRequirement(receipt, /(?:acceptance|accepted[-_]?outcome)/i) &&
        receipt.blockerRefs.length === 0,
    )
    .map(receipt =>
      blockerRef(workOrderRef, `payment-receipt-cannot-satisfy-acceptance:${receipt.receiptRef}`),
    )

const prDraftClaimBlockers = (
  workOrderRef: string,
  receipts: ReadonlyArray<ForgeArtifactReceiptReceiptItem>,
): ReadonlyArray<string> =>
  receipts
    .filter(
      receipt =>
        receipt.transitionKind === 'pr_draft' &&
        hasClaimRequirement(receipt, /(?:merge|deploy|customer[-_]?acceptance|acceptance)/i) &&
        receipt.blockerRefs.length === 0,
    )
    .map(receipt =>
      blockerRef(workOrderRef, `pr-draft-receipt-cannot-satisfy-final-claim:${receipt.receiptRef}`),
    )

const statusForView = (
  artifacts: ReadonlyArray<ForgeArtifactReceiptArtifactItem>,
  receipts: ReadonlyArray<ForgeArtifactReceiptReceiptItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeArtifactReceiptIndexStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (artifacts.length === 0 && receipts.length === 0) {
    return 'empty'
  }

  if (
    artifacts.some(artifact => artifact.freshness === 'stale') ||
    receipts.some(receipt => receipt.freshness === 'stale')
  ) {
    return 'stale'
  }

  return [...artifacts, ...receipts].every(entry => entry.freshness === 'unknown')
    ? 'unknown'
    : 'ready'
}

export const projectForgeArtifactReceiptIndex = (
  input: ForgeArtifactReceiptIndexInput,
): ForgeArtifactReceiptIndexView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedArtifacts = (input.artifacts ?? []).map(normalizeArtifact)
  const normalizedReceipts = (input.receipts ?? []).map(normalizeReceipt)
  const artifacts = normalizedArtifacts
    .flatMap(result => (result.artifact === null ? [] : [result.artifact]))
    .sort(
      (left, right) =>
        left.visibility.localeCompare(right.visibility) ||
        left.kind.localeCompare(right.kind) ||
        left.artifactRef.localeCompare(right.artifactRef),
    )
  const receipts = normalizedReceipts
    .flatMap(result => (result.receipt === null ? [] : [result.receipt]))
    .sort(
      (left, right) =>
        left.transitionKind.localeCompare(right.transitionKind) ||
        left.receiptRef.localeCompare(right.receiptRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedArtifacts.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0) +
    normalizedReceipts.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const hasEntries = (input.artifacts ?? []).length > 0 || (input.receipts ?? []).length > 0
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...artifacts.flatMap(artifact => artifact.blockerRefs),
      ...receipts.flatMap(receipt => receipt.blockerRefs),
      ...staleBlockers(input.workOrderRef, artifacts, receipts),
      ...publicArtifactBlockers(input.workOrderRef, artifacts),
      ...artifactReceiptBlockers(input.workOrderRef, artifacts),
      ...receiptContractBlockers(input.workOrderRef, receipts),
      ...claimRequirementBlockers(input.workOrderRef, receipts),
      ...paymentAcceptanceBlockers(input.workOrderRef, receipts),
      ...prDraftClaimBlockers(input.workOrderRef, receipts),
      ...(hasEntries && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-artifact-receipt-index-snapshot-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-artifact-receipt-material-omitted')]),
    ]),
  )

  return {
    artifacts,
    authority,
    blockerRefs,
    counts: counts(artifacts, receipts),
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    receipts,
    snapshotRef: snapshotRef.ref,
    status: statusForView(artifacts, receipts, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeArtifactReceiptIndexInput = (
  work: AutopilotWorkProjection,
): ForgeArtifactReceiptIndexInput => {
  const source: AutopilotWorkArtifactReceiptIndex | undefined =
    work.artifactReceiptIndex

  if (source === undefined) {
    return {
      generatedAt: work.generatedAt,
      workOrderRef: work.workOrderRef,
    }
  }

  return {
    generatedAt: source.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source.artifacts === undefined ? {} : { artifacts: source.artifacts }),
    ...(source.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source.receipts === undefined ? {} : { receipts: source.receipts }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
