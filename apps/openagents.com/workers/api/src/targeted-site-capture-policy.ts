import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const TargetedSiteCaptureRobotsState = S.Literals([
  'allowed',
  'disallowed',
  'unavailable',
  'unknown',
])
export type TargetedSiteCaptureRobotsState =
  typeof TargetedSiteCaptureRobotsState.Type

export const TargetedSiteCaptureSitemapState = S.Literals([
  'available',
  'missing',
  'unknown',
])
export type TargetedSiteCaptureSitemapState =
  typeof TargetedSiteCaptureSitemapState.Type

export const TargetedSiteCapturePolicyDecision = S.Literals([
  'allowed',
  'disallowed',
  'blocked',
  'manual_review',
  'customer_owned',
  'suppressed',
  'paid_escalation',
])
export type TargetedSiteCapturePolicyDecision =
  typeof TargetedSiteCapturePolicyDecision.Type

export const TargetedSiteCapturePolicyReason = S.Literals([
  'robots_allowed',
  'robots_disallowed',
  'robots_unavailable',
  'sitemap_available',
  'suppression_match',
  'customer_owned_domain',
  'contact_suppressed',
  'operator_manual_review',
  'paid_provider_required',
  'bot_protection_or_login',
  'unsupported_scheme',
  'unsafe_domain',
  'policy_override',
])
export type TargetedSiteCapturePolicyReason =
  typeof TargetedSiteCapturePolicyReason.Type

export const TargetedSiteCapturePolicyEvaluation = S.Struct({
  decision: TargetedSiteCapturePolicyDecision,
  fetchable: S.Boolean,
  reason: TargetedSiteCapturePolicyReason,
})
export type TargetedSiteCapturePolicyEvaluation =
  typeof TargetedSiteCapturePolicyEvaluation.Type

export const TargetedSiteCapturePolicyEventRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  createdAt: S.String,
  customerAuthorityRef: S.NullOr(S.String),
  decidedAt: S.String,
  decision: TargetedSiteCapturePolicyDecision,
  fetchable: S.Boolean,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  operatorActorUserId: S.NullOr(S.String),
  operatorNoteRef: S.NullOr(S.String),
  paidEscalationRef: S.NullOr(S.String),
  prospectId: S.NullOr(S.String),
  reason: TargetedSiteCapturePolicyReason,
  robotsRef: S.NullOr(S.String),
  sitemapRef: S.NullOr(S.String),
  sourceRef: S.String,
  suppressionRef: S.NullOr(S.String),
})
export type TargetedSiteCapturePolicyEventRecord =
  typeof TargetedSiteCapturePolicyEventRecord.Type

export const PublicTargetedSiteCapturePolicyProjection = S.Struct({
  campaignId: S.String,
  decidedAt: S.String,
  decision: TargetedSiteCapturePolicyDecision,
  fetchable: S.Boolean,
  normalizedDomain: S.String,
  prospectId: S.NullOr(S.String),
  sourceRef: S.String,
})
export type PublicTargetedSiteCapturePolicyProjection =
  typeof PublicTargetedSiteCapturePolicyProjection.Type

export const OperatorTargetedSiteCapturePolicyProjection = S.Struct({
  campaignId: S.String,
  customerAuthorityRef: S.NullOr(S.String),
  decidedAt: S.String,
  decision: TargetedSiteCapturePolicyDecision,
  fetchable: S.Boolean,
  hasOperatorNoteRef: S.Boolean,
  hasSuppressionRef: S.Boolean,
  normalizedDomain: S.String,
  paidEscalationRef: S.NullOr(S.String),
  prospectId: S.NullOr(S.String),
  reason: TargetedSiteCapturePolicyReason,
  robotsRef: S.NullOr(S.String),
  sitemapRef: S.NullOr(S.String),
  sourceRef: S.String,
})
export type OperatorTargetedSiteCapturePolicyProjection =
  typeof OperatorTargetedSiteCapturePolicyProjection.Type

