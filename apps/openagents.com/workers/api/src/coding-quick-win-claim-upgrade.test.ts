import { describe, expect, it } from 'vitest'

import { buildBusinessQuickWinReceipt } from './business-quick-win-receipt'
import {
  assessCodingQuickWinPaidDeliveryClaim,
  projectCodingQuickWinPaidDeliveryClaims,
} from './coding-quick-win-claim-upgrade'

const paidCodingReceipt = buildBusinessQuickWinReceipt({
  signupId: 'signup.public.coding_quick_win_1',
  offeringPromiseId: 'business.coding_quick_win.v1',
  quickWinSummary: 'Fix the failing checkout test suite with passing tests.',
  quickWinScopedRef: 'spec.public.coding_quick_win_1',
  deliveredEvidenceRef: 'delivery.public.coding_quick_win_1',
  outcomeAcceptedRef: 'acceptance.public.coding_quick_win_1',
  buyerPaidRef: 'payment.public.coding_quick_win_1',
})

describe('coding quick win paid delivery claim upgrade', () => {
  it('withholds the claim when owner sign-off is missing', () => {
    const claim = assessCodingQuickWinPaidDeliveryClaim({
      receipt: paidCodingReceipt,
      receiptRef: 'receipt.public.business.coding_quick_win.1',
    })

    expect(claim.gates.receiptVerifiesClean).toBe(true)
    expect(claim.gates.ownerSignOffPresent).toBe(false)
    expect(claim.paidDeliverySubstantiated).toBe(false)
    expect(claim.unclearedBlockerRefs).toEqual([
      'blocker.product_promises.business_coding_quick_win_paid_receipt_missing',
    ])
  })

  it('substantiates only when the paid receipt verifies and owner sign-off exists', () => {
    const claim = assessCodingQuickWinPaidDeliveryClaim({
      receipt: paidCodingReceipt,
      receiptRef: 'receipt.public.business.coding_quick_win.1',
      ownerSignOffRef: 'owner.signoff.business.coding_quick_win.1',
    })

    expect(claim.failingGateRefs).toEqual([])
    expect(claim.paidDeliverySubstantiated).toBe(true)
    expect(claim.unclearedBlockerRefs).toEqual([])
  })

  it('keeps the blocker when no real claims are present', () => {
    const projection = projectCodingQuickWinPaidDeliveryClaims([], {
      generatedAt: '2026-06-29T00:00:00.000Z',
    })

    expect(projection.totals).toEqual({
      assessedCount: 0,
      substantiatedCount: 0,
      withheldCount: 0,
    })
    expect(projection.paidDeliveryClaimSubstantiated).toBe(false)
    expect(projection.unclearedBlockerRefs).toEqual([
      'blocker.product_promises.business_coding_quick_win_paid_receipt_missing',
    ])
  })
})
