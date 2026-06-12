import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

export const PROVIDER_ACCOUNT_RETENTION_POLICY_VERSION =
  'provider-account-retention-policy:v1' as const

const PROVIDER_ACCOUNT_RETENTION_POLICY_COLLECTION =
  'provider_account_retention_policy_public'

const RETENTION_PRIVATE_MARKERS: ReadonlyArray<RegExp> = [
  /raw[_ -]prompt/i,
  /raw[_ -]provider[_ -]response/i,
  /private[_ -]repo/i,
  /shell[_ -]output/i,
  /transcript:/i,
  /\b\/Users\/[^/]+\/work\//,
  /git@github\.com:[^\s]+/,
]

export const PACK_B_RETENTION_DATA_CLASSES = [
  'credential',
  'account_lease',
  'account_health_telemetry',
  'provider_routing_decision',
  'policy_snapshot',
  'reconnect_state',
  'debug_support_record',
  'artifact',
  'receipt',
] as const

export type ProviderAccountRetentionDataClass =
  (typeof PACK_B_RETENTION_DATA_CLASSES)[number]

export type ProviderAccountRetentionClass =
  | 'ephemeral'
  | 'short'
  | 'standard'
  | 'audit'
  | 'receipt'

export type ProviderAccountDeletionBehavior =
  | 'delete'
  | 'redact_and_tombstone'
  | 'retain_ref_only'
  | 'revoke'

export type ProviderAccountProjectionInvalidationBehavior =
  | 'invalidate_cache'
  | 'rebuild_projection'
  | 'retain_audit_ref'
  | 'typed_blocker'

export type ProviderAccountDeletionSubjectType =
  | 'provider_account'
  | 'team'
  | 'user'
  | 'credential'

export type ProviderAccountDeletionReason =
  | 'account_deleted'
  | 'credential_revoked'
  | 'retention_expired'
  | 'team_deleted'
  | 'user_deleted'

export type ProviderAccountRetentionDataClassPolicy = Readonly<{
  auditRef?: string | undefined
  dataClass: ProviderAccountRetentionDataClass
  deletionBehavior: ProviderAccountDeletionBehavior
  projectionInvalidation: ProviderAccountProjectionInvalidationBehavior
  retentionClass: ProviderAccountRetentionClass
  ttlDays: number | null
}>

export type ProviderAccountRetentionPolicyInput = Readonly<{
  affectedArtifactRefs?: ReadonlyArray<string> | undefined
  affectedLeaseRefs?: ReadonlyArray<string> | undefined
  affectedProjectionRefs?: ReadonlyArray<string> | undefined
  affectedReceiptRefs?: ReadonlyArray<string> | undefined
  dataClassPolicies: ReadonlyArray<ProviderAccountRetentionDataClassPolicy>
  deletionReceiptRefs?: ReadonlyArray<string> | undefined
  generatedAt: string
  policyRef: string
  providerAccountRef?: string | undefined
  reason: ProviderAccountDeletionReason
  retainedAuditRefs?: ReadonlyArray<string> | undefined
  subjectRef: string
  subjectType: ProviderAccountDeletionSubjectType
  tombstoneRefs?: ReadonlyArray<string> | undefined
}>

export type ProviderAccountRetentionDataClassProjection =
  Omit<ProviderAccountRetentionDataClassPolicy, 'auditRef'> & Readonly<{
    auditRef: string | null
  }>

export type ProviderAccountRetentionPolicyProjection = Readonly<{
  generatedAt: string
  retentionVersion: typeof PROVIDER_ACCOUNT_RETENTION_POLICY_VERSION
  policyRef: string
  subjectRef: string
  subjectType: ProviderAccountDeletionSubjectType
  reason: ProviderAccountDeletionReason
  status: 'declared' | 'blocked'
  dataClassPolicies: ReadonlyArray<ProviderAccountRetentionDataClassProjection>
  missingDataClassRefs: ReadonlyArray<string>
  leaseInvalidationRefs: ReadonlyArray<string>
  cacheInvalidationRefs: ReadonlyArray<string>
  dependentBlockerRefs: ReadonlyArray<string>
  reconnectActionRef: string | null
  tombstoneRefs: ReadonlyArray<string>
  deletionReceiptRefs: ReadonlyArray<string>
  retainedAuditRefs: ReadonlyArray<string>
  affectedArtifactRefs: ReadonlyArray<string>
  affectedReceiptRefs: ReadonlyArray<string>
}>

class ProviderAccountRetentionPolicyUnsafe extends Error {
  constructor(context: string) {
    super(`${context} contains private retention material.`)
    this.name = 'ProviderAccountRetentionPolicyUnsafe'
  }
}

