import { Schema as S } from 'effect'

import { businessSourceRefForReferralCode } from './business-source-attribution'
import { liveAtReadStaleness } from './public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const BusinessAffiliateCodeState = S.Literals([
  'active',
  'paused',
  'archived',
])
export type BusinessAffiliateCodeState =
  typeof BusinessAffiliateCodeState.Type

export const BusinessAffiliateCodeRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  code: S.String,
  createdAt: S.String,
  issuedByRef: S.String,
  ownerRef: S.String,
  policyState: BusinessAffiliateCodeState,
  sourceRef: S.String,
  updatedAt: S.String,
})
export type BusinessAffiliateCodeRecord =
  typeof BusinessAffiliateCodeRecord.Type

export const BusinessAffiliateAttributionRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  attributionRef: S.String,
  businessSignupRequestId: S.String,
  code: S.String,
  createdAt: S.String,
  ownerRef: S.String,
  paymentReceiptRef: S.NullOr(S.String),
  pipelineRef: S.NullOr(S.String),
  policyState: S.Literal('active'),
  sourceRef: S.String,
  updatedAt: S.String,
})
export type BusinessAffiliateAttributionRecord =
  typeof BusinessAffiliateAttributionRecord.Type

export const BusinessAffiliateMetricStatus = S.Literals([
  'measured',
  'not_measured',
])
export type BusinessAffiliateMetricStatus =
  typeof BusinessAffiliateMetricStatus.Type

export const BusinessAffiliateConversionLeg = S.Struct({
  ref: S.NullOr(S.String),
  status: BusinessAffiliateMetricStatus,
})
export type BusinessAffiliateConversionLeg =
  typeof BusinessAffiliateConversionLeg.Type

export const BusinessAffiliateConversionRecord = S.Struct({
  attributionRef: S.String,
  businessSignupRequestId: S.String,
  code: S.String,
  intake: BusinessAffiliateConversionLeg,
  ownerRef: S.String,
  payment: BusinessAffiliateConversionLeg,
  pipeline: BusinessAffiliateConversionLeg,
  sourceRef: S.String,
})
export type BusinessAffiliateConversionRecord =
  typeof BusinessAffiliateConversionRecord.Type

export const BusinessAffiliateAttributionReport = S.Struct({
  authorityBoundary: S.String,
  code: BusinessAffiliateCodeRecord,
  conversions: S.Array(BusinessAffiliateConversionRecord),
  generatedAt: S.String,
  privacyBoundary: S.Struct({
    excludes: S.Array(S.String),
    exactOnly: S.Literal(true),
    opaqueRefsOnly: S.Literal(true),
  }),
  rates: S.Struct({
    intakeToPayment: S.Struct({
      denominator: S.Number,
      numerator: S.Number,
      status: BusinessAffiliateMetricStatus,
      value: S.NullOr(S.Number),
    }),
    intakeToPipeline: S.Struct({
      denominator: S.Number,
      numerator: S.Number,
      status: BusinessAffiliateMetricStatus,
      value: S.NullOr(S.Number),
    }),
  }),
  schemaVersion: S.Literal('openagents.business_affiliate_attribution.v1'),
  sourceRefs: S.Array(S.String),
  staleness: S.Struct({
    composition: S.String,
    rebuildsOn: S.Array(S.String),
  }),
  totals: S.Struct({
    attributedSignupCount: S.Number,
    paymentReceiptCount: S.Number,
    pipelineLinkedCount: S.Number,
  }),
})
export type BusinessAffiliateAttributionReport =
  typeof BusinessAffiliateAttributionReport.Type

