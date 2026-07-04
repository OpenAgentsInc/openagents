import { Schema as S } from 'effect'

import { BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF } from './business-commitment-ledger'
import { decodeBusinessSourceRef } from './business-source-attribution'
import { parseJsonStringArray } from './json-boundary'
import { liveAtReadStaleness } from './public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const BusinessPipelineStage = S.Literals([
  'intake_received',
  'scope_scheduled',
  'scope_completed',
  'receipt_plan_sent',
  'closed_won',
  'closed_lost',
  'quick_win_started',
])
export type BusinessPipelineStage = typeof BusinessPipelineStage.Type

export const BUSINESS_PIPELINE_STAGE_ORDER: ReadonlyArray<BusinessPipelineStage> = [
  'intake_received',
  'scope_scheduled',
  'scope_completed',
  'receipt_plan_sent',
  'closed_won',
  'closed_lost',
  'quick_win_started',
]

export const BusinessPipelineOwnerRole = S.Literals([
  'operator',
  'reviewer',
  'fulfillment_agent',
  'owner',
])
export type BusinessPipelineOwnerRole = typeof BusinessPipelineOwnerRole.Type

export const BusinessPipelineCommitmentKind = S.Literals([
  'deliverable',
  'send',
])
export type BusinessPipelineCommitmentKind =
  typeof BusinessPipelineCommitmentKind.Type

export const BusinessPipelineCommitmentDueState = S.Literals([
  'due',
  'blocked',
  'shipped',
  'parked',
])
export type BusinessPipelineCommitmentDueState =
  typeof BusinessPipelineCommitmentDueState.Type

export const BusinessPipelineQuotedBand = S.Struct({
  label: S.String,
  maxUsdCents: S.Number,
  minUsdCents: S.Number,
})
export type BusinessPipelineQuotedBand = typeof BusinessPipelineQuotedBand.Type

export const BusinessPipelineRow = S.Struct({
  blockerRef: S.NullOr(S.String),
  businessSignupRequestId: S.NullOr(S.String),
  createdAt: S.String,
  nextActionDueAt: S.NullOr(S.String),
  ownerRole: BusinessPipelineOwnerRole,
  partnerRouteFlag: S.Boolean,
  pipelineRef: S.String,
  quotedBand: BusinessPipelineQuotedBand,
  receiptRefs: S.Array(S.String),
  sourceRef: S.String,
  stage: BusinessPipelineStage,
  stageUpdatedAt: S.String,
  updatedAt: S.String,
  vertical: S.String,
})
export type BusinessPipelineRow = typeof BusinessPipelineRow.Type

export const BusinessPipelineCommitmentRecord = S.Struct({
  blockerRefs: S.Array(S.String),
  commitmentKind: BusinessPipelineCommitmentKind,
  commitmentRef: S.String,
  dueAt: S.String,
  dueState: BusinessPipelineCommitmentDueState,
  engagementRef: S.String,
  evidenceRefs: S.Array(S.String),
  ownerRef: S.String,
  pipelineRef: S.String,
  promisedObjectRef: S.String,
  shippedAt: S.NullOr(S.String),
  sourceRefs: S.Array(S.String),
})
export type BusinessPipelineCommitmentRecord =
  typeof BusinessPipelineCommitmentRecord.Type

export const BusinessPipelineMetricStatus = S.Literals([
  'measured',
  'not_measured',
])
export type BusinessPipelineMetricStatus =
  typeof BusinessPipelineMetricStatus.Type

export const BusinessPipelineRateMetric = S.Struct({
  denominator: S.Number,
  numerator: S.Number,
  status: BusinessPipelineMetricStatus,
  value: S.NullOr(S.Number),
})
export type BusinessPipelineRateMetric = typeof BusinessPipelineRateMetric.Type

const BusinessPipelineQualifiedPipelineMetric = S.Struct({
  maxUsdCents: S.Number,
  minUsdCents: S.Number,
  qualifiedRowCount: S.Number,
  status: BusinessPipelineMetricStatus,
  targetUsdCents: S.Literal(2_500_000),
})

const BusinessPipelineSourceRefMetric = S.Struct({
  qualifiedPipeline: BusinessPipelineQualifiedPipelineMetric,
  rates: S.Struct({
    closeRate: BusinessPipelineRateMetric,
    intakeToScopeRate: BusinessPipelineRateMetric,
  }),
  rowCount: S.Number,
  sourceRef: S.String,
  stageCounts: S.Array(S.Struct({
    count: S.Number,
    stage: BusinessPipelineStage,
  })),
})

