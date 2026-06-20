import { Schema as S } from 'effect'

import { MARKETING_AGENCY_DESIGN_PARTNER_TEMPLATE_REF } from './prefilled-workspace-vertical-templates'

/**
 * First-paid delivery receipt for a marketing-agency white-label work item
 * (landing page, welcome email, or operator-on-Autopilot admin lane) produced
 * by the forge.template.marketing_agency.white_label_launch.v1 prefilled
 * workspace.
 *
 * Promise: business.marketing_agency_workspace_pack.v1 (yellow).
 * Advances: blocker.product_promises.marketing_agency_pack_first_paid_delivery_receipt_missing.
 *
 * This module supplies the dereferenceable receipt FORMAT and the verification
 * machinery a first paid agency delivery would need. It does NOT assert that
 * any real paid delivery has happened; honesty rules below force the receipt to
 * label which lifecycle states are evidenced and which are not.
 *
 * HARD AUTHORITY INVARIANT (mirrors the seeded "Authority blocker" memory):
 * the pack grants no publish or send authority. A receipt may NEVER claim a
 * published landing page or a sent/scheduled email while any authority gate is
 * still blocked, and it may NEVER claim auto-publish or auto-send. Construction
 * throws if these are violated, so the receipt cannot launder an un-authorized
 * publish/send into "delivered".
 */

export const MARKETING_AGENCY_DELIVERY_RECEIPT_KIND =
  'marketing_agency_white_label_delivery' as const

// The starter accepted-outcome kinds the marketing-agency workspace can deliver
// (mirrors the seeded starter workflows / outcomeKinds).
export const MarketingAgencyOutcomeKind = S.Literals([
  'agency_white_label_landing_page',
  'agency_welcome_email',
  'agency_operator_autopilot_lane',
])
export type MarketingAgencyOutcomeKind = typeof MarketingAgencyOutcomeKind.Type

// Authority gates that must each be receipted before anything publishes or
// sends. These are the explicit blockers seeded into the workspace memory:
// client approval, domain authority (DNS/subdomain), channel access (email),
// publish permission, and send permission.
export const MarketingAgencyAuthorityGateId = S.Literals([
  'client_approval',
  'domain_authority',
  'channel_access',
  'publish_authority',
  'send_authority',
])
export type MarketingAgencyAuthorityGateId =
  typeof MarketingAgencyAuthorityGateId.Type

export const MarketingAgencyAuthorityGateState = S.Literals([
  // The holder has explicitly granted this authority with a receipt.
  'receipted',
  // Not granted yet; this gate blocks any publish/send.
  'blocked',
])
export type MarketingAgencyAuthorityGateState =
  typeof MarketingAgencyAuthorityGateState.Type

export const MarketingAgencyAuthorityGate = S.Struct({
  gateId: MarketingAgencyAuthorityGateId,
  state: MarketingAgencyAuthorityGateState,
})
export type MarketingAgencyAuthorityGate =
  typeof MarketingAgencyAuthorityGate.Type

// The settlement asset of the paid work item.
export const MarketingAgencyPaymentAsset = S.Literals([
  'usd',
  'sats',
  'credits',
])
export type MarketingAgencyPaymentAsset =
  typeof MarketingAgencyPaymentAsset.Type

// Honesty label for each lifecycle state on the receipt.
export const MarketingAgencyEvidenceState = S.Literals([
  // A draft artifact exists, reviewed by no one yet; nothing is live.
  'drafted',
  // A human reviewer accepted this state through the review gate.
  'review_accepted',
  // The buyer's payment for the work item is recorded.
  'paid',
  // No evidence exists for this state yet.
  'not_yet_evidenced',
])
export type MarketingAgencyEvidenceState =
  typeof MarketingAgencyEvidenceState.Type

// Overall delivery stage. Only `delivered` may carry a published landing page
// or a sent email, and only when every gate is receipted, the human-review
// gate is accepted, and the buyer payment is recorded.
export const MarketingAgencyDeliveryStage = S.Literals([
  'blocked',
  'drafted_for_review',
  'delivered',
])
export type MarketingAgencyDeliveryStage =
  typeof MarketingAgencyDeliveryStage.Type

// White-label subdomain lifecycle. 'live' requires domain authority granted.
export const MarketingAgencySubdomainState = S.Literals([
  'not_provisioned',
  'provisioned',
  'live',
])
export type MarketingAgencySubdomainState =
  typeof MarketingAgencySubdomainState.Type

// Email lifecycle. Anything past 'not_sent' requires send + channel authority.
export const MarketingAgencyEmailSendState = S.Literals([
  'not_sent',
  'scheduled',
  'sent',
])
export type MarketingAgencyEmailSendState =
  typeof MarketingAgencyEmailSendState.Type

export const MarketingAgencyPaidSettlement = S.Struct({
  amountCents: S.Number,
  asset: MarketingAgencyPaymentAsset,
  // True only when an actual buyer payment is recorded for this work item.
  evidenced: S.Boolean,
  // Public, dereferenceable payment/settlement reference (no secrets).
  publicPaymentRef: S.NullOr(S.String),
})
export type MarketingAgencyPaidSettlement =
  typeof MarketingAgencyPaidSettlement.Type

