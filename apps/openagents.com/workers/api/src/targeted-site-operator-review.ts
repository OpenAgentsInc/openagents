import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import {
  compactRandomId,
  currentIsoTimestamp,
  normalizeIsoTimestamp,
} from './runtime-primitives'
import type { TargetedSiteRemakeBriefRecord } from './targeted-site-remake-brief'
import type { TargetedSiteRemakePreviewGenerationRecord } from './targeted-site-remake-preview-generation'

export const TargetedSiteOperatorReviewDecision = S.Literals([
  'approve_preview',
  'reject_preview',
  'request_regeneration',
  'skip_target',
  'approve_outreach',
  'block_target',
  'archive',
])
export type TargetedSiteOperatorReviewDecision =
  typeof TargetedSiteOperatorReviewDecision.Type

export const TargetedSiteOperatorReviewState = S.Literals([
  'preview_approved',
  'preview_rejected',
  'regeneration_requested',
  'target_skipped',
  'outreach_approved',
  'target_blocked',
  'archived',
])
export type TargetedSiteOperatorReviewState =
  typeof TargetedSiteOperatorReviewState.Type

export const TargetedSiteSuppressionState = S.Literals([
  'unknown',
  'clear',
  'suppressed',
  'manual_review',
])
export type TargetedSiteSuppressionState =
  typeof TargetedSiteSuppressionState.Type

export const TargetedSiteOperatorActionAvailability = S.Struct({
  decision: TargetedSiteOperatorReviewDecision,
  enabled: S.Boolean,
  reason: S.NullOr(S.String),
})
export type TargetedSiteOperatorActionAvailability =
  typeof TargetedSiteOperatorActionAvailability.Type

export const TargetedSiteOperatorReviewViewModel = S.Struct({
  actionAvailability: S.Array(TargetedSiteOperatorActionAvailability),
  auditScoreLabel: S.String,
  campaignId: S.String,
  captureSummary: S.Struct({
    policyState: S.String,
    providerAdapterRunId: S.NullOr(S.String),
    renderedCaptureRunId: S.NullOr(S.String),
    staticCaptureRunId: S.NullOr(S.String),
  }),
  domain: S.String,
  meetingCtaReady: S.Boolean,
  preparedAtLabel: S.String,
  previewState: S.String,
  previewUrl: S.NullOr(S.String),
  remakeBriefState: S.String,
  sourceAuthorityCardCount: S.Number,
  suppressionState: TargetedSiteSuppressionState,
})
export type TargetedSiteOperatorReviewViewModel =
  typeof TargetedSiteOperatorReviewViewModel.Type

export const TargetedSiteOperatorReviewEventRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  createdAt: S.String,
  decidedAt: S.String,
  decision: TargetedSiteOperatorReviewDecision,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  idempotencyKey: S.String,
  meetingCtaRef: S.NullOr(S.String),
  metadata: S.Record(S.String, S.Unknown),
  nextState: TargetedSiteOperatorReviewState,
  normalizedDomain: S.String,
  operatorActorUserId: S.String,
  operatorNoteRef: S.NullOr(S.String),
  outreachDraftRef: S.NullOr(S.String),
  previousState: S.String,
  previewGenerationId: S.String,
  prospectId: S.NullOr(S.String),
  remakeBriefId: S.String,
  suppressionState: TargetedSiteSuppressionState,
})
export type TargetedSiteOperatorReviewEventRecord =
  typeof TargetedSiteOperatorReviewEventRecord.Type

export const PublicTargetedSiteOperatorReviewProjection = S.Struct({
  campaignId: S.String,
  normalizedDomain: S.String,
  previewGenerationId: S.String,
  state: TargetedSiteOperatorReviewState,
})
export type PublicTargetedSiteOperatorReviewProjection =
  typeof PublicTargetedSiteOperatorReviewProjection.Type

export const OperatorTargetedSiteOperatorReviewProjection = S.Struct({
  campaignId: S.String,
  decidedAt: S.String,
  decision: TargetedSiteOperatorReviewDecision,
  evidenceRefs: S.Array(S.String),
  hasMetadata: S.Boolean,
  meetingCtaRef: S.NullOr(S.String),
  nextState: TargetedSiteOperatorReviewState,
  normalizedDomain: S.String,
  operatorActorUserId: S.String,
  operatorNoteRef: S.NullOr(S.String),
  outreachDraftRef: S.NullOr(S.String),
  previewGenerationId: S.String,
  prospectId: S.NullOr(S.String),
  remakeBriefId: S.String,
  suppressionState: TargetedSiteSuppressionState,
})
export type OperatorTargetedSiteOperatorReviewProjection =
  typeof OperatorTargetedSiteOperatorReviewProjection.Type

