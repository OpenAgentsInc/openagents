import { Schema as S } from 'effect'

export const OmniDataClassification = S.Literals([
  'public',
  'customer',
  'team',
  'operator',
  'private',
  'legal_sensitive',
  'provider_private',
  'payment_private',
  'secret_bearing',
  'deletion_retention_sensitive',
])
export type OmniDataClassification = typeof OmniDataClassification.Type

export const OmniTrustTier = S.Literals([
  'verified',
  'reviewed',
  'unverified',
  'blocked',
])
export type OmniTrustTier = typeof OmniTrustTier.Type

export const OmniProjectionAudience = S.Literals([
  'public',
  'customer',
  'agent',
  'team',
  'operator',
  'private',
])
export type OmniProjectionAudience = typeof OmniProjectionAudience.Type

export const OmniClassifiedSurface = S.Literals([
  'agent_api_payload',
  'artifact',
  'customer_asset',
  'evidence_bundle',
  'forum_payment_ref',
  'forum_post',
  'forum_receipt',
  'forum_topic',
  'order',
  'payment_ref',
  'provider_account',
  'receipt',
  'site',
  'site_revision',
  'task_packet',
  'workroom',
])
export type OmniClassifiedSurface = typeof OmniClassifiedSurface.Type

export const OmniProjectionPolicyDecision = S.Literals([
  'allow',
  'deny',
  'omit',
  'redact',
])
export type OmniProjectionPolicyDecision =
  typeof OmniProjectionPolicyDecision.Type

export class OmniDataPolicyEnvelope extends S.Class<OmniDataPolicyEnvelope>(
  'OmniDataPolicyEnvelope',
)({
  classificationCaveatRef: S.String,
  dataClassification: OmniDataClassification,
  evidenceRefs: S.Array(S.String),
  exportPolicyRefs: S.Array(S.String),
  providerEligibilityRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  retentionPolicyRefs: S.Array(S.String),
  subjectRef: S.String,
  surface: OmniClassifiedSurface,
  trustTier: OmniTrustTier,
}) {}

export class OmniDataPolicyProjection extends S.Class<OmniDataPolicyProjection>(
  'OmniDataPolicyProjection',
)({
  audience: OmniProjectionAudience,
  classificationCaveatRef: S.String,
  dataClassification: OmniDataClassification,
  decision: OmniProjectionPolicyDecision,
  evidenceRefs: S.Array(S.String),
  exportAllowed: S.Boolean,
  exportPolicyRefs: S.Array(S.String),
  providerEligibilityRefs: S.Array(S.String),
  redactionPolicyRefs: S.Array(S.String),
  retentionPolicyRefs: S.Array(S.String),
  subjectRef: S.NullOr(S.String),
  surface: OmniClassifiedSurface,
  trustTier: OmniTrustTier,
}) {}

export const OmniClassifiedRecord = S.Struct({
  classificationCaveatRef: S.String,
  dataClassification: OmniDataClassification,
  trustTier: OmniTrustTier,
})
export type OmniClassifiedRecord = typeof OmniClassifiedRecord.Type

export class OmniDataClassificationValidationError extends S.TaggedErrorClass<OmniDataClassificationValidationError>()(
  'OmniDataClassificationValidationError',
  { reason: S.String },
) {}

const CLASSIFICATION_RANK: Readonly<Record<OmniDataClassification, number>> = {
  public: 0,
  customer: 1,
  team: 1,
  operator: 2,
  private: 3,
  legal_sensitive: 4,
  provider_private: 4,
  payment_private: 4,
  secret_bearing: 5,
  deletion_retention_sensitive: 5,
}

const AUDIENCE_ALLOWED: Readonly<
  Record<OmniProjectionAudience, ReadonlySet<OmniDataClassification>>
> = {
  agent: new Set(['public']),
  customer: new Set(['public', 'customer']),
  operator: new Set([
    'public',
    'customer',
    'team',
    'operator',
    'private',
    'legal_sensitive',
    'provider_private',
    'payment_private',
  ]),
  private: new Set([
    'public',
    'customer',
    'team',
    'operator',
    'private',
    'legal_sensitive',
    'provider_private',
    'payment_private',
    'secret_bearing',
    'deletion_retention_sensitive',
  ]),
  public: new Set(['public']),
  team: new Set(['public', 'team', 'operator']),
}

const safeDataPolicyRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeDataPolicyRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(grant|payload|token)|raw[_-]?(email|invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const exportDenyRefPattern =
  /(export\.deny|export\.never|retention\.delete_requested)/i
const explicitSensitiveExportRefPattern =
  /(export\.operator_safe|export\.redacted|export\.customer_safe|export\.team_safe)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const assertSafeDataPolicyRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeDataPolicyRefPattern.test(ref) ||
    unsafeDataPolicyRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OmniDataClassificationValidationError({
      reason: `${label} contains private customer data, provider grants, callback tokens, wallet material, payment proofs, raw runner logs, private repo refs, raw source archives, or raw timestamps.`,
    })
  }
}

const sensitiveExportClassifications: ReadonlySet<OmniDataClassification> =
  new Set([
    'legal_sensitive',
    'payment_private',
    'private',
    'provider_private',
  ])

export const isOmniClassificationMoreRestrictive = (
  next: OmniDataClassification,
  current: OmniDataClassification,
): boolean => CLASSIFICATION_RANK[next] > CLASSIFICATION_RANK[current]

export const isOmniClassificationDowngrade = (
  next: OmniDataClassification,
  current: OmniDataClassification,
): boolean => CLASSIFICATION_RANK[next] < CLASSIFICATION_RANK[current]

export const validateOmniClassificationTransition = (
  current: OmniDataClassification,
  next: OmniDataClassification,
  redactionEvidenceRef?: string | undefined,
): void => {
  if (!isOmniClassificationDowngrade(next, current)) {
    return
  }

  if (current === 'secret_bearing') {
    throw new OmniDataClassificationValidationError({
      reason: 'secret-bearing data cannot be downgraded by classification policy.',
    })
  }

  if (
    ['legal_sensitive', 'payment_private', 'provider_private'].includes(
      current,
    ) &&
    redactionEvidenceRef === undefined
  ) {
    throw new OmniDataClassificationValidationError({
      reason:
        'legal, payment, and provider-private data require redaction evidence before downgrade.',
    })
  }
}

export const canProjectOmniClassifiedRecord = (
  record: OmniClassifiedRecord,
  audience: OmniProjectionAudience,
): boolean =>
  record.trustTier !== 'blocked' &&
  AUDIENCE_ALLOWED[audience].has(record.dataClassification)

export const assertOmniClassifiedProjectionAllowed = (
  record: OmniClassifiedRecord,
  audience: OmniProjectionAudience,
): void => {
  if (!canProjectOmniClassifiedRecord(record, audience)) {
    throw new OmniDataClassificationValidationError({
      reason: `${record.dataClassification} data with ${record.trustTier} trust cannot project to ${audience}.`,
    })
  }
}

export const omniClassificationProjection = (
  record: OmniClassifiedRecord,
  audience: OmniProjectionAudience,
) => {
  assertOmniClassifiedProjectionAllowed(record, audience)

  return {
    classificationCaveatRef: record.classificationCaveatRef,
    dataClassification: record.dataClassification,
    trustTier: record.trustTier,
  }
}

const assertOmniDataPolicyEnvelopeSafe = (
  record: OmniDataPolicyEnvelope,
): void => {
  assertSafeDataPolicyRefs('Omni data policy subject refs', [
    record.subjectRef,
  ])
  assertSafeDataPolicyRefs('Omni data policy caveat refs', [
    record.classificationCaveatRef,
  ])
  assertSafeDataPolicyRefs('Omni data policy evidence refs', record.evidenceRefs)
  assertSafeDataPolicyRefs(
    'Omni data policy export refs',
    record.exportPolicyRefs,
  )
  assertSafeDataPolicyRefs(
    'Omni data policy provider eligibility refs',
    record.providerEligibilityRefs,
  )
  assertSafeDataPolicyRefs(
    'Omni data policy redaction refs',
    record.redactionPolicyRefs,
  )
  assertSafeDataPolicyRefs(
    'Omni data policy retention refs',
    record.retentionPolicyRefs,
  )
}

