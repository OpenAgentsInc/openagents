import type {
  AutopilotWorkProjection,
  AutopilotWorkTeamSharedMemory,
  AutopilotWorkTeamSharedMemoryEntry,
  AutopilotWorkTeamSharedMemoryFreshness,
  AutopilotWorkTeamSharedMemoryKind,
  AutopilotWorkTeamSharedMemoryRedactionClass,
  AutopilotWorkTeamSharedMemoryReviewState,
  AutopilotWorkTeamSharedMemoryScope,
  AutopilotWorkTeamSharedMemoryVisibility,
} from '../model'

export type ForgeTeamSharedMemoryStatus =
  | 'blocked'
  | 'empty'
  | 'pending_review'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeTeamSharedMemoryAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  deploymentAuthority: false
  memoryCreateAuthority: false
  memoryDeleteAuthority: false
  memoryPromotionAuthority: false
  memoryUpdateAuthority: false
  modelCallAuthority: false
  promptAssemblyAuthority: false
  publicClaimAuthority: false
  semanticRetrievalAuthority: false
  settlementAuthority: false
  teamRecordMutationAuthority: false
  toolGrantAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeTeamSharedMemoryItem = Readonly<{
  applicationReceiptRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  consentRefs: ReadonlyArray<string>
  deletionReceiptRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  expiryRefs: ReadonlyArray<string>
  freshness: AutopilotWorkTeamSharedMemoryFreshness
  kind: AutopilotWorkTeamSharedMemoryKind
  memoryRef: string
  ownerRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  promotionRefs: ReadonlyArray<string>
  redactionClass: AutopilotWorkTeamSharedMemoryRedactionClass
  retrievalPolicyRefs: ReadonlyArray<string>
  reviewRefs: ReadonlyArray<string>
  reviewState: AutopilotWorkTeamSharedMemoryReviewState
  scope: AutopilotWorkTeamSharedMemoryScope
  semanticQueryRefs: ReadonlyArray<string>
  teamRefs: ReadonlyArray<string>
  tombstoneRefs: ReadonlyArray<string>
  typedQueryRefs: ReadonlyArray<string>
  visibility: AutopilotWorkTeamSharedMemoryVisibility
}>

export type ForgeTeamSharedMemoryInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkTeamSharedMemoryEntry>
  generatedAt: string
  projectionRef?: string | null
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeTeamSharedMemoryCounts = Readonly<{
  accepted: number
  pendingReview: number
  publicVisible: number
  stale: number
  teamVisible: number
  total: number
}>

export type ForgeTeamSharedMemoryView = Readonly<{
  authority: ForgeTeamSharedMemoryAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeTeamSharedMemoryCounts
  entries: ReadonlyArray<ForgeTeamSharedMemoryItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  projectionRef: string | null
  publicSafe: true
  snapshotRef: string | null
  status: ForgeTeamSharedMemoryStatus
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
const PRIVATE_SHARED_MEMORY_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /memory[-_ ](?:body|content|statement|text)/i,
  /prompt[-_ ](?:body|content|text)/i,
  /raw[-_ ](?:artifact|body|command|content|customer|decision|file|instruction|log|memory|payload|prompt|provider|request|shell|source|statement|trace|transcript)/i,
  /private[-_ ](?:artifact|content|customer|instruction|memory|prompt|repo|source|statement|transcript|workspace)/i,
  /provider[-_ ]payload/i,
  /provider[-_ ]prompt/i,
  /customer[-_ ]data/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret|token)\b/i,
]