export type BuildTargetedSiteOperatorReviewViewModelInput = Readonly<{
  auditOverallScore: number
  capturePolicyState: string
  meetingCtaRef?: string | undefined
  outreachDraftRef?: string | undefined
  preview: TargetedSiteRemakePreviewGenerationRecord
  remakeBrief: TargetedSiteRemakeBriefRecord
  suppressionState: TargetedSiteSuppressionState
}>

export type RecordTargetedSiteOperatorReviewEventInput = Readonly<{
  decision: TargetedSiteOperatorReviewDecision
  evidenceRefs: ReadonlyArray<string>
  id?: string | undefined
  idempotencyKey: string
  meetingCtaRef?: string | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  operatorActorUserId: string
  operatorNoteRef?: string | undefined
  outreachDraftRef?: string | undefined
  previousState: string
  preview: TargetedSiteRemakePreviewGenerationRecord
  suppressionState: TargetedSiteSuppressionState
}>

type TargetedSiteOperatorReviewEventRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  created_at: string
  decided_at: string
  decision: TargetedSiteOperatorReviewDecision
  evidence_refs_json: string
  id: string
  idempotency_key: string
  meeting_cta_ref: string | null
  metadata_json: string
  next_state: TargetedSiteOperatorReviewState
  normalized_domain: string
  operator_actor_user_id: string
  operator_note_ref: string | null
  outreach_draft_ref: string | null
  previous_state: string
  preview_generation_id: string
  prospect_id: string | null
  remake_brief_id: string
  suppression_state: TargetedSiteSuppressionState
}>

type TargetedSiteOperatorReviewEventFilter =
  | Readonly<{ key: 'campaign_id'; value: string }>
  | Readonly<{ key: 'preview_generation_id'; value: string }>
  | Readonly<{ key: 'normalized_domain'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?payload|browser[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)|@/i

const nextStateByDecision: Readonly<
  Record<TargetedSiteOperatorReviewDecision, TargetedSiteOperatorReviewState>
> = {
  approve_outreach: 'outreach_approved',
  approve_preview: 'preview_approved',
  archive: 'archived',
  block_target: 'target_blocked',
  reject_preview: 'preview_rejected',
  request_regeneration: 'regeneration_requested',
  skip_target: 'target_skipped',
}

const allDecisions: ReadonlyArray<TargetedSiteOperatorReviewDecision> = [
  'approve_preview',
  'reject_preview',
  'request_regeneration',
  'skip_target',
  'approve_outreach',
  'block_target',
  'archive',
]

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) && textIsSafe(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!isSafeRef(value)) {
    throw new TargetedSiteOperatorReviewValidationError({
      reason: `${field} must be a public-safe ref without private capture, provider, contact, payment, wallet, or bypass material.`,
    })
  }
}

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteOperatorReviewValidationError({
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
    throw new TargetedSiteOperatorReviewValidationError({
      reason:
        'metadata must not contain private capture, provider, contact, payment, wallet, or bypass material.',
    })
  }
}

const formatOperatorTimestamp = (timestamp: string): string => {
  try {
    return normalizeIsoTimestamp(timestamp).slice(0, 16).replace('T', ' ') + ' UTC'
  } catch {
    return 'Unknown time'
  }
}

const availabilityReason = (
  decision: TargetedSiteOperatorReviewDecision,
  input: BuildTargetedSiteOperatorReviewViewModelInput,
): string | null => {
  if (decision === 'approve_preview' && input.preview.state !== 'generated') {
    return 'Preview must be generated before approval.'
  }

  if (decision === 'approve_outreach') {
    if (input.preview.state !== 'generated' || input.preview.previewUrl === null) {
      return 'Generated preview URL is required.'
    }

    if (input.outreachDraftRef === undefined) {
      return 'Outreach draft ref is required.'
    }

    if (input.meetingCtaRef === undefined) {
      return 'Meeting CTA ref is required.'
    }

    if (input.suppressionState !== 'clear') {
      return 'Suppression state must be clear.'
    }
  }

  if (decision === 'request_regeneration' && input.remakeBrief.state === 'blocked') {
    return 'Blocked briefs cannot request regeneration.'
  }

  return null
}

