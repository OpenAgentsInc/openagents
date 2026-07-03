import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const BusinessCaseStudyEndpoint =
  '/api/public/business/case-studies' as const

export const BUSINESS_CASE_STUDY_SCHEMA =
  'openagents.business.case_study.v1' as const

export const BusinessCaseStudyStatus = S.Literals([
  'draft',
  'published',
  'blocked',
  'archived',
])
export type BusinessCaseStudyStatus = typeof BusinessCaseStudyStatus.Type

export const BusinessCaseStudyVerticalDescriptor = S.Literals([
  'legal',
  'marketing_agency',
  'ecommerce',
  'health',
  'software',
  'other_business',
])
export type BusinessCaseStudyVerticalDescriptor =
  typeof BusinessCaseStudyVerticalDescriptor.Type

export const BusinessCaseStudySourceKind = S.Literals([
  'content',
  'outbound',
  'ai_search',
  'referral',
  'direct',
  'unknown',
])
export type BusinessCaseStudySourceKind =
  typeof BusinessCaseStudySourceKind.Type

export const BusinessCaseStudyMetric = S.Struct({
  metricRef: S.String,
  label: S.String,
  value: S.Number,
  unit: S.String,
  evidenceRef: S.String,
})
export type BusinessCaseStudyMetric = typeof BusinessCaseStudyMetric.Type

export const BusinessCaseStudyAttribution = S.Struct({
  sourceKind: BusinessCaseStudySourceKind,
  sourceRef: S.String,
  captureParam: S.Literal('caseStudyRef'),
  intakeAttributionRef: S.String,
})
export type BusinessCaseStudyAttribution =
  typeof BusinessCaseStudyAttribution.Type

export const BusinessCaseStudyPrivacyReview = S.Struct({
  reviewed: S.Boolean,
  reviewedAt: S.String,
  reviewerRef: S.String,
  decisionRef: S.String,
})
export type BusinessCaseStudyPrivacyReview =
  typeof BusinessCaseStudyPrivacyReview.Type

