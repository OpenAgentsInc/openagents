import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type {
  ReferralAttributionPolicyState,
  SiteReferralSourcePolicyState,
} from './site-referrals'
import type {
  ReferralWorkflowEventKind,
  ReferralWorkflowEventPolicyState,
} from './site-referral-workflow-events'

export const SiteReferralPolicySubjectKind = S.Literals([
  'referral_source',
  'referral_invite',
  'referral_attribution',
  'user_attribution',
  'order_attribution',
  'agent_attribution',
  'workflow_event',
])
export type SiteReferralPolicySubjectKind =
  typeof SiteReferralPolicySubjectKind.Type

export const SiteReferralPolicyDecisionState = S.Literals([
  'pending',
  'active',
  'held',
  'disputed',
  'capped',
  'reversed',
  'expired',
  'archived',
  'operator_overridden',
])
export type SiteReferralPolicyDecisionState =
  typeof SiteReferralPolicyDecisionState.Type

export const SiteReferralPolicyReason = S.Literals([
  'eligible',
  'self_referral',
  'duplicate_account',
  'collusion_risk',
  'chargeback_refund',
  'sanctions_compliance',
  'expired',
  'cap_exceeded',
  'clawback',
  'operator_override',
  'refund_or_reversal',
  'first_verified_wins',
  'manual_review',
])
export type SiteReferralPolicyReason = typeof SiteReferralPolicyReason.Type

export const SiteReferralPolicyEligibility = S.Literals([
  'eligible',
  'not_eligible',
  'manual_review',
])
export type SiteReferralPolicyEligibility =
  typeof SiteReferralPolicyEligibility.Type

export const SiteReferralPolicyCustomerStatus = S.Literals([
  'active',
  'under_review',
  'not_eligible',
  'expired',
])
export type SiteReferralPolicyCustomerStatus =
  typeof SiteReferralPolicyCustomerStatus.Type

export const SiteReferralPolicyDecision = S.Struct({
  customerStatus: SiteReferralPolicyCustomerStatus,
  decisionState: SiteReferralPolicyDecisionState,
  eligibility: SiteReferralPolicyEligibility,
  eligibleForFutureReward: S.Boolean,
  reason: SiteReferralPolicyReason,
})
export type SiteReferralPolicyDecision = typeof SiteReferralPolicyDecision.Type

export const PublicSiteReferralPolicyDecision = S.Struct({
  customerStatus: SiteReferralPolicyCustomerStatus,
  decisionState: SiteReferralPolicyDecisionState,
  eligibleForFutureReward: S.Boolean,
})
export type PublicSiteReferralPolicyDecision =
  typeof PublicSiteReferralPolicyDecision.Type

export const OperatorSiteReferralPolicyDecision = S.Struct({
  customerStatus: SiteReferralPolicyCustomerStatus,
  decisionState: SiteReferralPolicyDecisionState,
  eligibility: SiteReferralPolicyEligibility,
  eligibleForFutureReward: S.Boolean,
  reason: SiteReferralPolicyReason,
})
export type OperatorSiteReferralPolicyDecision =
  typeof OperatorSiteReferralPolicyDecision.Type

export const SiteReferralPolicyEventRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  customerStatus: SiteReferralPolicyCustomerStatus,
  decidedAt: S.String,
  decisionState: SiteReferralPolicyDecisionState,
  eligibility: SiteReferralPolicyEligibility,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  operatorActorUserId: S.NullOr(S.String),
  operatorNoteRef: S.NullOr(S.String),
  policyReason: SiteReferralPolicyReason,
  previousState: S.NullOr(S.String),
  referralAttributionId: S.NullOr(S.String),
  referralInviteId: S.NullOr(S.String),
  referralSourceId: S.NullOr(S.String),
  referralWorkflowEventId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  softwareOrderId: S.NullOr(S.String),
  subjectKind: SiteReferralPolicySubjectKind,
  subjectRef: S.String,
})
export type SiteReferralPolicyEventRecord =
  typeof SiteReferralPolicyEventRecord.Type

