import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  parseJsonStringArray,
  parseJsonWithSchema,
  parseJsonRecord,
} from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  TargetedSiteQualityAuditRecommendation,
  type TargetedSiteQualityAuditRecord,
} from './targeted-site-quality-audit'

export const TargetedSiteRemakeBriefState = S.Literals([
  'draft',
  'ready_for_operator_review',
  'approved_for_generation',
  'rejected',
  'blocked',
  'archived',
])
export type TargetedSiteRemakeBriefState =
  typeof TargetedSiteRemakeBriefState.Type

export const TargetedSiteSourceAuthorityKind = S.Literals([
  'original_screenshot',
  'original_copy',
  'original_image',
  'public_business_fact',
  'public_listing',
  'operator_note',
  'audit_finding',
])
export type TargetedSiteSourceAuthorityKind =
  typeof TargetedSiteSourceAuthorityKind.Type

export const TargetedSiteSourceAuthorityCard = S.Struct({
  allowedUse: S.String,
  caveats: S.Array(S.String),
  kind: TargetedSiteSourceAuthorityKind,
  publicRef: S.String,
  sourceHash: S.String,
})
export type TargetedSiteSourceAuthorityCard =
  typeof TargetedSiteSourceAuthorityCard.Type

export const TargetedSiteSourceAuthorityPack = S.Struct({
  cards: S.Array(TargetedSiteSourceAuthorityCard),
  prohibitedClaims: S.Array(S.String),
  requiredDisclosures: S.Array(S.String),
  sourcePackRef: S.String,
})
export type TargetedSiteSourceAuthorityPack =
  typeof TargetedSiteSourceAuthorityPack.Type

export const TargetedSiteGenerationConstraints = S.Struct({
  conceptOnly: S.Boolean,
  noFakeCaseResults: S.Boolean,
  noFakeCredentials: S.Boolean,
  noFakeReviews: S.Boolean,
  noLegalAdvice: S.Boolean,
  noMisleadingEndorsements: S.Boolean,
  noUnverifiableGuarantees: S.Boolean,
  notes: S.Array(S.String),
})
export type TargetedSiteGenerationConstraints =
  typeof TargetedSiteGenerationConstraints.Type

export const TargetedSiteRemakeBriefRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  auditFindingRefs: S.Array(S.String),
  campaignId: S.String,
  copiedImageRefs: S.Array(S.String),
  copiedTextRefs: S.Array(S.String),
  createdAt: S.String,
  generationConstraints: TargetedSiteGenerationConstraints,
  id: S.String,
  idempotencyKey: S.String,
  legalSensitive: S.Boolean,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  originalScreenshotRefs: S.Array(S.String),
  preparedAt: S.String,
  prospectId: S.NullOr(S.String),
  providerAdapterRunId: S.NullOr(S.String),
  qualityAuditId: S.String,
  qualityAuditRecommendation: S.NullOr(TargetedSiteQualityAuditRecommendation),
  renderedCaptureRunId: S.NullOr(S.String),
  reviewedAt: S.NullOr(S.String),
  sourceAuthorityPack: TargetedSiteSourceAuthorityPack,
  state: TargetedSiteRemakeBriefState,
  staticCaptureRunId: S.NullOr(S.String),
})
export type TargetedSiteRemakeBriefRecord =
  typeof TargetedSiteRemakeBriefRecord.Type

export const PublicTargetedSiteRemakeBriefProjection = S.Struct({
  campaignId: S.String,
  normalizedDomain: S.String,
  preparedAt: S.String,
  prospectId: S.NullOr(S.String),
  sourceAuthorityCardCount: S.Number,
  state: TargetedSiteRemakeBriefState,
})
export type PublicTargetedSiteRemakeBriefProjection =
  typeof PublicTargetedSiteRemakeBriefProjection.Type