export const MarketingAgencyDeliveryReceipt = S.Struct({
  receiptKind: S.Literal(MARKETING_AGENCY_DELIVERY_RECEIPT_KIND),
  templateRef: S.Literal(MARKETING_AGENCY_DESIGN_PARTNER_TEMPLATE_REF),
  workItemRef: S.String,
  outcomeKind: MarketingAgencyOutcomeKind,
  deliveryStage: MarketingAgencyDeliveryStage,
  evidenceState: MarketingAgencyEvidenceState,
  // Authority invariants: the pack never auto-publishes or auto-sends.
  noAutoPublish: S.Literal(true),
  noAutoSend: S.Literal(true),
  humanReviewAccepted: S.Boolean,
  authorityGates: S.Array(MarketingAgencyAuthorityGate),
  // Gate ids still blocking publish/send. Empty only when all are receipted.
  outstandingAuthorityBlockers: S.Array(MarketingAgencyAuthorityGateId),
  // Measurement contract (separated, per the seeded memory).
  approvedDeliverableRefs: S.Array(S.String),
  publishedArtifactRefs: S.Array(S.String),
  whiteLabelSubdomainState: MarketingAgencySubdomainState,
  emailSendState: MarketingAgencyEmailSendState,
  operatorLaneAcceptance: S.Boolean,
  metricWindow: S.NullOr(S.String),
  attributionCaveat: S.String,
  paidSettlement: MarketingAgencyPaidSettlement,
  freshnessTimestamp: S.String,
  publicSourceRefs: S.Array(S.String),
})
export type MarketingAgencyDeliveryReceipt =
  typeof MarketingAgencyDeliveryReceipt.Type

export class MarketingAgencyDeliveryReceiptInvariantError extends S.TaggedErrorClass<MarketingAgencyDeliveryReceiptInvariantError>()(
  'MarketingAgencyDeliveryReceiptInvariantError',
  { reason: S.String },
) {}

const ALL_GATE_IDS: ReadonlyArray<MarketingAgencyAuthorityGateId> = [
  'client_approval',
  'domain_authority',
  'channel_access',
  'publish_authority',
  'send_authority',
]

export type MarketingAgencyDeliveryInput = Readonly<{
  workItemRef: string
  outcomeKind: MarketingAgencyOutcomeKind
  humanReviewAccepted: boolean
  // Map of each authority gate to whether it is receipted (true) or blocked.
  receiptedGates: Readonly<Record<MarketingAgencyAuthorityGateId, boolean>>
  // Reviewed-but-not-necessarily-live deliverable refs (draft artifacts).
  approvedDeliverableRefs: ReadonlyArray<string>
  // Landing pages actually published live. Must be empty while any gate blocks.
  publishedArtifactRefs: ReadonlyArray<string>
  whiteLabelSubdomainState: MarketingAgencySubdomainState
  emailSendState: MarketingAgencyEmailSendState
  operatorLaneAcceptance: boolean
  metricWindow: string | null
  attributionCaveat: string
  paidSettlement: MarketingAgencyPaidSettlement
  freshnessTimestamp: string
  publicSourceRefs: ReadonlyArray<string>
}>

/**
 * Build a deterministic, dereferenceable delivery receipt for one
 * marketing-agency white-label work item.
 *
 * Honesty rules enforced by construction (throws on violation):
 * - noAutoPublish / noAutoSend are always true; this pack never self-publishes
 *   or self-sends.
 * - If ANY authority gate is blocked, the receipt may not carry published
 *   landing-page refs, may not carry a live white-label subdomain, and the
 *   email may not be scheduled or sent (no un-authorized publish/send can be
 *   laundered into a receipt).
 * - deliveryStage = 'delivered' requires every gate receipted, the human-review
 *   gate accepted, and a recorded (evidenced) buyer payment. Otherwise the
 *   stage is 'drafted_for_review' (review/payment pending with no blockers) or
 *   'blocked' (one or more authority gates still blocked).
 */
