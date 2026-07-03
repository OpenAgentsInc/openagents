import { Schema as S } from 'effect'

import { parseJsonStringArray } from './json-boundary'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF =
  'business.pipeline_review.weekly'

export const BusinessCommitmentDueState = S.Literals([
  'due',
  'blocked',
  'shipped',
  'parked',
])
export type BusinessCommitmentDueState =
  typeof BusinessCommitmentDueState.Type

export const BusinessCommitmentKind = S.Literals(['deliverable', 'send'])
export type BusinessCommitmentKind = typeof BusinessCommitmentKind.Type

export const BusinessCommitmentLedgerRecord = S.Struct({
  blockerRefs: S.Array(S.String),
  commitmentKind: BusinessCommitmentKind,
  commitmentRef: S.String,
  createdAt: S.String,
  dueAt: S.String,
  dueState: BusinessCommitmentDueState,
  engagementRef: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  ownerRef: S.String,
  promisedObjectRef: S.String,
  shippedAt: S.NullOr(S.String),
  sourceRefs: S.Array(S.String),
  updatedAt: S.String,
  verticalRef: S.String,
  weeklyReviewRef: S.String,
})
export type BusinessCommitmentLedgerRecord =
  typeof BusinessCommitmentLedgerRecord.Type

export class BusinessCommitmentLedgerValidationError extends S.TaggedErrorClass<BusinessCommitmentLedgerValidationError>()(
  'BusinessCommitmentLedgerValidationError',
  { reason: S.String },
) {}

export type BusinessCommitmentWeeklyReview = Readonly<{
  schemaVersion: 'openagents.business_commitment_weekly_review.v1'
  generatedAt: string
  staleness: PublicProjectionStalenessContract
  weeklyReviewRef: typeof BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF
  totals: Readonly<{
    commitmentCount: number
    dueCount: number
    blockedCount: number
    shippedCount: number
    parkedCount: number
    untrackedOwedCommitmentCount: 0
  }>
  owedMakeGoodRefs: ReadonlyArray<string>
  commitments: ReadonlyArray<BusinessCommitmentLedgerRecord>
  privacyBoundary: Readonly<{
    opaqueRefsOnly: true
    excludes: ReadonlyArray<string>
  }>
  evidenceRefs: ReadonlyArray<string>
}>

type BusinessCommitmentRow = Readonly<{
  blocker_refs_json: string
  commitment_kind: string
  commitment_ref: string
  created_at: string
  due_at: string
  due_state: string
  engagement_ref: string
  evidence_refs_json: string
  id: string
  owner_ref: string
  promised_object_ref: string
  shipped_at: string | null
  source_refs_json: string
  updated_at: string
  vertical_ref: string
  weekly_review_ref: string
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/=#-]{0,240}$/
const UNSAFE_REF_PATTERN =
  /\b(client|customer|contact|email|phone|raw|provider_payload|access_token|refresh_token|private_key|wallet_secret|payment_preimage|webhook_secret|xprv|mnemonic)\b|@/i

const assertPublicSafeRef = (field: string, value: string): void => {
  if (!SAFE_REF_PATTERN.test(value) || UNSAFE_REF_PATTERN.test(value)) {
    throw new BusinessCommitmentLedgerValidationError({
      reason: `${field} must be an opaque public-safe ref`,
    })
  }
}

const assertPublicSafeRefs = (
  field: string,
  values: ReadonlyArray<string>,
): void => values.forEach(value => assertPublicSafeRef(field, value))

const commitmentFromRow = (
  row: BusinessCommitmentRow,
): BusinessCommitmentLedgerRecord => {
  const record: BusinessCommitmentLedgerRecord = {
    blockerRefs: parseJsonStringArray(row.blocker_refs_json),
    commitmentKind: S.decodeUnknownSync(BusinessCommitmentKind)(
      row.commitment_kind,
    ),
    commitmentRef: row.commitment_ref,
    createdAt: row.created_at,
    dueAt: row.due_at,
    dueState: S.decodeUnknownSync(BusinessCommitmentDueState)(row.due_state),
    engagementRef: row.engagement_ref,
    evidenceRefs: parseJsonStringArray(row.evidence_refs_json),
    id: row.id,
    ownerRef: row.owner_ref,
    promisedObjectRef: row.promised_object_ref,
    shippedAt: row.shipped_at,
    sourceRefs: parseJsonStringArray(row.source_refs_json),
    updatedAt: row.updated_at,
    verticalRef: row.vertical_ref,
    weeklyReviewRef: row.weekly_review_ref,
  }

  assertPublicSafeRef('commitmentRef', record.commitmentRef)
  assertPublicSafeRef('engagementRef', record.engagementRef)
  assertPublicSafeRef('ownerRef', record.ownerRef)
  assertPublicSafeRef('promisedObjectRef', record.promisedObjectRef)
  assertPublicSafeRef('verticalRef', record.verticalRef)
  assertPublicSafeRef('weeklyReviewRef', record.weeklyReviewRef)
  assertPublicSafeRefs('sourceRefs', record.sourceRefs)
  assertPublicSafeRefs('blockerRefs', record.blockerRefs)
  assertPublicSafeRefs('evidenceRefs', record.evidenceRefs)

  return record
}

const countByState = (
  commitments: ReadonlyArray<BusinessCommitmentLedgerRecord>,
  state: BusinessCommitmentDueState,
): number =>
  commitments.filter(commitment => commitment.dueState === state).length

export const readBusinessCommitmentWeeklyReview = async (
  db: D1Database,
  nowIso: string,
): Promise<BusinessCommitmentWeeklyReview> => {
  const rows = await db
    .prepare(
      `SELECT *
         FROM business_commitment_ledger
        WHERE weekly_review_ref = ?
        ORDER BY
          CASE due_state
            WHEN 'due' THEN 0
            WHEN 'blocked' THEN 1
            WHEN 'parked' THEN 2
            WHEN 'shipped' THEN 3
          END ASC,
          due_at ASC,
          updated_at ASC`,
    )
    .bind(BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF)
    .all<BusinessCommitmentRow>()

  const commitments = (rows.results ?? []).map(commitmentFromRow)
  const owedMakeGoodRefs = commitments
    .filter(commitment =>
      commitment.commitmentRef.startsWith('business.commitment.owed.'),
    )
    .map(commitment => commitment.commitmentRef)

  return {
    schemaVersion: 'openagents.business_commitment_weekly_review.v1',
    generatedAt: nowIso,
    staleness: liveAtReadStaleness([
      'business_commitment_ledger.insert',
      'business_commitment_ledger.update',
    ]),
    weeklyReviewRef: BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
    totals: {
      commitmentCount: commitments.length,
      dueCount: countByState(commitments, 'due'),
      blockedCount: countByState(commitments, 'blocked'),
      shippedCount: countByState(commitments, 'shipped'),
      parkedCount: countByState(commitments, 'parked'),
      untrackedOwedCommitmentCount: 0,
    },
    owedMakeGoodRefs,
    commitments,
    privacyBoundary: {
      opaqueRefsOnly: true,
      excludes: [
        'client_name',
        'client_email',
        'contact_email',
        'raw_crm',
        'raw_email_body',
        'provider_payload',
        'wallet_material',
      ],
    },
    evidenceRefs: [
      'table:business_commitment_ledger',
      'issue:8115',
      'roadmap:BF-9.1',
    ],
  }
}
