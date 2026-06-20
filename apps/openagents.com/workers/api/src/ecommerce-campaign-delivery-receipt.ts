import { Schema as S } from 'effect'

import { parseJsonWithSchema } from './json-boundary'
import { ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF } from './prefilled-workspace-vertical-templates'

/**
 * First-paid delivery receipt for an e-commerce inventory-aware ad-campaign
 * work item produced by the forge.template.ecommerce.inventory_campaign.v1
 * prefilled workspace.
 *
 * Promise: business.ecommerce_workspace_pack.v1 (yellow).
 * Advances: blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing.
 *
 * This module supplies the dereferenceable receipt FORMAT and the verification
 * machinery a first paid e-commerce delivery would need. It does NOT assert
 * that any real paid delivery has happened; honesty rules below force the
 * receipt to label which lifecycle states are evidenced and which are not.
 *
 * HARD AUTHORITY INVARIANT (mirrors the seeded "Authority blocker" memory):
 * the pack grants no publish or spend authority. A receipt may NEVER claim a
 * published artifact or observed spend while any authority gate is still
 * blocked, and it may NEVER claim auto-publish or auto-spend. Construction
 * throws if these are violated, so the receipt cannot launder an
 * un-authorized publish/spend into "delivered".
 */

export const ECOMMERCE_CAMPAIGN_DELIVERY_RECEIPT_KIND =
  'ecommerce_inventory_campaign_delivery' as const

// The starter accepted-outcome kinds the e-commerce workspace can deliver.
export const EcommerceCampaignOutcomeKind = S.Literals([
  'inventory_aware_ad_campaign',
  'commerce_qa',
  'campaign_receipt_stats',
])
export type EcommerceCampaignOutcomeKind =
  typeof EcommerceCampaignOutcomeKind.Type

// Authority gates that must each be receipted before anything publishes or
// spends. These are the explicit blockers seeded into the workspace memory.
export const EcommerceCampaignAuthorityGateId = S.Literals([
  'merchant_approval',
  'channel_access',
  'ad_account_access',
  'spend_cap_accepted',
  'publish_authority',
])
export type EcommerceCampaignAuthorityGateId =
  typeof EcommerceCampaignAuthorityGateId.Type

export const EcommerceCampaignAuthorityGateState = S.Literals([
  // The holder has explicitly granted this authority with a receipt.
  'receipted',
  // Not granted yet; this gate blocks any publish/spend.
  'blocked',
])
export type EcommerceCampaignAuthorityGateState =
  typeof EcommerceCampaignAuthorityGateState.Type

export const EcommerceCampaignAuthorityGate = S.Struct({
  gateId: EcommerceCampaignAuthorityGateId,
  state: EcommerceCampaignAuthorityGateState,
})
export type EcommerceCampaignAuthorityGate =
  typeof EcommerceCampaignAuthorityGate.Type

// The settlement asset of the paid work item.
export const EcommerceCampaignPaymentAsset = S.Literals([
  'usd',
  'sats',
  'credits',
])
export type EcommerceCampaignPaymentAsset =
  typeof EcommerceCampaignPaymentAsset.Type

// Honesty label for each lifecycle state on the receipt.
export const EcommerceCampaignEvidenceState = S.Literals([
  // A draft artifact exists, reviewed by no one yet; nothing is live.
  'drafted',
  // A human reviewer accepted this state through the review gate.
  'review_accepted',
  // The buyer's payment for the work item is recorded.
  'paid',
  // No evidence exists for this state yet.
  'not_yet_evidenced',
])
export type EcommerceCampaignEvidenceState =
  typeof EcommerceCampaignEvidenceState.Type

// Overall delivery stage. Only `delivered` may carry published artifacts and
// observed spend, and only when every gate is receipted, the human-review gate
// is accepted, and the buyer payment is recorded.
export const EcommerceCampaignDeliveryStage = S.Literals([
  'blocked',
  'drafted_for_review',
  'delivered',
])
export type EcommerceCampaignDeliveryStage =
  typeof EcommerceCampaignDeliveryStage.Type