export class TargetedSiteOperatorReviewValidationError extends S.TaggedErrorClass<TargetedSiteOperatorReviewValidationError>()(
  'TargetedSiteOperatorReviewValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteOperatorReviewStorageError extends S.TaggedErrorClass<TargetedSiteOperatorReviewStorageError>()(
  'TargetedSiteOperatorReviewStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export const buildTargetedSiteOperatorReviewViewModel = (
  input: BuildTargetedSiteOperatorReviewViewModelInput,
): TargetedSiteOperatorReviewViewModel => ({
  actionAvailability: allDecisions.map(decision => {
    const reason = availabilityReason(decision, input)

    return {
      decision,
      enabled: reason === null,
      reason,
    }
  }),
  auditScoreLabel: `${Math.round(input.auditOverallScore)} / 100`,
  campaignId: input.preview.campaignId,
  captureSummary: {
    policyState: input.capturePolicyState,
    providerAdapterRunId: input.preview.providerAdapterRunId,
    renderedCaptureRunId: input.preview.renderedCaptureRunId,
    staticCaptureRunId: input.preview.staticCaptureRunId,
  },
  domain: input.preview.normalizedDomain,
  meetingCtaReady: input.meetingCtaRef !== undefined,
  preparedAtLabel: formatOperatorTimestamp(input.preview.requestedAt),
  previewState: input.preview.state,
  previewUrl: input.preview.previewUrl,
  remakeBriefState: input.remakeBrief.state,
  sourceAuthorityCardCount: input.remakeBrief.sourceAuthorityPack.cards.length,
  suppressionState: input.suppressionState,
})

const assertDecisionAllowed = (
  input: RecordTargetedSiteOperatorReviewEventInput,
): void => {
  const viewModel = buildTargetedSiteOperatorReviewViewModel({
    auditOverallScore: 0,
    capturePolicyState: 'unknown',
    meetingCtaRef: input.meetingCtaRef,
    outreachDraftRef: input.outreachDraftRef,
    preview: input.preview,
    remakeBrief: {
      archivedAt: null,
      auditFindingRefs: [],
      campaignId: input.preview.campaignId,
      copiedImageRefs: [],
      copiedTextRefs: [],
      createdAt: input.preview.createdAt,
      generationConstraints: input.preview.generationConstraints,
      id: input.preview.remakeBriefId,
      idempotencyKey: input.preview.remakeBriefId,
      legalSensitive: input.preview.legalSensitive,
      metadata: {},
      normalizedDomain: input.preview.normalizedDomain,
      originalScreenshotRefs: [],
      preparedAt: input.preview.requestedAt,
      prospectId: input.preview.prospectId,
      providerAdapterRunId: input.preview.providerAdapterRunId,
      qualityAuditId: input.preview.qualityAuditId,
      qualityAuditRecommendation: null,
      renderedCaptureRunId: input.preview.renderedCaptureRunId,
      reviewedAt: null,
      sourceAuthorityPack: {
        cards: [],
        prohibitedClaims: [],
        requiredDisclosures: [],
        sourcePackRef: input.preview.sourceAuthorityPackRef,
      },
      state: input.preview.state === 'blocked' ? 'blocked' : 'approved_for_generation',
      staticCaptureRunId: input.preview.staticCaptureRunId,
    },
    suppressionState: input.suppressionState,
  })
  const action = viewModel.actionAvailability.find(
    item => item.decision === input.decision,
  )

  if (action?.enabled !== true) {
    throw new TargetedSiteOperatorReviewValidationError({
      reason: action?.reason ?? 'operator review decision is not currently enabled.',
    })
  }
}

const assertValidInput = (
  input: RecordTargetedSiteOperatorReviewEventInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('campaignId', input.preview.campaignId)
  assertSafeRef('prospectId', input.preview.prospectId ?? undefined)
  assertSafeRef('remakeBriefId', input.preview.remakeBriefId)
  assertSafeRef('previewGenerationId', input.preview.id)
  assertSafeRef('operatorActorUserId', input.operatorActorUserId)
  assertSafeRef('operatorNoteRef', input.operatorNoteRef)
  assertSafeRef('outreachDraftRef', input.outreachDraftRef)
  assertSafeRef('meetingCtaRef', input.meetingCtaRef)
  input.evidenceRefs.forEach(ref => assertSafeRef('evidenceRefs', ref))
  assertSafeDomain(input.preview.normalizedDomain)
  assertSafeMetadata(input.metadata)
  assertDecisionAllowed(input)

  if (input.evidenceRefs.length === 0) {
    throw new TargetedSiteOperatorReviewValidationError({
      reason: 'operator review decisions require at least one evidence ref.',
    })
  }
}

