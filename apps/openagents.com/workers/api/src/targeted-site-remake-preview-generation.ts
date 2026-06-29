import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord, parseJsonWithSchema } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  TargetedSiteGenerationConstraints,
  type TargetedSiteRemakeBriefRecord,
} from './targeted-site-remake-brief'

export const TargetedSiteRemakePreviewState = S.Literals([
  'requested',
  'generating',
  'generated',
  'failed',
  'blocked',
  'archived',
])
export type TargetedSiteRemakePreviewState =
  typeof TargetedSiteRemakePreviewState.Type

export const TargetedSiteRemakePreviewGenerationRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  candidateSiteProjectRef: S.NullOr(S.String),
  candidateSiteVersionRef: S.NullOr(S.String),
  completedAt: S.NullOr(S.String),
  conceptSlug: S.String,
  createdAt: S.String,
  failureRef: S.NullOr(S.String),
  generatedArtifactRef: S.NullOr(S.String),
  generatedSourceRef: S.NullOr(S.String),
  generationConstraints: TargetedSiteGenerationConstraints,
  generationReceiptRef: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  legalSensitive: S.Boolean,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  previewUrl: S.NullOr(S.String),
  prospectId: S.NullOr(S.String),
  providerAdapterRunId: S.NullOr(S.String),
  qualityAuditId: S.String,
  remakeBriefId: S.String,
  renderedCaptureRunId: S.NullOr(S.String),
  requestedAt: S.String,
  sourceAuthorityPackRef: S.String,
  state: TargetedSiteRemakePreviewState,
  staticCaptureRunId: S.NullOr(S.String),
})
export type TargetedSiteRemakePreviewGenerationRecord =
  typeof TargetedSiteRemakePreviewGenerationRecord.Type

export const PublicTargetedSiteRemakePreviewGenerationProjection = S.Struct({
  campaignId: S.String,
  conceptSlug: S.String,
  normalizedDomain: S.String,
  previewUrl: S.NullOr(S.String),
  prospectId: S.NullOr(S.String),
  requestedAt: S.String,
  state: TargetedSiteRemakePreviewState,
})
export type PublicTargetedSiteRemakePreviewGenerationProjection =
  typeof PublicTargetedSiteRemakePreviewGenerationProjection.Type

export const OperatorTargetedSiteRemakePreviewGenerationProjection = S.Struct({
  campaignId: S.String,
  candidateSiteProjectRef: S.NullOr(S.String),
  candidateSiteVersionRef: S.NullOr(S.String),
  completedAt: S.NullOr(S.String),
  conceptSlug: S.String,
  failureRef: S.NullOr(S.String),
  generatedArtifactRef: S.NullOr(S.String),
  generatedSourceRef: S.NullOr(S.String),
  generationReceiptRef: S.NullOr(S.String),
  hasMetadata: S.Boolean,
  legalSensitive: S.Boolean,
  normalizedDomain: S.String,
  previewUrl: S.NullOr(S.String),
  prospectId: S.NullOr(S.String),
  providerAdapterRunId: S.NullOr(S.String),
  qualityAuditId: S.String,
  remakeBriefId: S.String,
  renderedCaptureRunId: S.NullOr(S.String),
  requestedAt: S.String,
  sourceAuthorityPackRef: S.String,
  state: TargetedSiteRemakePreviewState,
  staticCaptureRunId: S.NullOr(S.String),
})
export type OperatorTargetedSiteRemakePreviewGenerationProjection =
  typeof OperatorTargetedSiteRemakePreviewGenerationProjection.Type

export type RecordTargetedSiteRemakePreviewGenerationInput = Readonly<{
  candidateSiteProjectRef?: string | undefined
  candidateSiteVersionRef?: string | undefined
  completedAt?: string | undefined
  conceptSlug: string
  failureRef?: string | undefined
  generatedArtifactRef?: string | undefined
  generatedSourceRef?: string | undefined
  generationReceiptRef?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  previewUrl?: string | undefined
  remakeBrief: TargetedSiteRemakeBriefRecord
  requestedAt?: string | undefined
  state?: TargetedSiteRemakePreviewState | undefined
}>

type TargetedSiteRemakePreviewGenerationRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  candidate_site_project_ref: string | null
  candidate_site_version_ref: string | null
  completed_at: string | null
  concept_slug: string
  created_at: string
  failure_ref: string | null
  generated_artifact_ref: string | null
  generated_source_ref: string | null
  generation_constraints_json: string
  generation_receipt_ref: string | null
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  normalized_domain: string
  preview_url: string | null
  prospect_id: string | null
  provider_adapter_run_id: string | null
  quality_audit_id: string
  remake_brief_id: string
  rendered_capture_run_id: string | null
  requested_at: string
  source_authority_pack_ref: string
  state: TargetedSiteRemakePreviewState
  static_capture_run_id: string | null
}>

type TargetedSiteRemakePreviewFilter =
  | Readonly<{ key: 'campaign_id'; value: string }>
  | Readonly<{ key: 'prospect_id'; value: string }>
  | Readonly<{ key: 'normalized_domain'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{0,80}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?payload|browser[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)|@/i

const generatedStates: ReadonlySet<TargetedSiteRemakePreviewState> = new Set([
  'generated',
])

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) && textIsSafe(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!isSafeRef(value)) {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason: `${field} must be a public-safe ref without private capture, provider, contact, payment, wallet, copied raw content, or bypass material.`,
    })
  }
}

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
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
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason:
        'metadata must not contain private capture, provider, contact, payment, wallet, copied raw content, or bypass material.',
    })
  }
}

const expectedConceptPrefix = (
  campaignId: string,
  conceptSlug: string,
): string =>
  `https://sites.openagents.com/concepts/${encodeURIComponent(campaignId)}/${conceptSlug}`

const assertConceptSlug = (value: string): void => {
  if (!SAFE_SLUG_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason: 'conceptSlug must be a lowercase public-safe slug.',
    })
  }
}

const assertPreviewUrl = (
  campaignId: string,
  normalizedDomain: string,
  conceptSlug: string,
  value: string | undefined,
): void => {
  if (value === undefined) {
    return
  }

  let url: URL
  try {
    url = new URL(value)
  } catch {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason: 'previewUrl must be a valid concept preview URL.',
    })
  }

  if (
    url.protocol !== 'https:' ||
    url.hostname !== 'sites.openagents.com' ||
    !value.startsWith(expectedConceptPrefix(campaignId, conceptSlug))
  ) {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason:
        'previewUrl must use the OpenAgents concept preview domain and path.',
    })
  }

  if (url.hostname === normalizedDomain || value.includes(`//${normalizedDomain}`)) {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason: 'previewUrl must not impersonate the target domain.',
    })
  }
}

const assertGeneratedOutput = (
  input: RecordTargetedSiteRemakePreviewGenerationInput,
  state: TargetedSiteRemakePreviewState,
): void => {
  if (!generatedStates.has(state)) {
    return
  }

  if (input.remakeBrief.state !== 'approved_for_generation') {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason:
        'generated preview output requires an approved_for_generation remake brief.',
    })
  }

  if (
    input.previewUrl === undefined ||
    input.generatedArtifactRef === undefined ||
    input.generatedSourceRef === undefined ||
    input.candidateSiteVersionRef === undefined
  ) {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason:
        'generated preview output requires previewUrl, generatedArtifactRef, generatedSourceRef, and candidateSiteVersionRef.',
    })
  }
}

const assertConceptOnlyConstraints = (
  constraints: TargetedSiteGenerationConstraints,
): void => {
  if (
    constraints.conceptOnly !== true ||
    constraints.noFakeCaseResults !== true ||
    constraints.noFakeCredentials !== true ||
    constraints.noFakeReviews !== true ||
    constraints.noLegalAdvice !== true ||
    constraints.noMisleadingEndorsements !== true ||
    constraints.noUnverifiableGuarantees !== true
  ) {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason:
        'preview generation requires concept-only and law-firm safety constraints.',
    })
  }
}