export const BusinessCaseStudyRecord = S.Struct({
  schema: S.Literal(BUSINESS_CASE_STUDY_SCHEMA),
  caseStudyRef: S.String,
  engagementRef: S.String,
  status: BusinessCaseStudyStatus,
  verticalDescriptor: BusinessCaseStudyVerticalDescriptor,
  title: S.String,
  summary: S.String,
  startedAt: S.String,
  completedAt: S.String,
  publishedAt: S.String,
  cycleTimeHours: S.Number,
  acceptedOutcomeRef: S.String,
  publicProofBundleRef: S.String,
  receiptRefs: S.Array(S.String),
  metricDefinitionsRef: S.String,
  metrics: S.Array(BusinessCaseStudyMetric),
  attribution: BusinessCaseStudyAttribution,
  privacyReview: BusinessCaseStudyPrivacyReview,
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type BusinessCaseStudyRecord = typeof BusinessCaseStudyRecord.Type

export class BusinessCaseStudyInvariantError extends S.TaggedErrorClass<BusinessCaseStudyInvariantError>()(
  'BusinessCaseStudyInvariantError',
  { reason: S.String },
) {
  override get message() {
    return this.reason
  }
}

export type BusinessCaseStudyInput = Readonly<{
  caseStudyRef: string
  engagementRef: string
  status: BusinessCaseStudyStatus
  verticalDescriptor: BusinessCaseStudyVerticalDescriptor
  title: string
  summary: string
  startedAt: string
  completedAt: string
  publishedAt?: string | undefined
  acceptedOutcomeRef: string
  publicProofBundleRef: string
  receiptRefs: ReadonlyArray<string>
  metricDefinitionsRef: string
  metrics: ReadonlyArray<BusinessCaseStudyMetric>
  attribution: BusinessCaseStudyAttribution
  privacyReview: BusinessCaseStudyPrivacyReview
  sourceRefs: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string> | undefined
}>

const PUBLIC_SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{2,220}$/
const PRIVATE_TEXT_PATTERN =
  /(@|\/Users\/|access[_-]?token|auth\.json|bearer|client name|contact|customer|customer[_ -]?(email|name|phone)|doctor|email|founder name|invoice|law firm|lawyer|patient|payment[_ -]?(hash|preimage|secret)|phone|physician|private[_ -]?(customer|repo|source)|provider[_ -]?payload|raw[_ -]?(email|invoice|payment|prompt|run|source)|secret|stripe|token|wallet)/i

const DEFAULT_CAVEAT_REFS = [
  'caveat.business.case_study.public_safe_opaque_refs_only',
  'caveat.business.case_study.no_client_identifying_information',
  'caveat.business.case_study.not_payout_or_settlement_authority',
] as const

const trimOrNull = (value: string | null | undefined): string | null => {
  if (value === undefined || value === null) {
    return null
  }
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

const requirePublicSafeRef = (field: string, value: string): string => {
  const trimmed = trimOrNull(value)
  if (trimmed === null) {
    throw new BusinessCaseStudyInvariantError({
      reason: `${field} is required.`,
    })
  }
  if (!PUBLIC_SAFE_REF_PATTERN.test(trimmed) || PRIVATE_TEXT_PATTERN.test(trimmed)) {
    throw new BusinessCaseStudyInvariantError({
      reason: `${field} must be an opaque public-safe ref.`,
    })
  }
  return trimmed
}

const requirePublicSafeText = (field: string, value: string): string => {
  const trimmed = trimOrNull(value)
  if (trimmed === null) {
    throw new BusinessCaseStudyInvariantError({
      reason: `${field} is required.`,
    })
  }
  if (PRIVATE_TEXT_PATTERN.test(trimmed)) {
    throw new BusinessCaseStudyInvariantError({
      reason: `${field} must not contain client-identifying or private material.`,
    })
  }
  return trimmed
}

const requireIsoLike = (field: string, value: string): string => {
  const trimmed = trimOrNull(value)
  if (trimmed === null || Number.isNaN(Date.parse(trimmed))) {
    throw new BusinessCaseStudyInvariantError({
      reason: `${field} must be an ISO-like timestamp.`,
    })
  }
  return trimmed
}

const requireRefs = (
  field: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  if (refs.length === 0) {
    throw new BusinessCaseStudyInvariantError({
      reason: `${field} must include at least one public-safe ref.`,
    })
  }
  return refs.map(ref => requirePublicSafeRef(field, ref))
}

const cycleTimeHoursBetween = (startedAt: string, completedAt: string): number => {
  const hours =
    (Date.parse(completedAt) - Date.parse(startedAt)) / (1000 * 60 * 60)
  return Number(hours.toFixed(2))
}

const assertPublishedEvidence = (input: {
  status: BusinessCaseStudyStatus
  acceptedOutcomeRef: string
  publicProofBundleRef: string
  receiptRefs: ReadonlyArray<string>
  metrics: ReadonlyArray<BusinessCaseStudyMetric>
  privacyReview: BusinessCaseStudyPrivacyReview
}): void => {
  if (input.status !== 'published') {
    return
  }
  if (!input.privacyReview.reviewed) {
    throw new BusinessCaseStudyInvariantError({
      reason: 'published case studies require privacyReview.reviewed=true.',
    })
  }
  requirePublicSafeRef('acceptedOutcomeRef', input.acceptedOutcomeRef)
  requirePublicSafeRef('publicProofBundleRef', input.publicProofBundleRef)
  requireRefs('receiptRefs', input.receiptRefs)
  if (input.metrics.length === 0) {
    throw new BusinessCaseStudyInvariantError({
      reason: 'published case studies require at least one metric.',
    })
  }
}

export const buildBusinessCaseStudy = (
  input: BusinessCaseStudyInput,
): BusinessCaseStudyRecord => {
  const startedAt = requireIsoLike('startedAt', input.startedAt)
  const completedAt = requireIsoLike('completedAt', input.completedAt)
  const publishedAt = requireIsoLike(
    'publishedAt',
    input.publishedAt ?? currentIsoTimestamp(),
  )
  const cycleTimeHours = cycleTimeHoursBetween(startedAt, completedAt)

  if (cycleTimeHours <= 0) {
    throw new BusinessCaseStudyInvariantError({
      reason: 'completedAt must be after startedAt.',
    })
  }

  assertPublishedEvidence(input)

  const metrics = input.metrics.map(metric => ({
    metricRef: requirePublicSafeRef('metrics.metricRef', metric.metricRef),
    label: requirePublicSafeText('metrics.label', metric.label),
    value: metric.value,
    unit: requirePublicSafeText('metrics.unit', metric.unit),
    evidenceRef: requirePublicSafeRef('metrics.evidenceRef', metric.evidenceRef),
  }))

  for (const metric of metrics) {
    if (!Number.isFinite(metric.value) || metric.value < 0) {
      throw new BusinessCaseStudyInvariantError({
        reason: 'metrics.value must be a finite non-negative number.',
      })
    }
  }

  return {
    schema: BUSINESS_CASE_STUDY_SCHEMA,
    caseStudyRef: requirePublicSafeRef('caseStudyRef', input.caseStudyRef),
    engagementRef: requirePublicSafeRef('engagementRef', input.engagementRef),
    status: input.status,
    verticalDescriptor: input.verticalDescriptor,
    title: requirePublicSafeText('title', input.title),
    summary: requirePublicSafeText('summary', input.summary),
    startedAt,
    completedAt,
    publishedAt,
    cycleTimeHours,
    acceptedOutcomeRef: requirePublicSafeRef(
      'acceptedOutcomeRef',
      input.acceptedOutcomeRef,
    ),
    publicProofBundleRef: requirePublicSafeRef(
      'publicProofBundleRef',
      input.publicProofBundleRef,
    ),
    receiptRefs: [...requireRefs('receiptRefs', input.receiptRefs)],
    metricDefinitionsRef: requirePublicSafeRef(
      'metricDefinitionsRef',
      input.metricDefinitionsRef,
    ),
    metrics,
    attribution: {
      sourceKind: input.attribution.sourceKind,
      sourceRef: requirePublicSafeRef(
        'attribution.sourceRef',
        input.attribution.sourceRef,
      ),
      captureParam: 'caseStudyRef',
      intakeAttributionRef: requirePublicSafeRef(
        'attribution.intakeAttributionRef',
        input.attribution.intakeAttributionRef,
      ),
    },
    privacyReview: {
      reviewed: true,
      reviewedAt: requireIsoLike(
        'privacyReview.reviewedAt',
        input.privacyReview.reviewedAt,
      ),
      reviewerRef: requirePublicSafeRef(
        'privacyReview.reviewerRef',
        input.privacyReview.reviewerRef,
      ),
      decisionRef: requirePublicSafeRef(
        'privacyReview.decisionRef',
        input.privacyReview.decisionRef,
      ),
    },
    sourceRefs: [...requireRefs('sourceRefs', input.sourceRefs)],
    caveatRefs: [
      ...requireRefs('caveatRefs', input.caveatRefs ?? DEFAULT_CAVEAT_REFS),
    ],
  }
}

export const assertPublishableBusinessCaseStudy = (
  caseStudy: BusinessCaseStudyRecord,
): void => {
  if (caseStudy.status !== 'published') {
    throw new BusinessCaseStudyInvariantError({
      reason: 'case study must be published before it can feed acquisition.',
    })
  }
  buildBusinessCaseStudy(caseStudy)
}

export const BusinessCaseStudyStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'business_case_studies',
    'business_case_study_attributions',
    'omni_public_proof_bundles',
    'accepted_outcome_receipts',
  ])

export type BusinessCaseStudyStore = {
  list: () => ReadonlyArray<BusinessCaseStudyRecord>
}

export const makeInMemoryBusinessCaseStudyStore = (
  caseStudies: ReadonlyArray<BusinessCaseStudyRecord>,
): BusinessCaseStudyStore => ({
  list: () => caseStudies,
})

export const publicBusinessCaseStudyProjection = (
  caseStudy: BusinessCaseStudyRecord,
) => ({
  schema: caseStudy.schema,
  caseStudyRef: caseStudy.caseStudyRef,
  engagementRef: caseStudy.engagementRef,
  status: caseStudy.status,
  verticalDescriptor: caseStudy.verticalDescriptor,
  title: caseStudy.title,
  summary: caseStudy.summary,
  startedAt: caseStudy.startedAt,
  completedAt: caseStudy.completedAt,
  publishedAt: caseStudy.publishedAt,
  cycleTimeHours: caseStudy.cycleTimeHours,
  acceptedOutcomeRef: caseStudy.acceptedOutcomeRef,
  publicProofBundleRef: caseStudy.publicProofBundleRef,
  receiptRefs: caseStudy.receiptRefs,
  metricDefinitionsRef: caseStudy.metricDefinitionsRef,
  metrics: caseStudy.metrics,
  attribution: caseStudy.attribution,
  privacyDecisionRef: caseStudy.privacyReview.decisionRef,
  sourceRefs: caseStudy.sourceRefs,
  caveatRefs: caseStudy.caveatRefs,
})

export const projectBusinessCaseStudies = (
  caseStudies: ReadonlyArray<BusinessCaseStudyRecord>,
  options?: { generatedAt?: string },
) => {
  const generatedAt = options?.generatedAt ?? currentIsoTimestamp()
  const published = caseStudies
    .filter(caseStudy => caseStudy.status === 'published')
    .map(publicBusinessCaseStudyProjection)

  return {
    schema: 'openagents.business.case_studies.v1',
    generatedAt,
    staleness: BusinessCaseStudyStaleness,
    maxStalenessSeconds: BusinessCaseStudyStaleness.maxStalenessSeconds,
    promiseIds: ['business.intake_quick_win_offering.v1'],
    roadmapRefs: ['ROADMAP_AFTER.A0.9', 'ROADMAP_BIZ.BF-7.4'],
    totals: {
      caseStudyCount: published.length,
      cycleTimeHours: published.reduce(
        (total, caseStudy) => total + caseStudy.cycleTimeHours,
        0,
      ),
    },
    attributionContract: {
      captureParam: 'caseStudyRef',
      targetStage: 'visit_to_intake',
      sourceKinds: ['content', 'outbound', 'ai_search', 'referral', 'direct', 'unknown'],
      requiredIntakeField: 'source_ref',
    },
    caseStudies: published,
    authorityBoundary:
      'This projection publishes public-safe business case studies with opaque engagement refs, receipt refs, cycle-time metrics, and intake-attribution hooks only. It grants no customer identity, payout, settlement, self-serve, or promise-green authority.',
  }
}

export const firstBusinessCaseStudy = buildBusinessCaseStudy({
  caseStudyRef: 'case_study.business.legal.quick_win.001',
  engagementRef: 'engagement.business.quick_win.legal.001',
  status: 'published',
  verticalDescriptor: 'legal',
  title: 'Legal quick-win delivery with public-safe receipts',
  summary:
    'A bounded legal-vertical quick win is represented with opaque refs, accepted-outcome evidence, cycle time, and acquisition attribution.',
  startedAt: '2026-07-02T00:00:00.000Z',
  completedAt: '2026-07-03T00:00:00.000Z',
  publishedAt: '2026-07-03T12:00:00.000Z',
  acceptedOutcomeRef: 'accepted_outcome.business.quick_win.legal.001',
  publicProofBundleRef: 'proof_bundle.business.quick_win.legal.001',
  receiptRefs: ['receipt.business.quick_win.legal.001'],
  metricDefinitionsRef: 'docs/fable/ROADMAP_BIZ.md#BF-7.2',
  metrics: [
    {
      metricRef: 'metric.business.cycle_time_hours.001',
      label: 'Cycle time',
      value: 24,
      unit: 'hours',
      evidenceRef: 'receipt.business.quick_win.legal.001',
    },
  ],
  attribution: {
    sourceKind: 'content',
    sourceRef: 'source.public.business.case_study.legal.001',
    captureParam: 'caseStudyRef',
    intakeAttributionRef: 'attribution.business.case_study.legal.001',
  },
  privacyReview: {
    reviewed: true,
    reviewedAt: '2026-07-03T11:00:00.000Z',
    reviewerRef: 'privacy.review.operator.business_case_studies',
    decisionRef: 'privacy.decision.business.case_study.legal.001',
  },
  sourceRefs: [
    'docs/fable/ROADMAP_AFTER.md#A0.9',
    'docs/fable/ROADMAP_BIZ.md#BF-7.4',
  ],
  caveatRefs: [
    'caveat.business.case_study.operator_reported_public_safe_seed',
    'caveat.business.case_study.no_client_identifying_information',
    'caveat.business.case_study.not_payout_or_settlement_authority',
  ],
})