export const BusinessPipelineMetrics = S.Struct({
  schemaVersion: S.Literal('openagents.business_pipeline_metrics.v1'),
  commitmentCoverage: S.Struct({
    linkedPipelineRowCount: S.Number,
    missingCommitmentDefects: S.Array(S.String),
    status: BusinessPipelineMetricStatus,
    totalPipelineRowCount: S.Number,
  }),
  generatedAt: S.String,
  privacyBoundary: S.Struct({
    excludes: S.Array(S.String),
    opaqueRefsOnly: S.Literal(true),
  }),
  qualifiedPipeline: BusinessPipelineQualifiedPipelineMetric,
  rates: S.Struct({
    closeRate: BusinessPipelineRateMetric,
    intakeToScopeRate: BusinessPipelineRateMetric,
  }),
  sourceRefBreakdown: S.Array(BusinessPipelineSourceRefMetric),
  sourceRefs: S.Array(S.String),
  stageCounts: S.Array(S.Struct({
    count: S.Number,
    stage: BusinessPipelineStage,
  })),
  staleness: S.Struct({
    composition: S.String,
    rebuildsOn: S.Array(S.String),
  }),
})
export type BusinessPipelineMetrics = typeof BusinessPipelineMetrics.Type

export class BusinessPipelineValidationError extends S.TaggedErrorClass<BusinessPipelineValidationError>()(
  'BusinessPipelineValidationError',
  { reason: S.String },
) {}

export class BusinessPipelineStoreError extends S.TaggedErrorClass<BusinessPipelineStoreError>()(
  'BusinessPipelineStoreError',
  {
    kind: S.Literals(['conflict', 'not_found', 'storage_error', 'validation_error']),
    reason: S.String,
  },
) {}

export type BusinessPipelineCreateInput = Readonly<{
  blockerRef?: string | null
  businessSignupRequestId?: string | null
  nextActionDueAt?: string | null
  ownerRole: BusinessPipelineOwnerRole
  partnerRouteFlag?: boolean
  pipelineRef: string
  quotedBand?: Partial<BusinessPipelineQuotedBand> | null
  receiptRefs?: ReadonlyArray<string>
  sourceRef: string
  stage?: BusinessPipelineStage
  vertical: string
}>

export type BusinessPipelineAdvanceInput = Readonly<{
  blockerRef?: string | null
  nextActionDueAt?: string | null
  ownerRole?: BusinessPipelineOwnerRole
  quotedBand?: Partial<BusinessPipelineQuotedBand> | null
  receiptRef: string
  stage: BusinessPipelineStage
}>

export type BusinessPipelineCommitmentInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  commitmentKind: BusinessPipelineCommitmentKind
  commitmentRef: string
  dueAt: string
  dueState?: BusinessPipelineCommitmentDueState
  engagementRef?: string
  evidenceRefs?: ReadonlyArray<string>
  ownerRef: string
  pipelineRef: string
  promisedObjectRef: string
  shippedAt?: string | null
  sourceRefs?: ReadonlyArray<string>
}>

export type BusinessPipelineRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemBusinessPipelineRuntime: BusinessPipelineRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

type BusinessPipelineD1Row = Readonly<{
  blocker_ref: string | null
  business_signup_request_id: string | null
  created_at: string
  next_action_due_at: string | null
  owner_role: BusinessPipelineOwnerRole
  partner_route_flag: number
  pipeline_ref: string
  quoted_band_label: string
  quoted_max_usd_cents: number
  quoted_min_usd_cents: number
  receipt_refs_json: string
  source_ref: string
  stage: BusinessPipelineStage
  stage_updated_at: string
  updated_at: string
  vertical: string
}>

type BusinessPipelineCommitmentD1Row = Readonly<{
  blocker_refs_json: string
  commitment_kind: BusinessPipelineCommitmentKind
  commitment_ref: string
  due_at: string
  due_state: BusinessPipelineCommitmentDueState
  engagement_ref: string
  evidence_refs_json: string
  owner_ref: string
  pipeline_ref: string | null
  promised_object_ref: string
  shipped_at: string | null
  source_refs_json: string
}>

type CountRow = Readonly<{ count: number }>
type StageCountRow = Readonly<{ count: number; stage: BusinessPipelineStage }>
type PipelineCommitmentCountRow = Readonly<{
  commitment_count: number
  pipeline_ref: string
}>
type BusinessSignupPipelineLinkRow = Readonly<{
  id: string
  linked_pipeline_ref: string | null
  source_ref: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/=#-]{0,240}$/
const SAFE_DESCRIPTOR_PATTERN = /^[A-Za-z0-9][A-Za-z0-9 _/-]{0,79}$/
const UNSAFE_VALUE_PATTERN =
  /@|https?:\/\/|www\.|\b(client|customer|contact|email|phone|raw|provider_payload|access_token|refresh_token|private_key|wallet_secret|payment_preimage|webhook_secret|xprv|mnemonic)\b/i
const DOMAIN_LIKE_PATTERN =
  /\b[a-z0-9-]+\.(com|net|org|io|ai|co|dev|app|biz|info|us|co\.uk)\b/i

const safeRefPart = (value: string): string =>
  value.toLowerCase().replace(/[^a-z0-9_.-]+/gu, '_').replace(/^_+|_+$/gu, '')

export const businessPipelineSafeRefPart = safeRefPart

const validationError = (reason: string): BusinessPipelineValidationError =>
  new BusinessPipelineValidationError({ reason })

const storeValidationError = (reason: string): BusinessPipelineStoreError =>
  new BusinessPipelineStoreError({ kind: 'validation_error', reason })

const storageError = (error: unknown): BusinessPipelineStoreError =>
  error instanceof BusinessPipelineStoreError
    ? error
    : new BusinessPipelineStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      })

