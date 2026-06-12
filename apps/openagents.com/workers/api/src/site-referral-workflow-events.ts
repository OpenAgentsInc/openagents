import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const ReferralWorkflowEventKind = S.Literals([
  'paid_usage',
  'site_checkout',
  'l402_redemption',
  'accepted_outcome',
  'refund',
  'reversal',
  'eligibility_hold',
  'dispute_hold',
  'operator_adjustment',
])
export type ReferralWorkflowEventKind = typeof ReferralWorkflowEventKind.Type

export const ReferralWorkflowEventPolicyState = S.Literals([
  'recorded',
  'eligible',
  'held',
  'disputed',
  'refunded',
  'reversed',
  'ignored',
])
export type ReferralWorkflowEventPolicyState =
  typeof ReferralWorkflowEventPolicyState.Type

export const ReferralWorkflowEventAsset = S.Literals([
  'none',
  'credits',
  'sats',
  'usd',
])
export type ReferralWorkflowEventAsset = typeof ReferralWorkflowEventAsset.Type

export const ReferralWorkflowEventRecord = S.Struct({
  acceptedWorkRef: S.NullOr(S.String),
  amount: S.Number,
  archivedAt: S.NullOr(S.String),
  asset: ReferralWorkflowEventAsset,
  createdAt: S.String,
  entitlementRef: S.NullOr(S.String),
  eventKind: ReferralWorkflowEventKind,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  occurredAt: S.String,
  paidActionId: S.NullOr(S.String),
  paymentEventId: S.NullOr(S.String),
  paymentEvidenceRef: S.NullOr(S.String),
  policyState: ReferralWorkflowEventPolicyState,
  productId: S.NullOr(S.String),
  publicInviteRef: S.NullOr(S.String),
  publicReceiptRef: S.String,
  publicSourceRef: S.String,
  referralAttributionId: S.String,
  referralInviteId: S.NullOr(S.String),
  referralSourceId: S.String,
  relatedEventId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  siteVersionId: S.NullOr(S.String),
  softwareOrderId: S.NullOr(S.String),
})
export type ReferralWorkflowEventRecord =
  typeof ReferralWorkflowEventRecord.Type

export type RecordReferralWorkflowEventInput = Readonly<{
  acceptedWorkRef?: string | undefined
  amount: number
  asset: ReferralWorkflowEventAsset
  entitlementRef?: string | undefined
  eventKind: ReferralWorkflowEventKind
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  occurredAt?: string | undefined
  paidActionId?: string | undefined
  paymentEventId?: string | undefined
  paymentEvidenceRef?: string | undefined
  policyState: ReferralWorkflowEventPolicyState
  productId?: string | undefined
  publicInviteRef?: string | undefined
  publicReceiptRef: string
  publicSourceRef: string
  referralAttributionId: string
  referralInviteId?: string | undefined
  referralSourceId: string
  relatedEventId?: string | undefined
  siteId?: string | undefined
  siteVersionId?: string | undefined
  softwareOrderId?: string | undefined
}>

type ReferralWorkflowEventRow = Readonly<{
  accepted_work_ref: string | null
  amount: number
  archived_at: string | null
  asset: ReferralWorkflowEventAsset
  created_at: string
  entitlement_ref: string | null
  event_kind: ReferralWorkflowEventKind
  id: string
  idempotency_key: string
  metadata_json: string
  occurred_at: string
  paid_action_id: string | null
  payment_event_id: string | null
  payment_evidence_ref: string | null
  policy_state: ReferralWorkflowEventPolicyState
  product_id: string | null
  public_invite_ref: string | null
  public_receipt_ref: string
  public_source_ref: string
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  related_event_id: string | null
  site_id: string | null
  site_version_id: string | null
  software_order_id: string | null
}>

