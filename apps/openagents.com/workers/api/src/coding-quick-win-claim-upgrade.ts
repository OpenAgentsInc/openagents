import { Schema as S } from 'effect'

import {
  type BusinessQuickWinReceipt,
  assertFirstPaidQuickWinReceipt,
  publicBusinessQuickWinReceiptProjection,
} from './business-quick-win-receipt'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const CODING_QUICK_WIN_CLAIM_UPGRADE_SCHEMA =
  'openagents.business.coding_quick_win.paid_delivery_claim.v1' as const

export const CLAIM_UPGRADE_CONTRACT = 'proof.claim_upgrade_receipts.v1' as const

export const BUSINESS_CODING_QUICK_WIN_PROMISE =
  'business.coding_quick_win.v1' as const

export const BUSINESS_CODING_QUICK_WIN_PAID_RECEIPT_BLOCKER_REF =
  'blocker.product_promises.business_coding_quick_win_paid_receipt_missing' as const

export const PAID_DELIVERY_GATE_RECEIPT_VERIFIES =
  'gate.paid_delivery.receipt_verifies_clean' as const
export const PAID_DELIVERY_GATE_OWNER_SIGN_OFF =
  'gate.paid_delivery.owner_sign_off_present' as const

export const CodingQuickWinPaidDeliveryGates = S.Struct({
  receiptVerifiesClean: S.Boolean,
  ownerSignOffPresent: S.Boolean,
})
export type CodingQuickWinPaidDeliveryGates =
  typeof CodingQuickWinPaidDeliveryGates.Type

export const CodingQuickWinPaidDeliveryClaim = S.Struct({
  schema: S.Literal(CODING_QUICK_WIN_CLAIM_UPGRADE_SCHEMA),
  receiptRef: S.String,
  gates: CodingQuickWinPaidDeliveryGates,
  failingGateRefs: S.Array(S.String),
  paidDeliverySubstantiated: S.Boolean,
  contractRef: S.Literal(CLAIM_UPGRADE_CONTRACT),
  promiseIds: S.Tuple([S.Literal(BUSINESS_CODING_QUICK_WIN_PROMISE)]),
  promiseState: S.Literal('yellow'),
  unclearedBlockerRefs: S.Array(S.String),
  assessedAt: S.String,
})
export type CodingQuickWinPaidDeliveryClaim =
  typeof CodingQuickWinPaidDeliveryClaim.Type

export type CodingQuickWinPaidDeliveryClaimInput = Readonly<{
  receipt: BusinessQuickWinReceipt
  receiptRef: string
  ownerSignOffRef?: string | undefined
}>

const isNonEmpty = (value: string | undefined): value is string =>
  typeof value === 'string' && value.trim().length > 0

const receiptVerifies = (receipt: BusinessQuickWinReceipt): boolean => {
  try {
    assertFirstPaidQuickWinReceipt(receipt)
    return receipt.offeringPromiseId === BUSINESS_CODING_QUICK_WIN_PROMISE
  } catch {
    return false
  }
}

export const assessCodingQuickWinPaidDeliveryClaim = (
  input: CodingQuickWinPaidDeliveryClaimInput,
  options?: { assessedAt?: string },
): CodingQuickWinPaidDeliveryClaim => {
  const receiptVerifiesClean = receiptVerifies(input.receipt)
  const ownerSignOffPresent = isNonEmpty(input.ownerSignOffRef)

  const failingGateRefs: Array<
    | typeof PAID_DELIVERY_GATE_RECEIPT_VERIFIES
    | typeof PAID_DELIVERY_GATE_OWNER_SIGN_OFF
  > = []
  if (!receiptVerifiesClean) {
    failingGateRefs.push(PAID_DELIVERY_GATE_RECEIPT_VERIFIES)
  }
  if (!ownerSignOffPresent) {
    failingGateRefs.push(PAID_DELIVERY_GATE_OWNER_SIGN_OFF)
  }

  const paidDeliverySubstantiated = failingGateRefs.length === 0

  return {
    schema: CODING_QUICK_WIN_CLAIM_UPGRADE_SCHEMA,
    receiptRef: input.receiptRef,
    gates: {
      receiptVerifiesClean,
      ownerSignOffPresent,
    },
    failingGateRefs,
    paidDeliverySubstantiated,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    promiseIds: [BUSINESS_CODING_QUICK_WIN_PROMISE],
    promiseState: 'yellow',
    unclearedBlockerRefs: paidDeliverySubstantiated
      ? []
      : [BUSINESS_CODING_QUICK_WIN_PAID_RECEIPT_BLOCKER_REF],
    assessedAt: options?.assessedAt ?? currentIsoTimestamp(),
  }
}

