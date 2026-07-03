import { Schema as S } from 'effect'

/**
 * BF-8.1: marketing program as a sellable add-on package.
 *
 * This is the package receipt contract for campaign programs composed from the
 * existing BF-4.5/BF-4.6/BF-4.7 machinery:
 * - native sites authoring / publish receipts,
 * - native email sequences + list/form receipts,
 * - inventory-aware campaign workflow receipts.
 *
 * It does not claim a real engagement has been sold or delivered. A package
 * only verifies clean when the buyer payment, review gate, delivery evidence,
 * and per-channel receipts are all present. Public copy/promise state must
 * remain yellow until a real receipt is assessed through the claim-upgrade gate.
 */

export const BUSINESS_MARKETING_PROGRAM_PACKAGE_SCHEMA =
  'openagents.business.marketing_program_package.v1' as const

export const BUSINESS_MARKETING_PROGRAM_PROMISE_ID =
  'business.marketing_agency_workspace_pack.v1' as const

export const BUSINESS_MARKETING_PROGRAM_GATE_REF =
  'gate.business_marketing_program.sold_and_delivered' as const

export const BusinessMarketingProgramPackageTier = S.Literals([
  'content',
  'geo_content',
  'outbound_assist',
  'full_funnel',
])
export type BusinessMarketingProgramPackageTier =
  typeof BusinessMarketingProgramPackageTier.Type

export const BusinessMarketingProgramComponentKind = S.Literals([
  'site_or_landing_page',
  'email_sequence_or_list',
  'inventory_aware_campaign',
  'geo_content_brief',
  'outbound_assist_plan',
])
export type BusinessMarketingProgramComponentKind =
  typeof BusinessMarketingProgramComponentKind.Type

export const BusinessMarketingProgramComponentState = S.Literals([
  'planned',
  'drafted',
  'review_accepted',
  'delivered_with_receipt',
])
export type BusinessMarketingProgramComponentState =
  typeof BusinessMarketingProgramComponentState.Type

export const BusinessMarketingProgramDeliveryMode = S.Literals([
  'operator_assisted',
  'self_serve',
])
export type BusinessMarketingProgramDeliveryMode =
  typeof BusinessMarketingProgramDeliveryMode.Type

export const BusinessMarketingProgramPaymentAsset = S.Literals([
  'usd',
  'sats',
  'credits',
])
export type BusinessMarketingProgramPaymentAsset =
  typeof BusinessMarketingProgramPaymentAsset.Type

export const BusinessMarketingProgramComponent = S.Struct({
  kind: BusinessMarketingProgramComponentKind,
  state: BusinessMarketingProgramComponentState,
  // Public-safe receipt refs only. No client identity, private prompt, or raw
  // channel payload belongs in this package summary.
  receiptRefs: S.Array(S.String),
})
export type BusinessMarketingProgramComponent =
  typeof BusinessMarketingProgramComponent.Type

export const BusinessMarketingProgramPayment = S.Struct({
  amountCents: S.Number,
  asset: BusinessMarketingProgramPaymentAsset,
  evidenced: S.Boolean,
  publicPaymentRef: S.NullOr(S.String),
})
export type BusinessMarketingProgramPayment =
  typeof BusinessMarketingProgramPayment.Type

export const BusinessMarketingProgramPackageReceipt = S.Struct({
  schema: S.Literal(BUSINESS_MARKETING_PROGRAM_PACKAGE_SCHEMA),
  packageRef: S.String,
  tier: BusinessMarketingProgramPackageTier,
  promiseId: S.Literal(BUSINESS_MARKETING_PROGRAM_PROMISE_ID),
  deliveryMode: BusinessMarketingProgramDeliveryMode,
  noAutoPublish: S.Literal(true),
  noAutoSend: S.Literal(true),
  humanReviewAccepted: S.Boolean,
  buyerPayment: BusinessMarketingProgramPayment,
  components: S.Array(BusinessMarketingProgramComponent),
  deliveredComponentCount: S.Number,
  requiredComponentKinds: S.Array(BusinessMarketingProgramComponentKind),
  missingComponentKinds: S.Array(BusinessMarketingProgramComponentKind),
  blockerRefs: S.Array(S.String),
  publicSourceRefs: S.Array(S.String),
  freshnessTimestamp: S.String,
})
export type BusinessMarketingProgramPackageReceipt =
  typeof BusinessMarketingProgramPackageReceipt.Type

export class BusinessMarketingProgramPackageInvariantError extends S.TaggedErrorClass<BusinessMarketingProgramPackageInvariantError>()(
  'BusinessMarketingProgramPackageInvariantError',
  { reason: S.String },
) {}

export type BusinessMarketingProgramPackageInput = Readonly<{
  packageRef: string
  tier: BusinessMarketingProgramPackageTier
  deliveryMode?: BusinessMarketingProgramDeliveryMode | undefined
  humanReviewAccepted: boolean
  buyerPayment: BusinessMarketingProgramPayment
  components: ReadonlyArray<BusinessMarketingProgramComponent>
  publicSourceRefs: ReadonlyArray<string>
  freshnessTimestamp: string
}>

const REQUIRED_BY_TIER: Readonly<
  Record<
    BusinessMarketingProgramPackageTier,
    ReadonlyArray<BusinessMarketingProgramComponentKind>
  >
> = {
  content: ['geo_content_brief'],
  geo_content: ['site_or_landing_page', 'geo_content_brief'],
  outbound_assist: ['site_or_landing_page', 'outbound_assist_plan'],
  full_funnel: [
    'site_or_landing_page',
    'email_sequence_or_list',
    'inventory_aware_campaign',
    'geo_content_brief',
    'outbound_assist_plan',
  ],
}

