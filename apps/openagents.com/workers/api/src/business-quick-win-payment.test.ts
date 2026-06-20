import { describe, expect, it } from 'vitest'

import {
  BusinessQuickWinPaymentInvariantError,
  buildBusinessQuickWinPaymentEvidence,
  businessQuickWinPaidEvidenceRef,
  publicBusinessQuickWinPaymentProjection,
} from './business-quick-win-payment'

describe('business-quick-win-payment', () => {
  it('builds paid evidence for a settled payment', () => {
    const evidence = buildBusinessQuickWinPaymentEvidence({
      signupId: 'signup_123',
      amount: 50000,
      currency: 'usd',
      paymentStatus: 'settled',
      paymentRef: 'ch_12345',
    })

    expect(evidence.evidenceKind).toBe('business_quick_win_payment')
    expect(evidence.isPaid).toBe(true)
    expect(businessQuickWinPaidEvidenceRef(evidence)).toBe('ch_12345')
  })

  it('builds unpaid evidence for a pending payment', () => {
    const evidence = buildBusinessQuickWinPaymentEvidence({
      signupId: 'signup_123',
      amount: 50000,
      currency: 'usd',
      paymentStatus: 'pending',
      paymentRef: 'pi_12345',
    })

    expect(evidence.isPaid).toBe(false)
    expect(() => businessQuickWinPaidEvidenceRef(evidence)).toThrow(
      BusinessQuickWinPaymentInvariantError,
    )
  })

  it('builds unpaid evidence for a failed payment', () => {
    const evidence = buildBusinessQuickWinPaymentEvidence({
      signupId: 'signup_123',
      amount: 50000,
      currency: 'usd',
      paymentStatus: 'failed',
      paymentRef: 'ch_12345_failed',
    })

    expect(evidence.isPaid).toBe(false)
    expect(() => businessQuickWinPaidEvidenceRef(evidence)).toThrow(
      BusinessQuickWinPaymentInvariantError,
    )
  })

  it('requires all fields', () => {
    expect(() =>
      buildBusinessQuickWinPaymentEvidence({
        signupId: '  ',
        amount: 50000,
        currency: 'usd',
        paymentStatus: 'settled',
        paymentRef: 'ch_12345',
      }),
    ).toThrow(/signupId is required/)

    expect(() =>
      buildBusinessQuickWinPaymentEvidence({
        signupId: 'signup_123',
        amount: 0,
        currency: 'usd',
        paymentStatus: 'settled',
        paymentRef: 'ch_12345',
      }),
    ).toThrow(/amount must be greater than zero/)

    expect(() =>
      buildBusinessQuickWinPaymentEvidence({
        signupId: 'signup_123',
        amount: 50000,
        currency: '  ',
        paymentStatus: 'settled',
        paymentRef: 'ch_12345',
      }),
    ).toThrow(/currency is required/)

    expect(() =>
      buildBusinessQuickWinPaymentEvidence({
        signupId: 'signup_123',
        amount: 50000,
        currency: 'usd',
        paymentStatus: 'settled',
        paymentRef: '  ',
      }),
    ).toThrow(/paymentRef is required/)
  })

  it('projects safely', () => {
    const evidence = buildBusinessQuickWinPaymentEvidence({
      signupId: 'signup_123',
      amount: 50000,
      currency: 'usd',
      paymentStatus: 'settled',
      paymentRef: 'ch_12345',
    })

    const projection = publicBusinessQuickWinPaymentProjection(evidence)
    expect(projection).toEqual({
      evidenceKind: 'business_quick_win_payment',
      amount: 50000,
      currency: 'usd',
      paymentStatus: 'settled',
      isPaid: true,
    })
    // @ts-expect-error
    expect(projection.signupId).toBeUndefined()
    // @ts-expect-error
    expect(projection.paymentRef).toBeUndefined()
  })
})