export const EcommerceCampaignPaidSettlement = S.Struct({
  amountCents: S.Number,
  asset: EcommerceCampaignPaymentAsset,
  // True only when an actual buyer payment is recorded for this work item.
  evidenced: S.Boolean,
  // Public, dereferenceable payment/settlement reference (no secrets).
  publicPaymentRef: S.NullOr(S.String),
})
export type EcommerceCampaignPaidSettlement =
  typeof EcommerceCampaignPaidSettlement.Type

export const EcommerceCampaignDeliveryReceipt = S.Struct({
  receiptKind: S.Literal(ECOMMERCE_CAMPAIGN_DELIVERY_RECEIPT_KIND),
  templateRef: S.Literal(ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF),
  workItemRef: S.String,
  outcomeKind: EcommerceCampaignOutcomeKind,
  deliveryStage: EcommerceCampaignDeliveryStage,
  evidenceState: EcommerceCampaignEvidenceState,
  // Authority invariants: the pack never auto-publishes or auto-spends.
  noAutoPublish: S.Literal(true),
  noAutoSpend: S.Literal(true),
  humanReviewAccepted: S.Boolean,
  authorityGates: S.Array(EcommerceCampaignAuthorityGate),
  // Gate ids still blocking publish/spend. Empty only when all are receipted.
  outstandingAuthorityBlockers: S.Array(EcommerceCampaignAuthorityGateId),
  // Measurement contract (separated, per the seeded memory).
  publishedArtifactRefs: S.Array(S.String),
  spendCapCents: S.Number,
  spendObservedCents: S.NullOr(S.Number),
  statsWindow: S.NullOr(S.String),
  attributionCaveat: S.String,
  stockoutFollowUp: S.String,
  paidSettlement: EcommerceCampaignPaidSettlement,
  freshnessTimestamp: S.String,
  publicSourceRefs: S.Array(S.String),
})
export type EcommerceCampaignDeliveryReceipt =
  typeof EcommerceCampaignDeliveryReceipt.Type

export class EcommerceCampaignDeliveryReceiptInvariantError extends S.TaggedErrorClass<EcommerceCampaignDeliveryReceiptInvariantError>()(
  'EcommerceCampaignDeliveryReceiptInvariantError',
  { reason: S.String },
) {}

const ALL_GATE_IDS: ReadonlyArray<EcommerceCampaignAuthorityGateId> = [
  'merchant_approval',
  'channel_access',
  'ad_account_access',
  'spend_cap_accepted',
  'publish_authority',
]

export type EcommerceCampaignDeliveryInput = Readonly<{
  workItemRef: string
  outcomeKind: EcommerceCampaignOutcomeKind
  humanReviewAccepted: boolean
  // Map of each authority gate to whether it is receipted (true) or blocked.
  receiptedGates: Readonly<Record<EcommerceCampaignAuthorityGateId, boolean>>
  spendCapCents: number
  // Real observed spend on the campaign, if any. Null until a campaign runs.
  spendObservedCents: number | null
  publishedArtifactRefs: ReadonlyArray<string>
  statsWindow: string | null
  attributionCaveat: string
  stockoutFollowUp: string
  paidSettlement: EcommerceCampaignPaidSettlement
  freshnessTimestamp: string
  publicSourceRefs: ReadonlyArray<string>
}>

/**
 * Build a deterministic, dereferenceable delivery receipt for one e-commerce
 * inventory-aware campaign work item.
 *
 * Honesty rules enforced by construction (throws on violation):
 * - noAutoPublish / noAutoSpend are always true; this pack never self-publishes
 *   or self-spends.
 * - If ANY authority gate is blocked, the receipt may not carry published
 *   artifact refs and may not carry observed spend (no un-authorized
 *   publish/spend can be laundered into a receipt).
 * - Observed spend may never exceed the accepted spend cap.
 * - deliveryStage = 'delivered' requires every gate receipted, the human-review
 *   gate accepted, and a recorded (evidenced) buyer payment. Otherwise the
 *   stage is 'drafted_for_review' (review/payment pending with no blockers) or
 *   'blocked' (one or more authority gates still blocked).
 */