const BLOCKERS = {
  deliveryReceiptsMissing:
    'blocker.business_marketing_program.delivery_receipts_missing',
  humanReviewMissing:
    'blocker.business_marketing_program.human_review_missing',
  paidReceiptMissing: 'blocker.business_marketing_program.paid_receipt_missing',
  publicSourceRefsMissing:
    'blocker.business_marketing_program.public_source_refs_missing',
  selfServeUnsupported:
    'blocker.business_marketing_program.self_serve_unsupported',
} as const

const hasNonEmptyRef = (value: string | null): value is string =>
  typeof value === 'string' && value.trim().length > 0

const uniqueKinds = (
  values: ReadonlyArray<BusinessMarketingProgramComponentKind>,
): ReadonlyArray<BusinessMarketingProgramComponentKind> => [...new Set(values)]

const deliveredKinds = (
  components: ReadonlyArray<BusinessMarketingProgramComponent>,
): ReadonlyArray<BusinessMarketingProgramComponentKind> =>
  uniqueKinds(
    components
      .filter(
        component =>
          component.state === 'delivered_with_receipt' &&
          component.receiptRefs.length > 0,
      )
      .map(component => component.kind),
  )

export const buildBusinessMarketingProgramPackageReceipt = (
  input: BusinessMarketingProgramPackageInput,
): BusinessMarketingProgramPackageReceipt => {
  if (input.packageRef.trim().length === 0) {
    throw new BusinessMarketingProgramPackageInvariantError({
      reason: 'packageRef is required',
    })
  }

  if (input.buyerPayment.amountCents < 0) {
    throw new BusinessMarketingProgramPackageInvariantError({
      reason: 'buyer payment amount must not be negative',
    })
  }

  for (const component of input.components) {
    if (
      component.state === 'delivered_with_receipt' &&
      component.receiptRefs.length === 0
    ) {
      throw new BusinessMarketingProgramPackageInvariantError({
        reason: `component ${component.kind} is delivered without receipt refs`,
      })
    }
  }

  const deliveryMode = input.deliveryMode ?? 'operator_assisted'
  const requiredComponentKinds = REQUIRED_BY_TIER[input.tier]
  const deliveredComponentKinds = deliveredKinds(input.components)
  const missingComponentKinds = requiredComponentKinds.filter(
    kind => !deliveredComponentKinds.includes(kind),
  )

  const blockerRefs: Array<string> = []
  if (deliveryMode === 'self_serve') {
    blockerRefs.push(BLOCKERS.selfServeUnsupported)
  }
  if (!input.humanReviewAccepted) {
    blockerRefs.push(BLOCKERS.humanReviewMissing)
  }
  if (
    !input.buyerPayment.evidenced ||
    input.buyerPayment.amountCents <= 0 ||
    !hasNonEmptyRef(input.buyerPayment.publicPaymentRef)
  ) {
    blockerRefs.push(BLOCKERS.paidReceiptMissing)
  }
  if (missingComponentKinds.length > 0) {
    blockerRefs.push(BLOCKERS.deliveryReceiptsMissing)
  }
  if (input.publicSourceRefs.length === 0) {
    blockerRefs.push(BLOCKERS.publicSourceRefsMissing)
  }

  return {
    schema: BUSINESS_MARKETING_PROGRAM_PACKAGE_SCHEMA,
    packageRef: input.packageRef,
    tier: input.tier,
    promiseId: BUSINESS_MARKETING_PROGRAM_PROMISE_ID,
    deliveryMode,
    noAutoPublish: true,
    noAutoSend: true,
    humanReviewAccepted: input.humanReviewAccepted,
    buyerPayment: input.buyerPayment,
    components: [...input.components],
    deliveredComponentCount: deliveredComponentKinds.length,
    requiredComponentKinds: [...requiredComponentKinds],
    missingComponentKinds,
    blockerRefs,
    publicSourceRefs: [...input.publicSourceRefs],
    freshnessTimestamp: input.freshnessTimestamp,
  }
}

export const verifyBusinessMarketingProgramSoldAndDelivered = (
  receipt: BusinessMarketingProgramPackageReceipt,
): ReadonlyArray<string> => {
  const reasons: Array<string> = []

  if (receipt.promiseId !== BUSINESS_MARKETING_PROGRAM_PROMISE_ID) {
    reasons.push(`unexpected promise id: ${receipt.promiseId}`)
  }
  if (receipt.deliveryMode !== 'operator_assisted') {
    reasons.push('self-serve marketing program delivery is not proven')
  }
  if (!receipt.noAutoPublish || !receipt.noAutoSend) {
    reasons.push('package must not auto-publish or auto-send')
  }
  if (!receipt.humanReviewAccepted) {
    reasons.push('human-review gate not accepted')
  }
  if (!receipt.buyerPayment.evidenced) {
    reasons.push('buyer payment not recorded')
  }
  if (receipt.buyerPayment.evidenced && receipt.buyerPayment.amountCents <= 0) {
    reasons.push('buyer payment recorded with non-positive amount')
  }
  if (
    receipt.buyerPayment.evidenced &&
    !hasNonEmptyRef(receipt.buyerPayment.publicPaymentRef)
  ) {
    reasons.push('buyer payment lacks a dereferenceable public ref')
  }
  if (receipt.missingComponentKinds.length > 0) {
    reasons.push(
      'required package components missing delivered receipts: ' +
        receipt.missingComponentKinds.join(', '),
    )
  }
  if (receipt.blockerRefs.length > 0) {
    reasons.push(
      'package blockers still present: ' + receipt.blockerRefs.join(', '),
    )
  }
  if (receipt.publicSourceRefs.length === 0) {
    reasons.push('public source refs missing')
  }

  return reasons
}
