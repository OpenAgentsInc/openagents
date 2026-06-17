import { describe, expect, test } from 'bun:test'

import {
  classifyTreasuryPayoutFailure,
  paymentDestinationKind,
  treasuryPayoutFailureReasonRef,
} from './pay-failure.mjs'

describe('treasury payout failure classification', () => {
  test('classifies route failures', () => {
    expect(treasuryPayoutFailureReasonRef('No route found')).toBe(
      'reason.public.treasury_payout.no_route',
    )
  })

  test('classifies liquidity and capacity failures separately from balance', () => {
    expect(
      treasuryPayoutFailureReasonRef('temporary_channel_failure: capacity'),
    ).toBe('reason.public.treasury_payout.liquidity')
  })

  test('classifies invoice amount errors', () => {
    expect(treasuryPayoutFailureReasonRef('amount_out_of_range')).toBe(
      'reason.public.treasury_payout.invoice_rejected',
    )
  })

  test('keeps opaque failures generic', () => {
    expect(
      classifyTreasuryPayoutFailure(new Error('treasury_pay_failed')),
    ).toEqual({
      reasonClass: 'failed',
      reasonRef: 'reason.public.treasury_payout.failed',
    })
  })
})

describe('payment destination kind', () => {
  test('labels supported payment destination shapes without returning material', () => {
    expect(paymentDestinationKind('lnbc1private')).toBe('bolt11')
    expect(paymentDestinationKind('lno1private')).toBe('bolt12')
    expect(paymentDestinationKind('lnurl1private')).toBe('lnurl')
    expect(paymentDestinationKind('recipient@example.com')).toBe(
      'lightning_address',
    )
    expect(paymentDestinationKind('not-a-destination')).toBe('unknown')
  })
})