export type TargetedSiteCapturePolicySignals = Readonly<{
  botProtectionOrLogin?: boolean | undefined
  contactSuppressed?: boolean | undefined
  customerOwnedDomain?: boolean | undefined
  manualReviewRequested?: boolean | undefined
  paidProviderRequired?: boolean | undefined
  suppressionMatched?: boolean | undefined
  unsupportedScheme?: boolean | undefined
  unsafeDomain?: boolean | undefined
}>

export type EvaluateTargetedSiteCapturePolicyInput = Readonly<{
  campaignId: string
  customerAuthorityRef?: string | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  normalizedDomain: string
  operatorActorUserId?: string | undefined
  operatorNoteRef?: string | undefined
  paidEscalationRef?: string | undefined
  prospectId?: string | undefined
  robotsRef?: string | undefined
  robotsState?: TargetedSiteCaptureRobotsState | undefined
  signals?: TargetedSiteCapturePolicySignals | undefined
  sitemapRef?: string | undefined
  sitemapState?: TargetedSiteCaptureSitemapState | undefined
  sourceRef: string
  suppressionRef?: string | undefined
}>

export type RecordTargetedSiteCapturePolicyEventInput =
  EvaluateTargetedSiteCapturePolicyInput &
    Readonly<{
      decidedAt?: string | undefined
      id?: string | undefined
      idempotencyKey: string
    }>

type TargetedSiteCapturePolicyEventRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  created_at: string
  customer_authority_ref: string | null
  decided_at: string
  decision: TargetedSiteCapturePolicyDecision
  fetchable: number
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  operator_actor_user_id: string | null
  operator_note_ref: string | null
  paid_escalation_ref: string | null
  prospect_id: string | null
  reason: TargetedSiteCapturePolicyReason
  robots_ref: string | null
  sitemap_ref: string | null
  source_ref: string
  suppression_ref: string | null
}>