const authority: ForgeTeamSharedMemoryAuthority = {
  acceptedOutcomeAuthority: false,
  deploymentAuthority: false,
  memoryCreateAuthority: false,
  memoryDeleteAuthority: false,
  memoryPromotionAuthority: false,
  memoryUpdateAuthority: false,
  modelCallAuthority: false,
  promptAssemblyAuthority: false,
  publicClaimAuthority: false,
  semanticRetrievalAuthority: false,
  settlementAuthority: false,
  teamRecordMutationAuthority: false,
  toolGrantAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_SHARED_MEMORY_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-team-shared-memory-blocker:${workOrderRef}:${suffix}`

const normalizeEntry = (
  entry: AutopilotWorkTeamSharedMemoryEntry,
): Readonly<{
  entry: ForgeTeamSharedMemoryItem | null
  omittedUnsafeRefCount: number
}> => {
  const applicationReceiptRefs = safeRefs(entry.applicationReceiptRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const consentRefs = safeRefs(entry.consentRefs)
  const deletionReceiptRefs = safeRefs(entry.deletionReceiptRefs)
  const evidenceRefs = safeRefs(entry.evidenceRefs)
  const expiryRefs = safeRefs(entry.expiryRefs)
  const memoryRef = safeOptionalRef(entry.memoryRef)
  const ownerRefs = safeRefs(entry.ownerRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const promotionRefs = safeRefs(entry.promotionRefs)
  const retrievalPolicyRefs = safeRefs(entry.retrievalPolicyRefs)
  const reviewRefs = safeRefs(entry.reviewRefs)
  const semanticQueryRefs = safeRefs(entry.semanticQueryRefs)
  const teamRefs = safeRefs(entry.teamRefs)
  const tombstoneRefs = safeRefs(entry.tombstoneRefs)
  const typedQueryRefs = safeRefs(entry.typedQueryRefs)
  const omittedUnsafeRefCount =
    applicationReceiptRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount +
    consentRefs.omittedUnsafeRefCount +
    deletionReceiptRefs.omittedUnsafeRefCount +
    evidenceRefs.omittedUnsafeRefCount +
    expiryRefs.omittedUnsafeRefCount +
    memoryRef.omittedUnsafeRefCount +
    ownerRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    promotionRefs.omittedUnsafeRefCount +
    retrievalPolicyRefs.omittedUnsafeRefCount +
    reviewRefs.omittedUnsafeRefCount +
    semanticQueryRefs.omittedUnsafeRefCount +
    teamRefs.omittedUnsafeRefCount +
    tombstoneRefs.omittedUnsafeRefCount +
    typedQueryRefs.omittedUnsafeRefCount

  return memoryRef.ref === null
    ? { entry: null, omittedUnsafeRefCount }
    : {
        entry: {
          applicationReceiptRefs: applicationReceiptRefs.refs,
          blockerRefs: blockerRefs.refs,
          consentRefs: consentRefs.refs,
          deletionReceiptRefs: deletionReceiptRefs.refs,
          evidenceRefs: evidenceRefs.refs,
          expiryRefs: expiryRefs.refs,
          freshness: entry.freshness ?? 'unknown',
          kind: entry.kind,
          memoryRef: memoryRef.ref,
          ownerRefs: ownerRefs.refs,
          policyRefs: policyRefs.refs,
          promotionRefs: promotionRefs.refs,
          redactionClass: entry.redactionClass ?? 'private_ref',
          retrievalPolicyRefs: retrievalPolicyRefs.refs,
          reviewRefs: reviewRefs.refs,
          reviewState: entry.reviewState,
          scope: entry.scope,
          semanticQueryRefs: semanticQueryRefs.refs,
          teamRefs: teamRefs.refs,
          tombstoneRefs: tombstoneRefs.refs,
          typedQueryRefs: typedQueryRefs.refs,
          visibility: entry.visibility,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeTeamSharedMemoryItem>,
): ForgeTeamSharedMemoryCounts => ({
  accepted: entries.filter(entry => entry.reviewState === 'accepted').length,
  pendingReview: entries.filter(entry => entry.reviewState === 'pending_review').length,
  publicVisible: entries.filter(entry => entry.visibility === 'public').length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
  teamVisible: entries.filter(entry => entry.visibility === 'team').length,
  total: entries.length,
})

const staleBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeTeamSharedMemoryItem>,
): ReadonlyArray<string> =>
  entries
    .filter(entry => entry.freshness === 'stale' && entry.blockerRefs.length === 0)
    .map(entry => blockerRef(workOrderRef, `stale-shared-memory-evidence:${entry.memoryRef}`))

const visibilityBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeTeamSharedMemoryItem>,
): ReadonlyArray<string> =>
  entries.flatMap(entry => {
    if (entry.blockerRefs.length > 0) {
      return []
    }

    return [
      ...(entry.visibility === 'team' &&
      (entry.teamRefs.length === 0 || entry.policyRefs.length === 0)
        ? [blockerRef(workOrderRef, `team-memory-policy-missing:${entry.memoryRef}`)]
        : []),
      ...(entry.visibility === 'public' &&
      (entry.redactionClass !== 'public_safe' || entry.policyRefs.length === 0)
        ? [blockerRef(workOrderRef, `public-memory-redaction-policy-missing:${entry.memoryRef}`)]
        : []),
    ]
  })

const applicationReceiptBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeTeamSharedMemoryItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        (entry.typedQueryRefs.length > 0 || entry.semanticQueryRefs.length > 0) &&
        entry.applicationReceiptRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `applied-memory-receipt-missing:${entry.memoryRef}`))

const deletionBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeTeamSharedMemoryItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.reviewState === 'deleted' &&
        entry.deletionReceiptRefs.length === 0 &&
        entry.tombstoneRefs.length === 0 &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `deleted-memory-tombstone-missing:${entry.memoryRef}`))

const promotionBlockers = (
  workOrderRef: string,
  entries: ReadonlyArray<ForgeTeamSharedMemoryItem>,
): ReadonlyArray<string> =>
  entries
    .filter(
      entry =>
        entry.promotionRefs.length > 0 &&
        (entry.consentRefs.length === 0 || entry.policyRefs.length === 0) &&
        entry.blockerRefs.length === 0,
    )
    .map(entry => blockerRef(workOrderRef, `memory-promotion-consent-missing:${entry.memoryRef}`))

const statusForView = (
  entries: ReadonlyArray<ForgeTeamSharedMemoryItem>,
  blockerRefs: ReadonlyArray<string>,
): ForgeTeamSharedMemoryStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (entries.some(entry => entry.freshness === 'stale')) {
    return 'stale'
  }

  if (entries.some(entry => entry.reviewState === 'pending_review')) {
    return 'pending_review'
  }

  return entries.every(entry => entry.freshness === 'unknown') ? 'unknown' : 'ready'
}

export const projectForgeTeamSharedMemory = (
  input: ForgeTeamSharedMemoryInput,
): ForgeTeamSharedMemoryView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const projectionRef = safeOptionalRef(input.projectionRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const sourceBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(
      (left, right) =>
        left.scope.localeCompare(right.scope) ||
        left.visibility.localeCompare(right.visibility) ||
        left.kind.localeCompare(right.kind) ||
        left.memoryRef.localeCompare(right.memoryRef),
    )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    projectionRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    sourceBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omittedUnsafeRefCount, 0)
  const blockerRefs = Array.from(
    new Set([
      ...sourceBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...staleBlockers(input.workOrderRef, entries),
      ...visibilityBlockers(input.workOrderRef, entries),
      ...applicationReceiptBlockers(input.workOrderRef, entries),
      ...deletionBlockers(input.workOrderRef, entries),
      ...promotionBlockers(input.workOrderRef, entries),
      ...(input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'missing-team-shared-memory-snapshot-ref')]
        : []),
      ...(projectionRef.ref !== null && snapshotRef.ref === null
        ? [blockerRef(input.workOrderRef, 'projection-without-snapshot')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(input.workOrderRef, 'unsafe-team-shared-memory-material-omitted')]),
    ]),
  )

  return {
    authority,
    blockerRefs,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    projectionRef: projectionRef.ref,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusForView(entries, blockerRefs),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeTeamSharedMemoryInput = (
  work: AutopilotWorkProjection,
): ForgeTeamSharedMemoryInput => {
  const source: AutopilotWorkTeamSharedMemory | undefined = work.teamSharedMemory

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
    ...(source.projectionRef === undefined ? {} : { projectionRef: source.projectionRef }),
    snapshotRef: source.snapshotRef,
    ...(source.versionRef === undefined ? {} : { versionRef: source.versionRef }),
  }
}
