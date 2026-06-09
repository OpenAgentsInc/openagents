import { Schema as S } from 'effect'

import {
  OmniDataClassification,
} from './omni-data-classification'
import {
  OpenAgentsRunnerBackendKind,
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'

export const CodingAutopilotRepoTrustTier = S.Literals([
  'infra',
  'legal_sensitive',
  'payment_sensitive',
  'private',
  'public',
  'regulated',
  'sensitive',
  'unknown',
])
export type CodingAutopilotRepoTrustTier =
  typeof CodingAutopilotRepoTrustTier.Type

export const CodingAutopilotRepoPlacementDecision = S.Literals([
  'blocked',
  'eligible',
  'needs_customer_grant',
  'needs_operator_approval',
  'needs_provider_grant',
])
export type CodingAutopilotRepoPlacementDecision =
  typeof CodingAutopilotRepoPlacementDecision.Type

export class CodingAutopilotRepoPlacementRequest extends S.Class<CodingAutopilotRepoPlacementRequest>(
  'CodingAutopilotRepoPlacementRequest',
)({
  customerGrantRefs: S.Array(S.String),
  dataClassification: OmniDataClassification,
  evidenceRefs: S.Array(S.String),
  evaluatedAtIso: S.String,
  id: S.String,
  missionRef: S.String,
  operatorApprovalRefs: S.Array(S.String),
  providerGrantRefs: S.Array(S.String),
  publicProofProjectionRefs: S.Array(S.String),
  repoRef: S.String,
  runnerBackendKind: OpenAgentsRunnerBackendKind,
  runnerWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  trustTier: CodingAutopilotRepoTrustTier,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotRepoPlacementRecord extends S.Class<CodingAutopilotRepoPlacementRecord>(
  'CodingAutopilotRepoPlacementRecord',
)({
  blockerRefs: S.Array(S.String),
  customerGrantRefs: S.Array(S.String),
  customerSafeBlockedReasonRefs: S.Array(S.String),
  dataClassification: OmniDataClassification,
  decision: CodingAutopilotRepoPlacementDecision,
  eligible: S.Boolean,
  evidenceRefs: S.Array(S.String),
  evaluatedAtIso: S.String,
  id: S.String,
  missionRef: S.String,
  operatorApprovalRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  providerGrantRefs: S.Array(S.String),
  publicClaimAllowed: S.Boolean,
  publicProofProjectionRefs: S.Array(S.String),
  repoRef: S.String,
  runnerBackendKind: OpenAgentsRunnerBackendKind,
  runnerWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  trustTier: CodingAutopilotRepoTrustTier,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotRepoPlacementProjection extends S.Class<CodingAutopilotRepoPlacementProjection>(
  'CodingAutopilotRepoPlacementProjection',
)({
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  blockerRefs: S.Array(S.String),
  customerGrantRefs: S.Array(S.String),
  customerSafeBlockedReasonRefs: S.Array(S.String),
  dataClassification: OmniDataClassification,
  decision: CodingAutopilotRepoPlacementDecision,
  eligible: S.Boolean,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  missionRef: S.String,
  operatorApprovalRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  providerGrantRefs: S.Array(S.String),
  publicClaimAllowed: S.Boolean,
  publicProofProjectionRefs: S.Array(S.String),
  repoRef: S.String,
  runnerBackendKind: OpenAgentsRunnerBackendKind,
  runnerWorkloadTrust: OpenAgentsRunnerWorkloadTrust,
  trustTier: CodingAutopilotRepoTrustTier,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotRepoPlacementUnsafe extends S.TaggedErrorClass<CodingAutopilotRepoPlacementUnsafe>()(
  'CodingAutopilotRepoPlacementUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|payload|token)|raw[_-]?(email|invoice|payment|payload|patch|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet|webhook[_-]?secret|workroom[_-]?private)/i
const publicUnsafeRefPattern =
  /(customer[_-]?grant|operator[_-]?approval|provider[_-]?grant|repo\.private|workroom\.)/i
const customerUnsafeRefPattern =
  /(operator[_-]?approval|provider[_-]?grant|repo\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(provider[_-]?grant|repo\.private|workroom\.private)/i

const privateLikeTiers = new Set<CodingAutopilotRepoTrustTier>([
  'infra',
  'legal_sensitive',
  'payment_sensitive',
  'private',
  'regulated',
  'sensitive',
])
const operatorApprovalTiers = new Set<CodingAutopilotRepoTrustTier>([
  'infra',
  'legal_sensitive',
  'payment_sensitive',
  'regulated',
  'sensitive',
])
const shcOnlyTiers = new Set<CodingAutopilotRepoTrustTier>([
  'legal_sensitive',
  'payment_sensitive',
  'regulated',
])

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new CodingAutopilotRepoPlacementUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw artifact material.`,
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

const classificationIsBlocked = (
  classification: typeof OmniDataClassification.Type,
): boolean => classification === 'secret_bearing' ||
  classification === 'provider_private'

const needsProviderGrant = (
  request: CodingAutopilotRepoPlacementRequest,
): boolean => request.trustTier === 'payment_sensitive' ||
  request.dataClassification === 'payment_private'

const publicClaimAllowed = (
  request: CodingAutopilotRepoPlacementRequest,
  decision: CodingAutopilotRepoPlacementDecision,
): boolean =>
  decision === 'eligible' &&
  request.trustTier === 'public' &&
  request.dataClassification === 'public'

const placementDecision = (
  request: CodingAutopilotRepoPlacementRequest,
): CodingAutopilotRepoPlacementDecision => {
  if (request.trustTier === 'unknown' || classificationIsBlocked(request.dataClassification)) {
    return 'blocked'
  }

  if (
    shcOnlyTiers.has(request.trustTier) &&
    request.runnerBackendKind !== 'shc_vm'
  ) {
    return 'blocked'
  }

  if (
    privateLikeTiers.has(request.trustTier) &&
    request.customerGrantRefs.length === 0
  ) {
    return 'needs_customer_grant'
  }

  if (
    operatorApprovalTiers.has(request.trustTier) &&
    request.operatorApprovalRefs.length === 0
  ) {
    return 'needs_operator_approval'
  }

  if (needsProviderGrant(request) && request.providerGrantRefs.length === 0) {
    return 'needs_provider_grant'
  }

  return 'eligible'
}

const blockerRefsForDecision = (
  request: CodingAutopilotRepoPlacementRequest,
  decision: CodingAutopilotRepoPlacementDecision,
): ReadonlyArray<string> => {
  if (decision === 'eligible') {
    return []
  }

  if (decision === 'needs_customer_grant') {
    return ['blocker.repo_placement.customer_grant_required']
  }

  if (decision === 'needs_operator_approval') {
    return ['blocker.repo_placement.operator_approval_required']
  }

  if (decision === 'needs_provider_grant') {
    return ['blocker.repo_placement.provider_grant_required']
  }

  if (classificationIsBlocked(request.dataClassification)) {
    return ['blocker.repo_placement.classification_not_placeable']
  }

  if (
    shcOnlyTiers.has(request.trustTier) &&
    request.runnerBackendKind !== 'shc_vm'
  ) {
    return ['blocker.repo_placement.backend_not_allowed_for_trust_tier']
  }

  return ['blocker.repo_placement.trust_tier_unknown']
}

const customerSafeReasonsForDecision = (
  decision: CodingAutopilotRepoPlacementDecision,
): ReadonlyArray<string> => {
  if (decision === 'eligible') {
    return []
  }

  if (decision === 'needs_customer_grant') {
    return ['reason.repo_placement.customer_connection_needed']
  }

  if (decision === 'needs_operator_approval') {
    return ['reason.repo_placement.operator_review_needed']
  }

  if (decision === 'needs_provider_grant') {
    return ['reason.repo_placement.runner_capacity_needed']
  }

  return ['reason.repo_placement.not_available_for_this_context']
}

const assertRequestSafe = (
  request: CodingAutopilotRepoPlacementRequest,
): void => {
  assertSafeRefs('repo placement identity refs', [
    request.id,
    request.repoRef,
    request.missionRef,
  ])
  assertSafeRefs('repo placement workroom refs', request.workroomRefs)
  assertSafeRefs('repo placement customer grant refs', request.customerGrantRefs)
  assertSafeRefs('repo placement provider grant refs', request.providerGrantRefs)
  assertSafeRefs('repo placement operator approval refs', request.operatorApprovalRefs)
  assertSafeRefs('repo placement evidence refs', request.evidenceRefs)
  assertSafeRefs('repo placement proof refs', request.publicProofProjectionRefs)
}

export const evaluateCodingAutopilotRepoPlacement = (
  request: CodingAutopilotRepoPlacementRequest,
): CodingAutopilotRepoPlacementRecord => {
  assertRequestSafe(request)

  const decision = placementDecision(request)

  return {
    blockerRefs: blockerRefsForDecision(request, decision),
    customerGrantRefs: uniqueRefs(request.customerGrantRefs),
    customerSafeBlockedReasonRefs: customerSafeReasonsForDecision(decision),
    dataClassification: request.dataClassification,
    decision,
    eligible: decision === 'eligible',
    evidenceRefs: uniqueRefs(request.evidenceRefs),
    evaluatedAtIso: request.evaluatedAtIso,
    id: request.id,
    missionRef: request.missionRef,
    operatorApprovalRefs: uniqueRefs(request.operatorApprovalRefs),
    policyRefs: [
      `policy.repo_trust.${request.trustTier}`,
      `policy.repo_placement.${request.runnerBackendKind}`,
      `policy.data_classification.${request.dataClassification}`,
    ],
    providerGrantRefs: uniqueRefs(request.providerGrantRefs),
    publicClaimAllowed: publicClaimAllowed(request, decision),
    publicProofProjectionRefs: uniqueRefs(request.publicProofProjectionRefs),
    repoRef: request.repoRef,
    runnerBackendKind: request.runnerBackendKind,
    runnerWorkloadTrust: request.runnerWorkloadTrust,
    trustTier: request.trustTier,
    workroomRefs: uniqueRefs(request.workroomRefs),
  }
}

export const codingAutopilotRepoPlacementProjectionHasPrivateMaterial = (
  projection: CodingAutopilotRepoPlacementProjection,
): boolean => universallyUnsafeRefPattern.test(JSON.stringify(projection))

export const projectCodingAutopilotRepoPlacement = (
  record: CodingAutopilotRepoPlacementRecord,
  audience: 'public' | 'customer' | 'team' | 'operator',
): CodingAutopilotRepoPlacementProjection => {
  const projection: CodingAutopilotRepoPlacementProjection = {
    audience,
    blockerRefs: safeRefsForAudience('repo placement blocker refs', record.blockerRefs, audience),
    customerGrantRefs: audience === 'customer' ||
      audience === 'team' ||
      audience === 'operator'
      ? safeRefsForAudience('repo placement customer grant refs', record.customerGrantRefs, audience)
      : [],
    customerSafeBlockedReasonRefs: safeRefsForAudience(
      'repo placement customer reasons',
      record.customerSafeBlockedReasonRefs,
      audience,
    ),
    dataClassification: record.dataClassification,
    decision: record.decision,
    eligible: record.eligible,
    evidenceRefs: safeRefsForAudience('repo placement evidence refs', record.evidenceRefs, audience),
    id: record.id,
    missionRef: record.missionRef,
    operatorApprovalRefs: audience === 'team' || audience === 'operator'
      ? safeRefsForAudience('repo placement operator approval refs', record.operatorApprovalRefs, audience)
      : [],
    policyRefs: safeRefsForAudience('repo placement policy refs', record.policyRefs, audience),
    providerGrantRefs: audience === 'operator'
      ? safeRefsForAudience('repo placement provider grant refs', record.providerGrantRefs, audience)
      : [],
    publicClaimAllowed: record.publicClaimAllowed,
    publicProofProjectionRefs: safeRefsForAudience(
      'repo placement proof refs',
      record.publicProofProjectionRefs,
      audience,
    ),
    repoRef: record.trustTier === 'public' || audience === 'operator'
      ? safeRefsForAudience('repo placement repo ref', [record.repoRef], audience)[0] ??
        'repo.redacted'
      : 'repo.redacted',
    runnerBackendKind: record.runnerBackendKind,
    runnerWorkloadTrust: record.runnerWorkloadTrust,
    trustTier: record.trustTier,
    workroomRefs: audience === 'public'
      ? []
      : safeRefsForAudience('repo placement workroom refs', record.workroomRefs, audience),
  }

  if (codingAutopilotRepoPlacementProjectionHasPrivateMaterial(projection)) {
    throw new CodingAutopilotRepoPlacementUnsafe({
      reason: 'Repo placement projection contains private material.',
    })
  }

  return projection
}
