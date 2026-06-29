import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import { buildBusinessQuickWinReceipt } from './business-quick-win-receipt'
import { makeInMemoryCodingQuickWinPaidDeliveryClaimStore } from './coding-quick-win-claim-upgrade'
import {
  CodingQuickWinReceiptsEndpoint,
  makeCodingQuickWinReceiptPublicRoutes,
} from './coding-quick-win-receipt-public-routes'

const receiptRef = 'receipt.public.business.coding_quick_win.1'
const receipt = buildBusinessQuickWinReceipt({
  signupId: 'signup.public.coding_quick_win_1',
  offeringPromiseId: 'business.coding_quick_win.v1',
  quickWinSummary: 'Fix the failing checkout test suite with passing tests.',
  quickWinScopedRef: 'spec.public.coding_quick_win_1',
  deliveredEvidenceRef: 'delivery.public.coding_quick_win_1',
  outcomeAcceptedRef: 'acceptance.public.coding_quick_win_1',
  buyerPaidRef: 'payment.public.coding_quick_win_1',
})

describe('coding quick win receipt public routes', () => {
  it('projects empty paid delivery claims without clearing the blocker', async () => {
    const routes = makeCodingQuickWinReceiptPublicRoutes({
      makeClaimStore: () =>
        makeInMemoryCodingQuickWinPaidDeliveryClaimStore([]),
    })

    const request = new Request(
      `https://openagents.com${CodingQuickWinReceiptsEndpoint}?view=paid-delivery-claims`,
    )
    const responseEffect = routes.routeCodingQuickWinReceiptRequest(request, {})
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(200)
    const json = await response.json()

    expect(json).toMatchObject({
      schema: 'openagents.business.coding_quick_win.paid_delivery_claim.v1',
      promiseIds: ['business.coding_quick_win.v1'],
      promiseState: 'yellow',
      totals: {
        assessedCount: 0,
        substantiatedCount: 0,
        withheldCount: 0,
      },
      paidDeliveryClaimSubstantiated: false,
      unclearedBlockerRefs: [
        'blocker.product_promises.business_coding_quick_win_paid_receipt_missing',
      ],
    })
  })

  it('dereferences a public-safe receipt read from the claim store', async () => {
    const routes = makeCodingQuickWinReceiptPublicRoutes({
      makeClaimStore: () =>
        makeInMemoryCodingQuickWinPaidDeliveryClaimStore([
          {
            receipt,
            receiptRef,
            ownerSignOffRef: 'owner.signoff.business.coding_quick_win.1',
          },
        ]),
    })

    const request = new Request(
      `https://openagents.com${CodingQuickWinReceiptsEndpoint}/${encodeURIComponent(receiptRef)}`,
    )
    const responseEffect = routes.routeCodingQuickWinReceiptRequest(request, {})
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(200)
    const json = (await response.json()) as {
      receiptRef: string
      receipt: { lines: ReadonlyArray<Record<string, unknown>> }
      claim: { paidDeliverySubstantiated: boolean }
    }

    expect(json.receiptRef).toBe(receiptRef)
    expect(json.receipt).not.toHaveProperty('signupId')
    expect(json.receipt.lines[0]).not.toHaveProperty('evidenceRef')
    expect(json.claim.paidDeliverySubstantiated).toBe(true)
  })

  it('returns 404 for an unknown receipt ref', async () => {
    const routes = makeCodingQuickWinReceiptPublicRoutes({
      makeClaimStore: () =>
        makeInMemoryCodingQuickWinPaidDeliveryClaimStore([]),
    })

    const request = new Request(
      `https://openagents.com${CodingQuickWinReceiptsEndpoint}/unknown`,
    )
    const responseEffect = routes.routeCodingQuickWinReceiptRequest(request, {})
    expect(responseEffect).toBeDefined()

    const response = await Effect.runPromise(responseEffect!)
    expect(response.status).toBe(404)
  })
})
