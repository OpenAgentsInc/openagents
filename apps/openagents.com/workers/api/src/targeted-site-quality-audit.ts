import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const TargetedSiteQualityAuditState = S.Literals([
  'draft',
  'ready',
  'manual_review',
  'blocked',
  'archived',
])
export type TargetedSiteQualityAuditState =
  typeof TargetedSiteQualityAuditState.Type

export const TargetedSiteQualityAuditRecommendation = S.Literals([
  'skip',
  'monitor',
  'remake_candidate',
  'manual_review',
  'blocked',
])
export type TargetedSiteQualityAuditRecommendation =
  typeof TargetedSiteQualityAuditRecommendation.Type

export const TargetedSiteQualityDimensions = S.Struct({
  accessibility: S.Number,
  contentQuality: S.Number,
  ctaClarity: S.Number,
  designAge: S.Number,
  imageQuality: S.Number,
  informationArchitecture: S.Number,
  legalSensitiveClaims: S.Number,
  localSeoMetadata: S.Number,
  mobileResponsiveRisk: S.Number,
  performanceRisk: S.Number,
  staleBrokenMixedContent: S.Number,
  trustSignals: S.Number,
})
export type TargetedSiteQualityDimensions =
  typeof TargetedSiteQualityDimensions.Type

export const TargetedSiteQualityAuditRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  auditedAt: S.String,
  campaignId: S.String,
  createdAt: S.String,
  dimensions: TargetedSiteQualityDimensions,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  idempotencyKey: S.String,
  legalSensitive: S.Boolean,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  overallScore: S.Number,
  prospectId: S.NullOr(S.String),
  providerAdapterRunId: S.NullOr(S.String),
  recommendation: TargetedSiteQualityAuditRecommendation,
  renderedCaptureRunId: S.NullOr(S.String),
  state: TargetedSiteQualityAuditState,
  staticCaptureRunId: S.NullOr(S.String),
})
export type TargetedSiteQualityAuditRecord =
  typeof TargetedSiteQualityAuditRecord.Type

export const PublicTargetedSiteQualityAuditProjection = S.Struct({
  auditedAt: S.String,
  campaignId: S.String,
  evidenceCount: S.Number,
  legalSensitive: S.Boolean,
  normalizedDomain: S.String,
  overallScore: S.Number,
  prospectId: S.NullOr(S.String),
  recommendation: TargetedSiteQualityAuditRecommendation,
  state: TargetedSiteQualityAuditState,
})
export type PublicTargetedSiteQualityAuditProjection =
  typeof PublicTargetedSiteQualityAuditProjection.Type

export const OperatorTargetedSiteQualityAuditProjection = S.Struct({
  auditedAt: S.String,
  campaignId: S.String,
  dimensions: TargetedSiteQualityDimensions,
  evidenceRefs: S.Array(S.String),
  hasMetadata: S.Boolean,
  legalSensitive: S.Boolean,
  normalizedDomain: S.String,
  overallScore: S.Number,
  prospectId: S.NullOr(S.String),
  providerAdapterRunId: S.NullOr(S.String),
  recommendation: TargetedSiteQualityAuditRecommendation,
  renderedCaptureRunId: S.NullOr(S.String),
  state: TargetedSiteQualityAuditState,
  staticCaptureRunId: S.NullOr(S.String),
})
export type OperatorTargetedSiteQualityAuditProjection =
  typeof OperatorTargetedSiteQualityAuditProjection.Type

export type TargetedSiteQualityDimensionInput =
  Partial<Record<keyof TargetedSiteQualityDimensions, number>>

export type EvaluateTargetedSiteQualityAuditInput = Readonly<{
  blocked?: boolean | undefined
  dimensions: TargetedSiteQualityDimensionInput
  legalSensitive?: boolean | undefined
}>

export type TargetedSiteQualityAuditEvaluation = Readonly<{
  dimensions: TargetedSiteQualityDimensions
  legalSensitive: boolean
  overallScore: number
  recommendation: TargetedSiteQualityAuditRecommendation
  state: TargetedSiteQualityAuditState
}>

export type RecordTargetedSiteQualityAuditInput =
  EvaluateTargetedSiteQualityAuditInput &
    Readonly<{
      auditedAt?: string | undefined
      campaignId: string
      evidenceRefs: ReadonlyArray<string>
      id?: string | undefined
      idempotencyKey: string
      metadata?: Readonly<Record<string, unknown>> | undefined
      normalizedDomain: string
      prospectId?: string | undefined
      providerAdapterRunId?: string | undefined
      renderedCaptureRunId?: string | undefined
      staticCaptureRunId?: string | undefined
    }>

type TargetedSiteQualityAuditRow = Readonly<{
  archived_at: string | null
  audited_at: string
  campaign_id: string
  created_at: string
  dimensions_json: string
  evidence_refs_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  normalized_domain: string
  overall_score: number
  prospect_id: string | null
  provider_adapter_run_id: string | null
  recommendation: TargetedSiteQualityAuditRecommendation
  rendered_capture_run_id: string | null
  state: TargetedSiteQualityAuditState
  static_capture_run_id: string | null
}>

