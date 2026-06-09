import { Schema as S } from 'effect'

import {
  OmniAcceptedOutcomeWorkKind as OmniAcceptedOutcomeWorkKindSchema,
} from './omni-accepted-outcome-contracts'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const OmniMobileWorkroomAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type OmniMobileWorkroomAudience =
  typeof OmniMobileWorkroomAudience.Type

export const OmniMobileWorkroomStatus = S.Literals([
  'queued',
  'active',
  'blocked',
  'waiting_review',
  'completed',
  'unavailable',
  'archived',
])
export type OmniMobileWorkroomStatus =
  typeof OmniMobileWorkroomStatus.Type

export const OmniMobileApprovalActionKind = S.Literals([
  'crm_send',
  'coding_write',
  'runner_launch',
  'payment',
  'provider_action',
  'public_claim',
  'legal_sensitive',
])
export type OmniMobileApprovalActionKind =
  typeof OmniMobileApprovalActionKind.Type

export const OmniMobileApprovalRiskLevel = S.Literals([
  'low',
  'medium',
  'high',
  'critical',
])
export type OmniMobileApprovalRiskLevel =
  typeof OmniMobileApprovalRiskLevel.Type

export const OmniMobileApprovalRequirement = S.Literals([
  'not_required',
  'operator_required',
  'customer_required',
  'admin_required',
  'legal_required',
])
export type OmniMobileApprovalRequirement =
  typeof OmniMobileApprovalRequirement.Type

export const OmniMobileApprovalCardState = S.Literals([
  'draft',
  'pending',
  'approved',
  'rejected',
  'expired',
  'executed',
  'blocked',
])
export type OmniMobileApprovalCardState =
  typeof OmniMobileApprovalCardState.Type

export const OmniMobileApprovalExpiryState = S.Literals([
  'not_expiring',
  'active',
  'expired',
])
export type OmniMobileApprovalExpiryState =
  typeof OmniMobileApprovalExpiryState.Type

export const OmniMobileApprovalAuthorityBoundary = S.Literals([
  'read_only_mobile_workroom_projection',
])
export type OmniMobileApprovalAuthorityBoundary =
  typeof OmniMobileApprovalAuthorityBoundary.Type

export class OmniMobileWorkroomApprovalAuthority extends S.Class<OmniMobileWorkroomApprovalAuthority>(
  'OmniMobileWorkroomApprovalAuthority',
)({
  authorityBoundary: OmniMobileApprovalAuthorityBoundary,
  noApprovalMutation: S.Boolean,
  noExecutionMutation: S.Boolean,
  noNotificationMutation: S.Boolean,
  noPaymentMutation: S.Boolean,
  noProviderMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noRunnerLaunch: S.Boolean,
}) {}

export class OmniMobileApprovalCardRecord extends S.Class<OmniMobileApprovalCardRecord>(
  'OmniMobileApprovalCardRecord',
)({
  actionKind: OmniMobileApprovalActionKind,
  approvalReceiptRefs: S.Array(S.String),
  approvalRequirement: OmniMobileApprovalRequirement,
  artifactRefs: S.Array(S.String),
  authority: OmniMobileWorkroomApprovalAuthority,
  blockedReasonRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  executionReceiptRefs: S.Array(S.String),
  expiresAtIso: S.NullOr(S.String),
  id: S.String,
  idempotencyKeyRef: S.String,
  receiptRefs: S.Array(S.String),
  riskLevel: OmniMobileApprovalRiskLevel,
  serverAuthorityCaveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniMobileApprovalCardState,
  summaryRef: S.String,
  titleRef: S.String,
  updatedAtIso: S.String,
  workroomRef: S.String,
}) {}