export const OperatorTargetedSiteRemakeBriefProjection = S.Struct({
  auditFindingRefs: S.Array(S.String),
  campaignId: S.String,
  copiedImageRefs: S.Array(S.String),
  copiedTextRefs: S.Array(S.String),
  generationConstraints: TargetedSiteGenerationConstraints,
  hasMetadata: S.Boolean,
  legalSensitive: S.Boolean,
  normalizedDomain: S.String,
  originalScreenshotRefs: S.Array(S.String),
  preparedAt: S.String,
  prospectId: S.NullOr(S.String),
  providerAdapterRunId: S.NullOr(S.String),
  qualityAuditId: S.String,
  qualityAuditRecommendation: S.NullOr(TargetedSiteQualityAuditRecommendation),
  renderedCaptureRunId: S.NullOr(S.String),
  sourceAuthorityPack: TargetedSiteSourceAuthorityPack,
  state: TargetedSiteRemakeBriefState,
  staticCaptureRunId: S.NullOr(S.String),
})
export type OperatorTargetedSiteRemakeBriefProjection =
  typeof OperatorTargetedSiteRemakeBriefProjection.Type

export type RecordTargetedSiteRemakeBriefInput = Readonly<{
  auditFindingRefs: ReadonlyArray<string>
  copiedImageRefs?: ReadonlyArray<string> | undefined
  copiedTextRefs?: ReadonlyArray<string> | undefined
  generationConstraints?: Partial<TargetedSiteGenerationConstraints> | undefined
  id?: string | undefined
  idempotencyKey: string
  legalSensitive?: boolean | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  originalScreenshotRefs: ReadonlyArray<string>
  preparedAt?: string | undefined
  qualityAudit: TargetedSiteQualityAuditRecord
  reviewedAt?: string | undefined
  sourceAuthorityPack: TargetedSiteSourceAuthorityPack
  state?: TargetedSiteRemakeBriefState | undefined
}>

type TargetedSiteRemakeBriefRow = Readonly<{
  archived_at: string | null
  audit_finding_refs_json: string
  campaign_id: string
  copied_image_refs_json: string
  copied_text_refs_json: string
  created_at: string
  generation_constraints_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  normalized_domain: string
  original_screenshot_refs_json: string
  prepared_at: string
  prospect_id: string | null
  provider_adapter_run_id: string | null
  quality_audit_id: string
  rendered_capture_run_id: string | null
  reviewed_at: string | null
  source_authority_pack_json: string
  state: TargetedSiteRemakeBriefState
  static_capture_run_id: string | null
}>

type TargetedSiteRemakeBriefFilter =
  | Readonly<{ key: 'campaign_id'; value: string }>
  | Readonly<{ key: 'prospect_id'; value: string }>
  | Readonly<{ key: 'normalized_domain'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_HASH_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/=-]{7,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?payload|browser[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)|@/i
const LEGAL_SENSITIVE_PROHIBITED_PATTERN =
  /\b(guaranteed|guarantee|win rate|case result|settlement|verdict|testimonial|review|five star|5 star|attorney credential|bar certified|legal advice|endorsement)\b/i

const defaultGenerationConstraints: TargetedSiteGenerationConstraints = {
  conceptOnly: true,
  noFakeCaseResults: true,
  noFakeCredentials: true,
  noFakeReviews: true,
  noLegalAdvice: true,
  noMisleadingEndorsements: true,
  noUnverifiableGuarantees: true,
  notes: [],
}

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) && textIsSafe(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!isSafeRef(value)) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason: `${field} must be a public-safe ref without private capture, provider, contact, payment, wallet, or bypass material.`,
    })
  }
}

const assertSafeRefList = (
  field: string,
  values: ReadonlyArray<string>,
): void => values.forEach(value => assertSafeRef(field, value))

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason: 'normalizedDomain must be a public-safe normalized domain.',
    })
  }
}

const assertSafeText = (field: string, value: string): void => {
  if (value.trim() === '' || !textIsSafe(value)) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason: `${field} must be public-safe text without private capture, provider, contact, payment, wallet, or bypass material.`,
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
    throw new TargetedSiteRemakeBriefValidationError({
      reason:
        'metadata must not contain private capture, provider, contact, payment, wallet, or bypass material.',
    })
  }
}

const normalizeGenerationConstraints = (
  constraints: Partial<TargetedSiteGenerationConstraints> | undefined,
): TargetedSiteGenerationConstraints => ({
  ...defaultGenerationConstraints,
  ...constraints,
  conceptOnly: constraints?.conceptOnly ?? true,
  noFakeCaseResults: constraints?.noFakeCaseResults ?? true,
  noFakeCredentials: constraints?.noFakeCredentials ?? true,
  noFakeReviews: constraints?.noFakeReviews ?? true,
  noLegalAdvice: constraints?.noLegalAdvice ?? true,
  noMisleadingEndorsements:
    constraints?.noMisleadingEndorsements ?? true,
  noUnverifiableGuarantees:
    constraints?.noUnverifiableGuarantees ?? true,
  notes: [...(constraints?.notes ?? [])].slice(0, 40),
})

