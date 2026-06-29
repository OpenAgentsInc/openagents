import type {
  AutopilotWorkDataRetentionClass,
  AutopilotWorkDataRetentionEntry,
  AutopilotWorkDataRetentionFreshness,
  AutopilotWorkDataRetentionStatus,
  AutopilotWorkProjection,
} from '../model'

export type ForgeDataRetentionDeletionStatus =
  | 'blocked'
  | 'empty'
  | 'pending_deletion'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeDataRetentionDeletionAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  cachePurgeAuthority: false
  credentialReadAuthority: false
  credentialRevocationAuthority: false
  deletionExecutionAuthority: false
  exportGenerationAuthority: false
  exportReadAuthority: false
  privateDataReadAuthority: false
  projectionInvalidationAuthority: false
  publicProjectionMutationAuthority: false
  receiptDeletionAuthority: false
  retentionPolicyMutationAuthority: false
  retentionSweepAuthority: false
  settlementAuthority: false
  tombstoneCreationAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeDataRetentionDeletionItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  dataClass: AutopilotWorkDataRetentionClass
  dataClassRef: string
  deletionReceiptRefs: ReadonlyArray<string>
  deletionRequestRefs: ReadonlyArray<string>
  exportManifestRefs: ReadonlyArray<string>
  freshness: AutopilotWorkDataRetentionFreshness
  legalHoldRefs: ReadonlyArray<string>
  projectionFreshnessRefs: ReadonlyArray<string>
  projectionInvalidationRefs: ReadonlyArray<string>
  retentionPolicyRefs: ReadonlyArray<string>
  retentionSweepRefs: ReadonlyArray<string>
  status: AutopilotWorkDataRetentionStatus
  tombstoneRefs: ReadonlyArray<string>
}>

export type ForgeDataRetentionDeletionInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkDataRetentionEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeDataRetentionDeletionCounts = Readonly<{
  classes: number
  deleteRequested: number
  deletedOrTombstoned: number
  exportable: number
  legalOrPaymentCaveats: number
  publicProjectionClasses: number
  stale: number
}>

export type ForgeDataRetentionDeletionView = Readonly<{
  authority: ForgeDataRetentionDeletionAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeDataRetentionDeletionCounts
  entries: ReadonlyArray<ForgeDataRetentionDeletionItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeDataRetentionDeletionStatus
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
const PRIVATE_RETENTION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|cache|command|content|credential|data|deleted|event|export|file|log|memory|payload|shell|telemetry|token)/i,
  /private[-_ ](?:artifact|cache|content|credential|data|event|export|file|log|memory|payload|repo|source|telemetry|workspace)/i,
  /artifact[-_ ]content/i,
  /cache[-_ ]content/i,
  /credential[-_ ]value/i,
  /deleted[-_ ]payload/i,
  /event[-_ ]payload/i,
  /export[-_ ]content/i,
  /memory[-_ ]content/i,
  /telemetry[-_ ]payload/i,
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

const EXPORTABLE_CLASSES: ReadonlySet<AutopilotWorkDataRetentionClass> = new Set([
  'artifact_indexes',
  'artifact_payloads',
  'credential_metadata',
  'memory_records',
  'private_event_log_payloads',
  'product_receipts',
  'public_safe_event_refs',
  'session_summaries',
  'telemetry_aggregates',
])

const PUBLIC_PROJECTION_CLASSES: ReadonlySet<AutopilotWorkDataRetentionClass> =
  new Set(['product_receipts', 'public_projections', 'public_safe_event_refs'])

const POLICY_REQUIRED_STATUSES: ReadonlySet<AutopilotWorkDataRetentionStatus> =
  new Set(['active', 'blocked', 'delete_requested', 'legal_hold', 'retained'])

const DELETION_STATUSES: ReadonlySet<AutopilotWorkDataRetentionStatus> = new Set([
  'delete_requested',
  'deleted',
  'tombstoned',
])

const RETAINED_EXPORT_STATUSES: ReadonlySet<AutopilotWorkDataRetentionStatus> =
  new Set(['active', 'delete_requested', 'legal_hold', 'retained'])

