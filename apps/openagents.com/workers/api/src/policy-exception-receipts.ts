import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'

export const OpenAgentsPolicyExceptionFamily = S.Literals([
  'access_control',
  'email_delivery',
  'environment_secret_policy',
  'forum_moderation',
  'legal_sensitive_rule',
  'payment_l402',
  'provider_placement',
  'public_proof',
  'research_policy',
  'site_deployment',
])
export type OpenAgentsPolicyExceptionFamily =
  typeof OpenAgentsPolicyExceptionFamily.Type

export const OpenAgentsPolicyExceptionReviewState = S.Literals([
  'approved',
  'rejected',
  'requested',
  'revoked',
])
export type OpenAgentsPolicyExceptionReviewState =
  typeof OpenAgentsPolicyExceptionReviewState.Type

export const OpenAgentsPolicyExceptionAuthorityBoundary = S.Literals([
  'evidence_only',
])
export type OpenAgentsPolicyExceptionAuthorityBoundary =
  typeof OpenAgentsPolicyExceptionAuthorityBoundary.Type

export class OpenAgentsPolicyExceptionAuthority extends S.Class<OpenAgentsPolicyExceptionAuthority>(
  'OpenAgentsPolicyExceptionAuthority',
)({
  authorityBoundary: OpenAgentsPolicyExceptionAuthorityBoundary,
  noAccessGrant: S.Boolean,
  noDeploy: S.Boolean,
  noEmailSend: S.Boolean,
  noRuntimeDispatch: S.Boolean,
  noSourceMutation: S.Boolean,
  noSpend: S.Boolean,
}) {}