type TargetedSiteCapturePolicyEventFilter =
  | Readonly<{ key: 'campaign_id'; value: string }>
  | Readonly<{ key: 'prospect_id'; value: string }>
  | Readonly<{ key: 'normalized_domain'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)|@/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) && textIsSafe(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!isSafeRef(value)) {
    throw new TargetedSiteCapturePolicyValidationError({
      reason: `${field} must be a public-safe ref without raw contact, provider, payment, wallet, suppression-note, or bypass material.`,
    })
  }
}

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteCapturePolicyValidationError({
      reason: 'normalizedDomain must be a public-safe normalized domain.',
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

  if (containsProviderSecretMaterial(json) || PROHIBITED_TEXT_PATTERN.test(json)) {
    throw new TargetedSiteCapturePolicyValidationError({
      reason:
        'metadata must not contain raw contact, provider, payment, wallet, suppression-note, or bypass material.',
    })
  }
}

const assertValidInput = (
  input: EvaluateTargetedSiteCapturePolicyInput,
): void => {
  assertSafeRef('campaignId', input.campaignId)
  assertSafeRef('prospectId', input.prospectId)
  assertSafeRef('sourceRef', input.sourceRef)
  assertSafeRef('robotsRef', input.robotsRef)
  assertSafeRef('sitemapRef', input.sitemapRef)
  assertSafeRef('suppressionRef', input.suppressionRef)
  assertSafeRef('customerAuthorityRef', input.customerAuthorityRef)
  assertSafeRef('paidEscalationRef', input.paidEscalationRef)
  assertSafeRef('operatorActorUserId', input.operatorActorUserId)
  assertSafeRef('operatorNoteRef', input.operatorNoteRef)
  assertSafeDomain(input.normalizedDomain)
  assertSafeMetadata(input.metadata)

  if (
    input.signals?.paidProviderRequired === true &&
    input.paidEscalationRef === undefined
  ) {
    throw new TargetedSiteCapturePolicyValidationError({
      reason: 'paidEscalationRef is required for paid provider escalation.',
    })
  }
}

const evaluation = (
  decision: TargetedSiteCapturePolicyDecision,
  reason: TargetedSiteCapturePolicyReason,
): TargetedSiteCapturePolicyEvaluation => ({
  decision,
  fetchable: decision === 'allowed' || decision === 'paid_escalation',
  reason,
})

export class TargetedSiteCapturePolicyValidationError extends S.TaggedErrorClass<TargetedSiteCapturePolicyValidationError>()(
  'TargetedSiteCapturePolicyValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteCapturePolicyStorageError extends S.TaggedErrorClass<TargetedSiteCapturePolicyStorageError>()(
  'TargetedSiteCapturePolicyStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export const evaluateTargetedSiteCapturePolicy = (
  input: EvaluateTargetedSiteCapturePolicyInput,
): TargetedSiteCapturePolicyEvaluation => {
  assertValidInput(input)

  const signals = input.signals ?? {}

  if (signals.unsafeDomain === true) {
    return evaluation('blocked', 'unsafe_domain')
  }

  if (signals.unsupportedScheme === true) {
    return evaluation('blocked', 'unsupported_scheme')
  }

  if (signals.suppressionMatched === true || input.suppressionRef !== undefined) {
    return evaluation('suppressed', 'suppression_match')
  }

  if (signals.contactSuppressed === true) {
    return evaluation('suppressed', 'contact_suppressed')
  }

  if (signals.customerOwnedDomain === true) {
    return evaluation('customer_owned', 'customer_owned_domain')
  }

  if (input.robotsState === 'disallowed') {
    return evaluation('disallowed', 'robots_disallowed')
  }

  if (signals.botProtectionOrLogin === true) {
    return evaluation('blocked', 'bot_protection_or_login')
  }

  if (signals.manualReviewRequested === true) {
    return evaluation('manual_review', 'operator_manual_review')
  }

  if (signals.paidProviderRequired === true) {
    return evaluation('paid_escalation', 'paid_provider_required')
  }

  if (input.robotsState === 'unavailable' || input.robotsState === 'unknown') {
    return evaluation('manual_review', 'robots_unavailable')
  }

  if (input.robotsState === 'allowed' && input.sitemapState === 'available') {
    return evaluation('allowed', 'sitemap_available')
  }

  if (input.robotsState === 'allowed') {
    return evaluation('allowed', 'robots_allowed')
  }

  return evaluation('manual_review', 'robots_unavailable')
}

export const isTargetedSiteCaptureFetchable = (
  policy: Pick<TargetedSiteCapturePolicyEventRecord, 'decision' | 'fetchable'>,
): boolean =>
  policy.fetchable === true &&
  (policy.decision === 'allowed' || policy.decision === 'paid_escalation')

export const publicTargetedSiteCapturePolicyProjection = (
  record: TargetedSiteCapturePolicyEventRecord,
): PublicTargetedSiteCapturePolicyProjection => ({
  campaignId: record.campaignId,
  decidedAt: record.decidedAt,
  decision: record.decision,
  fetchable: isTargetedSiteCaptureFetchable(record),
  normalizedDomain: record.normalizedDomain,
  prospectId: record.prospectId,
  sourceRef: record.sourceRef,
})

export const operatorTargetedSiteCapturePolicyProjection = (
  record: TargetedSiteCapturePolicyEventRecord,
): OperatorTargetedSiteCapturePolicyProjection => ({
  campaignId: record.campaignId,
  customerAuthorityRef: record.customerAuthorityRef,
  decidedAt: record.decidedAt,
  decision: record.decision,
  fetchable: isTargetedSiteCaptureFetchable(record),
  hasOperatorNoteRef: record.operatorNoteRef !== null,
  hasSuppressionRef: record.suppressionRef !== null,
  normalizedDomain: record.normalizedDomain,
  paidEscalationRef: record.paidEscalationRef,
  prospectId: record.prospectId,
  reason: record.reason,
  robotsRef: record.robotsRef,
  sitemapRef: record.sitemapRef,
  sourceRef: record.sourceRef,
})

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const eventFromRow = (
  row: TargetedSiteCapturePolicyEventRow,
): TargetedSiteCapturePolicyEventRecord => ({
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  createdAt: row.created_at,
  customerAuthorityRef: row.customer_authority_ref,
  decidedAt: row.decided_at,
  decision: row.decision,
  fetchable: row.fetchable === 1,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: metadataFromJson(row.metadata_json),
  normalizedDomain: row.normalized_domain,
  operatorActorUserId: row.operator_actor_user_id,
  operatorNoteRef: row.operator_note_ref,
  paidEscalationRef: row.paid_escalation_ref,
  prospectId: row.prospect_id,
  reason: row.reason,
  robotsRef: row.robots_ref,
  sitemapRef: row.sitemap_ref,
  sourceRef: row.source_ref,
  suppressionRef: row.suppression_ref,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteCapturePolicyEventRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_capture_policy_events
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteCapturePolicyEventRow>()

  return row === null ? null : eventFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: TargetedSiteCapturePolicyEventFilter,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCapturePolicyEventRecord>> => {
  assertSafeRef(filter.key, filter.value)
  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_capture_policy_events
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY decided_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<TargetedSiteCapturePolicyEventRow>()

  return (rows.results ?? []).map(eventFromRow)
}

export const recordTargetedSiteCapturePolicyEvent = async (
  db: D1Database,
  input: RecordTargetedSiteCapturePolicyEventInput,
): Promise<TargetedSiteCapturePolicyEventRecord> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)

  const existing = await readByIdempotencyKey(db, input.idempotencyKey)

  if (existing !== null) {
    return existing
  }

  const evaluated = evaluateTargetedSiteCapturePolicy(input)
  const nowIso = currentIsoTimestamp()
  const decidedAt = input.decidedAt ?? nowIso
  const id = input.id ?? compactRandomId('targeted_site_capture_policy')

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_capture_policy_events (
         id,
         idempotency_key,
         campaign_id,
         prospect_id,
         normalized_domain,
         source_ref,
         decision,
         fetchable,
         reason,
         robots_ref,
         sitemap_ref,
         suppression_ref,
         customer_authority_ref,
         paid_escalation_ref,
         operator_actor_user_id,
         operator_note_ref,
         metadata_json,
         decided_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      input.campaignId,
      input.prospectId ?? null,
      input.normalizedDomain,
      input.sourceRef,
      evaluated.decision,
      evaluated.fetchable ? 1 : 0,
      evaluated.reason,
      input.robotsRef ?? null,
      input.sitemapRef ?? null,
      input.suppressionRef ?? null,
      input.customerAuthorityRef ?? null,
      input.paidEscalationRef ?? null,
      input.operatorActorUserId ?? null,
      input.operatorNoteRef ?? null,
      JSON.stringify(input.metadata ?? {}),
      decidedAt,
      nowIso,
    )
    .run()

  const record = await readByIdempotencyKey(db, input.idempotencyKey)

  if (record === null) {
    throw new TargetedSiteCapturePolicyStorageError({
      operation: 'recordTargetedSiteCapturePolicyEvent.readByIdempotencyKey',
      reason: 'inserted or existing targeted Site capture policy event was not readable.',
    })
  }

  return record
}

export const listTargetedSiteCapturePolicyEventsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCapturePolicyEventRecord>> =>
  listByFilter(db, { key: 'campaign_id', value: campaignId }, limit)

export const listTargetedSiteCapturePolicyEventsByProspect = async (
  db: D1Database,
  prospectId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCapturePolicyEventRecord>> =>
  listByFilter(db, { key: 'prospect_id', value: prospectId }, limit)

export const listTargetedSiteCapturePolicyEventsByDomain = async (
  db: D1Database,
  normalizedDomain: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCapturePolicyEventRecord>> => {
  assertSafeDomain(normalizedDomain)

  return listByFilter(
    db,
    { key: 'normalized_domain', value: normalizedDomain },
    limit,
  )
}
