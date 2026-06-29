import { describe, expect, test } from 'bun:test'

import {
  classifyTreasuryPayoutFailure,
  paymentDestinationKind,
  treasuryPayoutFailureDiagnostics,
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

  test('adds safe opaque diagnostics without returning raw daemon text', () => {
    const error = new Error('private daemon text with route material')
    error.code = 'ERR_PRIVATE_ROUTE'
    const diagnostics = treasuryPayoutFailureDiagnostics(error)

    expect(diagnostics).toEqual({
      errorCode: 'err_private_route',
      errorName: 'error',
      messageFingerprint:
        '47d5053bbbe4cf94c5f2a5e135adfded01c5f1d3a3b93fdd1229de6f40fa9b5b',
      reasonClass: 'no_route',
      reasonRef: 'reason.public.treasury_payout.no_route',
    })
    expect(JSON.stringify(diagnostics)).not.toContain('private daemon text')
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