const assertPublicSafeRef = (field: string, value: string): void => {
  if (
    !SAFE_REF_PATTERN.test(value) ||
    UNSAFE_VALUE_PATTERN.test(value) ||
    DOMAIN_LIKE_PATTERN.test(value)
  ) {
    throw validationError(`${field} must be an opaque public-safe ref`)
  }
}

const assertPublicSafeDescriptor = (field: string, value: string): void => {
  if (
    !SAFE_DESCRIPTOR_PATTERN.test(value) ||
    UNSAFE_VALUE_PATTERN.test(value) ||
    DOMAIN_LIKE_PATTERN.test(value)
  ) {
    throw validationError(`${field} must be a public-safe descriptor`)
  }
}

const assertPublicSafeRefs = (
  field: string,
  values: ReadonlyArray<string>,
): void => values.forEach(value => assertPublicSafeRef(field, value))

export const assertBusinessPipelinePublicSafeRef = assertPublicSafeRef
export const assertBusinessPipelinePublicSafeDescriptor =
  assertPublicSafeDescriptor

const normalizeNullableRef = (
  field: string,
  value: string | null | undefined,
): string | null => {
  if (value === undefined || value === null || value.trim() === '') {
    return null
  }
  const trimmed = value.trim()
  assertPublicSafeRef(field, trimmed)
  return trimmed
}

const normalizeReceiptRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const normalized = [...new Set((refs ?? []).map(ref => ref.trim()).filter(Boolean))]
  assertPublicSafeRefs('receiptRefs', normalized)
  return normalized
}

const normalizeQuotedBand = (
  band: Partial<BusinessPipelineQuotedBand> | null | undefined,
): BusinessPipelineQuotedBand => {
  const minUsdCents = Number(band?.minUsdCents ?? 0)
  const maxUsdCents = Number(band?.maxUsdCents ?? minUsdCents)
  const label = (band?.label ?? 'unquoted').trim().slice(0, 80)

  if (
    !Number.isInteger(minUsdCents) ||
    !Number.isInteger(maxUsdCents) ||
    minUsdCents < 0 ||
    maxUsdCents < minUsdCents
  ) {
    throw validationError('quotedBand must use non-negative integer cents')
  }
  assertPublicSafeDescriptor('quotedBand.label', label)
  return { label, maxUsdCents, minUsdCents }
}

const hasQuotedBand = (row: Pick<BusinessPipelineRow, 'quotedBand'>): boolean =>
  row.quotedBand.maxUsdCents > 0

const businessPipelineRowFromD1 = (row: BusinessPipelineD1Row): BusinessPipelineRow => {
  const record: BusinessPipelineRow = {
    blockerRef: row.blocker_ref,
    businessSignupRequestId: row.business_signup_request_id,
    createdAt: row.created_at,
    nextActionDueAt: row.next_action_due_at,
    ownerRole: S.decodeUnknownSync(BusinessPipelineOwnerRole)(row.owner_role),
    partnerRouteFlag: row.partner_route_flag === 1,
    pipelineRef: row.pipeline_ref,
    quotedBand: {
      label: row.quoted_band_label,
      maxUsdCents: Number(row.quoted_max_usd_cents),
      minUsdCents: Number(row.quoted_min_usd_cents),
    },
    receiptRefs: parseJsonStringArray(row.receipt_refs_json),
    sourceRef: row.source_ref,
    stage: S.decodeUnknownSync(BusinessPipelineStage)(row.stage),
    stageUpdatedAt: row.stage_updated_at,
    updatedAt: row.updated_at,
    vertical: row.vertical,
  }

  assertPublicSafeRef('pipelineRef', record.pipelineRef)
  assertPublicSafeDescriptor('vertical', record.vertical)
  assertPublicSafeRef('sourceRef', record.sourceRef)
  assertPublicSafeDescriptor('ownerRole', record.ownerRole)
  if (record.nextActionDueAt !== null) {
    assertPublicSafeDescriptor('nextActionDueAt', record.nextActionDueAt)
  }
  if (record.blockerRef !== null) {
    assertPublicSafeRef('blockerRef', record.blockerRef)
  }
  if (record.businessSignupRequestId !== null) {
    assertPublicSafeRef(
      'businessSignupRequestId',
      record.businessSignupRequestId,
    )
  }
  assertPublicSafeRefs('receiptRefs', record.receiptRefs)
  normalizeQuotedBand(record.quotedBand)

  return S.decodeUnknownSync(BusinessPipelineRow)(record)
}

