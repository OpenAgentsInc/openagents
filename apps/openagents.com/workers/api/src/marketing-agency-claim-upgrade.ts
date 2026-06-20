import { Schema as S } from 'effect'

import {
  type MarketingAgencyDeliveryReceipt,
  verifyMarketingAgencyPaidDelivery,
} from './marketing-agency-delivery-receipt'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const MARKETING_AGENCY_CLAIM_UPGRADE_SCHEMA =
  'openagents.marketing_agency.paid_delivery_claim.v1' as const

export const CLAIM_UPGRADE_CONTRACT = 'proof.claim_upgrade_receipts.v1' as const

export const MARKETING_AGENCY_WORKSPACE_PACK_PROMISE =
  'business.marketing_agency_workspace_pack.v1' as const

export const MARKETING_AGENCY_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF =
  'blocker.product_promises.marketing_agency_pack_first_paid_delivery_receipt_missing' as const

export const PAID_DELIVERY_GATE_RECEIPT_VERIFIES =
  'gate.paid_delivery.receipt_verifies_clean' as const
export const PAID_DELIVERY_GATE_OWNER_SIGN_OFF =
  'gate.paid_delivery.owner_sign_off_present' as const

export const MarketingAgencyPaidDeliveryGates = S.Struct({
  receiptVerifiesClean: S.Boolean,
  ownerSignOffPresent: S.Boolean,
})
export type MarketingAgencyPaidDeliveryGates = typeof MarketingAgencyPaidDeliveryGates.Type

export const MarketingAgencyPaidDeliveryClaim = S.Struct({
  schema: S.Literal(MARKETING_AGENCY_CLAIM_UPGRADE_SCHEMA),
  receiptRef: S.String,
  gates: MarketingAgencyPaidDeliveryGates,
  failingGateRefs: S.Array(S.String),
  paidDeliverySubstantiated: S.Boolean,
  contractRef: S.Literal(CLAIM_UPGRADE_CONTRACT),
  promiseIds: S.Tuple([S.Literal(MARKETING_AGENCY_WORKSPACE_PACK_PROMISE)]),
  promiseState: S.Literal('yellow'),
  unclearedBlockerRefs: S.Array(S.String),
  assessedAt: S.String,
})
export type MarketingAgencyPaidDeliveryClaim = typeof MarketingAgencyPaidDeliveryClaim.Type

export type MarketingAgencyPaidDeliveryClaimInput = Readonly<{
  receipt: MarketingAgencyDeliveryReceipt
  receiptRef: string
  ownerSignOffRef?: string | undefined
}>

const isNonEmpty = (value: string | undefined): value is string =>
  typeof value === 'string' && value.trim().length > 0

export const assessMarketingAgencyPaidDeliveryClaim = (
  input: MarketingAgencyPaidDeliveryClaimInput,
  options?: { assessedAt?: string },
): MarketingAgencyPaidDeliveryClaim => {
  const verificationReasons = verifyMarketingAgencyPaidDelivery(input.receipt)
  const receiptVerifiesClean = verificationReasons.length === 0
  const ownerSignOffPresent = isNonEmpty(input.ownerSignOffRef)

  const gates: MarketingAgencyPaidDeliveryGates = {
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
    schema: MARKETING_AGENCY_CLAIM_UPGRADE_SCHEMA,
    receiptRef: input.receiptRef,
    gates,
    failingGateRefs,
    paidDeliverySubstantiated,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    promiseIds: [MARKETING_AGENCY_WORKSPACE_PACK_PROMISE],
    promiseState: 'yellow',
    unclearedBlockerRefs: paidDeliverySubstantiated
      ? []
      : [MARKETING_AGENCY_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF],
    assessedAt: options?.assessedAt ?? currentIsoTimestamp(),
  }
}

export const MarketingAgencyPaidDeliveryClaimStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'marketing_agency_receipt_published',
    'product_promise_registry_updated',
  ])

export type MarketingAgencyPaidDeliveryClaimStore = {
  list: () => ReadonlyArray<MarketingAgencyPaidDeliveryClaimInput>
}

export const emptyMarketingAgencyPaidDeliveryClaimStore: MarketingAgencyPaidDeliveryClaimStore =
  {
    list: () => [],
  }

export const makeInMemoryMarketingAgencyPaidDeliveryClaimStore = (
  inputs: ReadonlyArray<MarketingAgencyPaidDeliveryClaimInput>,
): MarketingAgencyPaidDeliveryClaimStore => ({
  list: () => inputs,
})

export const projectMarketingAgencyPaidDeliveryClaims = (
  inputs: ReadonlyArray<MarketingAgencyPaidDeliveryClaimInput>,
  options?: { generatedAt?: string },
) => {
  const generatedAt = options?.generatedAt ?? currentIsoTimestamp()
  const claims = inputs.map(input =>
    assessMarketingAgencyPaidDeliveryClaim(input, { assessedAt: generatedAt }),
  )
  const substantiatedCount = claims.filter(c => c.paidDeliverySubstantiated).length

  return {
    schema: MARKETING_AGENCY_CLAIM_UPGRADE_SCHEMA,
    promiseIds: [MARKETING_AGENCY_WORKSPACE_PACK_PROMISE],
    promiseState: 'yellow' as const,
    generatedAt,
    staleness: MarketingAgencyPaidDeliveryClaimStaleness,
    maxStalenessSeconds: MarketingAgencyPaidDeliveryClaimStaleness.maxStalenessSeconds,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    totals: {
      assessedCount: claims.length,
      substantiatedCount,
      withheldCount: claims.length - substantiatedCount,
    },
    paidDeliveryClaimSubstantiated: substantiatedCount > 0,
    unclearedBlockerRefs: [MARKETING_AGENCY_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF],
    claims,
  }
}
