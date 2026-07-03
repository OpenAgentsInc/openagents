import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const BUSINESS_ALREADY_SOLD_ENGAGEMENT_RECEIPT_SCHEMA =
  'openagents.business.already_sold_engagement.payment_receipt.v1' as const

export const BusinessAlreadySoldEngagementKind = S.Literals([
  'quick_win',
  'fleet_sprint',
  'retainer_kickoff',
  'qa_swarm',
])
export type BusinessAlreadySoldEngagementKind =
  typeof BusinessAlreadySoldEngagementKind.Type

export const BusinessAlreadySoldVerticalDescriptor = S.Literals([
  'legal',
  'marketing_agency',
  'ecommerce',
  'health',
  'software',
  'other_business',
])
export type BusinessAlreadySoldVerticalDescriptor =
  typeof BusinessAlreadySoldVerticalDescriptor.Type

export const BusinessAlreadySoldPaymentCurrency = S.Literals(['usd', 'sat'])
export type BusinessAlreadySoldPaymentCurrency =
  typeof BusinessAlreadySoldPaymentCurrency.Type

export const BusinessAlreadySoldDemandProvenance = S.Literals([
  'external_founder_sold',
  'external_operator_sold',
])
export type BusinessAlreadySoldDemandProvenance =
  typeof BusinessAlreadySoldDemandProvenance.Type

export const BusinessAlreadySoldPrivacyReview = S.Struct({
  reviewed: S.Boolean,
  reviewedAt: S.String,
  reviewerRef: S.String,
  decisionRef: S.String,
})
export type BusinessAlreadySoldPrivacyReview =
  typeof BusinessAlreadySoldPrivacyReview.Type

export const BusinessAlreadySoldEngagementPaymentReceipt = S.Struct({
  schema: S.Literal(BUSINESS_ALREADY_SOLD_ENGAGEMENT_RECEIPT_SCHEMA),
  receiptKind: S.Literal('business.already_sold_engagement.payment'),
  receiptRef: S.String,
  engagementRef: S.String,
  buyerRef: S.String,
  buyerPaidRef: S.String,
  engagementKind: BusinessAlreadySoldEngagementKind,
  verticalDescriptor: BusinessAlreadySoldVerticalDescriptor,
  amountMinorUnits: S.Number,
  currency: BusinessAlreadySoldPaymentCurrency,
  paidAt: S.String,
  recordedAt: S.String,
  demandProvenance: BusinessAlreadySoldDemandProvenance,
  privacyReview: BusinessAlreadySoldPrivacyReview,
  sourceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type BusinessAlreadySoldEngagementPaymentReceipt =
  typeof BusinessAlreadySoldEngagementPaymentReceipt.Type

export class BusinessAlreadySoldEngagementReceiptInvariantError extends S.TaggedErrorClass<BusinessAlreadySoldEngagementReceiptInvariantError>()(
  'BusinessAlreadySoldEngagementReceiptInvariantError',
  { reason: S.String },
) {
  override get message() {
    return this.reason
  }
}

export type BusinessAlreadySoldEngagementPaymentReceiptInput = Readonly<{
  engagementRef: string
  buyerRef: string
  buyerPaidRef: string
  engagementKind: BusinessAlreadySoldEngagementKind
  verticalDescriptor: BusinessAlreadySoldVerticalDescriptor
  amountMinorUnits: number
  currency: BusinessAlreadySoldPaymentCurrency
  paidAt: string
  demandProvenance: BusinessAlreadySoldDemandProvenance
  privacyReview: BusinessAlreadySoldPrivacyReview
  sourceRefs: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string> | undefined
  recordedAt?: string | undefined
}>

const DEFAULT_CAVEAT_REFS = [
  'caveat.business.already_sold.operator_reported',
  'caveat.business.already_sold.opaque_buyer_ref_only',
  'caveat.business.already_sold.not_delivery_completion',
] as const

const PUBLIC_SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{2,180}$/

const CLIENT_IDENTIFYING_PATTERN =
  /(@|customer|client|law firm|attorney|doctor|physician|agency owner|founder name|contact|email|phone|invoice|stripe|secret|token|\s)/i

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
    throw new BusinessAlreadySoldEngagementReceiptInvariantError({
      reason: `${field} is required.`,
    })
  }
  if (!PUBLIC_SAFE_REF_PATTERN.test(trimmed)) {
    throw new BusinessAlreadySoldEngagementReceiptInvariantError({
      reason: `${field} must be an opaque public-safe ref.`,
    })
  }
  if (CLIENT_IDENTIFYING_PATTERN.test(trimmed)) {
    throw new BusinessAlreadySoldEngagementReceiptInvariantError({
      reason: `${field} must not contain client-identifying or private payment material.`,
    })
  }
  return trimmed
}