export const buildMarketingAgencyDeliveryReceipt = (
  input: MarketingAgencyDeliveryInput,
): MarketingAgencyDeliveryReceipt => {
  const authorityGates: ReadonlyArray<MarketingAgencyAuthorityGate> =
    ALL_GATE_IDS.map(gateId => ({
      gateId,
      state: input.receiptedGates[gateId] ? 'receipted' : 'blocked',
    }))

  const outstandingAuthorityBlockers = authorityGates
    .filter(gate => gate.state === 'blocked')
    .map(gate => gate.gateId)

  const hasBlockedGate = outstandingAuthorityBlockers.length > 0

  if (hasBlockedGate && input.publishedArtifactRefs.length > 0) {
    throw new MarketingAgencyDeliveryReceiptInvariantError({
      reason:
        'published landing-page refs present while authority gates are blocked: ' +
        outstandingAuthorityBlockers.join(', '),
    })
  }

  if (hasBlockedGate && input.whiteLabelSubdomainState === 'live') {
    throw new MarketingAgencyDeliveryReceiptInvariantError({
      reason:
        'white-label subdomain is live while authority gates are blocked: ' +
        outstandingAuthorityBlockers.join(', '),
    })
  }

  if (hasBlockedGate && input.emailSendState !== 'not_sent') {
    throw new MarketingAgencyDeliveryReceiptInvariantError({
      reason:
        `email is ${input.emailSendState} while authority gates are blocked: ` +
        outstandingAuthorityBlockers.join(', '),
    })
  }

  if (input.paidSettlement.amountCents < 0) {
    throw new MarketingAgencyDeliveryReceiptInvariantError({
      reason: 'paid settlement amount must not be negative',
    })
  }

  const allGatesReceipted = !hasBlockedGate
  const paymentRecorded = input.paidSettlement.evidenced

  const deliveryStage: MarketingAgencyDeliveryStage = hasBlockedGate
    ? 'blocked'
    : allGatesReceipted && input.humanReviewAccepted && paymentRecorded
      ? 'delivered'
      : 'drafted_for_review'

  const evidenceState: MarketingAgencyEvidenceState =
    deliveryStage === 'delivered'
      ? 'paid'
      : input.humanReviewAccepted
        ? 'review_accepted'
        : deliveryStage === 'blocked'
          ? 'not_yet_evidenced'
          : 'drafted'

  return {
    receiptKind: MARKETING_AGENCY_DELIVERY_RECEIPT_KIND,
    templateRef: MARKETING_AGENCY_DESIGN_PARTNER_TEMPLATE_REF,
    workItemRef: input.workItemRef,
    outcomeKind: input.outcomeKind,
    deliveryStage,
    evidenceState,
    noAutoPublish: true,
    noAutoSend: true,
    humanReviewAccepted: input.humanReviewAccepted,
    authorityGates,
    outstandingAuthorityBlockers,
    approvedDeliverableRefs: [...input.approvedDeliverableRefs],
    publishedArtifactRefs: [...input.publishedArtifactRefs],
    whiteLabelSubdomainState: input.whiteLabelSubdomainState,
    emailSendState: input.emailSendState,
    operatorLaneAcceptance: input.operatorLaneAcceptance,
    metricWindow: input.metricWindow,
    attributionCaveat: input.attributionCaveat,
    paidSettlement: input.paidSettlement,
    freshnessTimestamp: input.freshnessTimestamp,
    publicSourceRefs: [...input.publicSourceRefs],
  }
}

/**
 * The concrete delivered-outcome evidence a paid receipt must carry, by
 * outcomeKind: a landing page must be published, a welcome email must be sent,
 * and an operator-lane work item must be accepted. Returns the reason the
 * delivered outcome is not evidenced, or null when it is.
 */
const deliveredOutcomeGap = (
  receipt: MarketingAgencyDeliveryReceipt,
): string | null => {
  switch (receipt.outcomeKind) {
    case 'agency_white_label_landing_page':
      return receipt.publishedArtifactRefs.length === 0
        ? 'no published landing-page artifact refs'
        : null
    case 'agency_welcome_email':
      return receipt.emailSendState !== 'sent'
        ? `welcome email not sent (state: ${receipt.emailSendState})`
        : null
    case 'agency_operator_autopilot_lane':
      return receipt.operatorLaneAcceptance
        ? null
        : 'operator-on-Autopilot lane work item not accepted'
  }
}

/**
 * Verify a built receipt is internally consistent and a genuine first PAID
 * delivery (not merely a draft). Returns the list of reasons it does NOT yet
 * qualify as an evidenced first paid delivery; an empty list means it does.
 *
 * This is the gate the promise's green flip would consult: a first paid
 * delivery receipt is only "dereferenceable evidence" when this returns [].
 */
export const verifyMarketingAgencyPaidDelivery = (
  receipt: MarketingAgencyDeliveryReceipt,
): ReadonlyArray<string> => {
  const reasons: Array<string> = []

  if (receipt.deliveryStage !== 'delivered') {
    reasons.push(`delivery stage is ${receipt.deliveryStage}, not delivered`)
  }
  if (receipt.outstandingAuthorityBlockers.length > 0) {
    reasons.push(
      'outstanding authority blockers: ' +
        receipt.outstandingAuthorityBlockers.join(', '),
    )
  }
  if (!receipt.humanReviewAccepted) {
    reasons.push('human-review gate not accepted')
  }
  if (!receipt.paidSettlement.evidenced) {
    reasons.push('buyer payment not recorded')
  }
  if (
    receipt.paidSettlement.evidenced &&
    receipt.paidSettlement.amountCents <= 0
  ) {
    reasons.push('paid settlement recorded with non-positive amount')
  }
  if (
    receipt.paidSettlement.evidenced &&
    receipt.paidSettlement.publicPaymentRef == null
  ) {
    reasons.push('paid settlement recorded without a dereferenceable payment ref')
  }

  const outcomeGap = deliveredOutcomeGap(receipt)
  if (outcomeGap != null) {
    reasons.push(outcomeGap)
  }

  return reasons
}