const businessPipelineCommitmentFromD1 = (
  row: BusinessPipelineCommitmentD1Row,
): BusinessPipelineCommitmentRecord => {
  if (row.pipeline_ref === null) {
    throw validationError('pipelineRef is required for linked commitments')
  }

  const record: BusinessPipelineCommitmentRecord = {
    blockerRefs: parseJsonStringArray(row.blocker_refs_json),
    commitmentKind: S.decodeUnknownSync(BusinessPipelineCommitmentKind)(
      row.commitment_kind,
    ),
    commitmentRef: row.commitment_ref,
    dueAt: row.due_at,
    dueState: S.decodeUnknownSync(BusinessPipelineCommitmentDueState)(
      row.due_state,
    ),
    engagementRef: row.engagement_ref,
    evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
    ownerRef: row.owner_ref,
    pipelineRef: row.pipeline_ref,
    promisedObjectRef: row.promised_object_ref,
    shippedAt: row.shipped_at,
    sourceRefs: parseJsonStringArray(row.source_refs_json),
  }

  assertPublicSafeRef('commitmentRef', record.commitmentRef)
  assertPublicSafeRef('engagementRef', record.engagementRef)
  assertPublicSafeRef('ownerRef', record.ownerRef)
  assertPublicSafeRef('pipelineRef', record.pipelineRef)
  assertPublicSafeRef('promisedObjectRef', record.promisedObjectRef)
  assertPublicSafeRefs('sourceRefs', record.sourceRefs)
  assertPublicSafeRefs('blockerRefs', record.blockerRefs)
  assertPublicSafeRefs('evidenceRefs', record.evidenceRefs)

  return S.decodeUnknownSync(BusinessPipelineCommitmentRecord)(record)
}

const allowedTransitions: Record<BusinessPipelineStage, ReadonlyArray<BusinessPipelineStage>> = {
  closed_lost: [],
  closed_won: ['quick_win_started'],
  intake_received: ['scope_scheduled', 'closed_lost'],
  quick_win_started: [],
  receipt_plan_sent: ['closed_won', 'closed_lost'],
  scope_completed: ['receipt_plan_sent', 'closed_lost'],
  scope_scheduled: ['scope_completed', 'closed_lost'],
}

const qualifiedQuotedStages = new Set<BusinessPipelineStage>([
  'scope_scheduled',
  'scope_completed',
  'receipt_plan_sent',
  'closed_won',
  'quick_win_started',
])

const rateMetric = (
  numerator: number,
  denominator: number,
): BusinessPipelineRateMetric => ({
  denominator,
  numerator,
  status: denominator === 0 ? 'not_measured' : 'measured',
  value: denominator === 0 ? null : Number((numerator / denominator).toFixed(4)),
})

const pipelineRowSelect = `SELECT
  blocker_ref,
  business_signup_request_id,
  created_at,
  next_action_due_at,
  owner_role,
  partner_route_flag,
  pipeline_ref,
  quoted_band_label,
  quoted_max_usd_cents,
  quoted_min_usd_cents,
  receipt_refs_json,
  source_ref,
  stage,
  stage_updated_at,
  updated_at,
  vertical
 FROM business_pipeline_rows`

const qualifiedPipelineMetric = (
  rows: ReadonlyArray<BusinessPipelineRow>,
) => {
  const qualifiedRows = rows.filter(
    row => qualifiedQuotedStages.has(row.stage) && hasQuotedBand(row),
  )
  return {
    maxUsdCents: qualifiedRows.reduce(
      (sum, row) => sum + row.quotedBand.maxUsdCents,
      0,
    ),
    minUsdCents: qualifiedRows.reduce(
      (sum, row) => sum + row.quotedBand.minUsdCents,
      0,
    ),
    qualifiedRowCount: qualifiedRows.length,
    status:
      qualifiedRows.length === 0
        ? ('not_measured' as const)
        : ('measured' as const),
    targetUsdCents: 2_500_000 as const,
  }
}

const pipelineRates = (rows: ReadonlyArray<BusinessPipelineRow>) => {
  const receiptDecisions = rows.filter(
    row => row.stage === 'closed_won' || row.stage === 'closed_lost',
  ).length
  const scopeCompleted = rows.filter(row =>
    ['scope_completed', 'receipt_plan_sent', 'closed_won', 'quick_win_started'].includes(
      row.stage,
    ),
  ).length
  const scopedOrBeyond = rows.filter(row =>
    qualifiedQuotedStages.has(row.stage) || row.stage === 'closed_lost',
  ).length

  return {
    closeRate: rateMetric(
      rows.filter(row => row.stage === 'closed_won').length,
      receiptDecisions,
    ),
    intakeToScopeRate: rateMetric(scopeCompleted, scopedOrBeyond),
  }
}