const assertSafeGenerationConstraints = (
  constraints: TargetedSiteGenerationConstraints,
): void => {
  constraints.notes.forEach(note => assertSafeText('generationConstraints.notes', note))

  if (
    constraints.conceptOnly !== true ||
    constraints.noFakeCaseResults !== true ||
    constraints.noFakeCredentials !== true ||
    constraints.noFakeReviews !== true ||
    constraints.noLegalAdvice !== true ||
    constraints.noMisleadingEndorsements !== true ||
    constraints.noUnverifiableGuarantees !== true
  ) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason:
        'generation constraints must keep concept-only and law-firm safety controls enabled.',
    })
  }
}

const assertSafeSourceAuthorityPack = (
  pack: TargetedSiteSourceAuthorityPack,
): void => {
  assertSafeRef('sourceAuthorityPack.sourcePackRef', pack.sourcePackRef)
  assertSafeRefList('sourceAuthorityPack.prohibitedClaims', pack.prohibitedClaims)
  assertSafeRefList(
    'sourceAuthorityPack.requiredDisclosures',
    pack.requiredDisclosures,
  )

  if (pack.cards.length === 0) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason: 'source authority pack requires at least one source card.',
    })
  }

  pack.cards.slice(0, 100).forEach(card => {
    assertSafeRef('sourceAuthorityPack.cards.publicRef', card.publicRef)
    assertSafeText('sourceAuthorityPack.cards.allowedUse', card.allowedUse)
    card.caveats.forEach(caveat =>
      assertSafeText('sourceAuthorityPack.cards.caveats', caveat),
    )

    if (!SAFE_HASH_PATTERN.test(card.sourceHash) || !textIsSafe(card.sourceHash)) {
      throw new TargetedSiteRemakeBriefValidationError({
        reason:
          'source authority card sourceHash must be a public-safe content hash ref.',
      })
    }
  })
}

const assertLegalSensitivePack = (
  input: RecordTargetedSiteRemakeBriefInput,
  legalSensitive: boolean,
): void => {
  if (!legalSensitive) {
    return
  }

  const prohibitedText = [
    ...input.sourceAuthorityPack.prohibitedClaims,
    ...input.sourceAuthorityPack.cards.map(card => card.allowedUse),
    ...(input.generationConstraints?.notes ?? []),
  ].join(' ')

  if (LEGAL_SENSITIVE_PROHIBITED_PATTERN.test(prohibitedText)) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason:
        'legal-sensitive remake briefs may not authorize fake reviews, credentials, case results, legal advice, guarantees, or misleading endorsement claims.',
    })
  }
}

export class TargetedSiteRemakeBriefValidationError extends S.TaggedErrorClass<TargetedSiteRemakeBriefValidationError>()(
  'TargetedSiteRemakeBriefValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteRemakeBriefStorageError extends S.TaggedErrorClass<TargetedSiteRemakeBriefStorageError>()(
  'TargetedSiteRemakeBriefStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

const assertValidInput = (
  input: RecordTargetedSiteRemakeBriefInput,
): TargetedSiteGenerationConstraints => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('campaignId', input.qualityAudit.campaignId)
  assertSafeRef('prospectId', input.qualityAudit.prospectId ?? undefined)
  assertSafeRef('qualityAuditId', input.qualityAudit.id)
  assertSafeRef('staticCaptureRunId', input.qualityAudit.staticCaptureRunId ?? undefined)
  assertSafeRef(
    'renderedCaptureRunId',
    input.qualityAudit.renderedCaptureRunId ?? undefined,
  )
  assertSafeRef(
    'providerAdapterRunId',
    input.qualityAudit.providerAdapterRunId ?? undefined,
  )
  assertSafeDomain(input.qualityAudit.normalizedDomain)
  assertSafeRefList('auditFindingRefs', input.auditFindingRefs)
  assertSafeRefList('originalScreenshotRefs', input.originalScreenshotRefs)
  assertSafeRefList('copiedTextRefs', input.copiedTextRefs ?? [])
  assertSafeRefList('copiedImageRefs', input.copiedImageRefs ?? [])
  assertSafeSourceAuthorityPack(input.sourceAuthorityPack)
  assertSafeMetadata(input.metadata)

  const constraints = normalizeGenerationConstraints(input.generationConstraints)
  assertSafeGenerationConstraints(constraints)
  const legalSensitive =
    input.legalSensitive === true || input.qualityAudit.legalSensitive
  assertLegalSensitivePack(input, legalSensitive)

  if (input.auditFindingRefs.length === 0) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason: 'remake briefs require at least one audit finding ref.',
    })
  }

  if (input.originalScreenshotRefs.length === 0) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason: 'remake briefs require at least one original screenshot ref.',
    })
  }

  if (
    input.qualityAudit.recommendation === 'blocked' ||
    input.qualityAudit.state === 'blocked'
  ) {
    throw new TargetedSiteRemakeBriefValidationError({
      reason: 'blocked quality audits cannot produce remake briefs.',
    })
  }

  return constraints
}