const authority: ForgeDataRetentionDeletionAuthority = {
  acceptedOutcomeAuthority: false,
  cachePurgeAuthority: false,
  credentialReadAuthority: false,
  credentialRevocationAuthority: false,
  deletionExecutionAuthority: false,
  exportGenerationAuthority: false,
  exportReadAuthority: false,
  privateDataReadAuthority: false,
  projectionInvalidationAuthority: false,
  publicProjectionMutationAuthority: false,
  receiptDeletionAuthority: false,
  retentionPolicyMutationAuthority: false,
  retentionSweepAuthority: false,
  settlementAuthority: false,
  tombstoneCreationAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_RETENTION_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-data-retention-deletion-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkDataRetentionEntry,
): Readonly<{
  item: ForgeDataRetentionDeletionItem | null
  omittedUnsafeRefCount: number
}> => {
  const blockerRefs = safeRefs(item.blockerRefs)
  const caveatRefs = safeRefs(item.caveatRefs)
  const dataClassRef = safeOptionalRef(item.dataClassRef)
  const deletionReceiptRefs = safeRefs(item.deletionReceiptRefs)
  const deletionRequestRefs = safeRefs(item.deletionRequestRefs)
  const exportManifestRefs = safeRefs(item.exportManifestRefs)
  const legalHoldRefs = safeRefs(item.legalHoldRefs)
  const projectionFreshnessRefs = safeRefs(item.projectionFreshnessRefs)
  const projectionInvalidationRefs = safeRefs(item.projectionInvalidationRefs)
  const retentionPolicyRefs = safeRefs(item.retentionPolicyRefs)
  const retentionSweepRefs = safeRefs(item.retentionSweepRefs)
  const tombstoneRefs = safeRefs(item.tombstoneRefs)
  const omittedUnsafeRefCount =
    blockerRefs.omittedUnsafeRefCount +
    caveatRefs.omittedUnsafeRefCount +
    dataClassRef.omittedUnsafeRefCount +
    deletionReceiptRefs.omittedUnsafeRefCount +
    deletionRequestRefs.omittedUnsafeRefCount +
    exportManifestRefs.omittedUnsafeRefCount +
    legalHoldRefs.omittedUnsafeRefCount +
    projectionFreshnessRefs.omittedUnsafeRefCount +
    projectionInvalidationRefs.omittedUnsafeRefCount +
    retentionPolicyRefs.omittedUnsafeRefCount +
    retentionSweepRefs.omittedUnsafeRefCount +
    tombstoneRefs.omittedUnsafeRefCount

  return dataClassRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          blockerRefs: blockerRefs.refs,
          caveatRefs: caveatRefs.refs,
          dataClass: item.dataClass,
          dataClassRef: dataClassRef.ref,
          deletionReceiptRefs: deletionReceiptRefs.refs,
          deletionRequestRefs: deletionRequestRefs.refs,
          exportManifestRefs: exportManifestRefs.refs,
          freshness: item.freshness ?? 'unknown',
          legalHoldRefs: legalHoldRefs.refs,
          projectionFreshnessRefs: projectionFreshnessRefs.refs,
          projectionInvalidationRefs: projectionInvalidationRefs.refs,
          retentionPolicyRefs: retentionPolicyRefs.refs,
          retentionSweepRefs: retentionSweepRefs.refs,
          status: item.status,
          tombstoneRefs: tombstoneRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeDataRetentionDeletionItem>,
): ForgeDataRetentionDeletionCounts => ({
  classes: entries.length,
  deleteRequested: entries.filter(entry => entry.status === 'delete_requested')
    .length,
  deletedOrTombstoned: entries.filter(
    entry => entry.status === 'deleted' || entry.status === 'tombstoned',
  ).length,
  exportable: entries.filter(entry => EXPORTABLE_CLASSES.has(entry.dataClass))
    .length,
  legalOrPaymentCaveats: entries.filter(
    entry =>
      entry.dataClass === 'product_receipts' ||
      entry.status === 'legal_hold' ||
      entry.caveatRefs.length > 0 ||
      entry.legalHoldRefs.length > 0,
  ).length,
  publicProjectionClasses: entries.filter(entry =>
    PUBLIC_PROJECTION_CLASSES.has(entry.dataClass),
  ).length,
  stale: entries.filter(
    entry => entry.freshness === 'stale' || entry.status === 'expired',
  ).length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeDataRetentionDeletionItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]

  if (item.freshness === 'stale' || item.status === 'expired') {
    blockers.push(
      blockerRef(workOrderRef, `stale-retention-deletion-evidence:${item.dataClassRef}`),
    )
  }

  if (
    POLICY_REQUIRED_STATUSES.has(item.status) &&
    item.retentionPolicyRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `retained-data-class-policy-missing:${item.dataClassRef}`),
    )
  }

  if (
    DELETION_STATUSES.has(item.status) &&
    (item.deletionRequestRefs.length === 0 ||
      (item.deletionReceiptRefs.length === 0 && item.tombstoneRefs.length === 0))
  ) {
    blockers.push(
      blockerRef(workOrderRef, `deletion-state-receipt-or-tombstone-missing:${item.dataClassRef}`),
    )
  }

  if (
    item.status === 'tombstoned' &&
    item.projectionFreshnessRefs.length > 0 &&
    item.projectionInvalidationRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `tombstoned-record-projected-current:${item.dataClassRef}`),
    )
  }

  if (
    EXPORTABLE_CLASSES.has(item.dataClass) &&
    RETAINED_EXPORT_STATUSES.has(item.status) &&
    item.exportManifestRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `exportable-class-manifest-missing:${item.dataClassRef}`),
    )
  }

  if (
    PUBLIC_PROJECTION_CLASSES.has(item.dataClass) &&
    (item.projectionFreshnessRefs.length === 0 ||
      item.projectionInvalidationRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `public-projection-invalidation-missing:${item.dataClassRef}`),
    )
  }

  if (
    (item.dataClass === 'product_receipts' || item.status === 'legal_hold') &&
    item.caveatRefs.length === 0 &&
    item.legalHoldRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `legal-payment-caveat-missing:${item.dataClassRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeDataRetentionDeletionItem>,
  blockers: ReadonlyArray<string>,
): ForgeDataRetentionDeletionStatus => {
  if (blockers.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (
    entries.some(entry => entry.freshness === 'stale' || entry.status === 'expired')
  ) {
    return 'stale'
  }

  if (entries.some(entry => entry.status === 'delete_requested')) {
    return 'pending_deletion'
  }

  if (
    entries.every(entry =>
      ['active', 'deleted', 'legal_hold', 'retained', 'tombstoned'].includes(
        entry.status,
      ),
    )
  ) {
    return 'ready'
  }

  return 'unknown'
}

export const projectForgeDataRetentionDeletionEvidence = (
  input: ForgeDataRetentionDeletionInput,
): ForgeDataRetentionDeletionView => {
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
      blockerRef(input.workOrderRef, 'missing-data-retention-deletion-snapshot-ref'),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-data-retention-deletion-material-omitted'),
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

export const buildForgeDataRetentionDeletionInput = (
  work: AutopilotWorkProjection,
): ForgeDataRetentionDeletionInput => {
  const evidence = work.dataRetentionDeletionEvidence

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