export class BusinessAffiliateAttributionStoreError extends S.TaggedErrorClass<BusinessAffiliateAttributionStoreError>()(
  'BusinessAffiliateAttributionStoreError',
  {
    kind: S.Literals([
      'not_found',
      'storage_error',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

export type BusinessAffiliateAttributionRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemBusinessAffiliateAttributionRuntime: BusinessAffiliateAttributionRuntime =
  {
    makeId: compactRandomId,
    nowIso: currentIsoTimestamp,
  }

export type CreateBusinessAffiliateCodeInput = Readonly<{
  code: string
  issuedByRef?: string | undefined
  ownerRef: string
  policyState?: BusinessAffiliateCodeState | undefined
}>

type BusinessAffiliateCodeRow = Readonly<{
  archived_at: string | null
  code: string
  created_at: string
  issued_by_ref: string
  owner_ref: string
  policy_state: BusinessAffiliateCodeState
  source_ref: string
  updated_at: string
}>

type BusinessAffiliateAttributionRow = Readonly<{
  archived_at: string | null
  attribution_ref: string
  business_signup_request_id: string
  code: string
  created_at: string
  owner_ref: string
  payment_receipt_ref: string | null
  pipeline_ref: string | null
  policy_state: 'active'
  source_ref: string
  updated_at: string
}>

type BusinessAffiliateConversionRow = Readonly<{
  attribution_ref: string
  business_signup_request_id: string
  code: string
  owner_ref: string
  payment_receipt_ref: string | null
  pipeline_ref: string | null
  signup_pipeline_ref: string | null
  source_ref: string
}>

const SAFE_OPAQUE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,220}$/
const UNSAFE_OPAQUE_REF_PATTERN =
  /@|https?:\/\/|www\.|\b(client|contact|email|phone|raw|payload|token|secret|key|mnemonic|xprv)\b/i

const storageError = (
  error: unknown,
): BusinessAffiliateAttributionStoreError =>
  error instanceof BusinessAffiliateAttributionStoreError
    ? error
    : new BusinessAffiliateAttributionStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      })

const validationError = (
  reason: string,
): BusinessAffiliateAttributionStoreError =>
  new BusinessAffiliateAttributionStoreError({
    kind: 'validation_error',
    reason,
  })

const normalizeOpaqueRef = (field: string, value: string): string => {
  const ref = value.trim()
  if (
    !SAFE_OPAQUE_REF_PATTERN.test(ref) ||
    UNSAFE_OPAQUE_REF_PATTERN.test(ref)
  ) {
    throw validationError(`${field} must be an opaque public-safe ref`)
  }
  return ref
}

export const businessAffiliateCodeFromValue = (
  value: string,
): Readonly<{ code: string; sourceRef: string }> => {
  const sourceRef = businessSourceRefForReferralCode(value)
  if (sourceRef === 'affiliate_unknown') {
    throw validationError('code must produce a bounded affiliate sourceRef')
  }
  return {
    code: sourceRef.slice('affiliate_'.length),
    sourceRef,
  }
}

const codeFromRow = (
  row: BusinessAffiliateCodeRow,
): BusinessAffiliateCodeRecord =>
  S.decodeUnknownSync(BusinessAffiliateCodeRecord)({
    archivedAt: row.archived_at,
    code: row.code,
    createdAt: row.created_at,
    issuedByRef: row.issued_by_ref,
    ownerRef: row.owner_ref,
    policyState: row.policy_state,
    sourceRef: row.source_ref,
    updatedAt: row.updated_at,
  })

const attributionFromRow = (
  row: BusinessAffiliateAttributionRow,
): BusinessAffiliateAttributionRecord =>
  S.decodeUnknownSync(BusinessAffiliateAttributionRecord)({
    archivedAt: row.archived_at,
    attributionRef: row.attribution_ref,
    businessSignupRequestId: row.business_signup_request_id,
    code: row.code,
    createdAt: row.created_at,
    ownerRef: row.owner_ref,
    paymentReceiptRef: row.payment_receipt_ref,
    pipelineRef: row.pipeline_ref,
    policyState: row.policy_state,
    sourceRef: row.source_ref,
    updatedAt: row.updated_at,
  })

const readCodeByCode = async (
  db: D1Database,
  code: string,
): Promise<BusinessAffiliateCodeRecord | null> => {
  const row = await db
    .prepare(
      `SELECT code,
              source_ref,
              owner_ref,
              issued_by_ref,
              policy_state,
              created_at,
              updated_at,
              archived_at
         FROM business_affiliate_codes
        WHERE code = ?
        LIMIT 1`,
    )
    .bind(code)
    .first<BusinessAffiliateCodeRow>()

  return row === null ? null : codeFromRow(row)
}

const readActiveCodeBySourceRef = async (
  db: D1Database,
  sourceRef: string,
): Promise<BusinessAffiliateCodeRecord | null> => {
  const row = await db
    .prepare(
      `SELECT code,
              source_ref,
              owner_ref,
              issued_by_ref,
              policy_state,
              created_at,
              updated_at,
              archived_at
         FROM business_affiliate_codes
        WHERE source_ref = ?
          AND policy_state = 'active'
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(sourceRef)
    .first<BusinessAffiliateCodeRow>()

  return row === null ? null : codeFromRow(row)
}

const readAttributionForSignup = async (
  db: D1Database,
  businessSignupRequestId: string,
): Promise<BusinessAffiliateAttributionRecord | null> => {
  const row = await db
    .prepare(
      `SELECT attribution_ref,
              code,
              source_ref,
              owner_ref,
              business_signup_request_id,
              pipeline_ref,
              payment_receipt_ref,
              policy_state,
              created_at,
              updated_at,
              archived_at
         FROM business_affiliate_attributions
        WHERE business_signup_request_id = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(businessSignupRequestId)
    .first<BusinessAffiliateAttributionRow>()

  return row === null ? null : attributionFromRow(row)
}

export const createBusinessAffiliateCode = async (
  db: D1Database,
  input: CreateBusinessAffiliateCodeInput,
  runtime: BusinessAffiliateAttributionRuntime =
    systemBusinessAffiliateAttributionRuntime,
): Promise<BusinessAffiliateCodeRecord> => {
  try {
    const { code, sourceRef } = businessAffiliateCodeFromValue(input.code)
    const ownerRef = normalizeOpaqueRef('ownerRef', input.ownerRef)
    const issuedByRef = normalizeOpaqueRef(
      'issuedByRef',
      input.issuedByRef ?? 'operator.openagents.business',
    )
    const policyState = input.policyState ?? 'active'
    if (!['active', 'paused', 'archived'].includes(policyState)) {
      throw validationError('policyState must be active, paused, or archived')
    }
    const nowIso = runtime.nowIso()

    await db
      .prepare(
        `INSERT OR IGNORE INTO business_affiliate_codes (
          code,
          source_ref,
          owner_ref,
          issued_by_ref,
          policy_state,
          created_at,
          updated_at,
          archived_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        code,
        sourceRef,
        ownerRef,
        issuedByRef,
        policyState,
        nowIso,
        nowIso,
      )
      .run()

    const record = await readCodeByCode(db, code)
    if (record === null) {
      throw new BusinessAffiliateAttributionStoreError({
        kind: 'storage_error',
        reason: `affiliate code was not readable after create: ${code}`,
      })
    }
    return record
  } catch (error) {
    throw storageError(error)
  }
}

export type BusinessAffiliateSignupAttributionResult =
  | Readonly<{ _tag: 'no_affiliate_source' }>
  | Readonly<{ _tag: 'unregistered_code'; sourceRef: string }>
  | Readonly<{
      _tag: 'attributed'
      attribution: BusinessAffiliateAttributionRecord
    }>

export const recordBusinessAffiliateSignupAttribution = async (
  db: D1Database,
  input: Readonly<{
    businessSignupRequestId: string
    sourceRef: string
  }>,
  runtime: BusinessAffiliateAttributionRuntime =
    systemBusinessAffiliateAttributionRuntime,
): Promise<BusinessAffiliateSignupAttributionResult> => {
  try {
    if (!input.sourceRef.startsWith('affiliate_')) {
      return { _tag: 'no_affiliate_source' }
    }

    const code = await readActiveCodeBySourceRef(db, input.sourceRef)
    if (code === null) {
      return { _tag: 'unregistered_code', sourceRef: input.sourceRef }
    }

    const existing = await readAttributionForSignup(
      db,
      input.businessSignupRequestId,
    )
    if (existing !== null) {
      return { _tag: 'attributed', attribution: existing }
    }

    const nowIso = runtime.nowIso()
    const attributionRef = runtime.makeId('business_affiliate_attribution')

    await db
      .prepare(
        `INSERT OR IGNORE INTO business_affiliate_attributions (
          attribution_ref,
          code,
          source_ref,
          owner_ref,
          business_signup_request_id,
          pipeline_ref,
          payment_receipt_ref,
          policy_state,
          created_at,
          updated_at,
          archived_at
        ) VALUES (?, ?, ?, ?, ?, NULL, NULL, 'active', ?, ?, NULL)`,
      )
      .bind(
        attributionRef,
        code.code,
        code.sourceRef,
        code.ownerRef,
        input.businessSignupRequestId,
        nowIso,
        nowIso,
      )
      .run()

    const attribution = await readAttributionForSignup(
      db,
      input.businessSignupRequestId,
    )
    if (attribution === null) {
      throw new BusinessAffiliateAttributionStoreError({
        kind: 'storage_error',
        reason: `affiliate attribution was not readable after create: ${input.businessSignupRequestId}`,
      })
    }
    return { _tag: 'attributed', attribution }
  } catch (error) {
    throw storageError(error)
  }
}

export const linkBusinessAffiliateAttributionToPipeline = async (
  db: D1Database,
  input: Readonly<{
    businessSignupRequestId: string
    pipelineRef: string
    updatedAt: string
  }>,
): Promise<void> => {
  try {
    await db
      .prepare(
        `UPDATE business_affiliate_attributions
            SET pipeline_ref = COALESCE(pipeline_ref, ?),
                updated_at = ?
          WHERE business_signup_request_id = ?
            AND archived_at IS NULL
            AND policy_state = 'active'
            AND (pipeline_ref IS NULL OR pipeline_ref = ?)`,
      )
      .bind(
        input.pipelineRef,
        input.updatedAt,
        input.businessSignupRequestId,
        input.pipelineRef,
      )
      .run()
  } catch (error) {
    throw storageError(error)
  }
}

const measuredLeg = (ref: string): BusinessAffiliateConversionLeg => ({
  ref,
  status: 'measured',
})

const notMeasuredLeg = (): BusinessAffiliateConversionLeg => ({
  ref: null,
  status: 'not_measured',
})

const rateMetric = (
  numerator: number,
  denominator: number,
): BusinessAffiliateAttributionReport['rates']['intakeToPipeline'] => ({
  denominator,
  numerator,
  status: denominator === 0 ? 'not_measured' : 'measured',
  value: denominator === 0 ? null : Number((numerator / denominator).toFixed(4)),
})

export const readBusinessAffiliateAttributionReport = async (
  db: D1Database,
  input: Readonly<{ code: string; nowIso: string }>,
): Promise<BusinessAffiliateAttributionReport> => {
  try {
    const { code } = businessAffiliateCodeFromValue(input.code)
    const record = await readCodeByCode(db, code)
    if (record === null || record.archivedAt !== null) {
      throw new BusinessAffiliateAttributionStoreError({
        kind: 'not_found',
        reason: `affiliate code not found: ${code}`,
      })
    }

    const rows = await db
      .prepare(
        `SELECT attr.attribution_ref,
                attr.code,
                attr.source_ref,
                attr.owner_ref,
                attr.business_signup_request_id,
                attr.pipeline_ref,
                signup.linked_pipeline_ref AS signup_pipeline_ref,
                COALESCE(attr.payment_receipt_ref, pay.public_receipt_ref)
                  AS payment_receipt_ref
           FROM business_affiliate_attributions AS attr
           LEFT JOIN business_signup_requests AS signup
             ON signup.id = attr.business_signup_request_id
           LEFT JOIN (
             SELECT business_signup_request_id,
                    MIN(public_receipt_ref) AS public_receipt_ref
               FROM business_checkout_kickoffs
              GROUP BY business_signup_request_id
           ) AS pay
             ON pay.business_signup_request_id = attr.business_signup_request_id
          WHERE attr.code = ?
            AND attr.archived_at IS NULL
          ORDER BY attr.created_at ASC, attr.attribution_ref ASC`,
      )
      .bind(record.code)
      .all<BusinessAffiliateConversionRow>()

    const conversions = (rows.results ?? []).map(row => {
      const pipelineRef = row.pipeline_ref ?? row.signup_pipeline_ref
      const paymentReceiptRef = row.payment_receipt_ref
      return S.decodeUnknownSync(BusinessAffiliateConversionRecord)({
        attributionRef: row.attribution_ref,
        businessSignupRequestId: row.business_signup_request_id,
        code: row.code,
        intake: measuredLeg(`business_signup:${row.business_signup_request_id}`),
        ownerRef: row.owner_ref,
        payment:
          paymentReceiptRef === null
            ? notMeasuredLeg()
            : measuredLeg(paymentReceiptRef),
        pipeline:
          pipelineRef === null
            ? notMeasuredLeg()
            : measuredLeg(pipelineRef),
        sourceRef: row.source_ref,
      })
    })
    const pipelineLinkedCount = conversions.filter(
      conversion => conversion.pipeline.status === 'measured',
    ).length
    const paymentReceiptCount = conversions.filter(
      conversion => conversion.payment.status === 'measured',
    ).length

    return S.decodeUnknownSync(BusinessAffiliateAttributionReport)({
      authorityBoundary:
        'Attribution and conversion report only; grants no payout, settlement, public earning claim, spend, send, or agent authority.',
      code: record,
      conversions,
      generatedAt: input.nowIso,
      privacyBoundary: {
        excludes: [
          'contact_email',
          'prospect_name',
          'raw_utm',
          'raw_referrer_identity',
          'payment_payload',
          'payout_destination',
        ],
        exactOnly: true,
        opaqueRefsOnly: true,
      },
      rates: {
        intakeToPayment: rateMetric(paymentReceiptCount, conversions.length),
        intakeToPipeline: rateMetric(pipelineLinkedCount, conversions.length),
      },
      schemaVersion: 'openagents.business_affiliate_attribution.v1',
      sourceRefs: [
        'github:OpenAgentsInc/openagents#8269',
        'docs/fable/2026-07-03-apollo-outbound-sales-plan.md#10-episode-247-monday-revenue-stack',
        'docs/fable/ROADMAP_BIZ.md#bf-8--retain-and-multiply',
      ],
      staleness: liveAtReadStaleness([
        'business_affiliate_codes.insert',
        'business_affiliate_attributions.insert',
        'business_affiliate_attributions.update',
        'business_signup_requests.insert',
        'business_pipeline_rows.insert',
        'business_checkout_kickoffs.insert',
      ]),
      totals: {
        attributedSignupCount: conversions.length,
        paymentReceiptCount,
        pipelineLinkedCount,
      },
    })
  } catch (error) {
    throw storageError(error)
  }
}