const sourceAuthorityPackFromJson = (
  value: string,
): TargetedSiteSourceAuthorityPack =>
  parseJsonWithSchema(TargetedSiteSourceAuthorityPack, value)

const generationConstraintsFromJson = (
  value: string,
): TargetedSiteGenerationConstraints =>
  parseJsonWithSchema(TargetedSiteGenerationConstraints, value)

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const briefFromRow = (
  row: TargetedSiteRemakeBriefRow,
): TargetedSiteRemakeBriefRecord => ({
  archivedAt: row.archived_at,
  auditFindingRefs: [...parseJsonStringArray(row.audit_finding_refs_json)],
  campaignId: row.campaign_id,
  copiedImageRefs: [...parseJsonStringArray(row.copied_image_refs_json)],
  copiedTextRefs: [...parseJsonStringArray(row.copied_text_refs_json)],
  createdAt: row.created_at,
  generationConstraints: generationConstraintsFromJson(
    row.generation_constraints_json,
  ),
  id: row.id,
  idempotencyKey: row.idempotency_key,
  legalSensitive: row.legal_sensitive === 1,
  metadata: metadataFromJson(row.metadata_json),
  normalizedDomain: row.normalized_domain,
  originalScreenshotRefs: [
    ...parseJsonStringArray(row.original_screenshot_refs_json),
  ],
  preparedAt: row.prepared_at,
  prospectId: row.prospect_id,
  providerAdapterRunId: row.provider_adapter_run_id,
  qualityAuditId: row.quality_audit_id,
  qualityAuditRecommendation: null,
  renderedCaptureRunId: row.rendered_capture_run_id,
  reviewedAt: row.reviewed_at,
  sourceAuthorityPack: sourceAuthorityPackFromJson(
    row.source_authority_pack_json,
  ),
  state: row.state,
  staticCaptureRunId: row.static_capture_run_id,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteRemakeBriefRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_remake_briefs
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteRemakeBriefRow>()

  return row === null ? null : briefFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: TargetedSiteRemakeBriefFilter,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRemakeBriefRecord>> => {
  if (filter.key === 'normalized_domain') {
    assertSafeDomain(filter.value)
  } else {
    assertSafeRef(filter.key, filter.value)
  }

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_remake_briefs
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY prepared_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<TargetedSiteRemakeBriefRow>()

  return (rows.results ?? []).map(briefFromRow)
}

export const publicTargetedSiteRemakeBriefProjection = (
  record: TargetedSiteRemakeBriefRecord,
): PublicTargetedSiteRemakeBriefProjection => ({
  campaignId: record.campaignId,
  normalizedDomain: record.normalizedDomain,
  preparedAt: record.preparedAt,
  prospectId: record.prospectId,
  sourceAuthorityCardCount: record.sourceAuthorityPack.cards.length,
  state: record.state,
})

export const operatorTargetedSiteRemakeBriefProjection = (
  record: TargetedSiteRemakeBriefRecord,
): OperatorTargetedSiteRemakeBriefProjection => ({
  auditFindingRefs: record.auditFindingRefs,
  campaignId: record.campaignId,
  copiedImageRefs: record.copiedImageRefs,
  copiedTextRefs: record.copiedTextRefs,
  generationConstraints: record.generationConstraints,
  hasMetadata: Object.keys(record.metadata).length > 0,
  legalSensitive: record.legalSensitive,
  normalizedDomain: record.normalizedDomain,
  originalScreenshotRefs: record.originalScreenshotRefs,
  preparedAt: record.preparedAt,
  prospectId: record.prospectId,
  providerAdapterRunId: record.providerAdapterRunId,
  qualityAuditId: record.qualityAuditId,
  qualityAuditRecommendation: record.qualityAuditRecommendation,
  renderedCaptureRunId: record.renderedCaptureRunId,
  sourceAuthorityPack: record.sourceAuthorityPack,
  state: record.state,
  staticCaptureRunId: record.staticCaptureRunId,
})

export const recordTargetedSiteRemakeBrief = async (
  db: D1Database,
  input: RecordTargetedSiteRemakeBriefInput,
): Promise<TargetedSiteRemakeBriefRecord> => {
  const constraints = assertValidInput(input)
  const existing = await readByIdempotencyKey(db, input.idempotencyKey)

  if (existing !== null) {
    return existing
  }

  const nowIso = currentIsoTimestamp()
  const legalSensitive =
    input.legalSensitive === true || input.qualityAudit.legalSensitive
  const state =
    input.state ??
    (input.qualityAudit.recommendation === 'manual_review'
      ? 'ready_for_operator_review'
      : 'draft')
  const id = input.id ?? compactRandomId('targeted_site_remake_brief')

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_remake_briefs (
         id,
         idempotency_key,
         campaign_id,
         prospect_id,
         normalized_domain,
         quality_audit_id,
         static_capture_run_id,
         rendered_capture_run_id,
         provider_adapter_run_id,
         state,
         legal_sensitive,
         source_authority_pack_json,
         audit_finding_refs_json,
         original_screenshot_refs_json,
         copied_text_refs_json,
         copied_image_refs_json,
         generation_constraints_json,
         metadata_json,
         prepared_at,
         reviewed_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      input.qualityAudit.campaignId,
      input.qualityAudit.prospectId,
      input.qualityAudit.normalizedDomain,
      input.qualityAudit.id,
      input.qualityAudit.staticCaptureRunId,
      input.qualityAudit.renderedCaptureRunId,
      input.qualityAudit.providerAdapterRunId,
      state,
      legalSensitive ? 1 : 0,
      JSON.stringify({
        ...input.sourceAuthorityPack,
        cards: input.sourceAuthorityPack.cards.slice(0, 100),
      }),
      JSON.stringify([...input.auditFindingRefs].slice(0, 100)),
      JSON.stringify([...input.originalScreenshotRefs].slice(0, 100)),
      JSON.stringify([...(input.copiedTextRefs ?? [])].slice(0, 100)),
      JSON.stringify([...(input.copiedImageRefs ?? [])].slice(0, 100)),
      JSON.stringify(constraints),
      JSON.stringify(input.metadata ?? {}),
      input.preparedAt ?? nowIso,
      input.reviewedAt ?? null,
      nowIso,
    )
    .run()

  const record = await readByIdempotencyKey(db, input.idempotencyKey)

  if (record === null) {
    throw new TargetedSiteRemakeBriefStorageError({
      operation: 'recordTargetedSiteRemakeBrief.readByIdempotencyKey',
      reason: 'inserted or existing targeted Site remake brief was not readable.',
    })
  }

  return {
    ...record,
    qualityAuditRecommendation: input.qualityAudit.recommendation,
  }
}

export const listTargetedSiteRemakeBriefsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRemakeBriefRecord>> =>
  listByFilter(db, { key: 'campaign_id', value: campaignId }, limit)

export const listTargetedSiteRemakeBriefsByProspect = async (
  db: D1Database,
  prospectId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRemakeBriefRecord>> =>
  listByFilter(db, { key: 'prospect_id', value: prospectId }, limit)

export const listTargetedSiteRemakeBriefsByDomain = async (
  db: D1Database,
  normalizedDomain: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRemakeBriefRecord>> =>
  listByFilter(db, { key: 'normalized_domain', value: normalizedDomain }, limit)