export type SiteReferralPolicyWorkflowEvent = Readonly<{
  amount: number
  eventKind: ReferralWorkflowEventKind
  id: string
  policyState: ReferralWorkflowEventPolicyState
}>

export type SiteReferralPolicySignals = Readonly<{
  chargebackOrRefund?: boolean | undefined
  clawbackRequired?: boolean | undefined
  collusionRisk?: boolean | undefined
  duplicateAccount?: boolean | undefined
  sanctionsOrComplianceHold?: boolean | undefined
}>

export type SiteReferralPolicyCaps = Readonly<{
  maxEligibleAmount?: number | undefined
  maxEligibleWorkflowEvents?: number | undefined
}>

export type SiteReferralOperatorOverride = Readonly<{
  actorUserId: string
  decisionState: Extract<
    SiteReferralPolicyDecisionState,
    'active' | 'held' | 'disputed' | 'capped' | 'reversed' | 'archived'
  >
  eligibility: SiteReferralPolicyEligibility
  noteRef: string
}>

export type EvaluateSiteReferralPolicyInput = Readonly<{
  attributionExpiresAt: string
  attributionPolicyState: ReferralAttributionPolicyState
  caps?: SiteReferralPolicyCaps | undefined
  existingUserAttributionId?: string | undefined
  nowIso?: string | undefined
  operatorOverride?: SiteReferralOperatorOverride | undefined
  referredUserId?: string | undefined
  referrerUserId: string
  referralAttributionId: string
  signals?: SiteReferralPolicySignals | undefined
  sourcePolicyState: SiteReferralSourcePolicyState
  workflowEvents?: ReadonlyArray<SiteReferralPolicyWorkflowEvent> | undefined
}>

export type RecordSiteReferralPolicyEventInput = Readonly<{
  customerStatus: SiteReferralPolicyCustomerStatus
  decisionState: SiteReferralPolicyDecisionState
  eligibility: SiteReferralPolicyEligibility
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  operatorActorUserId?: string | undefined
  operatorNoteRef?: string | undefined
  policyReason: SiteReferralPolicyReason
  previousState?: string | undefined
  referralAttributionId?: string | undefined
  referralInviteId?: string | undefined
  referralSourceId?: string | undefined
  referralWorkflowEventId?: string | undefined
  siteId?: string | undefined
  softwareOrderId?: string | undefined
  subjectKind: SiteReferralPolicySubjectKind
  subjectRef: string
}>

type SiteReferralPolicyEventRow = Readonly<{
  archived_at: string | null
  created_at: string
  customer_status: SiteReferralPolicyCustomerStatus
  decided_at: string
  decision_state: SiteReferralPolicyDecisionState
  eligibility: SiteReferralPolicyEligibility
  id: string
  idempotency_key: string
  metadata_json: string
  operator_actor_user_id: string | null
  operator_note_ref: string | null
  policy_reason: SiteReferralPolicyReason
  previous_state: string | null
  referral_attribution_id: string | null
  referral_invite_id: string | null
  referral_source_id: string | null
  referral_workflow_event_id: string | null
  site_id: string | null
  software_order_id: string | null
  subject_kind: SiteReferralPolicySubjectKind
  subject_ref: string
}>