type TargetedSiteQualityAuditFilter =
  | Readonly<{ key: 'campaign_id'; value: string }>
  | Readonly<{ key: 'prospect_id'; value: string }>
  | Readonly<{ key: 'normalized_domain'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|raw[_ -]?payload|browser[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)|@/i

const dimensionKeys: ReadonlyArray<keyof TargetedSiteQualityDimensions> = [
  'accessibility',
  'contentQuality',
  'ctaClarity',
  'designAge',
  'imageQuality',
  'informationArchitecture',
  'legalSensitiveClaims',
  'localSeoMetadata',
  'mobileResponsiveRisk',
  'performanceRisk',
  'staleBrokenMixedContent',
  'trustSignals',
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
    throw new TargetedSiteQualityAuditValidationError({
      reason: `${field} must be a public-safe ref without private capture, provider, contact, payment, wallet, or bypass material.`,
    })
  }
}

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteQualityAuditValidationError({
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
    throw new TargetedSiteQualityAuditValidationError({
      reason:
        'metadata must not contain private capture, provider, contact, payment, wallet, or bypass material.',
    })
  }
}

const clampScore = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return 0
  }

  return Math.max(0, Math.min(100, Number(value.toFixed(2))))
}

const normalizeDimensions = (
  dimensions: TargetedSiteQualityDimensionInput,
): TargetedSiteQualityDimensions => ({
  accessibility: clampScore(dimensions.accessibility),
  contentQuality: clampScore(dimensions.contentQuality),
  ctaClarity: clampScore(dimensions.ctaClarity),
  designAge: clampScore(dimensions.designAge),
  imageQuality: clampScore(dimensions.imageQuality),
  informationArchitecture: clampScore(dimensions.informationArchitecture),
  legalSensitiveClaims: clampScore(dimensions.legalSensitiveClaims),
  localSeoMetadata: clampScore(dimensions.localSeoMetadata),
  mobileResponsiveRisk: clampScore(dimensions.mobileResponsiveRisk),
  performanceRisk: clampScore(dimensions.performanceRisk),
  staleBrokenMixedContent: clampScore(dimensions.staleBrokenMixedContent),
  trustSignals: clampScore(dimensions.trustSignals),
})

const overallScore = (dimensions: TargetedSiteQualityDimensions): number =>
  Number(
    (
      dimensionKeys.reduce((total, key) => total + dimensions[key], 0) /
      dimensionKeys.length
    ).toFixed(2),
  )

const recommendationFor = (
  score: number,
  legalSensitive: boolean,
  blocked: boolean,
): readonly [
  TargetedSiteQualityAuditState,
  TargetedSiteQualityAuditRecommendation,
] => {
  if (blocked) {
    return ['blocked', 'blocked']
  }

  if (legalSensitive) {
    return ['manual_review', 'manual_review']
  }

  if (score <= 65) {
    return ['ready', 'remake_candidate']
  }

  if (score <= 80) {
    return ['ready', 'monitor']
  }

  return ['ready', 'skip']
}

export class TargetedSiteQualityAuditValidationError extends S.TaggedErrorClass<TargetedSiteQualityAuditValidationError>()(
  'TargetedSiteQualityAuditValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteQualityAuditStorageError extends S.TaggedErrorClass<TargetedSiteQualityAuditStorageError>()(
  'TargetedSiteQualityAuditStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export const evaluateTargetedSiteQualityAudit = (
  input: EvaluateTargetedSiteQualityAuditInput,
): TargetedSiteQualityAuditEvaluation => {
  const dimensions = normalizeDimensions(input.dimensions)
  const legalSensitive =
    input.legalSensitive === true || dimensions.legalSensitiveClaims > 0
  const score = overallScore(dimensions)
  const [state, recommendation] = recommendationFor(
    score,
    legalSensitive,
    input.blocked === true,
  )

  return {
    dimensions,
    legalSensitive,
    overallScore: score,
    recommendation,
    state,
  }
}

const assertValidInput = (
  input: RecordTargetedSiteQualityAuditInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('campaignId', input.campaignId)
  assertSafeRef('prospectId', input.prospectId)
  assertSafeRef('staticCaptureRunId', input.staticCaptureRunId)
  assertSafeRef('renderedCaptureRunId', input.renderedCaptureRunId)
  assertSafeRef('providerAdapterRunId', input.providerAdapterRunId)
  input.evidenceRefs.forEach(ref => assertSafeRef('evidenceRefs', ref))
  assertSafeDomain(input.normalizedDomain)
  assertSafeMetadata(input.metadata)

  if (input.evidenceRefs.length === 0) {
    throw new TargetedSiteQualityAuditValidationError({
      reason: 'at least one evidence ref is required for quality audits.',
    })
  }
}