const sourceRefBreakdown = (
  rows: ReadonlyArray<BusinessPipelineRow>,
): BusinessPipelineMetrics['sourceRefBreakdown'] => {
  const rowsBySourceRef = new Map<string, Array<BusinessPipelineRow>>()
  for (const row of rows) {
    const existing = rowsBySourceRef.get(row.sourceRef) ?? []
    existing.push(row)
    rowsBySourceRef.set(row.sourceRef, existing)
  }

  return [...rowsBySourceRef.entries()]
    .map(([sourceRef, sourceRows]) => {
      const stageCountMap = new Map<BusinessPipelineStage, number>()
      for (const row of sourceRows) {
        stageCountMap.set(row.stage, (stageCountMap.get(row.stage) ?? 0) + 1)
      }

      return {
        qualifiedPipeline: qualifiedPipelineMetric(sourceRows),
        rates: pipelineRates(sourceRows),
        rowCount: sourceRows.length,
        sourceRef,
        stageCounts: BUSINESS_PIPELINE_STAGE_ORDER.map(stage => ({
          count: stageCountMap.get(stage) ?? 0,
          stage,
        })),
      }
    })
    .sort((a, b) =>
      b.rowCount === a.rowCount
        ? a.sourceRef.localeCompare(b.sourceRef)
        : b.rowCount - a.rowCount,
    )
}

export type BusinessPipelineStore = Readonly<{
  appendPipelineReceiptRefs: (
    pipelineRef: string,
    receiptRefs: ReadonlyArray<string>,
    runtime?: BusinessPipelineRuntime,
  ) => Promise<BusinessPipelineRow>
  advancePipelineRow: (
    pipelineRef: string,
    input: BusinessPipelineAdvanceInput,
    runtime?: BusinessPipelineRuntime,
  ) => Promise<BusinessPipelineRow>
  createCommitment: (
    input: BusinessPipelineCommitmentInput,
    runtime?: BusinessPipelineRuntime,
  ) => Promise<BusinessPipelineCommitmentRecord>
  createPipelineRow: (
    input: BusinessPipelineCreateInput,
    runtime?: BusinessPipelineRuntime,
  ) => Promise<BusinessPipelineRow>
  listCommitmentsForPipeline: (
    pipelineRef: string,
  ) => Promise<ReadonlyArray<BusinessPipelineCommitmentRecord>>
  listPipelineRows: () => Promise<ReadonlyArray<BusinessPipelineRow>>
  readMetrics: (nowIso: string) => Promise<BusinessPipelineMetrics>
  readPipelineRow: (pipelineRef: string) => Promise<BusinessPipelineRow | null>
}>