type SiteReferralPolicyEventFilter =
  | Readonly<{ key: 'referral_attribution_id'; value: string }>
  | Readonly<{ key: 'referral_source_id'; value: string }>
  | Readonly<{ key: 'referral_workflow_event_id'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/
const PROHIBITED_VALUE_PATTERN =
  /\b(provider[_ -]?account|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic)/i

const customerStatusFor = (
  decisionState: SiteReferralPolicyDecisionState,
  eligibility: SiteReferralPolicyEligibility,
): SiteReferralPolicyCustomerStatus => {
  if (decisionState === 'expired') {
    return 'expired'
  }

  if (eligibility === 'eligible') {
    return 'active'
  }

  if (eligibility === 'manual_review') {
    return 'under_review'
  }

  return 'not_eligible'
}

const decision = (
  decisionState: SiteReferralPolicyDecisionState,
  reason: SiteReferralPolicyReason,
  eligibility: SiteReferralPolicyEligibility,
): SiteReferralPolicyDecision => ({
  customerStatus: customerStatusFor(decisionState, eligibility),
  decisionState,
  eligibility,
  eligibleForFutureReward: eligibility === 'eligible',
  reason,
})

const hasRefundOrReversal = (
  events: ReadonlyArray<SiteReferralPolicyWorkflowEvent>,
): boolean =>
  events.some(
    event =>
      event.eventKind === 'refund' ||
      event.eventKind === 'reversal' ||
      event.policyState === 'refunded' ||
      event.policyState === 'reversed',
  )

const hasWorkflowHold = (
  events: ReadonlyArray<SiteReferralPolicyWorkflowEvent>,
): boolean =>
  events.some(
    event =>
      event.policyState === 'held' ||
      event.policyState === 'disputed' ||
      event.eventKind === 'eligibility_hold' ||
      event.eventKind === 'dispute_hold',
  )

const totalWorkflowAmount = (
  events: ReadonlyArray<SiteReferralPolicyWorkflowEvent>,
): number =>
  events.reduce(
    (total, event) => total + (Number.isFinite(event.amount) ? event.amount : 0),
    0,
  )

export const evaluateSiteReferralPolicy = (
  input: EvaluateSiteReferralPolicyInput,
): SiteReferralPolicyDecision => {
  const nowIso = input.nowIso ?? currentIsoTimestamp()
  const events = input.workflowEvents ?? []
  const signals = input.signals ?? {}

  if (input.operatorOverride !== undefined) {
    return decision(
      'operator_overridden',
      'operator_override',
      input.operatorOverride.eligibility,
    )
  }

  if (input.sourcePolicyState === 'archived' || input.attributionPolicyState === 'archived') {
    return decision('archived', 'manual_review', 'not_eligible')
  }

  if (
    input.sourcePolicyState === 'disputed' ||
    input.attributionPolicyState === 'disputed'
  ) {
    return decision('disputed', 'manual_review', 'manual_review')
  }

  if (signals.collusionRisk === true) {
    return decision('disputed', 'collusion_risk', 'manual_review')
  }

  if (
    input.sourcePolicyState === 'disabled' ||
    input.attributionPolicyState === 'disabled'
  ) {
    return decision('held', 'manual_review', 'manual_review')
  }

  if (
    input.sourcePolicyState === 'expired' ||
    input.attributionPolicyState === 'expired' ||
    input.attributionExpiresAt <= nowIso
  ) {
    return decision('expired', 'expired', 'not_eligible')
  }

  if (
    input.referredUserId !== undefined &&
    input.referrerUserId === input.referredUserId
  ) {
    return decision('held', 'self_referral', 'manual_review')
  }

  if (
    input.existingUserAttributionId !== undefined &&
    input.existingUserAttributionId !== input.referralAttributionId
  ) {
    return decision('held', 'first_verified_wins', 'not_eligible')
  }

  if (signals.duplicateAccount === true) {
    return decision('held', 'duplicate_account', 'manual_review')
  }

  if (signals.sanctionsOrComplianceHold === true) {
    return decision('held', 'sanctions_compliance', 'manual_review')
  }

  if (signals.chargebackOrRefund === true) {
    return decision('reversed', 'chargeback_refund', 'not_eligible')
  }

  if (hasRefundOrReversal(events)) {
    return decision('reversed', 'refund_or_reversal', 'not_eligible')
  }

  if (signals.clawbackRequired === true) {
    return decision('reversed', 'clawback', 'not_eligible')
  }

  if (hasWorkflowHold(events)) {
    return decision('disputed', 'manual_review', 'manual_review')
  }

  if (
    input.caps?.maxEligibleWorkflowEvents !== undefined &&
    events.length >= input.caps.maxEligibleWorkflowEvents
  ) {
    return decision('capped', 'cap_exceeded', 'not_eligible')
  }

  if (
    input.caps?.maxEligibleAmount !== undefined &&
    totalWorkflowAmount(events) >= input.caps.maxEligibleAmount
  ) {
    return decision('capped', 'cap_exceeded', 'not_eligible')
  }

  if (input.attributionPolicyState === 'pending') {
    return decision('pending', 'eligible', 'manual_review')
  }

  return decision('active', 'eligible', 'eligible')
}

export const publicSiteReferralPolicyDecision = (
  policyDecision: SiteReferralPolicyDecision,
): PublicSiteReferralPolicyDecision => ({
  customerStatus: policyDecision.customerStatus,
  decisionState:
    policyDecision.decisionState === 'operator_overridden'
      ? 'held'
      : policyDecision.decisionState,
  eligibleForFutureReward: policyDecision.eligibleForFutureReward,
})

export const operatorSiteReferralPolicyDecision = (
  policyDecision: SiteReferralPolicyDecision,
): OperatorSiteReferralPolicyDecision => ({
  customerStatus: policyDecision.customerStatus,
  decisionState: policyDecision.decisionState,
  eligibility: policyDecision.eligibility,
  eligibleForFutureReward: policyDecision.eligibleForFutureReward,
  reason: policyDecision.reason,
})

const optionalRefs = (
  input: RecordSiteReferralPolicyEventInput,
): ReadonlyArray<readonly [string, string | undefined]> => [
  ['id', input.id],
  ['idempotencyKey', input.idempotencyKey],
  ['operatorActorUserId', input.operatorActorUserId],
  ['operatorNoteRef', input.operatorNoteRef],
  ['previousState', input.previousState],
  ['referralAttributionId', input.referralAttributionId],
  ['referralInviteId', input.referralInviteId],
  ['referralSourceId', input.referralSourceId],
  ['referralWorkflowEventId', input.referralWorkflowEventId],
  ['siteId', input.siteId],
  ['softwareOrderId', input.softwareOrderId],
  ['subjectRef', input.subjectRef],
]

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !PROHIBITED_VALUE_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!isSafeRef(value)) {
    throw new SiteReferralPolicyValidationError({
      reason: `${field} must be a public-safe ref, not private payment, wallet, provider, or abuse-detail material.`,
    })
  }
}

const assertSafeMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): void => {
  if (metadata === undefined) {
    return
  }

  const json = JSON.stringify(metadata)

  if (containsProviderSecretMaterial(json) || PROHIBITED_VALUE_PATTERN.test(json)) {
    throw new SiteReferralPolicyValidationError({
      reason:
        'metadata must not contain private payment, wallet, provider, or abuse-detail material.',
    })
  }
}

const assertValidEventInput = (
  input: RecordSiteReferralPolicyEventInput,
): void => {
  optionalRefs(input).forEach(([field, value]) => assertSafeRef(field, value))
  assertSafeMetadata(input.metadata)
}

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const eventFromRow = (
  row: SiteReferralPolicyEventRow,
): SiteReferralPolicyEventRecord => ({
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  customerStatus: row.customer_status,
  decidedAt: row.decided_at,
  decisionState: row.decision_state,
  eligibility: row.eligibility,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: metadataFromJson(row.metadata_json),
  operatorActorUserId: row.operator_actor_user_id,
  operatorNoteRef: row.operator_note_ref,
  policyReason: row.policy_reason,
  previousState: row.previous_state,
  referralAttributionId: row.referral_attribution_id,
  referralInviteId: row.referral_invite_id,
  referralSourceId: row.referral_source_id,
  referralWorkflowEventId: row.referral_workflow_event_id,
  siteId: row.site_id,
  softwareOrderId: row.software_order_id,
  subjectKind: row.subject_kind,
  subjectRef: row.subject_ref,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<SiteReferralPolicyEventRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM site_referral_policy_events
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<SiteReferralPolicyEventRow>()

  return row === null ? null : eventFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: SiteReferralPolicyEventFilter,
  limit = 100,
): Promise<ReadonlyArray<SiteReferralPolicyEventRecord>> => {
  assertSafeRef(filter.key, filter.value)

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM site_referral_policy_events
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY decided_at DESC, created_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<SiteReferralPolicyEventRow>()

  return (rows.results ?? []).map(eventFromRow)
}

export class SiteReferralPolicyValidationError extends S.TaggedErrorClass<SiteReferralPolicyValidationError>()(
  'SiteReferralPolicyValidationError',
  {
    reason: S.String,
  },
) {}

export class SiteReferralPolicyStorageError extends S.TaggedErrorClass<SiteReferralPolicyStorageError>()(
  'SiteReferralPolicyStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export const recordSiteReferralPolicyEvent = async (
  db: D1Database,
  input: RecordSiteReferralPolicyEventInput,
): Promise<SiteReferralPolicyEventRecord> => {
  assertValidEventInput(input)

  const nowIso = currentIsoTimestamp()
  const id = input.id ?? compactRandomId('site_referral_policy_event')
  const metadataJson = JSON.stringify(input.metadata ?? {})

  await db
    .prepare(
      `INSERT OR IGNORE INTO site_referral_policy_events (
         id,
         idempotency_key,
         subject_kind,
         subject_ref,
         referral_attribution_id,
         referral_source_id,
         referral_invite_id,
         referral_workflow_event_id,
         software_order_id,
         site_id,
         previous_state,
         decision_state,
         policy_reason,
         eligibility,
         customer_status,
         operator_actor_user_id,
         operator_note_ref,
         metadata_json,
         decided_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      input.subjectKind,
      input.subjectRef,
      input.referralAttributionId ?? null,
      input.referralSourceId ?? null,
      input.referralInviteId ?? null,
      input.referralWorkflowEventId ?? null,
      input.softwareOrderId ?? null,
      input.siteId ?? null,
      input.previousState ?? null,
      input.decisionState,
      input.policyReason,
      input.eligibility,
      input.customerStatus,
      input.operatorActorUserId ?? null,
      input.operatorNoteRef ?? null,
      metadataJson,
      nowIso,
      nowIso,
    )
    .run()

  const event = await readByIdempotencyKey(db, input.idempotencyKey)

  if (event === null) {
    throw new SiteReferralPolicyStorageError({
      operation: 'recordSiteReferralPolicyEvent.readByIdempotencyKey',
      reason: 'inserted or existing Site referral policy event was not readable.',
    })
  }

  return event
}

export const recordOperatorSiteReferralPolicyOverride = (
  db: D1Database,
  input: Readonly<{
    idempotencyKey: string
    override: SiteReferralOperatorOverride
    previousState?: string | undefined
    referralAttributionId?: string | undefined
    referralSourceId?: string | undefined
    siteId?: string | undefined
    subjectKind: SiteReferralPolicySubjectKind
    subjectRef: string
  }>,
): Promise<SiteReferralPolicyEventRecord> => {
  const customerStatus = customerStatusFor(
    input.override.decisionState,
    input.override.eligibility,
  )

  return recordSiteReferralPolicyEvent(db, {
    customerStatus,
    decisionState: 'operator_overridden',
    eligibility: input.override.eligibility,
    idempotencyKey: input.idempotencyKey,
    operatorActorUserId: input.override.actorUserId,
    operatorNoteRef: input.override.noteRef,
    policyReason: 'operator_override',
    previousState: input.previousState,
    referralAttributionId: input.referralAttributionId,
    referralSourceId: input.referralSourceId,
    siteId: input.siteId,
    subjectKind: input.subjectKind,
    subjectRef: input.subjectRef,
  })
}

export const listSiteReferralPolicyEventsByAttribution = (
  db: D1Database,
  referralAttributionId: string,
  limit?: number,
): Promise<ReadonlyArray<SiteReferralPolicyEventRecord>> =>
  listByFilter(
    db,
    { key: 'referral_attribution_id', value: referralAttributionId },
    limit,
  )

export const listSiteReferralPolicyEventsBySource = (
  db: D1Database,
  referralSourceId: string,
  limit?: number,
): Promise<ReadonlyArray<SiteReferralPolicyEventRecord>> =>
  listByFilter(db, { key: 'referral_source_id', value: referralSourceId }, limit)

export const listSiteReferralPolicyEventsByWorkflowEvent = (
  db: D1Database,
  referralWorkflowEventId: string,
  limit?: number,
): Promise<ReadonlyArray<SiteReferralPolicyEventRecord>> =>
  listByFilter(
    db,
    { key: 'referral_workflow_event_id', value: referralWorkflowEventId },
    limit,
  )
