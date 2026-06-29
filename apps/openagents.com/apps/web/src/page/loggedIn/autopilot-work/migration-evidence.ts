import type {
  AutopilotWorkMigrationDomain,
  AutopilotWorkMigrationEntry,
  AutopilotWorkMigrationFreshness,
  AutopilotWorkMigrationStatus,
  AutopilotWorkProjection,
} from '../model'

export type ForgeMigrationEvidenceStatus =
  | 'blocked'
  | 'empty'
  | 'failed'
  | 'ready'
  | 'required'
  | 'unknown'

export type ForgeMigrationEvidenceAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  cacheRebuildAuthority: false
  deploymentAuthority: false
  downgradeExecutionAuthority: false
  exportGenerationAuthority: false
  migrationExecutionAuthority: false
  publicClaimAuthority: false
  registryMutationAuthority: false
  restoreAuthority: false
  rollbackAuthority: false
  settlementAuthority: false
  snapshotCreationAuthority: false
  startupRecoveryTransitionAuthority: false
  validationExecutionAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeMigrationEvidenceItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  domain: AutopilotWorkMigrationDomain
  domainRef: string
  downgradeRefs: ReadonlyArray<string>
  downgradeRequired: boolean
  freshness: AutopilotWorkMigrationFreshness
  idempotencyRefs: ReadonlyArray<string>
  migrationRefs: ReadonlyArray<string>
  optionalCache: boolean
  optionalCacheRebuildRefs: ReadonlyArray<string>
  policyRefs: ReadonlyArray<string>
  receiptRefs: ReadonlyArray<string>
  recoveryRefs: ReadonlyArray<string>
  redactionRefs: ReadonlyArray<string>
  registryRefs: ReadonlyArray<string>
  required: boolean
  restorePointRefs: ReadonlyArray<string>
  rollbackBoundaryRefs: ReadonlyArray<string>
  schemaFromRef: string
  schemaToRef: string
  status: AutopilotWorkMigrationStatus
  validationRefs: ReadonlyArray<string>
}>

export type ForgeMigrationEvidenceInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkMigrationEntry>
  generatedAt: string
  registryRefs?: ReadonlyArray<string>
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeMigrationEvidenceCounts = Readonly<{
  completed: number
  domains: number
  failed: number
  rebuildable: number
  required: number
  stale: number
}>

export type ForgeMigrationEvidenceView = Readonly<{
  authority: ForgeMigrationEvidenceAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeMigrationEvidenceCounts
  entries: ReadonlyArray<ForgeMigrationEvidenceItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  registryRefs: ReadonlyArray<string>
  snapshotRef: string | null
  status: ForgeMigrationEvidenceStatus
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
const PRIVATE_MIGRATION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:artifact|cache|command|content|credential|data|detail|event|file|fixture|log|memory|output|payload|provider|record|setting|shell|snapshot|state|transcript)/i,
  /private[-_ ](?:artifact|cache|code|content|credential|data|event|file|fixture|log|memory|payload|provider|record|repo|setting|source|state|workspace)/i,
  /credential[-_ ]value/i,
  /fixture[-_ ]body/i,
  /migration[-_ ]log/i,
  /provider[-_ ]payload/i,
  /state[-_ ]payload/i,
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

