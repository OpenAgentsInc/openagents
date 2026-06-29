import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

import type { ProviderAccountProvider } from './provider-account-domain'

export const PROVIDER_ACCOUNT_SECURITY_REVIEW_VERSION =
  'provider-account-security-review:v1' as const

const PROVIDER_ACCOUNT_SECURITY_REVIEW_COLLECTION =
  'provider_account_security_review_public'

export type ProviderAccountSecurityReviewScope =
  | 'account_connect'
  | 'lease_selection'
  | 'provider_peer_expansion'

export type ProviderAccountSecurityReviewStatus =
  | 'approved'
  | 'blocked'
  | 'scoped_exception'

export type ProviderAccountSecurityReviewInput = Readonly<{
  approvalRef?: string | undefined
  credentialBoundaryRef?: string | undefined
  debugBoundaryRef?: string | undefined
  denialRef?: string | undefined
  generatedAt: string
  highRiskFlow?: boolean | undefined
  incidentBoundaryRef?: string | undefined
  provider: ProviderAccountProvider
  redactionFixtureRefs?: ReadonlyArray<string> | undefined
  retentionPolicyRef?: string | undefined
  reviewRef: string
  revocationFixtureRefs?: ReadonlyArray<string> | undefined
  rollbackRef?: string | undefined
  scope: ProviderAccountSecurityReviewScope
  scopedExceptionRef?: string | undefined
  telemetryPrivacyRef?: string | undefined
  threatModelRef?: string | undefined
  tosReviewRef?: string | undefined
}>

export type ProviderAccountSecurityReviewProjection = Readonly<{
  generatedAt: string
  reviewVersion: typeof PROVIDER_ACCOUNT_SECURITY_REVIEW_VERSION
  reviewRef: string
  provider: ProviderAccountProvider
  scope: ProviderAccountSecurityReviewScope
  status: ProviderAccountSecurityReviewStatus
  tosReviewRef: string | null
  credentialBoundaryRef: string | null
  threatModelRef: string | null
  telemetryPrivacyRef: string | null
  retentionPolicyRef: string | null
  redactionFixtureRefs: ReadonlyArray<string>
  revocationFixtureRefs: ReadonlyArray<string>
  highRiskControlRefs: ReadonlyArray<string>
  scopedExceptionRef: string | null
  blockerRefs: ReadonlyArray<string>
}>

const refOrNull = (field: string, value: string | undefined): string | null => {
  if (value === undefined) {
    return null
  }

  assertNoProviderSecretMaterial(value, field)

  return value
}

const refs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  (values ?? []).map(value => {
    assertNoProviderSecretMaterial(value, field)

    return value
  })

const requiredRefBlockers = (
  reviewRef: string,
  input: ProviderAccountSecurityReviewInput,
): ReadonlyArray<string> => [
  ...(input.tosReviewRef === undefined
    ? [`provider-account-security-blocker:${reviewRef}:missing:tos_review`]
    : []),
  ...(input.credentialBoundaryRef === undefined
    ? [
        `provider-account-security-blocker:${reviewRef}:missing:credential_boundary`,
      ]
    : []),
  ...(input.threatModelRef === undefined
    ? [`provider-account-security-blocker:${reviewRef}:missing:threat_model`]
    : []),
  ...(input.telemetryPrivacyRef === undefined
    ? [
        `provider-account-security-blocker:${reviewRef}:missing:telemetry_privacy`,
      ]
    : []),
  ...(input.retentionPolicyRef === undefined
    ? [`provider-account-security-blocker:${reviewRef}:missing:retention_policy`]
    : []),
  ...((input.redactionFixtureRefs ?? []).length === 0
    ? [
        `provider-account-security-blocker:${reviewRef}:missing:redaction_fixture`,
      ]
    : []),
  ...((input.revocationFixtureRefs ?? []).length === 0
    ? [
        `provider-account-security-blocker:${reviewRef}:missing:revocation_fixture`,
      ]
    : []),
]

