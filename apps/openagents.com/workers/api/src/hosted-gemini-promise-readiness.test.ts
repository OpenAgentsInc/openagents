import { describe, expect, test } from 'vitest'

import {
  buildHostedGeminiPromiseReadiness,
  evaluateHostedGeminiProductionReceipt,
} from './hosted-gemini-promise-readiness'
import type { PublicInferenceReceiptProjection } from './inference-receipts'
import type { PromiseTransitionReceipt } from './promise-transition-receipt-routes'

const generatedAt = '2026-06-29T12:00:00.000Z'
const receiptRef = 'receipt.inference.charge.chatcmpl_hosted_gemini_prod'

const receipt = (
  input: Partial<PublicInferenceReceiptProjection> = {},
): PublicInferenceReceiptProjection => ({
  authorityBoundary: 'Public proof only.',
  caveatRefs: ['caveat.public.no_private_payment_material'],
  generatedAt,
  kind: 'charge',
  ledgerState: 'paid',
  modelEvidence: {
    requested_model: 'openagents/khala',
    served_model: 'models/gemini-3.5-flash',
    supply_lane: 'vertex-gemini',
    total_tokens: 321,
    worker: 'vertex-gemini',
  },
  receiptRef,
  schemaVersion: 'openagents.inference.receipt.v1',
  sourceRefs: [`route:/api/public/inference/receipts/${receiptRef}`],
  staleness: {
    composition: 'live_at_read',
    contractVersion: 'projection_staleness.v1',
    maxStalenessSeconds: 0,
    rebuildsOn: ['pay_ins.public_receipt_ref'],
  },
  stateChangedAt: '2026-06-29T11:59:00.000Z',
  ...input,
})

const transition = (
  input: Partial<PromiseTransitionReceipt> = {},
): PromiseTransitionReceipt => ({
  checkedAt: '2026-06-29T12:01:00.000Z',
  checks: [],
  evidenceRefs: [receiptRef],
  exception: {
    approvedByRef: 'owner:openagents',
    expiresAt: '2026-07-06',
    reasonRef: 'owner_signoff.hosted_gemini.production_receipt',
  },
  fromState: 'yellow',
  promiseId: 'api.hosted_gemini.v1',
  receiptId: 'promise_transition_hosted_gemini_green_1',
  registryVersion: '2026-06-29.1',
  result: 'exception',
  toState: 'green',
  ...input,
})

describe('Hosted Gemini product-promise readiness', () => {
  test('accepts only paid Vertex Gemini inference charge receipts as production evidence', () => {
    expect(evaluateHostedGeminiProductionReceipt(receipt())).toMatchObject({
      result: 'passed',
      status: 'production_receipt_present',
    })
    expect(
      evaluateHostedGeminiProductionReceipt(
        receipt({
          modelEvidence: {
            served_model: 'accounts/fireworks/models/deepseek-v4-flash',
            supply_lane: 'fireworks',
            total_tokens: 321,
            worker: 'fireworks',
          },
        }),
      ),
    ).toMatchObject({
      result: 'failed',
      status: 'not_hosted_gemini',
    })
    expect(
      evaluateHostedGeminiProductionReceipt(
        receipt({ ledgerState: 'free_allowance' }),
      ),
    ).toMatchObject({
      result: 'failed',
      status: 'not_metered_paid',
    })
  })

  test('keeps blocker refs until the owner transition cites the same production receipt', () => {
    const withoutOwner = buildHostedGeminiPromiseReadiness({
      generatedAt,
      receipt: receipt(),
      receiptRef,
      transitionReceipts: [],
    })
    const withOwner = buildHostedGeminiPromiseReadiness({
      generatedAt,
      receipt: receipt(),
      receiptRef,
      transitionReceipts: [transition()],
    })

    expect(withoutOwner.greenGateMet).toBe(false)
    expect(withoutOwner.blockerRefs).toEqual([
      'blocker.product_promises.hosted_gemini_owner_upgrade_signoff_pending',
    ])
    expect(withoutOwner.clearsBlockerRefs).toEqual([])
    expect(withOwner.greenGateMet).toBe(true)
    expect(withOwner.blockerRefs).toEqual([])
    expect(withOwner.clearsBlockerRefs).toEqual([
      'blocker.product_promises.hosted_gemini_production_receipt_pending',
      'blocker.product_promises.hosted_gemini_owner_upgrade_signoff_pending',
    ])
  })

  test('does not let an unrelated owner transition clear the signoff blocker', () => {
    const readiness = buildHostedGeminiPromiseReadiness({
      generatedAt,
      receipt: receipt(),
      receiptRef,
      transitionReceipts: [
        transition({
          evidenceRefs: ['receipt.inference.charge.other'],
        }),
      ],
    })

    expect(readiness.greenGateMet).toBe(false)
    expect(readiness.ownerTransitionCheck).toMatchObject({
      result: 'failed',
      status: 'missing_owner_transition',
    })
  })
})