export class OmniMobileWorkroomCompactRecord extends S.Class<OmniMobileWorkroomCompactRecord>(
  'OmniMobileWorkroomCompactRecord',
)({
  activeOutcomeRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  authority: OmniMobileWorkroomApprovalAuthority,
  blockerRefs: S.Array(S.String),
  createdAtIso: S.String,
  id: S.String,
  providerStateRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  siteRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: OmniMobileWorkroomStatus,
  statusRef: S.String,
  titleRef: S.String,
  updatedAtIso: S.String,
  walletStateRefs: S.Array(S.String),
  workKind: OmniAcceptedOutcomeWorkKindSchema,
}) {}

export class OmniMobileApprovalCardProjection extends S.Class<OmniMobileApprovalCardProjection>(
  'OmniMobileApprovalCardProjection',
)({
  actionKind: OmniMobileApprovalActionKind,
  approvalMutationAllowed: S.Boolean,
  approvalReceiptRefs: S.Array(S.String),
  approvalRequirement: OmniMobileApprovalRequirement,
  approvalRequired: S.Boolean,
  artifactRefs: S.Array(S.String),
  authority: OmniMobileWorkroomApprovalAuthority,
  blockedReasonRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  executionMutationAllowed: S.Boolean,
  executionReceiptRefs: S.Array(S.String),
  expiresAtDisplay: S.NullOr(S.String),
  expiryState: OmniMobileApprovalExpiryState,
  id: S.String,
  idempotencyKeyRef: S.String,
  notificationMutationAllowed: S.Boolean,
  paymentMutationAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  receiptRefs: S.Array(S.String),
  riskLabel: S.String,
  riskLevel: OmniMobileApprovalRiskLevel,
  runnerLaunchAllowed: S.Boolean,
  serverAuthorityCaveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: OmniMobileApprovalCardState,
  stateLabel: S.String,
  summaryRef: S.String,
  titleRef: S.String,
  updatedAtDisplay: S.String,
  workroomRef: S.String,
}) {}

export class OmniMobileWorkroomProjection extends S.Class<OmniMobileWorkroomProjection>(
  'OmniMobileWorkroomProjection',
)({
  activeOutcomeRefs: S.Array(S.String),
  approvalCards: S.Array(OmniMobileApprovalCardProjection),
  approvalMutationAllowed: S.Boolean,
  artifactCount: S.Number,
  artifactRefs: S.Array(S.String),
  audience: OmniMobileWorkroomAudience,
  authority: OmniMobileWorkroomApprovalAuthority,
  blockedApprovalCount: S.Number,
  blockerRefs: S.Array(S.String),
  criticalApprovalCount: S.Number,
  evidenceRefCount: S.Number,
  executionMutationAllowed: S.Boolean,
  expiredApprovalCount: S.Number,
  id: S.String,
  pendingApprovalCount: S.Number,
  providerMutationAllowed: S.Boolean,
  providerStateRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  receiptCount: S.Number,
  receiptRefs: S.Array(S.String),
  runnerLaunchAllowed: S.Boolean,
  siteRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: OmniMobileWorkroomStatus,
  statusLabel: S.String,
  statusRef: S.String,
  titleRef: S.String,
  updatedAtDisplay: S.String,
  walletStateRefs: S.Array(S.String),
  workKind: OmniAcceptedOutcomeWorkKindSchema,
}) {}

export class OmniMobileWorkroomApprovalUnsafe extends S.TaggedErrorClass<OmniMobileWorkroomApprovalUnsafe>()(
  'OmniMobileWorkroomApprovalUnsafe',
  {
    reason: S.String,
  },
) {}

export const OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY:
  OmniMobileWorkroomApprovalAuthority = {
    authorityBoundary: 'read_only_mobile_workroom_projection',
    noApprovalMutation: true,
    noExecutionMutation: true,
    noNotificationMutation: true,
    noPaymentMutation: true,
    noProviderMutation: true,
    noPublicClaimUpgrade: true,
    noRunnerLaunch: true,
  }

const stateLabelByState: Readonly<
  Record<OmniMobileApprovalCardState, string>