const highRiskBlockers = (
  reviewRef: string,
  input: ProviderAccountSecurityReviewInput,
): ReadonlyArray<string> =>
  input.highRiskFlow === true
    ? [
        ...(input.approvalRef === undefined
          ? [
              `provider-account-security-blocker:${reviewRef}:missing:approval_ref`,
            ]
          : []),
        ...(input.denialRef === undefined
          ? [`provider-account-security-blocker:${reviewRef}:missing:denial_ref`]
          : []),
        ...(input.rollbackRef === undefined
          ? [
              `provider-account-security-blocker:${reviewRef}:missing:rollback_ref`,
            ]
          : []),
        ...(input.incidentBoundaryRef === undefined
          ? [
              `provider-account-security-blocker:${reviewRef}:missing:incident_boundary`,
            ]
          : []),
        ...(input.debugBoundaryRef === undefined
          ? [
              `provider-account-security-blocker:${reviewRef}:missing:debug_boundary`,
            ]
          : []),
      ]
    : []

class ProviderAccountSecurityReviewMissingRef extends Error {
  constructor() {
    super('Provider account security review requires reviewRef.')
    this.name = 'ProviderAccountSecurityReviewMissingRef'
  }
}

export const reviewProviderAccountSecurityGate = (
  input: ProviderAccountSecurityReviewInput,
): ProviderAccountSecurityReviewProjection => {
  const reviewRef = refOrNull(
    'provider-account-security.reviewRef',
    input.reviewRef,
  )

  if (reviewRef === null) {
    throw new ProviderAccountSecurityReviewMissingRef()
  }

  const blockers = [
    ...requiredRefBlockers(reviewRef, input),
    ...highRiskBlockers(reviewRef, input),
  ]
  const scopedExceptionRef = refOrNull(
    'provider-account-security.scopedExceptionRef',
    input.scopedExceptionRef,
  )
  const projection: ProviderAccountSecurityReviewProjection = {
    generatedAt: input.generatedAt,
    reviewVersion: PROVIDER_ACCOUNT_SECURITY_REVIEW_VERSION,
    reviewRef,
    provider: input.provider,
    scope: input.scope,
    status:
      blockers.length === 0
        ? 'approved'
        : scopedExceptionRef === null
          ? 'blocked'
          : 'scoped_exception',
    tosReviewRef: refOrNull(
      'provider-account-security.tosReviewRef',
      input.tosReviewRef,
    ),
    credentialBoundaryRef: refOrNull(
      'provider-account-security.credentialBoundaryRef',
      input.credentialBoundaryRef,
    ),
    threatModelRef: refOrNull(
      'provider-account-security.threatModelRef',
      input.threatModelRef,
    ),
    telemetryPrivacyRef: refOrNull(
      'provider-account-security.telemetryPrivacyRef',
      input.telemetryPrivacyRef,
    ),
    retentionPolicyRef: refOrNull(
      'provider-account-security.retentionPolicyRef',
      input.retentionPolicyRef,
    ),
    redactionFixtureRefs: refs(
      'provider-account-security.redactionFixtureRefs',
      input.redactionFixtureRefs,
    ),
    revocationFixtureRefs: refs(
      'provider-account-security.revocationFixtureRefs',
      input.revocationFixtureRefs,
    ),
    highRiskControlRefs: refs('provider-account-security.highRiskControlRefs', [
      ...(input.approvalRef === undefined ? [] : [input.approvalRef]),
      ...(input.denialRef === undefined ? [] : [input.denialRef]),
      ...(input.rollbackRef === undefined ? [] : [input.rollbackRef]),
      ...(input.incidentBoundaryRef === undefined
        ? []
        : [input.incidentBoundaryRef]),
      ...(input.debugBoundaryRef === undefined ? [] : [input.debugBoundaryRef]),
    ]),
    scopedExceptionRef,
    blockerRefs: blockers,
  }

  assertNoProviderSecretMaterial(
    projection,
    PROVIDER_ACCOUNT_SECURITY_REVIEW_COLLECTION,
  )

  return projection
}