const requireIsoLike = (field: string, value: string): string => {
  const trimmed = trimOrNull(value)
  if (trimmed === null || Number.isNaN(Date.parse(trimmed))) {
    throw new BusinessAlreadySoldEngagementReceiptInvariantError({
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
    throw new BusinessAlreadySoldEngagementReceiptInvariantError({
      reason: `${field} must contain at least one public-safe source ref.`,
    })
  }
  return refs.map(ref => requirePublicSafeRef(field, ref))
}

const receiptRefFor = (
  engagementKind: BusinessAlreadySoldEngagementKind,
  engagementRef: string,
  buyerPaidRef: string,
): string =>
  `receipt.business.${engagementKind}.${engagementRef.replaceAll(':', '.')}.${buyerPaidRef.replaceAll(':', '.')}`

export const buildBusinessAlreadySoldEngagementPaymentReceipt = (
  input: BusinessAlreadySoldEngagementPaymentReceiptInput,
): BusinessAlreadySoldEngagementPaymentReceipt => {
  const engagementRef = requirePublicSafeRef(
    'engagementRef',
    input.engagementRef,
  )
  const buyerRef = requirePublicSafeRef('buyerRef', input.buyerRef)
  const buyerPaidRef = requirePublicSafeRef('buyerPaidRef', input.buyerPaidRef)
  const reviewerRef = requirePublicSafeRef(
    'privacyReview.reviewerRef',
    input.privacyReview.reviewerRef,
  )
  const decisionRef = requirePublicSafeRef(
    'privacyReview.decisionRef',
    input.privacyReview.decisionRef,
  )

  if (!input.privacyReview.reviewed) {
    throw new BusinessAlreadySoldEngagementReceiptInvariantError({
      reason: 'privacyReview.reviewed must be true before the receipt can publish.',
    })
  }
  if (!Number.isInteger(input.amountMinorUnits) || input.amountMinorUnits <= 0) {
    throw new BusinessAlreadySoldEngagementReceiptInvariantError({
      reason: 'amountMinorUnits must be a positive integer.',
    })
  }

  const paidAt = requireIsoLike('paidAt', input.paidAt)
  const recordedAt = requireIsoLike(
    'recordedAt',
    input.recordedAt ?? currentIsoTimestamp(),
  )
  const reviewedAt = requireIsoLike(
    'privacyReview.reviewedAt',
    input.privacyReview.reviewedAt,
  )

  return {
    schema: BUSINESS_ALREADY_SOLD_ENGAGEMENT_RECEIPT_SCHEMA,
    receiptKind: 'business.already_sold_engagement.payment',
    receiptRef: receiptRefFor(input.engagementKind, engagementRef, buyerPaidRef),
    engagementRef,
    buyerRef,
    buyerPaidRef,
    engagementKind: input.engagementKind,
    verticalDescriptor: input.verticalDescriptor,
    amountMinorUnits: input.amountMinorUnits,
    currency: input.currency,
    paidAt,
    recordedAt,
    demandProvenance: input.demandProvenance,
    privacyReview: {
      reviewed: true,
      reviewedAt,
      reviewerRef,
      decisionRef,
    },
    sourceRefs: [...requireRefs('sourceRefs', input.sourceRefs)],
    caveatRefs: [...requireRefs('caveatRefs', input.caveatRefs ?? DEFAULT_CAVEAT_REFS)],
  }
}

export const assertPaidBusinessReceipt = (
  receipt: BusinessAlreadySoldEngagementPaymentReceipt,
): void => {
  if (receipt.receiptRef !== receiptRefFor(receipt.engagementKind, receipt.engagementRef, receipt.buyerPaidRef)) {
    throw new BusinessAlreadySoldEngagementReceiptInvariantError({
      reason: 'receiptRef does not match the engagement/payment refs.',
    })
  }
  buildBusinessAlreadySoldEngagementPaymentReceipt(receipt)
}

export const BusinessAlreadySoldReceiptStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'business_already_sold_engagement_payment_receipts',
    'privacy_review_decisions',
  ])

