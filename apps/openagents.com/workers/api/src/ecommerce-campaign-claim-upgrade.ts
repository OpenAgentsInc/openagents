import { Schema as S } from 'effect'

import {
  type EcommerceCampaignDeliveryReceiptDocument,
  verifyEcommerceCampaignPaidDelivery,
} from './ecommerce-campaign-delivery-receipt'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const ECOMMERCE_CAMPAIGN_CLAIM_UPGRADE_SCHEMA =
  'openagents.ecommerce_campaign.paid_delivery_claim.v1' as const

export const CLAIM_UPGRADE_CONTRACT = 'proof.claim_upgrade_receipts.v1' as const

export const ECOMMERCE_WORKSPACE_PACK_PROMISE =
  'business.ecommerce_workspace_pack.v1' as const

export const ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF =
  'blocker.product_promises.ecommerce_pack_first_paid_delivery_receipt_missing' as const

export const PAID_DELIVERY_GATE_RECEIPT_VERIFIES =
  'gate.paid_delivery.receipt_verifies_clean' as const
export const PAID_DELIVERY_GATE_OWNER_SIGN_OFF =
  'gate.paid_delivery.owner_sign_off_present' as const

export const EcommerceCampaignPaidDeliveryGates = S.Struct({
  receiptVerifiesClean: S.Boolean,
  ownerSignOffPresent: S.Boolean,
})
export type EcommerceCampaignPaidDeliveryGates = typeof EcommerceCampaignPaidDeliveryGates.Type

export const EcommerceCampaignPaidDeliveryClaim = S.Struct({
  schema: S.Literal(ECOMMERCE_CAMPAIGN_CLAIM_UPGRADE_SCHEMA),
  receiptRef: S.String,
  gates: EcommerceCampaignPaidDeliveryGates,
  failingGateRefs: S.Array(S.String),
  paidDeliverySubstantiated: S.Boolean,
  contractRef: S.Literal(CLAIM_UPGRADE_CONTRACT),
  promiseIds: S.Tuple([S.Literal(ECOMMERCE_WORKSPACE_PACK_PROMISE)]),
  promiseState: S.Literal('yellow'),
  unclearedBlockerRefs: S.Array(S.String),
  assessedAt: S.String,
})
export type EcommerceCampaignPaidDeliveryClaim = typeof EcommerceCampaignPaidDeliveryClaim.Type

export type EcommerceCampaignPaidDeliveryClaimInput = Readonly<{
  document: EcommerceCampaignDeliveryReceiptDocument
  receiptRef: string
  ownerSignOffRef?: string | undefined
}>

const isNonEmpty = (value: string | undefined): value is string =>
  typeof value === 'string' && value.trim().length > 0

export const assessEcommerceCampaignPaidDeliveryClaim = (
  input: EcommerceCampaignPaidDeliveryClaimInput,
  options?: { assessedAt?: string },
): EcommerceCampaignPaidDeliveryClaim => {
  const verificationReasons = verifyEcommerceCampaignPaidDelivery(input.document.receipt)
  const receiptVerifiesClean = verificationReasons.length === 0
  const ownerSignOffPresent = isNonEmpty(input.ownerSignOffRef)

  const gates: EcommerceCampaignPaidDeliveryGates = {
    receiptVerifiesClean,
    ownerSignOffPresent,
  }

  const failingGateRefs: string[] = []
  if (!receiptVerifiesClean) {
    failingGateRefs.push(PAID_DELIVERY_GATE_RECEIPT_VERIFIES)
  }
  if (!ownerSignOffPresent) {
    failingGateRefs.push(PAID_DELIVERY_GATE_OWNER_SIGN_OFF)
  }

  const paidDeliverySubstantiated = failingGateRefs.length === 0

  return {
    schema: ECOMMERCE_CAMPAIGN_CLAIM_UPGRADE_SCHEMA,
    receiptRef: input.receiptRef,
    gates,
    failingGateRefs,
    paidDeliverySubstantiated,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    promiseIds: [ECOMMERCE_WORKSPACE_PACK_PROMISE],
    promiseState: 'yellow',
    unclearedBlockerRefs: paidDeliverySubstantiated
      ? []
      : [ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF],
    assessedAt: options?.assessedAt ?? currentIsoTimestamp(),
  }
}

export const EcommerceCampaignPaidDeliveryClaimStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'ecommerce_campaign_receipt_published',
    'product_promise_registry_updated',
  ])

export type EcommerceCampaignPaidDeliveryClaimStore = {
  list: () => ReadonlyArray<EcommerceCampaignPaidDeliveryClaimInput>
}

export const emptyEcommerceCampaignPaidDeliveryClaimStore: EcommerceCampaignPaidDeliveryClaimStore =
  {
    list: () => [],
  }

export const makeInMemoryEcommerceCampaignPaidDeliveryClaimStore = (
  inputs: ReadonlyArray<EcommerceCampaignPaidDeliveryClaimInput>,
): EcommerceCampaignPaidDeliveryClaimStore => ({
  list: () => inputs,
})

export const projectEcommerceCampaignPaidDeliveryClaims = (
  inputs: ReadonlyArray<EcommerceCampaignPaidDeliveryClaimInput>,
  options?: { generatedAt?: string },
) => {
  const generatedAt = options?.generatedAt ?? currentIsoTimestamp()
  const claims = inputs.map(input =>
    assessEcommerceCampaignPaidDeliveryClaim(input, { assessedAt: generatedAt }),
  )
  const substantiatedCount = claims.filter(c => c.paidDeliverySubstantiated).length

  return {
    schema: ECOMMERCE_CAMPAIGN_CLAIM_UPGRADE_SCHEMA,
    promiseIds: [ECOMMERCE_WORKSPACE_PACK_PROMISE],
    promiseState: 'yellow' as const,
    generatedAt,
    staleness: EcommerceCampaignPaidDeliveryClaimStaleness,
    maxStalenessSeconds: EcommerceCampaignPaidDeliveryClaimStaleness.maxStalenessSeconds,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    totals: {
      assessedCount: claims.length,
      substantiatedCount,
      withheldCount: claims.length - substantiatedCount,
    },
    paidDeliveryClaimSubstantiated: substantiatedCount > 0,
    unclearedBlockerRefs: [ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF],
    claims,
  }
}