export class OpenAgentsPolicyExceptionReceipt extends S.Class<OpenAgentsPolicyExceptionReceipt>(
  'OpenAgentsPolicyExceptionReceipt',
)({
  approvedByRef: S.NullOr(S.String),
  authority: OpenAgentsPolicyExceptionAuthority,
  blockerRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  expiresAtIso: S.NullOr(S.String),
  family: OpenAgentsPolicyExceptionFamily,
  id: S.String,
  requestedByRef: S.String,
  reviewState: OpenAgentsPolicyExceptionReviewState,
  riskRefs: S.Array(S.String),
  scopeRefs: S.Array(S.String),
  subjectRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class OpenAgentsPolicyExceptionProjection extends S.Class<OpenAgentsPolicyExceptionProjection>(
  'OpenAgentsPolicyExceptionProjection',
)({
  appliesNow: S.Boolean,
  approvedByRef: S.NullOr(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  blockerRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  expired: S.Boolean,
  expiresAtDisplay: S.String,
  family: OpenAgentsPolicyExceptionFamily,
  id: S.String,
  overbroad: S.Boolean,
  requestedByRef: S.NullOr(S.String),
  reviewState: OpenAgentsPolicyExceptionReviewState,
  riskRefs: S.Array(S.String),
  runtimeAuthorityPresent: S.Boolean,
  scopeRefs: S.Array(S.String),
  subjectRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class OpenAgentsPolicyExceptionUnsafe extends S.TaggedErrorClass<OpenAgentsPolicyExceptionUnsafe>()(
  'OpenAgentsPolicyExceptionUnsafe',
  {
    reason: S.String,
  },
) {}

export const OPENAGENTS_POLICY_EXCEPTION_NO_AUTHORITY:
  OpenAgentsPolicyExceptionAuthority = {
    authorityBoundary: 'evidence_only',
    noAccessGrant: true,
    noDeploy: true,
    noEmailSend: true,
    noRuntimeDispatch: true,
    noSourceMutation: true,
    noSpend: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafePolicyExceptionRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(grant|payload|token)|raw[_-]?(email|invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret[_-]?value|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const overbroadScopePattern =
  /(^all$|scope\.all|scope\.\*|scope\.wildcard|wildcard)/i
const publicUnsafeRefPattern =
  /(approved_by|blocker\.private|evidence\.operator|requested_by|risk\.internal|scope\.private|subject\.private)/i
const customerUnsafeRefPattern =
  /(approved_by|evidence\.operator|risk\.internal|scope\.private|subject\.private)/i
const teamUnsafeRefPattern =
  /(evidence\.operator|scope\.private|subject\.private)/i

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
    unsafePolicyExceptionRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsPolicyExceptionUnsafe({
      reason: `${label} contains raw secrets, tokens, provider grants, wallet material, payment proofs, raw emails, private repo refs, raw logs, raw source archives, or raw timestamps.`,
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

const safeNullableRefForAudience = (
  label: string,
  ref: string | null,
  audience: 'public' | 'customer' | 'team' | 'operator',
): string | null =>
  ref === null
    ? null
    : safeRefsForAudience(label, [ref], audience)[0] ?? null

const assertPolicyExceptionSafe = (
  receipt: OpenAgentsPolicyExceptionReceipt,
): void => {
  assertSafeRefs('policy exception identity refs', [
    receipt.id,
    receipt.requestedByRef,
  ])

  if (receipt.approvedByRef !== null) {
    assertSafeRefs('policy exception approved-by refs', [receipt.approvedByRef])
  }

  assertSafeRefs('policy exception subject refs', receipt.subjectRefs)
  assertSafeRefs('policy exception scope refs', receipt.scopeRefs)
  assertSafeRefs('policy exception risk refs', receipt.riskRefs)
  assertSafeRefs('policy exception blocker refs', receipt.blockerRefs)
  assertSafeRefs('policy exception evidence refs', receipt.evidenceRefs)
}

export const openAgentsPolicyExceptionHasRuntimeAuthority = (
  receipt: OpenAgentsPolicyExceptionReceipt,
): boolean =>
  receipt.authority.authorityBoundary !== 'evidence_only' ||
  !receipt.authority.noAccessGrant ||
  !receipt.authority.noDeploy ||
  !receipt.authority.noEmailSend ||
  !receipt.authority.noRuntimeDispatch ||
  !receipt.authority.noSourceMutation ||
  !receipt.authority.noSpend

export const openAgentsPolicyExceptionIsExpired = (
  receipt: OpenAgentsPolicyExceptionReceipt,
  nowIso: string,
): boolean => {
  if (receipt.expiresAtIso === null) {
    return false
  }

  const expiresAt = Date.parse(receipt.expiresAtIso)
  const now = Date.parse(nowIso)

  return Number.isFinite(expiresAt) &&
    Number.isFinite(now) &&
    expiresAt <= now
}

export const openAgentsPolicyExceptionIsRejected = (
  receipt: OpenAgentsPolicyExceptionReceipt,
): boolean => receipt.reviewState === 'rejected'

export const openAgentsPolicyExceptionIsRevoked = (
  receipt: OpenAgentsPolicyExceptionReceipt,
): boolean => receipt.reviewState === 'revoked'

export const openAgentsPolicyExceptionIsUnreviewed = (
  receipt: OpenAgentsPolicyExceptionReceipt,
): boolean => receipt.reviewState === 'requested'

export const openAgentsPolicyExceptionIsOverbroad = (
  receipt: OpenAgentsPolicyExceptionReceipt,
): boolean =>
  !hasRefs(receipt.subjectRefs) ||
  !hasRefs(receipt.scopeRefs) ||
  receipt.scopeRefs.some(ref => overbroadScopePattern.test(ref))

export const openAgentsPolicyExceptionAppliesNow = (
  receipt: OpenAgentsPolicyExceptionReceipt,
  nowIso: string,
): boolean =>
  receipt.reviewState === 'approved' &&
  receipt.approvedByRef !== null &&
  !openAgentsPolicyExceptionHasRuntimeAuthority(receipt) &&
  !openAgentsPolicyExceptionIsExpired(receipt, nowIso) &&
  !openAgentsPolicyExceptionIsOverbroad(receipt)

const expiresAtDisplay = (
  receipt: OpenAgentsPolicyExceptionReceipt,
  nowIso: string,
): string => {
  if (receipt.expiresAtIso === null) {
    return 'No expiration recorded'
  }

  return openAgentsPolicyExceptionIsExpired(receipt, nowIso)
    ? 'Expired'
    : 'Active'
}

export const projectOpenAgentsPolicyException = (
  receipt: OpenAgentsPolicyExceptionReceipt,
  audience: 'public' | 'customer' | 'team' | 'operator',
  nowIso: string,
): OpenAgentsPolicyExceptionProjection => {
  assertPolicyExceptionSafe(receipt)

  const runtimeAuthorityPresent =
    openAgentsPolicyExceptionHasRuntimeAuthority(receipt)
  const expired = openAgentsPolicyExceptionIsExpired(receipt, nowIso)
  const overbroad = openAgentsPolicyExceptionIsOverbroad(receipt)

  const projection: OpenAgentsPolicyExceptionProjection = {
    appliesNow: openAgentsPolicyExceptionAppliesNow(receipt, nowIso),
    approvedByRef:
      audience === 'team' || audience === 'operator'
        ? safeNullableRefForAudience(
            'policy exception approved-by ref',
            receipt.approvedByRef,
            audience,
          )
        : null,
    audience,
    blockerRefs: safeRefsForAudience(
      'policy exception blocker refs',
      receipt.blockerRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      receipt.createdAtIso,
      nowIso,
    ),
    evidenceRefs:
      audience === 'operator'
        ? safeRefsForAudience(
            'policy exception evidence refs',
            receipt.evidenceRefs,
            audience,
          )
        : [],
    expired,
    expiresAtDisplay: expiresAtDisplay(receipt, nowIso),
    family: receipt.family,
    id: safeRefsForAudience(
      'policy exception id',
      [receipt.id],
      audience,
    )[0] ?? 'policy_exception.redacted',
    overbroad,
    requestedByRef:
      audience === 'operator'
        ? safeNullableRefForAudience(
            'policy exception requested-by ref',
            receipt.requestedByRef,
            audience,
          )
        : null,
    reviewState: receipt.reviewState,
    riskRefs: safeRefsForAudience(
      'policy exception risk refs',
      receipt.riskRefs,
      audience,
    ),
    runtimeAuthorityPresent,
    scopeRefs: safeRefsForAudience(
      'policy exception scope refs',
      receipt.scopeRefs,
      audience,
    ),
    subjectRefs: safeRefsForAudience(
      'policy exception subject refs',
      receipt.subjectRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      receipt.updatedAtIso,
      nowIso,
    ),
  }

  if (openAgentsPolicyExceptionProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsPolicyExceptionUnsafe({
      reason: 'Policy exception projection contains unsafe material.',
    })
  }

  return projection
}

export const openAgentsPolicyExceptionProjectionHasPrivateMaterial = (
  projection: OpenAgentsPolicyExceptionProjection,
): boolean => {
  const refs = [
    projection.id,
    projection.approvedByRef,
    projection.requestedByRef,
    ...projection.blockerRefs,
    ...projection.evidenceRefs,
    ...projection.riskRefs,
    ...projection.scopeRefs,
    ...projection.subjectRefs,
  ].filter((ref): ref is string => ref !== null)

  return refs.some(ref =>
    unsafePolicyExceptionRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )
}