export class TargetedSiteRemakePreviewGenerationValidationError extends S.TaggedErrorClass<TargetedSiteRemakePreviewGenerationValidationError>()(
  'TargetedSiteRemakePreviewGenerationValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteRemakePreviewGenerationStorageError extends S.TaggedErrorClass<TargetedSiteRemakePreviewGenerationStorageError>()(
  'TargetedSiteRemakePreviewGenerationStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

const assertValidInput = (
  input: RecordTargetedSiteRemakePreviewGenerationInput,
): TargetedSiteRemakePreviewState => {
  const brief = input.remakeBrief
  const state = input.state ?? 'requested'
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('campaignId', brief.campaignId)
  assertSafeRef('prospectId', brief.prospectId ?? undefined)
  assertSafeRef('remakeBriefId', brief.id)
  assertSafeRef('qualityAuditId', brief.qualityAuditId)
  assertSafeRef('staticCaptureRunId', brief.staticCaptureRunId ?? undefined)
  assertSafeRef('renderedCaptureRunId', brief.renderedCaptureRunId ?? undefined)
  assertSafeRef('providerAdapterRunId', brief.providerAdapterRunId ?? undefined)
  assertSafeRef(
    'sourceAuthorityPackRef',
    brief.sourceAuthorityPack.sourcePackRef,
  )
  assertSafeRef('generatedArtifactRef', input.generatedArtifactRef)
  assertSafeRef('generatedSourceRef', input.generatedSourceRef)
  assertSafeRef('candidateSiteProjectRef', input.candidateSiteProjectRef)
  assertSafeRef('candidateSiteVersionRef', input.candidateSiteVersionRef)
  assertSafeRef('generationReceiptRef', input.generationReceiptRef)
  assertSafeRef('failureRef', input.failureRef)
  assertSafeDomain(brief.normalizedDomain)
  assertConceptSlug(input.conceptSlug)
  assertPreviewUrl(
    brief.campaignId,
    brief.normalizedDomain,
    input.conceptSlug,
    input.previewUrl,
  )
  assertSafeMetadata(input.metadata)
  assertConceptOnlyConstraints(brief.generationConstraints)
  assertGeneratedOutput(input, state)

  if (brief.state === 'blocked' || brief.state === 'rejected') {
    throw new TargetedSiteRemakePreviewGenerationValidationError({
      reason: 'blocked or rejected remake briefs cannot produce preview records.',
    })
  }

  return state
}

const generationConstraintsFromJson = (
  value: string,
): TargetedSiteGenerationConstraints =>
  parseJsonWithSchema(TargetedSiteGenerationConstraints, value)

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const previewFromRow = (
  row: TargetedSiteRemakePreviewGenerationRow,
): TargetedSiteRemakePreviewGenerationRecord => ({
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  candidateSiteProjectRef: row.candidate_site_project_ref,
  candidateSiteVersionRef: row.candidate_site_version_ref,
  completedAt: row.completed_at,
  conceptSlug: row.concept_slug,
  createdAt: row.created_at,
  failureRef: row.failure_ref,
  generatedArtifactRef: row.generated_artifact_ref,
  generatedSourceRef: row.generated_source_ref,
  generationConstraints: generationConstraintsFromJson(
    row.generation_constraints_json,
  ),
  generationReceiptRef: row.generation_receipt_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  legalSensitive: row.legal_sensitive === 1,
  metadata: metadataFromJson(row.metadata_json),
  normalizedDomain: row.normalized_domain,
  previewUrl: row.preview_url,
  prospectId: row.prospect_id,
  providerAdapterRunId: row.provider_adapter_run_id,
  qualityAuditId: row.quality_audit_id,
  remakeBriefId: row.remake_brief_id,
  renderedCaptureRunId: row.rendered_capture_run_id,
  requestedAt: row.requested_at,
  sourceAuthorityPackRef: row.source_authority_pack_ref,
  state: row.state,
  staticCaptureRunId: row.static_capture_run_id,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteRemakePreviewGenerationRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_remake_preview_generations
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteRemakePreviewGenerationRow>()

  return row === null ? null : previewFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: TargetedSiteRemakePreviewFilter,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRemakePreviewGenerationRecord>> => {
  if (filter.key === 'normalized_domain') {
    assertSafeDomain(filter.value)
  } else {
    assertSafeRef(filter.key, filter.value)
  }

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_remake_preview_generations
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY requested_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<TargetedSiteRemakePreviewGenerationRow>()

  return (rows.results ?? []).map(previewFromRow)
}

export const publicTargetedSiteRemakePreviewGenerationProjection = (
  record: TargetedSiteRemakePreviewGenerationRecord,
): PublicTargetedSiteRemakePreviewGenerationProjection => ({
  campaignId: record.campaignId,
  conceptSlug: record.conceptSlug,
  normalizedDomain: record.normalizedDomain,
  previewUrl: record.previewUrl,
  prospectId: record.prospectId,
  requestedAt: record.requestedAt,
  state: record.state,
})

export const operatorTargetedSiteRemakePreviewGenerationProjection = (
  record: TargetedSiteRemakePreviewGenerationRecord,
): OperatorTargetedSiteRemakePreviewGenerationProjection => ({
  campaignId: record.campaignId,
  candidateSiteProjectRef: record.candidateSiteProjectRef,
  candidateSiteVersionRef: record.candidateSiteVersionRef,
  completedAt: record.completedAt,
  conceptSlug: record.conceptSlug,
  failureRef: record.failureRef,
  generatedArtifactRef: record.generatedArtifactRef,
  generatedSourceRef: record.generatedSourceRef,
  generationReceiptRef: record.generationReceiptRef,
  hasMetadata: Object.keys(record.metadata).length > 0,
  legalSensitive: record.legalSensitive,
  normalizedDomain: record.normalizedDomain,
  previewUrl: record.previewUrl,
  prospectId: record.prospectId,
  providerAdapterRunId: record.providerAdapterRunId,
  qualityAuditId: record.qualityAuditId,
  remakeBriefId: record.remakeBriefId,
  renderedCaptureRunId: record.renderedCaptureRunId,
  requestedAt: record.requestedAt,
  sourceAuthorityPackRef: record.sourceAuthorityPackRef,
  state: record.state,
  staticCaptureRunId: record.staticCaptureRunId,
})

export const recordTargetedSiteRemakePreviewGeneration = async (
  db: D1Database,
  input: RecordTargetedSiteRemakePreviewGenerationInput,
): Promise<TargetedSiteRemakePreviewGenerationRecord> => {
  const state = assertValidInput(input)
  const existing = await readByIdempotencyKey(db, input.idempotencyKey)

  if (existing !== null) {
    return existing
  }

  const nowIso = currentIsoTimestamp()
  const id = input.id ?? compactRandomId('targeted_site_remake_preview')

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_remake_preview_generations (
         id,
         idempotency_key,
         campaign_id,
         prospect_id,
         normalized_domain,
         remake_brief_id,
         quality_audit_id,
         static_capture_run_id,
         rendered_capture_run_id,
         provider_adapter_run_id,
         state,
         preview_url,
         concept_slug,
         source_authority_pack_ref,
         generated_artifact_ref,
         generated_source_ref,
         candidate_site_project_ref,
         candidate_site_version_ref,
         generation_receipt_ref,
         failure_ref,
         legal_sensitive,
         generation_constraints_json,
         metadata_json,
         requested_at,
         completed_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      input.remakeBrief.campaignId,
      input.remakeBrief.prospectId,
      input.remakeBrief.normalizedDomain,
      input.remakeBrief.id,
      input.remakeBrief.qualityAuditId,
      input.remakeBrief.staticCaptureRunId,
      input.remakeBrief.renderedCaptureRunId,
      input.remakeBrief.providerAdapterRunId,
      state,
      input.previewUrl ?? null,
      input.conceptSlug,
      input.remakeBrief.sourceAuthorityPack.sourcePackRef,
      input.generatedArtifactRef ?? null,
      input.generatedSourceRef ?? null,
      input.candidateSiteProjectRef ?? null,
      input.candidateSiteVersionRef ?? null,
      input.generationReceiptRef ?? null,
      input.failureRef ?? null,
      input.remakeBrief.legalSensitive ? 1 : 0,
      JSON.stringify(input.remakeBrief.generationConstraints),
      JSON.stringify(input.metadata ?? {}),
      input.requestedAt ?? nowIso,
      input.completedAt ?? null,
      nowIso,
    )
    .run()

  const record = await readByIdempotencyKey(db, input.idempotencyKey)

  if (record === null) {
    throw new TargetedSiteRemakePreviewGenerationStorageError({
      operation:
        'recordTargetedSiteRemakePreviewGeneration.readByIdempotencyKey',
      reason:
        'inserted or existing targeted Site remake preview generation was not readable.',
    })
  }

  return record
}

export const listTargetedSiteRemakePreviewGenerationsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRemakePreviewGenerationRecord>> =>
  listByFilter(db, { key: 'campaign_id', value: campaignId }, limit)

export const listTargetedSiteRemakePreviewGenerationsByProspect = async (
  db: D1Database,
  prospectId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRemakePreviewGenerationRecord>> =>
  listByFilter(db, { key: 'prospect_id', value: prospectId }, limit)

export const listTargetedSiteRemakePreviewGenerationsByDomain = async (
  db: D1Database,
  normalizedDomain: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRemakePreviewGenerationRecord>> =>
  listByFilter(db, { key: 'normalized_domain', value: normalizedDomain }, limit)