type ReferralWorkflowEventFilter =
  | Readonly<{ key: 'referral_attribution_id'; value: string }>
  | Readonly<{ key: 'referral_source_id'; value: string }>
  | Readonly<{ key: 'software_order_id'; value: string }>
  | Readonly<{ key: 'site_id'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/
const PROHIBITED_VALUE_PATTERN =
  /\b(lnbc|lntb|lnbcrt|lno1|payment_preimage|payment_secret|mnemonic|xprv|private_key|wallet_secret|webhook_secret|mdk_access_token|access_token|refresh_token|device_auth_id|code_verifier|gho_[a-z0-9_]+)/i

const optionalRefs = (
  input: RecordReferralWorkflowEventInput,
): ReadonlyArray<readonly [string, string | undefined]> => [
  ['acceptedWorkRef', input.acceptedWorkRef],
  ['entitlementRef', input.entitlementRef],
  ['id', input.id],
  ['idempotencyKey', input.idempotencyKey],
  ['paidActionId', input.paidActionId],
  ['paymentEventId', input.paymentEventId],
  ['paymentEvidenceRef', input.paymentEvidenceRef],
  ['productId', input.productId],
  ['publicInviteRef', input.publicInviteRef],
  ['publicReceiptRef', input.publicReceiptRef],
  ['publicSourceRef', input.publicSourceRef],
  ['referralAttributionId', input.referralAttributionId],
  ['referralInviteId', input.referralInviteId],
  ['referralSourceId', input.referralSourceId],
  ['relatedEventId', input.relatedEventId],
  ['siteId', input.siteId],
  ['siteVersionId', input.siteVersionId],
  ['softwareOrderId', input.softwareOrderId],
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
    throw new ReferralWorkflowEventValidationError({
      reason: `${field} must be a public-safe ref, not raw payment, wallet, provider, or secret material.`,
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
    throw new ReferralWorkflowEventValidationError({
      reason:
        'metadata must not contain raw payment, wallet, provider, or secret material.',
    })
  }
}

const assertValidInput = (input: RecordReferralWorkflowEventInput): void => {
  optionalRefs(input).forEach(([field, value]) => assertSafeRef(field, value))
  assertSafeMetadata(input.metadata)

  if (!Number.isFinite(input.amount) || input.amount < 0) {
    throw new ReferralWorkflowEventValidationError({
      reason: 'amount must be finite and non-negative.',
    })
  }

  if (
    (input.eventKind === 'refund' || input.eventKind === 'reversal') &&
    input.relatedEventId === undefined
  ) {
    throw new ReferralWorkflowEventValidationError({
      reason: 'refund and reversal events must link to a related event.',
    })
  }

  if (input.asset === 'none' && input.amount !== 0) {
    throw new ReferralWorkflowEventValidationError({
      reason: 'asset none may only be used with amount 0.',
    })
  }
}

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const eventFromRow = (row: ReferralWorkflowEventRow): ReferralWorkflowEventRecord => ({
  acceptedWorkRef: row.accepted_work_ref,
  amount: Number(row.amount),
  archivedAt: row.archived_at,
  asset: row.asset,
  createdAt: row.created_at,
  entitlementRef: row.entitlement_ref,
  eventKind: row.event_kind,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: metadataFromJson(row.metadata_json),
  occurredAt: row.occurred_at,
  paidActionId: row.paid_action_id,
  paymentEventId: row.payment_event_id,
  paymentEvidenceRef: row.payment_evidence_ref,
  policyState: row.policy_state,
  productId: row.product_id,
  publicInviteRef: row.public_invite_ref,
  publicReceiptRef: row.public_receipt_ref,
  publicSourceRef: row.public_source_ref,
  referralAttributionId: row.referral_attribution_id,
  referralInviteId: row.referral_invite_id,
  referralSourceId: row.referral_source_id,
  relatedEventId: row.related_event_id,
  siteId: row.site_id,
  siteVersionId: row.site_version_id,
  softwareOrderId: row.software_order_id,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<ReferralWorkflowEventRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM referral_workflow_events
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<ReferralWorkflowEventRow>()

  return row === null ? null : eventFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: ReferralWorkflowEventFilter,
  limit = 100,
): Promise<ReadonlyArray<ReferralWorkflowEventRecord>> => {
  assertSafeRef(filter.key, filter.value)

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM referral_workflow_events
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY occurred_at DESC, created_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<ReferralWorkflowEventRow>()

  return (rows.results ?? []).map(eventFromRow)
}

export class ReferralWorkflowEventValidationError extends S.TaggedErrorClass<ReferralWorkflowEventValidationError>()(
  'ReferralWorkflowEventValidationError',
  {
    reason: S.String,
  },
) {}

export class ReferralWorkflowEventStorageError extends S.TaggedErrorClass<ReferralWorkflowEventStorageError>()(
  'ReferralWorkflowEventStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export const recordReferralWorkflowEvent = async (
  db: D1Database,
  input: RecordReferralWorkflowEventInput,
): Promise<ReferralWorkflowEventRecord> => {
  assertValidInput(input)

  const nowIso = currentIsoTimestamp()
  const createdAt = nowIso
  const occurredAt = input.occurredAt ?? nowIso
  const id = input.id ?? compactRandomId('referral_workflow_event')
  const metadataJson = JSON.stringify(input.metadata ?? {})

  await db
    .prepare(
      `INSERT OR IGNORE INTO referral_workflow_events (
         id,
         idempotency_key,
         event_kind,
         referral_attribution_id,
         referral_source_id,
         referral_invite_id,
         public_source_ref,
         public_invite_ref,
         software_order_id,
         site_id,
         site_version_id,
         product_id,
         paid_action_id,
         payment_event_id,
         payment_evidence_ref,
         entitlement_ref,
         accepted_work_ref,
         related_event_id,
         public_receipt_ref,
         policy_state,
         amount,
         asset,
         metadata_json,
         occurred_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      input.eventKind,
      input.referralAttributionId,
      input.referralSourceId,
      input.referralInviteId ?? null,
      input.publicSourceRef,
      input.publicInviteRef ?? null,
      input.softwareOrderId ?? null,
      input.siteId ?? null,
      input.siteVersionId ?? null,
      input.productId ?? null,
      input.paidActionId ?? null,
      input.paymentEventId ?? null,
      input.paymentEvidenceRef ?? null,
      input.entitlementRef ?? null,
      input.acceptedWorkRef ?? null,
      input.relatedEventId ?? null,
      input.publicReceiptRef,
      input.policyState,
      input.amount,
      input.asset,
      metadataJson,
      occurredAt,
      createdAt,
      null,
    )
    .run()

  const event = await readByIdempotencyKey(db, input.idempotencyKey)

  if (event === null) {
    throw new ReferralWorkflowEventStorageError({
      operation: 'recordReferralWorkflowEvent.readByIdempotencyKey',
      reason: 'inserted or existing referral workflow event was not readable.',
    })
  }

  return event
}

export const listReferralWorkflowEventsByAttribution = (
  db: D1Database,
  referralAttributionId: string,
  limit?: number,
): Promise<ReadonlyArray<ReferralWorkflowEventRecord>> =>
  listByFilter(
    db,
    { key: 'referral_attribution_id', value: referralAttributionId },
    limit,
  )

export const listReferralWorkflowEventsBySource = (
  db: D1Database,
  referralSourceId: string,
  limit?: number,
): Promise<ReadonlyArray<ReferralWorkflowEventRecord>> =>
  listByFilter(db, { key: 'referral_source_id', value: referralSourceId }, limit)

export const listReferralWorkflowEventsByOrder = (
  db: D1Database,
  softwareOrderId: string,
  limit?: number,
): Promise<ReadonlyArray<ReferralWorkflowEventRecord>> =>
  listByFilter(db, { key: 'software_order_id', value: softwareOrderId }, limit)

export const listReferralWorkflowEventsBySite = (
  db: D1Database,
  siteId: string,
  limit?: number,
): Promise<ReadonlyArray<ReferralWorkflowEventRecord>> =>
  listByFilter(db, { key: 'site_id', value: siteId }, limit)