const authority: ForgeMigrationEvidenceAuthority = {
  acceptedOutcomeAuthority: false,
  cacheRebuildAuthority: false,
  deploymentAuthority: false,
  downgradeExecutionAuthority: false,
  exportGenerationAuthority: false,
  migrationExecutionAuthority: false,
  publicClaimAuthority: false,
  registryMutationAuthority: false,
  restoreAuthority: false,
  rollbackAuthority: false,
  settlementAuthority: false,
  snapshotCreationAuthority: false,
  startupRecoveryTransitionAuthority: false,
  validationExecutionAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_MIGRATION_MARKERS.some(marker => marker.test(trimmed))
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
  `forge-migration-evidence-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkMigrationEntry,
): Readonly<{
  item: ForgeMigrationEvidenceItem | null
  omittedUnsafeRefCount: number
}> => {
  const blockerRefs = safeRefs(item.blockerRefs)
  const domainRef = safeOptionalRef(item.domainRef)
  const downgradeRefs = safeRefs(item.downgradeRefs)
  const idempotencyRefs = safeRefs(item.idempotencyRefs)
  const migrationRefs = safeRefs(item.migrationRefs)
  const optionalCacheRebuildRefs = safeRefs(item.optionalCacheRebuildRefs)
  const policyRefs = safeRefs(item.policyRefs)
  const receiptRefs = safeRefs(item.receiptRefs)
  const recoveryRefs = safeRefs(item.recoveryRefs)
  const redactionRefs = safeRefs(item.redactionRefs)
  const registryRefs = safeRefs(item.registryRefs)
  const restorePointRefs = safeRefs(item.restorePointRefs)
  const rollbackBoundaryRefs = safeRefs(item.rollbackBoundaryRefs)
  const schemaFromRef = safeOptionalRef(item.schemaFromRef)
  const schemaToRef = safeOptionalRef(item.schemaToRef)
  const validationRefs = safeRefs(item.validationRefs)
  const omittedUnsafeRefCount =
    blockerRefs.omittedUnsafeRefCount +
    domainRef.omittedUnsafeRefCount +
    downgradeRefs.omittedUnsafeRefCount +
    idempotencyRefs.omittedUnsafeRefCount +
    migrationRefs.omittedUnsafeRefCount +
    optionalCacheRebuildRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    receiptRefs.omittedUnsafeRefCount +
    recoveryRefs.omittedUnsafeRefCount +
    redactionRefs.omittedUnsafeRefCount +
    registryRefs.omittedUnsafeRefCount +
    restorePointRefs.omittedUnsafeRefCount +
    rollbackBoundaryRefs.omittedUnsafeRefCount +
    schemaFromRef.omittedUnsafeRefCount +
    schemaToRef.omittedUnsafeRefCount +
    validationRefs.omittedUnsafeRefCount

  return domainRef.ref === null ||
    schemaFromRef.ref === null ||
    schemaToRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          blockerRefs: blockerRefs.refs,
          domain: item.domain,
          domainRef: domainRef.ref,
          downgradeRefs: downgradeRefs.refs,
          downgradeRequired: item.downgradeRequired ?? false,
          freshness: item.freshness ?? 'unknown',
          idempotencyRefs: idempotencyRefs.refs,
          migrationRefs: migrationRefs.refs,
          optionalCache: item.optionalCache ?? false,
          optionalCacheRebuildRefs: optionalCacheRebuildRefs.refs,
          policyRefs: policyRefs.refs,
          receiptRefs: receiptRefs.refs,
          recoveryRefs: recoveryRefs.refs,
          redactionRefs: redactionRefs.refs,
          registryRefs: registryRefs.refs,
          required: item.required ?? item.status === 'required',
          restorePointRefs: restorePointRefs.refs,
          rollbackBoundaryRefs: rollbackBoundaryRefs.refs,
          schemaFromRef: schemaFromRef.ref,
          schemaToRef: schemaToRef.ref,
          status: item.status,
          validationRefs: validationRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeMigrationEvidenceItem>,
): ForgeMigrationEvidenceCounts => ({
  completed: entries.filter(entry => entry.status === 'completed').length,
  domains: entries.length,
  failed: entries.filter(entry => entry.status === 'failed').length,
  rebuildable: entries.filter(entry => entry.status === 'rebuildable').length,
  required: entries.filter(entry => entry.required || entry.status === 'required')
    .length,
  stale: entries.filter(entry => entry.freshness === 'stale').length,
})

const itemBlockers = (
  workOrderRef: string,
  item: ForgeMigrationEvidenceItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]
  const requiresRestoreBoundary =
    item.required || item.status === 'required' || item.status === 'completed'

  if (item.freshness === 'stale') {
    blockers.push(
      blockerRef(workOrderRef, `stale-migration-evidence:${item.domainRef}`),
    )
  }

  if (item.required && (item.status === 'failed' || item.status === 'blocked')) {
    blockers.push(
      blockerRef(workOrderRef, `failed-required-migration:${item.domainRef}`),
    )
  }

  if (
    requiresRestoreBoundary &&
    item.restorePointRefs.length === 0 &&
    item.rollbackBoundaryRefs.length === 0
  ) {
    blockers.push(
      blockerRef(
        workOrderRef,
        `required-migration-missing-restore-boundary:${item.domainRef}`,
      ),
    )
  }

  if (item.status === 'completed' && item.validationRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `completed-migration-missing-validation:${item.domainRef}`),
    )
  }

  if (item.status === 'completed' && item.receiptRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `completed-migration-missing-receipt:${item.domainRef}`),
    )
  }

  if (
    item.optionalCache &&
    item.status === 'rebuildable' &&
    (item.optionalCacheRebuildRefs.length === 0 || item.policyRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(
        workOrderRef,
        `optional-cache-rebuild-policy-missing:${item.domainRef}`,
      ),
    )
  }

  if (
    item.downgradeRequired &&
    (item.downgradeRefs.length === 0 || item.policyRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `downgrade-policy-missing:${item.domainRef}`),
    )
  }

  if (item.redactionRefs.length === 0 && item.receiptRefs.length > 0) {
    blockers.push(
      blockerRef(workOrderRef, `migration-receipt-redaction-missing:${item.domainRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeMigrationEvidenceItem>,
  blockers: ReadonlyArray<string>,
): ForgeMigrationEvidenceStatus => {
  if (blockers.length > 0) {
    return 'blocked'
  }

  if (entries.length === 0) {
    return 'empty'
  }

  if (
    entries.some(
      entry =>
        entry.status === 'required' ||
        (entry.required &&
          ['pending', 'unknown'].includes(entry.status)),
    )
  ) {
    return 'required'
  }

  if (entries.some(entry => entry.status === 'failed')) {
    return 'failed'
  }

  if (
    entries.every(entry =>
      ['completed', 'rebuildable', 'rolled_back', 'skipped'].includes(
        entry.status,
      ),
    )
  ) {
    return 'ready'
  }

  return 'unknown'
}

export const projectForgeMigrationEvidence = (
  input: ForgeMigrationEvidenceInput,
): ForgeMigrationEvidenceView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const registryRefs = safeRefs(input.registryRefs)
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
    registryRefs.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedOmissions
  const blockers = [
    ...inputBlockerRefs.refs,
    ...entries.flatMap(entry => itemBlockers(input.workOrderRef, entry)),
  ]

  if (input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null) {
    blockers.push(
      blockerRef(input.workOrderRef, 'missing-migration-evidence-snapshot-ref'),
    )
  }

  if (omittedUnsafeRefCount > 0) {
    blockers.push(
      blockerRef(input.workOrderRef, 'unsafe-migration-evidence-material-omitted'),
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
    registryRefs: registryRefs.refs,
    snapshotRef: snapshotRef.ref,
    status: statusFrom(entries, uniqueBlockers),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeMigrationEvidenceInput = (
  work: AutopilotWorkProjection,
): ForgeMigrationEvidenceInput => {
  const migrationEvidence = work.migrationEvidence

  return {
    generatedAt: migrationEvidence?.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(migrationEvidence?.blockerRefs === undefined
      ? {}
      : { blockerRefs: migrationEvidence.blockerRefs }),
    ...(migrationEvidence?.entries === undefined
      ? {}
      : { entries: migrationEvidence.entries }),
    ...(migrationEvidence?.registryRefs === undefined
      ? {}
      : { registryRefs: migrationEvidence.registryRefs }),
    ...(migrationEvidence?.snapshotRef === undefined
      ? {}
      : { snapshotRef: migrationEvidence.snapshotRef }),
    ...(migrationEvidence?.versionRef === undefined
      ? {}
      : { versionRef: migrationEvidence.versionRef }),
  }
}