const dimensionsFromJson = (json: string): TargetedSiteQualityDimensions =>
  normalizeDimensions(parseJsonRecord(json) as TargetedSiteQualityDimensionInput)

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const auditFromRow = (
  row: TargetedSiteQualityAuditRow,
): TargetedSiteQualityAuditRecord => ({
  archivedAt: row.archived_at,
  auditedAt: row.audited_at,
  campaignId: row.campaign_id,
  createdAt: row.created_at,
  dimensions: dimensionsFromJson(row.dimensions_json),
  evidenceRefs: [...parseJsonStringArray(row.evidence_refs_json)],
  id: row.id,
  idempotencyKey: row.idempotency_key,
  legalSensitive: row.legal_sensitive === 1,
  metadata: metadataFromJson(row.metadata_json),
  normalizedDomain: row.normalized_domain,
  overallScore: Number(row.overall_score),
  prospectId: row.prospect_id,
  providerAdapterRunId: row.provider_adapter_run_id,
  recommendation: row.recommendation,
  renderedCaptureRunId: row.rendered_capture_run_id,
  state: row.state,
  staticCaptureRunId: row.static_capture_run_id,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteQualityAuditRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_quality_audits
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteQualityAuditRow>()

  return row === null ? null : auditFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: TargetedSiteQualityAuditFilter,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteQualityAuditRecord>> => {
  if (filter.key === 'normalized_domain') {
    assertSafeDomain(filter.value)
  } else {
    assertSafeRef(filter.key, filter.value)
  }

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_quality_audits
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY audited_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<TargetedSiteQualityAuditRow>()

  return (rows.results ?? []).map(auditFromRow)
}

export const publicTargetedSiteQualityAuditProjection = (
  record: TargetedSiteQualityAuditRecord,
): PublicTargetedSiteQualityAuditProjection => ({
  auditedAt: record.auditedAt,
  campaignId: record.campaignId,
  evidenceCount: record.evidenceRefs.length,
  legalSensitive: record.legalSensitive,
  normalizedDomain: record.normalizedDomain,
  overallScore: record.overallScore,
  prospectId: record.prospectId,
  recommendation: record.recommendation,
  state: record.state,
})

export const operatorTargetedSiteQualityAuditProjection = (
  record: TargetedSiteQualityAuditRecord,
): OperatorTargetedSiteQualityAuditProjection => ({
  auditedAt: record.auditedAt,
  campaignId: record.campaignId,
  dimensions: record.dimensions,
  evidenceRefs: record.evidenceRefs,
  hasMetadata: Object.keys(record.metadata).length > 0,
  legalSensitive: record.legalSensitive,
  normalizedDomain: record.normalizedDomain,
  overallScore: record.overallScore,
  prospectId: record.prospectId,
  providerAdapterRunId: record.providerAdapterRunId,
  recommendation: record.recommendation,
  renderedCaptureRunId: record.renderedCaptureRunId,
  state: record.state,
  staticCaptureRunId: record.staticCaptureRunId,
})

export const recordTargetedSiteQualityAudit = async (
  db: D1Database,
  input: RecordTargetedSiteQualityAuditInput,
): Promise<TargetedSiteQualityAuditRecord> => {
  assertValidInput(input)

  const existing = await readByIdempotencyKey(db, input.idempotencyKey)

  if (existing !== null) {
    return existing
  }

  const nowIso = currentIsoTimestamp()
  const evaluated = evaluateTargetedSiteQualityAudit(input)
  const id = input.id ?? compactRandomId('targeted_site_quality_audit')

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_quality_audits (
         id,
         idempotency_key,
         campaign_id,
         prospect_id,
         normalized_domain,
         static_capture_run_id,
         rendered_capture_run_id,
         provider_adapter_run_id,
         state,
         recommendation,
         overall_score,
         legal_sensitive,
         dimensions_json,
         evidence_refs_json,
         metadata_json,
         audited_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      input.campaignId,
      input.prospectId ?? null,
      input.normalizedDomain,
      input.staticCaptureRunId ?? null,
      input.renderedCaptureRunId ?? null,
      input.providerAdapterRunId ?? null,
      evaluated.state,
      evaluated.recommendation,
      evaluated.overallScore,
      evaluated.legalSensitive ? 1 : 0,
      JSON.stringify(evaluated.dimensions),
      JSON.stringify([...input.evidenceRefs].slice(0, 100)),
      JSON.stringify(input.metadata ?? {}),
      input.auditedAt ?? nowIso,
      nowIso,
    )
    .run()

  const record = await readByIdempotencyKey(db, input.idempotencyKey)

  if (record === null) {
    throw new TargetedSiteQualityAuditStorageError({
      operation: 'recordTargetedSiteQualityAudit.readByIdempotencyKey',
      reason: 'inserted or existing targeted Site quality audit was not readable.',
    })
  }

  return record
}

export const listTargetedSiteQualityAuditsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteQualityAuditRecord>> =>
  listByFilter(db, { key: 'campaign_id', value: campaignId }, limit)

export const listTargetedSiteQualityAuditsByProspect = async (
  db: D1Database,
  prospectId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteQualityAuditRecord>> =>
  listByFilter(db, { key: 'prospect_id', value: prospectId }, limit)

export const listTargetedSiteQualityAuditsByDomain = async (
  db: D1Database,
  normalizedDomain: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteQualityAuditRecord>> =>
  listByFilter(db, { key: 'normalized_domain', value: normalizedDomain }, limit)