export const buildEcommerceCampaignDeliveryReceipt = (
  input: EcommerceCampaignDeliveryInput,
): EcommerceCampaignDeliveryReceipt => {
  const authorityGates: ReadonlyArray<EcommerceCampaignAuthorityGate> =
    ALL_GATE_IDS.map(gateId => ({
      gateId,
      state: input.receiptedGates[gateId] ? 'receipted' : 'blocked',
    }))

  const outstandingAuthorityBlockers = authorityGates
    .filter(gate => gate.state === 'blocked')
    .map(gate => gate.gateId)

  const hasBlockedGate = outstandingAuthorityBlockers.length > 0
  const spendObserved = input.spendObservedCents

  if (hasBlockedGate && input.publishedArtifactRefs.length > 0) {
    throw new EcommerceCampaignDeliveryReceiptInvariantError({
      reason:
        'published artifact refs present while authority gates are blocked: ' +
        outstandingAuthorityBlockers.join(', '),
    })
  }

  if (hasBlockedGate && spendObserved != null && spendObserved > 0) {
    throw new EcommerceCampaignDeliveryReceiptInvariantError({
      reason:
        'observed spend present while authority gates are blocked: ' +
        outstandingAuthorityBlockers.join(', '),
    })
  }

  if (spendObserved != null && spendObserved > input.spendCapCents) {
    throw new EcommerceCampaignDeliveryReceiptInvariantError({
      reason: `observed spend ${spendObserved} exceeds accepted spend cap ${input.spendCapCents}`,
    })
  }

  if (input.paidSettlement.amountCents < 0) {
    throw new EcommerceCampaignDeliveryReceiptInvariantError({
      reason: 'paid settlement amount must not be negative',
    })
  }

  const allGatesReceipted = !hasBlockedGate
  const paymentRecorded = input.paidSettlement.evidenced

  const deliveryStage: EcommerceCampaignDeliveryStage = hasBlockedGate
    ? 'blocked'
    : allGatesReceipted && input.humanReviewAccepted && paymentRecorded
      ? 'delivered'
      : 'drafted_for_review'

  const evidenceState: EcommerceCampaignEvidenceState =
    deliveryStage === 'delivered'
      ? 'paid'
      : input.humanReviewAccepted
        ? 'review_accepted'
        : deliveryStage === 'blocked'
          ? 'not_yet_evidenced'
          : 'drafted'

  return {
    receiptKind: ECOMMERCE_CAMPAIGN_DELIVERY_RECEIPT_KIND,
    templateRef: ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF,
    workItemRef: input.workItemRef,
    outcomeKind: input.outcomeKind,
    deliveryStage,
    evidenceState,
    noAutoPublish: true,
    noAutoSpend: true,
    humanReviewAccepted: input.humanReviewAccepted,
    authorityGates,
    outstandingAuthorityBlockers,
    publishedArtifactRefs: [...input.publishedArtifactRefs],
    spendCapCents: input.spendCapCents,
    spendObservedCents: spendObserved,
    statsWindow: input.statsWindow,
    attributionCaveat: input.attributionCaveat,
    stockoutFollowUp: input.stockoutFollowUp,
    paidSettlement: input.paidSettlement,
    freshnessTimestamp: input.freshnessTimestamp,
    publicSourceRefs: [...input.publicSourceRefs],
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
export const verifyEcommerceCampaignPaidDelivery = (
  receipt: EcommerceCampaignDeliveryReceipt,
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
  if (receipt.paidSettlement.evidenced && receipt.paidSettlement.amountCents <= 0) {
    reasons.push('paid settlement recorded with non-positive amount')
  }
  if (
    receipt.paidSettlement.evidenced &&
    receipt.paidSettlement.publicPaymentRef == null
  ) {
    reasons.push('paid settlement recorded without a dereferenceable payment ref')
  }
  if (receipt.publishedArtifactRefs.length === 0) {
    reasons.push('no published artifact refs')
  }

  return reasons
}

/**
 * Dereferenceable public DOCUMENT contract.
 *
 * The promise's green path requires "a dereferenceable first paid e-commerce
 * work-item delivery receipt". A receipt held only in memory is not
 * dereferenceable: a consumer at a public route must be able to FETCH the
 * receipt as a stable, versioned JSON document and INDEPENDENTLY re-validate
 * and re-verify it without trusting the producer.
 *
 * This section supplies that contract: a versioned envelope, a deterministic
 * serializer, a decode that validates the wire shape, and a single
 * dereference-and-verify entrypoint a consumer (or a future public route)
 * calls on the raw fetched body. It does NOT assert any real paid delivery has
 * happened — it only makes the existing receipt format actually transportable
 * and re-verifiable across a trust boundary.
 */
export const ECOMMERCE_CAMPAIGN_DELIVERY_RECEIPT_DOC_VERSION =
  'ecommerce.campaign.delivery_receipt.v1' as const

export const EcommerceCampaignDeliveryReceiptDocument = S.Struct({
  // Schema version pin so consumers can refuse unknown wire shapes.
  docVersion: S.Literal(ECOMMERCE_CAMPAIGN_DELIVERY_RECEIPT_DOC_VERSION),
  receipt: EcommerceCampaignDeliveryReceipt,
})
export type EcommerceCampaignDeliveryReceiptDocument =
  typeof EcommerceCampaignDeliveryReceiptDocument.Type

const encodeReceiptDocument = S.encodeSync(
  EcommerceCampaignDeliveryReceiptDocument,
)

/**
 * Wrap a built receipt into its versioned dereferenceable document envelope.
 */
export const toEcommerceCampaignDeliveryReceiptDocument = (
  receipt: EcommerceCampaignDeliveryReceipt,
): EcommerceCampaignDeliveryReceiptDocument => ({
  docVersion: ECOMMERCE_CAMPAIGN_DELIVERY_RECEIPT_DOC_VERSION,
  receipt,
})

/**
 * Deterministically serialize a receipt document to the JSON body that a public
 * route would serve. Deterministic so the same receipt always produces the same
 * bytes (a stable artifact a consumer can hash/cache).
 */
export const serializeEcommerceCampaignDeliveryReceiptDocument = (
  document: EcommerceCampaignDeliveryReceiptDocument,
): string => JSON.stringify(encodeReceiptDocument(document))

/**
 * Decode and VALIDATE a dereferenced JSON body back into a receipt document.
 * Throws if the body is not valid JSON or does not match the versioned wire
 * shape — a consumer must never trust an unvalidated body.
 */
export const decodeEcommerceCampaignDeliveryReceiptDocument = (
  body: string,
): EcommerceCampaignDeliveryReceiptDocument =>
  parseJsonWithSchema(EcommerceCampaignDeliveryReceiptDocument, body)

/**
 * The single entrypoint a consumer (or future public route handler) calls on a
 * raw dereferenced body. Returns the reasons the dereferenced receipt does NOT
 * qualify as an evidenced first paid delivery; an empty list means it does.
 *
 * Decode/validation failures are surfaced as a reason (never thrown) so a
 * malformed or wrong-version body can never silently pass as "delivered".
 */
export const verifyDereferencedEcommerceCampaignReceipt = (
  body: string,
): ReadonlyArray<string> => {
  let document: EcommerceCampaignDeliveryReceiptDocument
  try {
    document = decodeEcommerceCampaignDeliveryReceiptDocument(body)
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error)
    return [`receipt document failed to decode: ${detail}`]
  }

  return verifyEcommerceCampaignPaidDelivery(document.receipt)
}
