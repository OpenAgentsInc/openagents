import { Schema as S } from 'effect'

import {
  OmniClassifiedSurface as OmniClassifiedSurfaceSchema,
  OmniDataClassification,
  OmniDataPolicyEnvelope,
  omniDataPolicyProjectionDecision,
  omniRequiredProviderEligibilityRefs,
} from './omni-data-classification'
import {
  OpenAgentsRunnerBackendKind,
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'

export const OpenAgentsProviderTrustTier = S.Literals([
  'blocked',
  'customer_visible',
  'internal_only',
  'legal_sensitive',
  'payment_private',
  'provider_private',
  'public',
  'reviewed_private',
])
export type OpenAgentsProviderTrustTier =
  typeof OpenAgentsProviderTrustTier.Type

export const OpenAgentsProviderState = S.Literals([
  'available',
  'blocked',
  'cooldown',
  'disabled',
  'draining',
])
export type OpenAgentsProviderState = typeof OpenAgentsProviderState.Type

export const OpenAgentsPlacementWorkKind = S.Literals([
  'agent_api_action',
  'artifact_build',
  'customer_asset_processing',
  'forum_payment',
  'forum_post',
  'legal_sensitive_work',
  'order',
  'payment_sensitive_action',
  'private_repo',
  'site',
  'site_revision',
])
export type OpenAgentsPlacementWorkKind =
  typeof OpenAgentsPlacementWorkKind.Type

export const OpenAgentsProviderPlacementDecision = S.Literals([
  'allowed',
  'denied',
])
export type OpenAgentsProviderPlacementDecision =
  typeof OpenAgentsProviderPlacementDecision.Type

export class OpenAgentsProviderPolicy extends S.Class<OpenAgentsProviderPolicy>(
  'OpenAgentsProviderPolicy',
)({
  allowedDataClassifications: S.Array(OmniDataClassification),
  allowedSurfaces: S.Array(OmniClassifiedSurfaceSchema),
  allowedWorkKinds: S.Array(OpenAgentsPlacementWorkKind),
  backendKind: OpenAgentsRunnerBackendKind,
  caveatRefs: S.Array(S.String),
  cooldownRefs: S.Array(S.String),
  disabledReasonRefs: S.Array(S.String),
  id: S.String,
  maxWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  policyRefs: S.Array(S.String),
  providerEligibilityRefs: S.Array(S.String),
  state: OpenAgentsProviderState,
  trustTier: OpenAgentsProviderTrustTier,
}) {}

export class OpenAgentsProviderPlacementRequest extends S.Class<OpenAgentsProviderPlacementRequest>(
  'OpenAgentsProviderPlacementRequest',
)({
  dataPolicy: OmniDataPolicyEnvelope,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  legalReviewRefs: S.Array(S.String),
  operatorApprovalRefs: S.Array(S.String),
  ownerGrantRefs: S.Array(S.String),
  paymentPolicyRefs: S.Array(S.String),
  policyExceptionRefs: S.Array(S.String),
  requestedBackendKind: OpenAgentsRunnerBackendKind,
  requiredWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  workKind: OpenAgentsPlacementWorkKind,
}) {}

export class OpenAgentsProviderPlacementRecord extends S.Class<OpenAgentsProviderPlacementRecord>(
  'OpenAgentsProviderPlacementRecord',
)({
  allowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  customerSafeBlockerRefs: S.Array(S.String),
  dataClassification: OmniDataClassification,
  decision: OpenAgentsProviderPlacementDecision,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  legalReviewRefs: S.Array(S.String),
  operatorApprovalRefs: S.Array(S.String),
  ownerGrantRefs: S.Array(S.String),
  paymentPolicyRefs: S.Array(S.String),
  policyExceptionRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  providerRef: S.String,
  requestedBackendKind: OpenAgentsRunnerBackendKind,
  requiredProviderEligibilityRefs: S.Array(S.String),
  requiredWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  state: OpenAgentsProviderState,
  surface: OmniClassifiedSurfaceSchema,
  trustTier: OpenAgentsProviderTrustTier,
  workKind: OpenAgentsPlacementWorkKind,
}) {}

export class OpenAgentsProviderPlacementProjection extends S.Class<OpenAgentsProviderPlacementProjection>(
  'OpenAgentsProviderPlacementProjection',
)({
  allowed: S.Boolean,
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  blockerRefs: S.Array(S.String),
  customerSafeBlockerRefs: S.Array(S.String),
  dataClassification: OmniDataClassification,
  decision: OpenAgentsProviderPlacementDecision,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  legalReviewRefs: S.Array(S.String),
  operatorApprovalRefs: S.Array(S.String),
  ownerGrantRefs: S.Array(S.String),
  paymentPolicyRefs: S.Array(S.String),
  policyExceptionRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  providerRef: S.String,
  requestedBackendKind: OpenAgentsRunnerBackendKind,
  requiredProviderEligibilityRefs: S.Array(S.String),
  requiredWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  state: OpenAgentsProviderState,
  surface: OmniClassifiedSurfaceSchema,
  trustTier: OpenAgentsProviderTrustTier,
  workKind: OpenAgentsPlacementWorkKind,
}) {}

export class OpenAgentsProviderPlacementUnsafe extends S.TaggedErrorClass<OpenAgentsProviderPlacementUnsafe>()(
  'OpenAgentsProviderPlacementUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeProviderPlacementPattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(legal[_-]?review|operator[_-]?approval|owner[_-]?grant|payment[_-]?policy|policy[_-]?exception|provider\.private|provider_account|workroom\.private)/i
const customerUnsafeRefPattern =
  /(legal[_-]?review|operator[_-]?approval|payment[_-]?policy|policy[_-]?exception|provider\.private|provider_account|workroom\.private)/i
const teamUnsafeRefPattern =
  /(payment[_-]?policy|provider\.private|provider_account|workroom\.private)/i

const workloadTrustRank: Readonly<Record<
  typeof OpenAgentsRunnerWorkloadTrust.Type,
  number
>> = {
  low: 0,
  medium: 1,
  sensitive: 2,
}

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeProviderPlacementPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsProviderPlacementUnsafe({
      reason: `${label} contains provider secrets, grants, raw auth state, raw capability payloads, private customer data, wallet/payment material, private repo refs, raw runner logs, raw source archives, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: 'public' | 'customer' | 'team' | 'operator',
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: 'public' | 'customer' | 'team' | 'operator',
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const providerTrustEligibilityRef = (
  trustTier: OpenAgentsProviderTrustTier,
): string =>
  trustTier === 'public'
    ? 'provider.eligibility.public'
    : trustTier === 'customer_visible'
      ? 'provider.eligibility.customer_visible'
      : trustTier === 'reviewed_private'
        ? 'provider.eligibility.reviewed_private'
        : trustTier === 'legal_sensitive'
          ? 'provider.eligibility.legal_sensitive'
          : trustTier === 'payment_private'
            ? 'provider.eligibility.payment_private'
            : trustTier === 'provider_private'
              ? 'provider.eligibility.provider_private'
              : trustTier === 'internal_only'
                ? 'provider.eligibility.no_external_provider'
                : 'provider.eligibility.blocked'

const requestRequiredProviderEligibilityRefs = (
  request: OpenAgentsProviderPlacementRequest,
): ReadonlyArray<string> =>
  omniRequiredProviderEligibilityRefs(request.dataPolicy)

const hardBlockerRefs = (
  provider: OpenAgentsProviderPolicy,
  request: OpenAgentsProviderPlacementRequest,
): ReadonlyArray<string> => {
  const refs: Array<string> = []
  const requiredProviderEligibilityRefs =
    requestRequiredProviderEligibilityRefs(request)

  if (provider.state === 'blocked') {
    refs.push('blocker.provider_placement.provider_blocked')
  }

  if (provider.state === 'disabled') {
    refs.push('blocker.provider_placement.provider_disabled')
  }

  if (provider.state === 'cooldown' || provider.state === 'draining') {
    refs.push('blocker.provider_placement.provider_not_available')
  }

  if (provider.trustTier === 'blocked') {
    refs.push('blocker.provider_placement.trust_tier_blocked')
  }

  if (
    requiredProviderEligibilityRefs.includes(
      'provider.eligibility.no_external_provider',
    ) &&
    provider.trustTier !== 'internal_only'
  ) {
    refs.push('blocker.provider_placement.no_external_provider_allowed')
  }

  return refs
}

const overrideableBlockerRefs = (
  provider: OpenAgentsProviderPolicy,
  request: OpenAgentsProviderPlacementRequest,
): ReadonlyArray<string> => {
  const refs: Array<string> = []
  const requiredProviderEligibilityRefs =
    requestRequiredProviderEligibilityRefs(request)
  const providerEligibilityRefs = uniqueRefs([
    ...provider.providerEligibilityRefs,
    providerTrustEligibilityRef(provider.trustTier),
  ])

  if (provider.backendKind !== request.requestedBackendKind) {
    refs.push('blocker.provider_placement.backend_not_allowed')
  }

  if (
    workloadTrustRank[provider.maxWorkloadTrust] <
    workloadTrustRank[request.requiredWorkloadTrust]
  ) {
    refs.push('blocker.provider_placement.workload_trust_too_low')
  }

  if (!provider.allowedWorkKinds.includes(request.workKind)) {
    refs.push('blocker.provider_placement.work_kind_not_allowed')
  }

  if (!provider.allowedSurfaces.includes(request.dataPolicy.surface)) {
    refs.push('blocker.provider_placement.surface_not_allowed')
  }

  if (
    !provider.allowedDataClassifications.includes(
      request.dataPolicy.dataClassification,
    )
  ) {
    refs.push('blocker.provider_placement.classification_not_allowed')
  }

  if (
    requiredProviderEligibilityRefs.some(
      ref => !providerEligibilityRefs.includes(ref),
    )
  ) {
    refs.push('blocker.provider_placement.provider_eligibility_missing')
  }

  if (
    request.workKind === 'private_repo' &&
    !hasRefs(request.ownerGrantRefs)
  ) {
    refs.push('blocker.provider_placement.owner_grant_required')
  }

  if (
    (request.workKind === 'legal_sensitive_work' ||
      request.dataPolicy.dataClassification === 'legal_sensitive') &&
    !hasRefs(request.legalReviewRefs) &&
    !hasRefs(request.operatorApprovalRefs)
  ) {
    refs.push('blocker.provider_placement.legal_review_required')
  }

  if (
    (request.workKind === 'payment_sensitive_action' ||
      request.dataPolicy.dataClassification === 'payment_private') &&
    !hasRefs(request.paymentPolicyRefs)
  ) {
    refs.push('blocker.provider_placement.payment_policy_required')
  }

  return refs
}

const customerSafeBlockerRefs = (
  blockerRefs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  uniqueRefs(blockerRefs.map(ref =>
    ref === 'blocker.provider_placement.owner_grant_required'
      ? 'reason.provider_placement.connection_needed'
      : ref === 'blocker.provider_placement.legal_review_required'
        ? 'reason.provider_placement.review_needed'
        : ref === 'blocker.provider_placement.payment_policy_required'
          ? 'reason.provider_placement.payment_policy_needed'
          : ref === 'blocker.provider_placement.provider_not_available' ||
              ref === 'blocker.provider_placement.provider_disabled' ||
              ref === 'blocker.provider_placement.provider_blocked'
            ? 'reason.provider_placement.provider_unavailable'
            : 'reason.provider_placement.not_available_for_this_context'
  ))

const assertProviderPlacementSafe = (
  provider: OpenAgentsProviderPolicy,
  request: OpenAgentsProviderPlacementRequest,
): void => {
  try {
    omniDataPolicyProjectionDecision(request.dataPolicy, 'operator')
  } catch {
    throw new OpenAgentsProviderPlacementUnsafe({
      reason: 'Provider placement data policy contains unsafe material.',
    })
  }

  assertSafeRefs('provider placement identity refs', [provider.id, request.id])
  assertSafeRefs('provider placement caveat refs', provider.caveatRefs)
  assertSafeRefs('provider placement cooldown refs', provider.cooldownRefs)
  assertSafeRefs(
    'provider placement disabled refs',
    provider.disabledReasonRefs,
  )
  assertSafeRefs('provider placement policy refs', provider.policyRefs)
  assertSafeRefs(
    'provider placement eligibility refs',
    provider.providerEligibilityRefs,
  )
  assertSafeRefs('provider placement evidence refs', request.evidenceRefs)
  assertSafeRefs(
    'provider placement legal review refs',
    request.legalReviewRefs,
  )
  assertSafeRefs(
    'provider placement operator approval refs',
    request.operatorApprovalRefs,
  )
  assertSafeRefs('provider placement owner grant refs', request.ownerGrantRefs)
  assertSafeRefs(
    'provider placement payment policy refs',
    request.paymentPolicyRefs,
  )
  assertSafeRefs(
    'provider placement policy exception refs',
    request.policyExceptionRefs,
  )
}

export const evaluateOpenAgentsProviderPlacement = (
  provider: OpenAgentsProviderPolicy,
  request: OpenAgentsProviderPlacementRequest,
): OpenAgentsProviderPlacementRecord => {
  assertProviderPlacementSafe(provider, request)

  const hard = hardBlockerRefs(provider, request)
  const overrideable = overrideableBlockerRefs(provider, request)
  const overrideApplied = hard.length === 0 &&
    overrideable.length > 0 &&
    hasRefs(request.policyExceptionRefs)
  const blockerRefs = overrideApplied ? [] : uniqueRefs([...hard, ...overrideable])
  const allowed = blockerRefs.length === 0

  return {
    allowed,
    blockerRefs,
    customerSafeBlockerRefs: customerSafeBlockerRefs(blockerRefs),
    dataClassification: request.dataPolicy.dataClassification,
    decision: allowed ? 'allowed' : 'denied',
    evidenceRefs: uniqueRefs(request.evidenceRefs),
    id: request.id,
    legalReviewRefs: uniqueRefs(request.legalReviewRefs),
    operatorApprovalRefs: uniqueRefs(request.operatorApprovalRefs),
    ownerGrantRefs: uniqueRefs(request.ownerGrantRefs),
    paymentPolicyRefs: uniqueRefs(request.paymentPolicyRefs),
    policyExceptionRefs: uniqueRefs(request.policyExceptionRefs),
    policyRefs: uniqueRefs([
      ...provider.policyRefs,
      ...provider.caveatRefs,
      ...(overrideApplied
        ? ['policy.provider_placement.explicit_exception_applied']
        : []),
    ]),
    providerRef: provider.id,
    requestedBackendKind: request.requestedBackendKind,
    requiredProviderEligibilityRefs:
      requestRequiredProviderEligibilityRefs(request),
    requiredWorkloadTrust: request.requiredWorkloadTrust,
    state: provider.state,
    surface: request.dataPolicy.surface,
    trustTier: provider.trustTier,
    workKind: request.workKind,
  }
}

export const projectOpenAgentsProviderPlacement = (
  record: OpenAgentsProviderPlacementRecord,
  audience: 'public' | 'customer' | 'team' | 'operator',
): OpenAgentsProviderPlacementProjection => {
  const projection: OpenAgentsProviderPlacementProjection = {
    allowed: record.allowed,
    audience,
    blockerRefs: safeRefsForAudience(
      'provider placement blocker refs',
      record.blockerRefs,
      audience,
    ),
    customerSafeBlockerRefs: safeRefsForAudience(
      'provider placement customer blocker refs',
      record.customerSafeBlockerRefs,
      audience,
    ),
    dataClassification: record.dataClassification,
    decision: record.decision,
    evidenceRefs:
      audience === 'operator'
        ? safeRefsForAudience(
            'provider placement evidence refs',
            record.evidenceRefs,
            audience,
          )
        : [],
    id: safeRefsForAudience(
      'provider placement id',
      [record.id],
      audience,
    )[0] ?? 'provider_placement.redacted',
    legalReviewRefs:
      audience === 'operator'
        ? safeRefsForAudience(
            'provider placement legal review refs',
            record.legalReviewRefs,
            audience,
          )
        : [],
    operatorApprovalRefs:
      audience === 'operator'
        ? safeRefsForAudience(
            'provider placement operator approval refs',
            record.operatorApprovalRefs,
            audience,
          )
        : [],
    ownerGrantRefs:
      audience === 'customer' || audience === 'team' || audience === 'operator'
        ? safeRefsForAudience(
            'provider placement owner grant refs',
            record.ownerGrantRefs,
            audience,
          )
        : [],
    paymentPolicyRefs:
      audience === 'operator'
        ? safeRefsForAudience(
            'provider placement payment policy refs',
            record.paymentPolicyRefs,
            audience,
          )
        : [],
    policyExceptionRefs:
      audience === 'operator'
        ? safeRefsForAudience(
            'provider placement exception refs',
            record.policyExceptionRefs,
            audience,
          )
        : [],
    policyRefs: safeRefsForAudience(
      'provider placement policy refs',
      record.policyRefs,
      audience,
    ),
    providerRef:
      audience === 'operator'
        ? safeRefsForAudience(
            'provider placement provider ref',
            [record.providerRef],
            audience,
          )[0] ?? 'provider.redacted'
        : 'provider.redacted',
    requestedBackendKind: record.requestedBackendKind,
    requiredProviderEligibilityRefs:
      audience === 'operator'
        ? safeRefsForAudience(
            'provider placement eligibility refs',
            record.requiredProviderEligibilityRefs,
            audience,
          )
        : [],
    requiredWorkloadTrust: record.requiredWorkloadTrust,
    state: record.state,
    surface: record.surface,
    trustTier: record.trustTier,
    workKind: record.workKind,
  }

  if (openAgentsProviderPlacementProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsProviderPlacementUnsafe({
      reason: 'Provider placement projection contains private material.',
    })
  }

  return projection
}

export const openAgentsProviderPlacementProjectionHasPrivateMaterial = (
  projection: OpenAgentsProviderPlacementProjection,
): boolean => unsafeProviderPlacementPattern.test(JSON.stringify(projection))