> = {
  approved: 'Approved',
  blocked: 'Blocked',
  draft: 'Draft',
  executed: 'Executed',
  expired: 'Expired',
  pending: 'Pending approval',
  rejected: 'Rejected',
}

const statusLabelByStatus: Readonly<Record<OmniMobileWorkroomStatus, string>> =
  {
    active: 'Active',
    archived: 'Archived',
    blocked: 'Blocked',
    completed: 'Completed',
    queued: 'Queued',
    unavailable: 'Unavailable',
    waiting_review: 'Waiting for review',
  }

const riskLabelByRisk: Readonly<Record<OmniMobileApprovalRiskLevel, string>> =
  {
    critical: 'Critical risk',
    high: 'High risk',
    low: 'Low risk',
    medium: 'Medium risk',
  }

const mandatoryApprovalActions =
  new Set<OmniMobileApprovalActionKind>([
    'legal_sensitive',
    'payment',
    'provider_action',
    'public_claim',
    'runner_launch',
  ])

const publicAudiencePattern =
  /(artifact\.private|approval\.private|blocker\.private|caveat\.private|evidence\.private|idempotency\.private|outcome\.private|provider\.|receipt\.private|server_authority\.private|site\.private|source\.private|status\.private|summary\.private|title\.private|wallet\.)/i
const agentAudiencePattern =
  /(approval\.private|idempotency\.private|provider\.private|receipt\.private|server_authority\.private|wallet\.)/i