const assertNoPrivateRetentionMaterial = (
  value: unknown,
  context: string,
): void => {
  assertNoProviderSecretMaterial(value, context)

  const json = typeof value === 'string' ? value : JSON.stringify(value)

  if (RETENTION_PRIVATE_MARKERS.some(marker => marker.test(json))) {
    throw new ProviderAccountRetentionPolicyUnsafe(context)
  }
}

const safeRef = (field: string, value: string): string => {
  assertNoPrivateRetentionMaterial(value, field)

  return value.trim()
}

const safeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => (values ?? []).map(value => safeRef(field, value))

const policyProjection = (
  policy: ProviderAccountRetentionDataClassPolicy,
): ProviderAccountRetentionDataClassProjection => {
  assertNoPrivateRetentionMaterial(
    policy,
    'provider-account-retention.dataClassPolicy',
  )

  return {
    ...policy,
    auditRef:
      policy.auditRef === undefined
        ? null
        : safeRef('provider-account-retention.auditRef', policy.auditRef),
  }
}

const invalidatesDependentLeases = (
  reason: ProviderAccountDeletionReason,
): boolean =>
  reason === 'account_deleted' ||
  reason === 'credential_revoked' ||
  reason === 'team_deleted' ||
  reason === 'user_deleted'

export const projectProviderAccountRetentionPolicy = (
  input: ProviderAccountRetentionPolicyInput,
): ProviderAccountRetentionPolicyProjection => {
  const policyRef = safeRef('provider-account-retention.policyRef', input.policyRef)
  const subjectRef = safeRef(
    'provider-account-retention.subjectRef',
    input.subjectRef,
  )
  const declaredClasses = new Set(
    input.dataClassPolicies.map(policy => policy.dataClass),
  )
  const missingDataClassRefs = PACK_B_RETENTION_DATA_CLASSES.filter(
    dataClass => !declaredClasses.has(dataClass),
  ).map(
    dataClass =>
      `provider-account-retention-blocker:${policyRef}:missing:${dataClass}`,
  )
  const leaseInvalidationRefs = invalidatesDependentLeases(input.reason)
    ? safeRefs(
        'provider-account-retention.affectedLeaseRefs',
        input.affectedLeaseRefs,
      ).map(leaseRef => `provider-account-lease-invalidation:${leaseRef}`)
    : []
  const cacheInvalidationRefs = [
    ...safeRefs(
      'provider-account-retention.affectedProjectionRefs',
      input.affectedProjectionRefs,
    ).map(ref => `provider-account-projection-cache:${ref}`),
    ...(input.providerAccountRef === undefined
      ? []
      : [
          `provider-account-cache:${safeRef(
            'provider-account-retention.providerAccountRef',
            input.providerAccountRef,
          )}`,
        ]),
  ]
  const dependentBlockerRefs = invalidatesDependentLeases(input.reason)
    ? safeRefs(
        'provider-account-retention.affectedLeaseRefs',
        input.affectedLeaseRefs,
      ).map(
        leaseRef =>
          `provider-account-retention-dependent-blocker:${leaseRef}:${input.reason}`,
      )
    : []
  const projection: ProviderAccountRetentionPolicyProjection = {
    generatedAt: input.generatedAt,
    retentionVersion: PROVIDER_ACCOUNT_RETENTION_POLICY_VERSION,
    policyRef,
    subjectRef,
    subjectType: input.subjectType,
    reason: input.reason,
    status: missingDataClassRefs.length === 0 ? 'declared' : 'blocked',
    dataClassPolicies: input.dataClassPolicies
      .map(policyProjection)
      .sort((left, right) => left.dataClass.localeCompare(right.dataClass)),
    missingDataClassRefs,
    leaseInvalidationRefs,
    cacheInvalidationRefs,
    dependentBlockerRefs,
    reconnectActionRef:
      input.providerAccountRef === undefined || leaseInvalidationRefs.length === 0
        ? null
        : `provider-account-reconnect:${safeRef(
            'provider-account-retention.providerAccountRef',
            input.providerAccountRef,
          )}`,
    tombstoneRefs: safeRefs(
      'provider-account-retention.tombstoneRefs',
      input.tombstoneRefs,
    ),
    deletionReceiptRefs: safeRefs(
      'provider-account-retention.deletionReceiptRefs',
      input.deletionReceiptRefs,
    ),
    retainedAuditRefs: safeRefs(
      'provider-account-retention.retainedAuditRefs',
      input.retainedAuditRefs,
    ),
    affectedArtifactRefs: safeRefs(
      'provider-account-retention.affectedArtifactRefs',
      input.affectedArtifactRefs,
    ),
    affectedReceiptRefs: safeRefs(
      'provider-account-retention.affectedReceiptRefs',
      input.affectedReceiptRefs,
    ),
  }

  assertNoPrivateRetentionMaterial(
    projection,
    PROVIDER_ACCOUNT_RETENTION_POLICY_COLLECTION,
  )

  return projection
}