const eventFromRow = (
  row: TargetedSiteOperatorReviewEventRow,
): TargetedSiteOperatorReviewEventRecord => ({
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  createdAt: row.created_at,
  decidedAt: row.decided_at,
  decision: row.decision,
  evidenceRefs: [...parseJsonStringArray(row.evidence_refs_json)],
  id: row.id,
  idempotencyKey: row.idempotency_key,
  meetingCtaRef: row.meeting_cta_ref,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  nextState: row.next_state,
  normalizedDomain: row.normalized_domain,
  operatorActorUserId: row.operator_actor_user_id,
  operatorNoteRef: row.operator_note_ref,
  outreachDraftRef: row.outreach_draft_ref,
  previousState: row.previous_state,
  previewGenerationId: row.preview_generation_id,
  prospectId: row.prospect_id,
  remakeBriefId: row.remake_brief_id,
  suppressionState: row.suppression_state,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteOperatorReviewEventRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_operator_review_events
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteOperatorReviewEventRow>()

  return row === null ? null : eventFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: TargetedSiteOperatorReviewEventFilter,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteOperatorReviewEventRecord>> => {
  if (filter.key === 'normalized_domain') {
    assertSafeDomain(filter.value)
  } else {
    assertSafeRef(filter.key, filter.value)
  }

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_operator_review_events
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY decided_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<TargetedSiteOperatorReviewEventRow>()

  return (rows.results ?? []).map(eventFromRow)
}

export const publicTargetedSiteOperatorReviewProjection = (
  record: TargetedSiteOperatorReviewEventRecord,
): PublicTargetedSiteOperatorReviewProjection => ({
  campaignId: record.campaignId,
  normalizedDomain: record.normalizedDomain,
  previewGenerationId: record.previewGenerationId,
  state: record.nextState,
})

export const operatorTargetedSiteOperatorReviewProjection = (
  record: TargetedSiteOperatorReviewEventRecord,
): OperatorTargetedSiteOperatorReviewProjection => ({
  campaignId: record.campaignId,
  decidedAt: record.decidedAt,
  decision: record.decision,
  evidenceRefs: record.evidenceRefs,
  hasMetadata: Object.keys(record.metadata).length > 0,
  meetingCtaRef: record.meetingCtaRef,
  nextState: record.nextState,
  normalizedDomain: record.normalizedDomain,
  operatorActorUserId: record.operatorActorUserId,
  operatorNoteRef: record.operatorNoteRef,
  outreachDraftRef: record.outreachDraftRef,
  previewGenerationId: record.previewGenerationId,
  prospectId: record.prospectId,
  remakeBriefId: record.remakeBriefId,
  suppressionState: record.suppressionState,
})

export const recordTargetedSiteOperatorReviewEvent = async (
  db: D1Database,
  input: RecordTargetedSiteOperatorReviewEventInput,
): Promise<TargetedSiteOperatorReviewEventRecord> => {
  assertValidInput(input)
  const existing = await readByIdempotencyKey(db, input.idempotencyKey)

  if (existing !== null) {
    return existing
  }

  const nowIso = currentIsoTimestamp()
  const id = input.id ?? compactRandomId('targeted_site_operator_review')

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_operator_review_events (
         id,
         idempotency_key,
         campaign_id,
         prospect_id,
         normalized_domain,
         remake_brief_id,
         preview_generation_id,
         decision,
         previous_state,
         next_state,
         operator_actor_user_id,
         operator_note_ref,
         outreach_draft_ref,
         meeting_cta_ref,
         suppression_state,
         evidence_refs_json,
         metadata_json,
         decided_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      input.preview.campaignId,
      input.preview.prospectId,
      input.preview.normalizedDomain,
      input.preview.remakeBriefId,
      input.preview.id,
      input.decision,
      input.previousState,
      nextStateByDecision[input.decision],
      input.operatorActorUserId,
      input.operatorNoteRef ?? null,
      input.outreachDraftRef ?? null,
      input.meetingCtaRef ?? null,
      input.suppressionState,
      JSON.stringify([...input.evidenceRefs].slice(0, 100)),
      JSON.stringify(input.metadata ?? {}),
      nowIso,
      nowIso,
    )
    .run()

  const record = await readByIdempotencyKey(db, input.idempotencyKey)

  if (record === null) {
    throw new TargetedSiteOperatorReviewStorageError({
      operation: 'recordTargetedSiteOperatorReviewEvent.readByIdempotencyKey',
      reason: 'inserted or existing targeted Site operator review was not readable.',
    })
  }

  return record
}

export const listTargetedSiteOperatorReviewsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteOperatorReviewEventRecord>> =>
  listByFilter(db, { key: 'campaign_id', value: campaignId }, limit)

export const listTargetedSiteOperatorReviewsByPreview = async (
  db: D1Database,
  previewGenerationId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteOperatorReviewEventRecord>> =>
  listByFilter(db, { key: 'preview_generation_id', value: previewGenerationId }, limit)

export const listTargetedSiteOperatorReviewsByDomain = async (
  db: D1Database,
  normalizedDomain: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteOperatorReviewEventRecord>> =>
  listByFilter(db, { key: 'normalized_domain', value: normalizedDomain }, limit)
