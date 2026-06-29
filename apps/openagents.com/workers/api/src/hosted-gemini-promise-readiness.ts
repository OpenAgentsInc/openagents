import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import type { PublicInferenceReceiptProjection } from './inference-receipts'
import type { PromiseTransitionReceipt } from './promise-transition-receipt-routes'

export const HostedGeminiPromiseReadinessRoute =
  '/api/public/product-promises/api.hosted_gemini.v1/readiness' as const

export type HostedGeminiProductionReceiptCheck = Readonly<{
  evidenceRefs: ReadonlyArray<string>
  result: 'failed' | 'passed'
  status:
    | 'missing_receipt'
    | 'not_hosted_gemini'
    | 'not_metered_paid'
    | 'production_receipt_present'
}>

export type HostedGeminiOwnerTransitionCheck = Readonly<{
  evidenceRefs: ReadonlyArray<string>
  receiptId: string | null
  result: 'failed' | 'passed'
  status: 'missing_owner_transition' | 'owner_transition_present'
}>

export type HostedGeminiPromiseReadiness = Readonly<{
  authorityBoundary: string
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  clearsBlockerRefs: ReadonlyArray<string>
  generatedAt: string
  greenGateMet: boolean
  ownerTransitionCheck: HostedGeminiOwnerTransitionCheck
  promiseId: 'api.hosted_gemini.v1'
  receiptCheck: HostedGeminiProductionReceiptCheck
  receiptRef: string
  requestPath: 'POST /api/v1/chat/completions'
  responseShapeRef: 'response.openai_compatible.chat_completion.success'
  schemaVersion: 'openagents.product_promise.hosted_gemini_readiness.v1'
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
}>

const promiseId = 'api.hosted_gemini.v1' as const
const productionReceiptBlocker =
  'blocker.product_promises.hosted_gemini_production_receipt_pending'
const ownerTransitionBlocker =
  'blocker.product_promises.hosted_gemini_owner_upgrade_signoff_pending'

const hostedGeminiReceiptEvidenceRefs = (
  receipt: PublicInferenceReceiptProjection,
): ReadonlyArray<string> => [
  receipt.receiptRef,
  ...receipt.sourceRefs,
  'route:/api/public/inference/receipts/{receiptRef}',
  'route:/api/v1/chat/completions',
  'lane:vertex-gemini',
]

export const evaluateHostedGeminiProductionReceipt = (
  receipt: PublicInferenceReceiptProjection | null,
): HostedGeminiProductionReceiptCheck => {
  if (receipt === null) {
    return {
      evidenceRefs: [],
      result: 'failed',
      status: 'missing_receipt',
    }
  }

  if (
    receipt.kind !== 'charge' ||
    receipt.ledgerState !== 'paid' ||
    receipt.modelEvidence === undefined ||
    receipt.modelEvidence.total_tokens <= 0
  ) {
    return {
      evidenceRefs: hostedGeminiReceiptEvidenceRefs(receipt),
      result: 'failed',
      status: 'not_metered_paid',
    }
  }

  if (
    receipt.modelEvidence.supply_lane !== 'vertex-gemini' ||
    receipt.modelEvidence.worker !== 'vertex-gemini'
  ) {
    return {
      evidenceRefs: hostedGeminiReceiptEvidenceRefs(receipt),
      result: 'failed',
      status: 'not_hosted_gemini',
    }
  }

  return {
    evidenceRefs: hostedGeminiReceiptEvidenceRefs(receipt),
    result: 'passed',
    status: 'production_receipt_present',
  }
}

export const evaluateHostedGeminiOwnerTransition = (
  receipts: ReadonlyArray<PromiseTransitionReceipt>,
  evidenceRefs: ReadonlyArray<string>,
): HostedGeminiOwnerTransitionCheck => {
  const matchingReceipt = receipts.find(
    receipt =>
      receipt.promiseId === promiseId &&
      receipt.toState === 'green' &&
      receipt.result !== 'failed' &&
      receipt.evidenceRefs.some(ref => evidenceRefs.includes(ref)),
  )

  return matchingReceipt === undefined
    ? {
        evidenceRefs: [],
        receiptId: null,
        result: 'failed',
        status: 'missing_owner_transition',
      }
    : {
        evidenceRefs: [matchingReceipt.receiptId, ...matchingReceipt.evidenceRefs],
        receiptId: matchingReceipt.receiptId,
        result: 'passed',
        status: 'owner_transition_present',
      }
}

export const buildHostedGeminiPromiseReadiness = (input: {
  generatedAt: string
  receipt: PublicInferenceReceiptProjection | null
  receiptRef: string
  transitionReceipts: ReadonlyArray<PromiseTransitionReceipt>
}): HostedGeminiPromiseReadiness => {
  const receiptCheck = evaluateHostedGeminiProductionReceipt(input.receipt)
  const ownerTransitionCheck = evaluateHostedGeminiOwnerTransition(
    input.transitionReceipts,
    receiptCheck.evidenceRefs,
  )
  const greenGateMet =
    receiptCheck.result === 'passed' &&
    ownerTransitionCheck.result === 'passed'
  const blockerRefs = [
    ...(receiptCheck.result === 'passed' ? [] : [productionReceiptBlocker]),
    ...(ownerTransitionCheck.result === 'passed' ? [] : [ownerTransitionBlocker]),
  ]

  return {
    authorityBoundary:
      'Read-only product-promise evidence. This readiness projection grants no spend, refund, payout, provider, registry-state, or owner-signoff authority; the registry flips only through the product-promise transition receipt path.',
    blockerRefs,
    caveatRefs: [
      'caveat.public.hosted_gemini_receipt_requires_vertex_gemini_paid_charge',
      'caveat.public.hosted_gemini_green_requires_owner_transition_receipt',
      'caveat.public.no_prompts_provider_payloads_keys_or_private_user_data',
    ],
    clearsBlockerRefs: greenGateMet
      ? [productionReceiptBlocker, ownerTransitionBlocker]
      : [],
    generatedAt: input.generatedAt,
    greenGateMet,
    ownerTransitionCheck,
    promiseId,
    receiptCheck,
    receiptRef: input.receiptRef,
    requestPath: 'POST /api/v1/chat/completions',
    responseShapeRef: 'response.openai_compatible.chat_completion.success',
    schemaVersion: 'openagents.product_promise.hosted_gemini_readiness.v1',
    sourceRefs: [
      HostedGeminiPromiseReadinessRoute,
      'https://openagents.com/api/public/product-promises',
      'https://openagents.com/api/public/product-promises/transitions',
      'docs/launch/vertex-fleet/api.hosted_gemini.v1.md',
      ...receiptCheck.evidenceRefs,
      ...ownerTransitionCheck.evidenceRefs,
    ],
    staleness: liveAtReadStaleness([
      'pay_ins.public_receipt_ref',
      'product_promise_transition_receipt_recorded',
    ]),
  }
}