const customerAudiencePattern =
  /(approval\.private|idempotency\.private|provider\.private|server_authority\.private|wallet\.private)/i

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeMobileRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|record|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|key|source|wallet)|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(approval|auth|connector|customer|email|invoice|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|transcript|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeMobileRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason: `${label} contains private customer, provider, wallet, payment, raw runner, email, secret, private repo, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: OmniMobileWorkroomAudience,
): RegExp | null => {
  switch (audience) {
    case 'agent':
      return agentAudiencePattern
    case 'customer':
      return customerAudiencePattern
    case 'public':
      return publicAudiencePattern
    case 'operator':
    case 'team':
      return null
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: OmniMobileWorkroomAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const primaryRefForAudience = (
  label: string,
  ref: string,
  audience: OmniMobileWorkroomAudience,
  redactedRef: string,
): string =>
  refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (
  authority: OmniMobileWorkroomApprovalAuthority,
): void => {
  if (
    authority.noApprovalMutation !== true ||
    authority.noExecutionMutation !== true ||
    authority.noNotificationMutation !== true ||
    authority.noPaymentMutation !== true ||
    authority.noProviderMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noRunnerLaunch !== true
  ) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason:
        'Mobile workroom projections are read-only and cannot approve, execute, notify, spend, mutate providers, upgrade claims, or launch runners.',
    })
  }
}

const assertValidIso = (label: string, iso: string | null): void => {
  if (iso === null) {
    return
  }

  if (!Number.isFinite(Date.parse(iso))) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertCardRecord = (card: OmniMobileApprovalCardRecord): void => {
  assertReadOnlyAuthority(card.authority)
  assertValidIso('Approval card createdAtIso', card.createdAtIso)
  assertValidIso('Approval card updatedAtIso', card.updatedAtIso)
  assertValidIso('Approval card expiresAtIso', card.expiresAtIso)
  assertSafeRefs('Approval card identity refs', [
    card.id,
    card.idempotencyKeyRef,
    card.summaryRef,
    card.titleRef,
    card.workroomRef,
  ])
  assertSafeRefs('Approval card approval refs', card.approvalReceiptRefs)
  assertSafeRefs('Approval card artifact refs', card.artifactRefs)
  assertSafeRefs('Approval card blocker refs', card.blockedReasonRefs)
  assertSafeRefs('Approval card caveat refs', card.caveatRefs)
  assertSafeRefs('Approval card evidence refs', card.evidenceRefs)
  assertSafeRefs('Approval card execution refs', card.executionReceiptRefs)
  assertSafeRefs('Approval card receipt refs', card.receiptRefs)
  assertSafeRefs(
    'Approval card server authority caveat refs',
    card.serverAuthorityCaveatRefs,
  )
  assertSafeRefs('Approval card source refs', card.sourceRefs)

  if (card.serverAuthorityCaveatRefs.length === 0) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason: 'Approval cards require server-authority caveat refs.',
    })
  }

  if (
    ['high', 'critical'].includes(card.riskLevel) &&
    (card.evidenceRefs.length === 0 ||
      card.approvalRequirement === 'not_required')
  ) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason:
        'High and critical approval cards require evidence refs and an explicit approval requirement.',
    })
  }

  if (
    mandatoryApprovalActions.has(card.actionKind) &&
    card.approvalRequirement === 'not_required'
  ) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason:
        'Runner launches, payments, provider actions, public claims, and legal-sensitive cards require approval.',
    })
  }

  if (
    ['approved', 'executed'].includes(card.state) &&
    card.approvalRequirement !== 'not_required' &&
    card.approvalReceiptRefs.length === 0
  ) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason: 'Approved or executed approval cards require approval receipts.',
    })
  }

  if (
    card.state === 'executed' &&
    card.executionReceiptRefs.length === 0
  ) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason: 'Executed approval cards require execution receipts.',
    })
  }

  if (card.state === 'expired' && card.expiresAtIso === null) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason: 'Expired approval cards require an expiry timestamp.',
    })
  }

  if (card.state === 'blocked' && card.blockedReasonRefs.length === 0) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason: 'Blocked approval cards require blocked reason refs.',
    })
  }
}

const assertWorkroomRecord = (
  workroom: OmniMobileWorkroomCompactRecord,
): void => {
  assertReadOnlyAuthority(workroom.authority)
  assertValidIso('Mobile workroom createdAtIso', workroom.createdAtIso)
  assertValidIso('Mobile workroom updatedAtIso', workroom.updatedAtIso)
  assertSafeRefs('Mobile workroom identity refs', [
    workroom.id,
    workroom.statusRef,
    workroom.titleRef,
  ])
  assertSafeRefs('Mobile workroom active outcome refs', workroom.activeOutcomeRefs)
  assertSafeRefs('Mobile workroom artifact refs', workroom.artifactRefs)
  assertSafeRefs('Mobile workroom blocker refs', workroom.blockerRefs)
  assertSafeRefs('Mobile workroom provider refs', workroom.providerStateRefs)
  assertSafeRefs('Mobile workroom receipt refs', workroom.receiptRefs)
  assertSafeRefs('Mobile workroom site refs', workroom.siteRefs)
  assertSafeRefs('Mobile workroom source refs', workroom.sourceRefs)
  assertSafeRefs('Mobile workroom wallet refs', workroom.walletStateRefs)
}

const durationLabel = (elapsedMs: number): string => {
  const minuteMs = 60_000
  const hourMs = 60 * minuteMs
  const dayMs = 24 * hourMs
  const durationMs = Math.max(0, elapsedMs)

  if (durationMs < minuteMs) {
    return 'less than 1 minute'
  }

  if (durationMs < hourMs) {
    const minutes = Math.floor(durationMs / minuteMs)

    return minutes === 1 ? '1 minute' : `${minutes} minutes`
  }

  if (durationMs < dayMs) {
    const hours = Math.floor(durationMs / hourMs)

    return hours === 1 ? '1 hour' : `${hours} hours`
  }

  const days = Math.floor(durationMs / dayMs)

  return days === 1 ? '1 day' : `${days} days`
}

const expiryProjection = (
  expiresAtIso: string | null,
  nowIso: string,
): Readonly<{
  expiresAtDisplay: string | null
  expiryState: OmniMobileApprovalExpiryState
}> => {
  if (expiresAtIso === null) {
    return { expiresAtDisplay: null, expiryState: 'not_expiring' }
  }

  const expiresAt = Date.parse(expiresAtIso)
  const now = Date.parse(nowIso)

  if (!Number.isFinite(expiresAt) || !Number.isFinite(now)) {
    return { expiresAtDisplay: 'Recently', expiryState: 'active' }
  }

  if (expiresAt <= now) {
    return {
      expiresAtDisplay: `Expired ${durationLabel(now - expiresAt)} ago`,
      expiryState: 'expired',
    }
  }

  return {
    expiresAtDisplay: `Expires in ${durationLabel(expiresAt - now)}`,
    expiryState: 'active',
  }
}

const cardProjection = (
  card: OmniMobileApprovalCardRecord,
  audience: OmniMobileWorkroomAudience,
  nowIso: string,
): OmniMobileApprovalCardProjection => {
  const expiry = expiryProjection(card.expiresAtIso, nowIso)

  return {
    actionKind: card.actionKind,
    approvalMutationAllowed: false,
    approvalReceiptRefs: refsForAudience(
      'Approval card approval receipt refs',
      card.approvalReceiptRefs,
      audience,
    ),
    approvalRequirement: card.approvalRequirement,
    approvalRequired: card.approvalRequirement !== 'not_required',
    artifactRefs: refsForAudience(
      'Approval card artifact refs',
      card.artifactRefs,
      audience,
    ),
    authority: OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
    blockedReasonRefs: refsForAudience(
      'Approval card blocked reason refs',
      card.blockedReasonRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Approval card caveat refs',
      card.caveatRefs,
      audience,
    ),
    evidenceRefs: refsForAudience(
      'Approval card evidence refs',
      card.evidenceRefs,
      audience,
    ),
    executionMutationAllowed: false,
    executionReceiptRefs: refsForAudience(
      'Approval card execution receipt refs',
      card.executionReceiptRefs,
      audience,
    ),
    expiresAtDisplay: expiry.expiresAtDisplay,
    expiryState: card.state === 'expired' ? 'expired' : expiry.expiryState,
    id: primaryRefForAudience(
      'Approval card id refs',
      card.id,
      audience,
      'approval_card.redacted',
    ),
    idempotencyKeyRef: primaryRefForAudience(
      'Approval card idempotency refs',
      card.idempotencyKeyRef,
      audience,
      'idempotency.redacted',
    ),
    notificationMutationAllowed: false,
    paymentMutationAllowed: false,
    providerMutationAllowed: false,
    publicClaimUpgradeAllowed: false,
    receiptRefs: refsForAudience(
      'Approval card receipt refs',
      card.receiptRefs,
      audience,
    ),
    riskLabel: riskLabelByRisk[card.riskLevel],
    riskLevel: card.riskLevel,
    runnerLaunchAllowed: false,
    serverAuthorityCaveatRefs: refsForAudience(
      'Approval card authority caveat refs',
      card.serverAuthorityCaveatRefs,
      audience,
    ),
    sourceRefs: refsForAudience(
      'Approval card source refs',
      card.sourceRefs,
      audience,
    ),
    state: card.state,
    stateLabel: stateLabelByState[card.state],
    summaryRef: primaryRefForAudience(
      'Approval card summary refs',
      card.summaryRef,
      audience,
      'summary.redacted',
    ),
    titleRef: primaryRefForAudience(
      'Approval card title refs',
      card.titleRef,
      audience,
      'title.redacted',
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      card.updatedAtIso,
      nowIso,
    ),
    workroomRef: primaryRefForAudience(
      'Approval card workroom refs',
      card.workroomRef,
      audience,
      'workroom.redacted',
    ),
  }
}

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(item => [...stringValues(item)])
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(item => [...stringValues(item)])
  }

  return []
}

const projectionHasPrivateMaterial = (
  projection: OmniMobileWorkroomProjection,
): boolean => {
  const text = stringValues(projection).join(' ')
  const pattern = audienceUnsafePattern(projection.audience)

  return (
    unsafeMobileRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
  )
}

export const projectOmniMobileWorkroom = (
  workroom: OmniMobileWorkroomCompactRecord,
  cards: ReadonlyArray<OmniMobileApprovalCardRecord>,
  audience: OmniMobileWorkroomAudience,
  nowIso: string,
): OmniMobileWorkroomProjection => {
  assertWorkroomRecord(workroom)
  cards.forEach(assertCardRecord)

  const approvalCards = cards
    .filter(card => card.workroomRef === workroom.id)
    .map(card => cardProjection(card, audience, nowIso))

  const projection: OmniMobileWorkroomProjection = {
    activeOutcomeRefs: refsForAudience(
      'Mobile workroom active outcome refs',
      workroom.activeOutcomeRefs,
      audience,
    ),
    approvalCards,
    approvalMutationAllowed: false,
    artifactCount: workroom.artifactRefs.length,
    artifactRefs: refsForAudience(
      'Mobile workroom artifact refs',
      workroom.artifactRefs,
      audience,
    ),
    audience,
    authority: OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
    blockedApprovalCount: approvalCards.filter(card => card.state === 'blocked')
      .length,
    blockerRefs: refsForAudience(
      'Mobile workroom blocker refs',
      workroom.blockerRefs,
      audience,
    ),
    criticalApprovalCount: approvalCards.filter(
      card => card.riskLevel === 'critical',
    ).length,
    evidenceRefCount: approvalCards.reduce(
      (count, card) => count + card.evidenceRefs.length,
      0,
    ),
    executionMutationAllowed: false,
    expiredApprovalCount: approvalCards.filter(
      card => card.expiryState === 'expired',
    ).length,
    id: primaryRefForAudience(
      'Mobile workroom id refs',
      workroom.id,
      audience,
      'workroom.redacted',
    ),
    pendingApprovalCount: approvalCards.filter(
      card => card.state === 'pending',
    ).length,
    providerMutationAllowed: false,
    providerStateRefs: refsForAudience(
      'Mobile workroom provider state refs',
      workroom.providerStateRefs,
      audience,
    ),
    publicClaimUpgradeAllowed: false,
    receiptCount: workroom.receiptRefs.length,
    receiptRefs: refsForAudience(
      'Mobile workroom receipt refs',
      workroom.receiptRefs,
      audience,
    ),
    runnerLaunchAllowed: false,
    siteRefs: refsForAudience(
      'Mobile workroom site refs',
      workroom.siteRefs,
      audience,
    ),
    sourceRefs: refsForAudience(
      'Mobile workroom source refs',
      workroom.sourceRefs,
      audience,
    ),
    status: workroom.status,
    statusLabel: statusLabelByStatus[workroom.status],
    statusRef: primaryRefForAudience(
      'Mobile workroom status refs',
      workroom.statusRef,
      audience,
      'status.redacted',
    ),
    titleRef: primaryRefForAudience(
      'Mobile workroom title refs',
      workroom.titleRef,
      audience,
      'title.redacted',
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      workroom.updatedAtIso,
      nowIso,
    ),
    walletStateRefs: refsForAudience(
      'Mobile workroom wallet refs',
      workroom.walletStateRefs,
      audience,
    ),
    workKind: workroom.workKind,
  }

  if (projectionHasPrivateMaterial(projection)) {
    throw new OmniMobileWorkroomApprovalUnsafe({
      reason:
        'Mobile workroom projection contains private customer, provider, wallet, payment, raw runner, email, secret, private repo, raw timestamp, or audience-inappropriate refs.',
    })
  }

  return projection
}

export const exampleOmniMobileWorkroom = (
  overrides: Partial<OmniMobileWorkroomCompactRecord> = {},
): OmniMobileWorkroomCompactRecord => ({
  activeOutcomeRefs: ['outcome.public.otec_revision_3'],
  artifactRefs: [
    'artifact.public.site_revision',
    'artifact.private.operator_diff_summary',
  ],
  authority: OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
  blockerRefs: [],
  createdAtIso: '2026-06-06T22:00:00.000Z',
  id: 'workroom.public.otec_site_revision',
  providerStateRefs: [
    'provider.public.model_route_ready',
    'provider.private.operator_route_notes',
  ],
  receiptRefs: ['receipt.public.revision_ready_email'],
  siteRefs: ['site.public.otec'],
  sourceRefs: [
    'source.public.customer_feedback',
    'source.private.operator_notes',
  ],
  status: 'waiting_review',
  statusRef: 'status.public.waiting_review',
  titleRef: 'title.public.otec_revision_workroom',
  updatedAtIso: '2026-06-06T22:25:00.000Z',
  walletStateRefs: ['wallet.public.no_live_payment'],
  workKind: 'site',
  ...overrides,
})

export const exampleOmniMobileApprovalCard = (
  overrides: Partial<OmniMobileApprovalCardRecord> = {},
): OmniMobileApprovalCardRecord => ({
  actionKind: 'crm_send',
  approvalReceiptRefs: [],
  approvalRequirement: 'operator_required',
  artifactRefs: ['artifact.public.email_preview'],
  authority: OMNI_MOBILE_WORKROOM_READ_ONLY_AUTHORITY,
  blockedReasonRefs: [],
  caveatRefs: ['caveat.public.preview_only'],
  createdAtIso: '2026-06-06T22:10:00.000Z',
  evidenceRefs: ['evidence.public.revision_ready'],
  executionReceiptRefs: [],
  expiresAtIso: '2026-06-06T23:30:00.000Z',
  id: 'approval.public.revision_ready_email',
  idempotencyKeyRef: 'idempotency.public.revision_ready_email',
  receiptRefs: ['receipt.public.email_draft'],
  riskLevel: 'medium',
  serverAuthorityCaveatRefs: ['server_authority.public.operator_gate_required'],
  sourceRefs: ['source.public.customer_feedback'],
  state: 'pending',
  summaryRef: 'summary.public.send_review_ready_email',
  titleRef: 'title.public.send_review_ready_email',
  updatedAtIso: '2026-06-06T22:25:00.000Z',
  workroomRef: 'workroom.public.otec_site_revision',
  ...overrides,
})

export const exampleOmniMobileApprovalCards =
  (): ReadonlyArray<OmniMobileApprovalCardRecord> => [
    exampleOmniMobileApprovalCard(),
    exampleOmniMobileApprovalCard({
      actionKind: 'payment',
      approvalRequirement: 'admin_required',
      approvalReceiptRefs: ['approval.public.admin_reviewed'],
      evidenceRefs: ['evidence.public.price_preview'],
      expiresAtIso: '2026-06-06T22:20:00.000Z',
      id: 'approval.public.payment_preview',
      idempotencyKeyRef: 'idempotency.public.payment_preview',
      receiptRefs: ['receipt.public.price_preview'],
      riskLevel: 'high',
      state: 'expired',
      summaryRef: 'summary.public.payment_preview',
      titleRef: 'title.public.payment_preview',
      updatedAtIso: '2026-06-06T22:20:00.000Z',
    }),
    exampleOmniMobileApprovalCard({
      actionKind: 'public_claim',
      approvalRequirement: 'operator_required',
      blockedReasonRefs: ['blocker.public.awaiting_evidence_review'],
      evidenceRefs: ['evidence.public.proof_bundle'],
      expiresAtIso: null,
      id: 'approval.public.public_claim',
      idempotencyKeyRef: 'idempotency.public.public_claim',
      receiptRefs: [],
      riskLevel: 'critical',
      state: 'blocked',
      summaryRef: 'summary.public.publish_claim',
      titleRef: 'title.public.publish_claim',
      updatedAtIso: '2026-06-06T22:24:00.000Z',
    }),
  ]