export type BusinessAlreadySoldEngagementReceiptStore = {
  list: () => ReadonlyArray<BusinessAlreadySoldEngagementPaymentReceipt>
}

export const makeInMemoryBusinessAlreadySoldEngagementReceiptStore = (
  receipts: ReadonlyArray<BusinessAlreadySoldEngagementPaymentReceipt>,
): BusinessAlreadySoldEngagementReceiptStore => ({
  list: () => receipts,
})

export const publicBusinessAlreadySoldEngagementReceiptProjection = (
  receipt: BusinessAlreadySoldEngagementPaymentReceipt,
) => ({
  schema: receipt.schema,
  receiptKind: receipt.receiptKind,
  receiptRef: receipt.receiptRef,
  engagementRef: receipt.engagementRef,
  buyerRef: receipt.buyerRef,
  engagementKind: receipt.engagementKind,
  verticalDescriptor: receipt.verticalDescriptor,
  amountMinorUnits: receipt.amountMinorUnits,
  currency: receipt.currency,
  paidAt: receipt.paidAt,
  recordedAt: receipt.recordedAt,
  demandProvenance: receipt.demandProvenance,
  privacyReviewed: receipt.privacyReview.reviewed,
  privacyDecisionRef: receipt.privacyReview.decisionRef,
  sourceRefs: receipt.sourceRefs,
  caveatRefs: receipt.caveatRefs,
})

export const projectBusinessAlreadySoldEngagementReceipts = (
  receipts: ReadonlyArray<BusinessAlreadySoldEngagementPaymentReceipt>,
  options?: { generatedAt?: string },
) => {
  const generatedAt = options?.generatedAt ?? currentIsoTimestamp()
  const paidReceipts = receipts.map(publicBusinessAlreadySoldEngagementReceiptProjection)

  return {
    schema: 'openagents.business.already_sold_engagement.payment_receipts.v1',
    generatedAt,
    staleness: BusinessAlreadySoldReceiptStaleness,
    maxStalenessSeconds: BusinessAlreadySoldReceiptStaleness.maxStalenessSeconds,
    promiseIds: ['business.intake_quick_win_offering.v1'],
    roadmapRefs: ['ROADMAP_AFTER.A0.1', 'ROADMAP_BIZ.BF-2.5'],
    totals: {
      receiptCount: paidReceipts.length,
      paidBusinessReceiptCount: paidReceipts.length,
      amountMinorUnitsByCurrency: paidReceipts.reduce<Record<string, number>>(
        (totals, receipt) => ({
          ...totals,
          [receipt.currency]:
            (totals[receipt.currency] ?? 0) + receipt.amountMinorUnits,
        }),
        {},
      ),
    },
    paidBusinessReceiptRecorded: paidReceipts.length > 0,
    receipts: paidReceipts,
    authorityBoundary:
      'This public-safe projection records opaque already-sold business payment receipts only. It grants no delivery completion, payout, settlement, self-serve, promise-green, or customer identity authority.',
  }
}

export const firstAlreadySoldBusinessQuickWinReceipt =
  buildBusinessAlreadySoldEngagementPaymentReceipt({
    engagementRef: 'engagement.business.quick_win.legal.001',
    buyerRef: 'buyer.business.opaque.legal.001',
    buyerPaidRef: 'payment.business.opaque.legal.001',
    engagementKind: 'quick_win',
    verticalDescriptor: 'legal',
    amountMinorUnits: 100000,
    currency: 'usd',
    paidAt: '2026-07-02T00:00:00.000Z',
    recordedAt: '2026-07-03T00:00:00.000Z',
    demandProvenance: 'external_founder_sold',
    privacyReview: {
      reviewed: true,
      reviewedAt: '2026-07-03T00:00:00.000Z',
      reviewerRef: 'privacy.review.operator.business_receipts',
      decisionRef: 'privacy.decision.business.opaque_receipts.001',
    },
    sourceRefs: [
      'docs/fable/ROADMAP_AFTER.md#A0.1',
      'docs/fable/ROADMAP_BIZ.md#BF-2.5',
    ],
  })