export const omniDataPolicyProjectionDecision = (
  record: OmniDataPolicyEnvelope,
  audience: OmniProjectionAudience,
): OmniProjectionPolicyDecision => {
  assertOmniDataPolicyEnvelopeSafe(record)

  if (record.trustTier === 'blocked') {
    return 'deny'
  }

  if (
    record.dataClassification === 'secret_bearing' &&
    audience !== 'private'
  ) {
    return 'deny'
  }

  if (
    record.dataClassification === 'deletion_retention_sensitive' &&
    audience !== 'private'
  ) {
    return hasRefs(record.redactionPolicyRefs) ? 'redact' : 'deny'
  }

  if (AUDIENCE_ALLOWED[audience].has(record.dataClassification)) {
    return 'allow'
  }

  return hasRefs(record.redactionPolicyRefs) ? 'redact' : 'omit'
}

export const omniRequiredProviderEligibilityRefs = (
  record: OmniDataPolicyEnvelope,
): ReadonlyArray<string> => {
  const required =
    record.dataClassification === 'public'
      ? 'provider.eligibility.public'
      : record.dataClassification === 'customer'
        ? 'provider.eligibility.customer_visible'
        : record.dataClassification === 'team' ||
            record.dataClassification === 'operator' ||
            record.dataClassification === 'private'
          ? 'provider.eligibility.reviewed_private'
          : record.dataClassification === 'legal_sensitive'
            ? 'provider.eligibility.legal_sensitive'
            : record.dataClassification === 'provider_private'
              ? 'provider.eligibility.provider_private'
              : record.dataClassification === 'payment_private'
                ? 'provider.eligibility.payment_private'
                : 'provider.eligibility.no_external_provider'

  return uniqueRefs([...record.providerEligibilityRefs, required])
}

export const omniDataPolicyExportAllowed = (
  record: OmniDataPolicyEnvelope,
  audience: OmniProjectionAudience,
): boolean => {
  const decision = omniDataPolicyProjectionDecision(record, audience)

  if (decision !== 'allow') {
    return false
  }

  if (
    record.exportPolicyRefs.some(ref => exportDenyRefPattern.test(ref)) ||
    record.retentionPolicyRefs.some(ref => exportDenyRefPattern.test(ref))
  ) {
    return false
  }

  if (
    record.dataClassification === 'secret_bearing' ||
    record.dataClassification === 'deletion_retention_sensitive'
  ) {
    return false
  }

  if (
    sensitiveExportClassifications.has(record.dataClassification) &&
    !record.exportPolicyRefs.some(ref =>
      explicitSensitiveExportRefPattern.test(ref)
    )
  ) {
    return false
  }

  return true
}

export const projectOmniDataPolicyEnvelope = (
  record: OmniDataPolicyEnvelope,
  audience: OmniProjectionAudience,
): OmniDataPolicyProjection => {
  assertOmniDataPolicyEnvelopeSafe(record)

  const decision = omniDataPolicyProjectionDecision(record, audience)
  const included = decision === 'allow'
  const redacted = decision === 'redact'

  return {
    audience,
    classificationCaveatRef: record.classificationCaveatRef,
    dataClassification: record.dataClassification,
    decision,
    evidenceRefs: included ? uniqueRefs(record.evidenceRefs) : [],
    exportAllowed: omniDataPolicyExportAllowed(record, audience),
    exportPolicyRefs:
      decision === 'deny' ? [] : uniqueRefs(record.exportPolicyRefs),
    providerEligibilityRefs:
      decision === 'deny' ? [] : omniRequiredProviderEligibilityRefs(record),
    redactionPolicyRefs:
      redacted || included ? uniqueRefs(record.redactionPolicyRefs) : [],
    retentionPolicyRefs:
      decision === 'deny' ? [] : uniqueRefs(record.retentionPolicyRefs),
    subjectRef: included
      ? record.subjectRef
      : redacted
        ? `${record.surface}.redacted`
        : null,
    surface: record.surface,
    trustTier: record.trustTier,
  }
}