export const CodingQuickWinPaidDeliveryClaimStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'business_coding_quick_win_receipt_published',
    'product_promise_registry_updated',
  ])

export type CodingQuickWinPaidDeliveryClaimStore = {
  list: () => ReadonlyArray<CodingQuickWinPaidDeliveryClaimInput>
}

export const emptyCodingQuickWinPaidDeliveryClaimStore: CodingQuickWinPaidDeliveryClaimStore =
  {
    list: () => [],
  }

export const makeInMemoryCodingQuickWinPaidDeliveryClaimStore = (
  inputs: ReadonlyArray<CodingQuickWinPaidDeliveryClaimInput>,
): CodingQuickWinPaidDeliveryClaimStore => ({
  list: () => inputs,
})

export const projectCodingQuickWinPaidDeliveryClaims = (
  inputs: ReadonlyArray<CodingQuickWinPaidDeliveryClaimInput>,
  options?: { generatedAt?: string },
) => {
  const generatedAt = options?.generatedAt ?? currentIsoTimestamp()
  const claims = inputs.map(input =>
    assessCodingQuickWinPaidDeliveryClaim(input, { assessedAt: generatedAt }),
  )
  const substantiatedCount = claims.filter(
    claim => claim.paidDeliverySubstantiated,
  ).length

  return {
    schema: CODING_QUICK_WIN_CLAIM_UPGRADE_SCHEMA,
    promiseIds: [BUSINESS_CODING_QUICK_WIN_PROMISE],
    promiseState: 'yellow' as const,
    generatedAt,
    staleness: CodingQuickWinPaidDeliveryClaimStaleness,
    maxStalenessSeconds:
      CodingQuickWinPaidDeliveryClaimStaleness.maxStalenessSeconds,
    contractRef: CLAIM_UPGRADE_CONTRACT,
    totals: {
      assessedCount: claims.length,
      substantiatedCount,
      withheldCount: claims.length - substantiatedCount,
    },
    paidDeliveryClaimSubstantiated: substantiatedCount > 0,
    unclearedBlockerRefs:
      substantiatedCount > 0
        ? []
        : [BUSINESS_CODING_QUICK_WIN_PAID_RECEIPT_BLOCKER_REF],
    claims,
  }
}

export const projectCodingQuickWinReceiptRead = (
  input: CodingQuickWinPaidDeliveryClaimInput,
  options?: { generatedAt?: string },
) => {
  const generatedAt = options?.generatedAt ?? currentIsoTimestamp()
  return {
    schema: 'openagents.business.coding_quick_win.receipt_read.v1',
    receiptRef: input.receiptRef,
    promiseIds: [BUSINESS_CODING_QUICK_WIN_PROMISE],
    promiseState: 'yellow' as const,
    generatedAt,
    staleness: CodingQuickWinPaidDeliveryClaimStaleness,
    receipt: publicBusinessQuickWinReceiptProjection(input.receipt),
    claim: assessCodingQuickWinPaidDeliveryClaim(input, {
      assessedAt: generatedAt,
    }),
    authorityBoundary:
      'This public-safe receipt read verifies and projects a coding quick-win paid-delivery receipt. It grants no auto-merge, deploy, spend, payout, settlement, or green-claim authority.',
  }
}