export const makeD1BusinessPipelineStore = (db: D1Database): BusinessPipelineStore => {
  const readPipelineRow = async (
    pipelineRef: string,
  ): Promise<BusinessPipelineRow | null> => {
    assertPublicSafeRef('pipelineRef', pipelineRef)
    const row = await db
      .prepare(`${pipelineRowSelect} WHERE pipeline_ref = ?`)
      .bind(pipelineRef)
      .first<BusinessPipelineD1Row>()

    return row === null ? null : businessPipelineRowFromD1(row)
  }

  const listPipelineRows = async (): Promise<ReadonlyArray<BusinessPipelineRow>> => {
    const rows = await db
      .prepare(`${pipelineRowSelect} ORDER BY updated_at DESC, pipeline_ref ASC`)
      .all<BusinessPipelineD1Row>()

    return (rows.results ?? []).map(businessPipelineRowFromD1)
  }

  const listCommitmentsForPipeline = async (
    pipelineRef: string,
  ): Promise<ReadonlyArray<BusinessPipelineCommitmentRecord>> => {
    assertPublicSafeRef('pipelineRef', pipelineRef)
    const rows = await db
      .prepare(
        `SELECT
          blocker_refs_json,
          commitment_kind,
          commitment_ref,
          due_at,
          due_state,
          engagement_ref,
          evidence_refs_json,
          owner_ref,
          pipeline_ref,
          promised_object_ref,
          shipped_at,
          source_refs_json
         FROM business_commitment_ledger
         WHERE pipeline_ref = ?
         ORDER BY due_at ASC, commitment_ref ASC`,
      )
      .bind(pipelineRef)
      .all<BusinessPipelineCommitmentD1Row>()

    return (rows.results ?? []).map(businessPipelineCommitmentFromD1)
  }

  const readBusinessSignupPipelineLink = async (
    businessSignupRequestId: string | null,
    pipelineRef: string,
    sourceRef: string,
  ): Promise<BusinessSignupPipelineLinkRow | null> => {
    if (businessSignupRequestId === null) {
      return null
    }

    const row = await db
      .prepare(
        `SELECT id, source_ref, linked_pipeline_ref
           FROM business_signup_requests
          WHERE id = ?`,
      )
      .bind(businessSignupRequestId)
      .first<BusinessSignupPipelineLinkRow>()

    if (row === null) {
      throw new BusinessPipelineStoreError({
        kind: 'not_found',
        reason: `business signup request not found: ${businessSignupRequestId}`,
      })
    }

    if ((row.source_ref ?? 'direct') !== sourceRef) {
      throw validationError(
        'sourceRef must match linked business signup sourceRef',
      )
    }
    if (
      row.linked_pipeline_ref !== null &&
      row.linked_pipeline_ref !== pipelineRef
    ) {
      throw new BusinessPipelineStoreError({
        kind: 'conflict',
        reason: `business signup request already linked: ${businessSignupRequestId}`,
      })
    }

    return row
  }

  const linkBusinessSignupToPipeline = async (
    businessSignupRequestId: string | null,
    pipelineRef: string,
    nowIso: string,
  ): Promise<void> => {
    if (businessSignupRequestId === null) {
      return
    }

    await db
      .prepare(
        `UPDATE business_signup_requests
            SET linked_pipeline_ref = ?,
                updated_at = ?
          WHERE id = ?
            AND (linked_pipeline_ref IS NULL OR linked_pipeline_ref = ?)`,
      )
      .bind(pipelineRef, nowIso, businessSignupRequestId, pipelineRef)
      .run()
  }

  const createPipelineRow = async (
    input: BusinessPipelineCreateInput,
    runtime: BusinessPipelineRuntime = systemBusinessPipelineRuntime,
  ): Promise<BusinessPipelineRow> => {
    try {
      const pipelineRef = input.pipelineRef.trim()
      const vertical = input.vertical.trim().toLowerCase()
      const decodedSourceRef = decodeBusinessSourceRef(input.sourceRef)
      if ('reason' in decodedSourceRef) {
        throw validationError(decodedSourceRef.reason)
      }
      const sourceRef = decodedSourceRef.sourceRef
      const stage = input.stage ?? 'intake_received'
      const receiptRefs = normalizeReceiptRefs(input.receiptRefs)
      const quotedBand = normalizeQuotedBand(input.quotedBand)
      const nextActionDueAt = input.nextActionDueAt?.trim() || null
      const blockerRef = normalizeNullableRef('blockerRef', input.blockerRef)
      const businessSignupRequestId = normalizeNullableRef(
        'businessSignupRequestId',
        input.businessSignupRequestId,
      )
      const nowIso = runtime.nowIso()

      assertPublicSafeRef('pipelineRef', pipelineRef)
      assertPublicSafeDescriptor('vertical', vertical)
      if (nextActionDueAt !== null) {
        assertPublicSafeDescriptor('nextActionDueAt', nextActionDueAt)
      }
      if (stage !== 'intake_received' && receiptRefs.length === 0) {
        throw validationError('non-intake pipeline rows require at least one receipt ref')
      }
      await readBusinessSignupPipelineLink(
        businessSignupRequestId,
        pipelineRef,
        sourceRef,
      )

      const result = await db
        .prepare(
          `INSERT OR IGNORE INTO business_pipeline_rows (
            pipeline_ref,
            business_signup_request_id,
            vertical,
            source_ref,
            stage,
            quoted_min_usd_cents,
            quoted_max_usd_cents,
            quoted_band_label,
            owner_role,
            next_action_due_at,
            blocker_ref,
            receipt_refs_json,
            partner_route_flag,
            created_at,
            updated_at,
            stage_updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          pipelineRef,
          businessSignupRequestId,
          vertical,
          sourceRef,
          stage,
          quotedBand.minUsdCents,
          quotedBand.maxUsdCents,
          quotedBand.label,
          input.ownerRole,
          nextActionDueAt,
          blockerRef,
          JSON.stringify(receiptRefs),
          input.partnerRouteFlag === true ? 1 : 0,
          nowIso,
          nowIso,
          nowIso,
        )
        .run()

      if (Number(result.meta?.changes ?? 0) === 0) {
        throw new BusinessPipelineStoreError({
          kind: 'conflict',
          reason: `pipeline row already exists: ${pipelineRef}`,
        })
      }
      await linkBusinessSignupToPipeline(
        businessSignupRequestId,
        pipelineRef,
        nowIso,
      )

      const created = await readPipelineRow(pipelineRef)
      if (created === null) {
        throw new BusinessPipelineStoreError({
          kind: 'storage_error',
          reason: `pipeline row was not readable after create: ${pipelineRef}`,
        })
      }
      return created
    } catch (error) {
      if (error instanceof BusinessPipelineValidationError) {
        throw storeValidationError(error.reason)
      }
      throw storageError(error)
    }
  }

  const advancePipelineRow = async (
    pipelineRef: string,
    input: BusinessPipelineAdvanceInput,
    runtime: BusinessPipelineRuntime = systemBusinessPipelineRuntime,
  ): Promise<BusinessPipelineRow> => {
    try {
      const current = await readPipelineRow(pipelineRef)
      if (current === null) {
        throw new BusinessPipelineStoreError({
          kind: 'not_found',
          reason: `pipeline row not found: ${pipelineRef}`,
        })
      }

      if (!allowedTransitions[current.stage].includes(input.stage)) {
        throw validationError(`invalid transition ${current.stage} -> ${input.stage}`)
      }

      const receiptRef = input.receiptRef.trim()
      assertPublicSafeRef('receiptRef', receiptRef)
      const receiptRefs = [...new Set([...current.receiptRefs, receiptRef])]
      const quotedBand =
        input.quotedBand === undefined || input.quotedBand === null
          ? current.quotedBand
          : normalizeQuotedBand(input.quotedBand)
      const nextActionDueAt =
        input.nextActionDueAt === undefined
          ? current.nextActionDueAt
          : (input.nextActionDueAt?.trim() || null)
      const blockerRef =
        input.blockerRef === undefined
          ? current.blockerRef
          : normalizeNullableRef('blockerRef', input.blockerRef)
      if (nextActionDueAt !== null) {
        assertPublicSafeDescriptor('nextActionDueAt', nextActionDueAt)
      }
      const nowIso = runtime.nowIso()

      await db
        .prepare(
          `UPDATE business_pipeline_rows
              SET stage = ?,
                  quoted_min_usd_cents = ?,
                  quoted_max_usd_cents = ?,
                  quoted_band_label = ?,
                  owner_role = ?,
                  next_action_due_at = ?,
                  blocker_ref = ?,
                  receipt_refs_json = ?,
                  updated_at = ?,
                  stage_updated_at = ?
            WHERE pipeline_ref = ?`,
        )
        .bind(
          input.stage,
          quotedBand.minUsdCents,
          quotedBand.maxUsdCents,
          quotedBand.label,
          input.ownerRole ?? current.ownerRole,
          nextActionDueAt,
          blockerRef,
          JSON.stringify(receiptRefs),
          nowIso,
          nowIso,
          current.pipelineRef,
        )
        .run()

      const updated = await readPipelineRow(current.pipelineRef)
      if (updated === null) {
        throw new BusinessPipelineStoreError({
          kind: 'storage_error',
          reason: `pipeline row was not readable after advance: ${pipelineRef}`,
        })
      }
      return updated
    } catch (error) {
      if (error instanceof BusinessPipelineValidationError) {
        throw storeValidationError(error.reason)
      }
      throw storageError(error)
    }
  }

  const appendPipelineReceiptRefs = async (
    pipelineRef: string,
    refs: ReadonlyArray<string>,
    runtime: BusinessPipelineRuntime = systemBusinessPipelineRuntime,
  ): Promise<BusinessPipelineRow> => {
    try {
      const current = await readPipelineRow(pipelineRef)
      if (current === null) {
        throw new BusinessPipelineStoreError({
          kind: 'not_found',
          reason: `pipeline row not found: ${pipelineRef}`,
        })
      }

      const receiptRefs = [
        ...new Set([
          ...current.receiptRefs,
          ...normalizeReceiptRefs(refs),
        ]),
      ]
      const nowIso = runtime.nowIso()

      await db
        .prepare(
          `UPDATE business_pipeline_rows
              SET receipt_refs_json = ?,
                  updated_at = ?
            WHERE pipeline_ref = ?`,
        )
        .bind(JSON.stringify(receiptRefs), nowIso, current.pipelineRef)
        .run()

      const updated = await readPipelineRow(current.pipelineRef)
      if (updated === null) {
        throw new BusinessPipelineStoreError({
          kind: 'storage_error',
          reason: `pipeline row was not readable after receipt append: ${pipelineRef}`,
        })
      }
      return updated
    } catch (error) {
      if (error instanceof BusinessPipelineValidationError) {
        throw storeValidationError(error.reason)
      }
      throw storageError(error)
    }
  }

  const createCommitment = async (
    input: BusinessPipelineCommitmentInput,
    runtime: BusinessPipelineRuntime = systemBusinessPipelineRuntime,
  ): Promise<BusinessPipelineCommitmentRecord> => {
    try {
      const pipeline = await readPipelineRow(input.pipelineRef)
      if (pipeline === null) {
        throw new BusinessPipelineStoreError({
          kind: 'not_found',
          reason: `pipeline row not found: ${input.pipelineRef}`,
        })
      }

      const commitmentRef = input.commitmentRef.trim()
      const ownerRef = input.ownerRef.trim()
      const promisedObjectRef = input.promisedObjectRef.trim()
      const engagementRef =
        input.engagementRef?.trim() ??
        `business.engagement.pipeline_${safeRefPart(pipeline.pipelineRef)}`
      const sourceRefs = normalizeReceiptRefs(input.sourceRefs)
      const blockerRefs = normalizeReceiptRefs(input.blockerRefs)
      const evidenceRefs = normalizeReceiptRefs(input.evidenceRefs)

      assertPublicSafeRef('commitmentRef', commitmentRef)
      assertPublicSafeRef('ownerRef', ownerRef)
      assertPublicSafeRef('promisedObjectRef', promisedObjectRef)
      assertPublicSafeRef('engagementRef', engagementRef)

      const nowIso = runtime.nowIso()
      const result = await db
        .prepare(
          `INSERT OR IGNORE INTO business_commitment_ledger (
            id,
            commitment_ref,
            engagement_ref,
            owner_ref,
            vertical_ref,
            promised_object_ref,
            commitment_kind,
            due_state,
            due_at,
            shipped_at,
            weekly_review_ref,
            source_refs_json,
            blocker_refs_json,
            evidence_refs_json,
            created_at,
            updated_at,
            pipeline_ref
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          runtime.makeId('business_pipeline_commitment'),
          commitmentRef,
          engagementRef,
          ownerRef,
          `vertical.${safeRefPart(pipeline.vertical)}`,
          promisedObjectRef,
          input.commitmentKind,
          input.dueState ?? 'due',
          input.dueAt,
          input.shippedAt ?? null,
          BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
          JSON.stringify(sourceRefs),
          JSON.stringify(blockerRefs),
          JSON.stringify(evidenceRefs),
          nowIso,
          nowIso,
          pipeline.pipelineRef,
        )
        .run()

      if (Number(result.meta?.changes ?? 0) === 0) {
        throw new BusinessPipelineStoreError({
          kind: 'conflict',
          reason: `commitment already exists: ${commitmentRef}`,
        })
      }

      const commitments = await listCommitmentsForPipeline(pipeline.pipelineRef)
      const created = commitments.find(
        commitment => commitment.commitmentRef === commitmentRef,
      )
      if (created === undefined) {
        throw new BusinessPipelineStoreError({
          kind: 'storage_error',
          reason: `commitment was not readable after create: ${commitmentRef}`,
        })
      }
      return created
    } catch (error) {
      if (error instanceof BusinessPipelineValidationError) {
        throw storeValidationError(error.reason)
      }
      throw storageError(error)
    }
  }

  const readMetrics = async (nowIso: string): Promise<BusinessPipelineMetrics> => {
    const rows = await listPipelineRows()
    const commitmentRows = await db
      .prepare(
        `SELECT pipeline_ref, COUNT(*) AS commitment_count
           FROM business_commitment_ledger
          WHERE pipeline_ref IS NOT NULL
          GROUP BY pipeline_ref`,
      )
      .all<PipelineCommitmentCountRow>()
    const stageCountRows = await db
      .prepare(
        `SELECT stage, COUNT(*) AS count
           FROM business_pipeline_rows
          GROUP BY stage`,
      )
      .all<StageCountRow>()
    const total = await db
      .prepare(`SELECT COUNT(*) AS count FROM business_pipeline_rows`)
      .first<CountRow>()

    const commitmentCounts = new Map(
      (commitmentRows.results ?? []).map(row => [
        row.pipeline_ref,
        Number(row.commitment_count),
      ]),
    )
    const missingCommitmentDefects = rows
      .filter(row => (commitmentCounts.get(row.pipelineRef) ?? 0) === 0)
      .map(row => `commitment.untracked pipelineRef=${row.pipelineRef} vertical=${row.vertical}`)
    const stageCountMap = new Map(
      (stageCountRows.results ?? []).map(row => [row.stage, Number(row.count)]),
    )

    return S.decodeUnknownSync(BusinessPipelineMetrics)({
      schemaVersion: 'openagents.business_pipeline_metrics.v1',
      commitmentCoverage: {
        linkedPipelineRowCount: rows.length - missingCommitmentDefects.length,
        missingCommitmentDefects,
        status: rows.length === 0 ? 'not_measured' : 'measured',
        totalPipelineRowCount: Number(total?.count ?? rows.length),
      },
      generatedAt: nowIso,
      privacyBoundary: {
        excludes: [
          'prospect_name',
          'contact_email',
          'domain',
          'raw_call_notes',
          'raw_crm_payload',
          'payment_payload',
        ],
        opaqueRefsOnly: true,
      },
      qualifiedPipeline: qualifiedPipelineMetric(rows),
      rates: pipelineRates(rows),
      sourceRefBreakdown: sourceRefBreakdown(rows),
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8263',
        'github:OpenAgentsInc/openagents#8267',
        'docs/fable/2026-07-03-bf-9-2-weekly-pipeline-review.md',
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#7-pipeline-definition-and-the-25k-math-honest',
      ],
      stageCounts: BUSINESS_PIPELINE_STAGE_ORDER.map(stage => ({
        count: stageCountMap.get(stage) ?? 0,
        stage,
      })),
      staleness: liveAtReadStaleness([
        'business_pipeline_rows.insert',
        'business_pipeline_rows.update',
        'business_signup_requests.update',
        'business_commitment_ledger.insert',
        'business_commitment_ledger.update',
      ]),
    })
  }

  return {
    appendPipelineReceiptRefs,
    advancePipelineRow,
    createCommitment,
    createPipelineRow,
    listCommitmentsForPipeline,
    listPipelineRows,
    readMetrics,
    readPipelineRow,
  }
}
