import { describe, expect, it } from 'vitest'

import {
  BusinessAlreadySoldEngagementReceiptInvariantError,
  assertPaidBusinessReceipt,
  buildBusinessAlreadySoldEngagementPaymentReceipt,
  firstAlreadySoldBusinessQuickWinReceipt,
  projectBusinessAlreadySoldEngagementReceipts,
  publicBusinessAlreadySoldEngagementReceiptProjection,
} from './business-already-sold-engagement-receipt'

const baseInput = {
  engagementRef: 'engagement.business.quick_win.legal.002',
  buyerRef: 'buyer.business.opaque.legal.002',
  buyerPaidRef: 'payment.business.opaque.legal.002',
  engagementKind: 'quick_win' as const,
  verticalDescriptor: 'legal' as const,
  amountMinorUnits: 250000,
  currency: 'usd' as const,
  paidAt: '2026-07-02T12:00:00.000Z',
  recordedAt: '2026-07-03T12:00:00.000Z',
  demandProvenance: 'external_founder_sold' as const,
  privacyReview: {
    reviewed: true,
    reviewedAt: '2026-07-03T12:00:00.000Z',
    reviewerRef: 'privacy.review.operator.business_receipts',
    decisionRef: 'privacy.decision.business.opaque_receipts.002',
  },
  sourceRefs: ['docs/fable/ROADMAP_AFTER.md#A0.1'],
}

describe('business already-sold engagement payment receipts', () => {
  it('builds a deterministic paid business receipt with opaque refs', () => {
    const receipt = buildBusinessAlreadySoldEngagementPaymentReceipt(baseInput)

    expect(receipt.receiptKind).toBe(
      'business.already_sold_engagement.payment',
    )
    expect(receipt.receiptRef).toBe(
      'receipt.business.quick_win.engagement.business.quick_win.legal.002.payment.business.opaque.legal.002',
    )
    expect(receipt.privacyReview.reviewed).toBe(true)
    expect(() => assertPaidBusinessReceipt(receipt)).not.toThrow()
  })

  it('rejects private buyer/payment material before publication', () => {
    expect(() =>
      buildBusinessAlreadySoldEngagementPaymentReceipt({
        ...baseInput,
        buyerRef: 'buyer.customer@example.com',
      }),
    ).toThrow(BusinessAlreadySoldEngagementReceiptInvariantError)

    expect(() =>
      buildBusinessAlreadySoldEngagementPaymentReceipt({
        ...baseInput,
        buyerPaidRef: 'stripe.invoice.in_123',
      }),
    ).toThrow(BusinessAlreadySoldEngagementReceiptInvariantError)
  })

  it('requires privacy review and positive amount', () => {
    expect(() =>
      buildBusinessAlreadySoldEngagementPaymentReceipt({
        ...baseInput,
        privacyReview: { ...baseInput.privacyReview, reviewed: false },
      }),
    ).toThrow(/privacyReview.reviewed must be true/)

    expect(() =>
      buildBusinessAlreadySoldEngagementPaymentReceipt({
        ...baseInput,
        amountMinorUnits: 0,
      }),
    ).toThrow(/amountMinorUnits must be a positive integer/)
  })

  it('projects without the internal buyerPaidRef', () => {
    const receipt = buildBusinessAlreadySoldEngagementPaymentReceipt(baseInput)
    const projection =
      publicBusinessAlreadySoldEngagementReceiptProjection(receipt)

    expect(projection.receiptRef).toBe(receipt.receiptRef)
    expect(projection.buyerRef).toBe('buyer.business.opaque.legal.002')
    expect(projection).not.toHaveProperty('buyerPaidRef')
    expect(projection).not.toHaveProperty('privacyReview')
  })

  it('projects the first already-sold business receipt as recorded evidence', () => {
    const projection = projectBusinessAlreadySoldEngagementReceipts([
      firstAlreadySoldBusinessQuickWinReceipt,
    ])

    expect(projection.paidBusinessReceiptRecorded).toBe(true)
    expect(projection.totals).toMatchObject({
      receiptCount: 1,
      paidBusinessReceiptCount: 1,
      amountMinorUnitsByCurrency: { usd: 100000 },
    })
    expect(projection.receipts[0]).toMatchObject({
      engagementKind: 'quick_win',
      verticalDescriptor: 'legal',
      buyerRef: 'buyer.business.opaque.legal.001',
    })
  })
})

