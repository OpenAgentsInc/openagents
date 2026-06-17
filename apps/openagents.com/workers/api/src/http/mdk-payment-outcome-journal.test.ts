import { describe, expect, test } from 'vitest'

import {
  durableMdkPaymentOutcomeResponse,
  mdkPaymentIdFromPayload,
  mdkPaymentIdFromStatusPath,
  mdkPaymentOutcomeStorageKey,
  mdkTerminalOutcomeFromPayload,
} from './mdk-payment-outcome-journal'

describe('MDK payment outcome journal helpers', () => {
  test('builds a scoped storage key for private payment ids', () => {
    expect(mdkPaymentOutcomeStorageKey('pay_123')).toBe(
      'mdk-payment-outcome:pay_123',
    )
  })

  test('extracts payment ids only from payment-status paths', () => {
    expect(mdkPaymentIdFromStatusPath('/payments/pay_123')).toBe('pay_123')
    expect(mdkPaymentIdFromStatusPath('/payments/pay%2B123')).toBe('pay+123')
    expect(mdkPaymentIdFromStatusPath('/pay')).toBeNull()
    expect(mdkPaymentIdFromStatusPath('/payments/')).toBeNull()
  })

  test('extracts payment ids from response payloads', () => {
    expect(mdkPaymentIdFromPayload({ paymentId: ' pay_123 ' })).toBe('pay_123')
    expect(mdkPaymentIdFromPayload({ paymentId: '' })).toBeNull()
    expect(mdkPaymentIdFromPayload({ paymentId: 123 })).toBeNull()
  })

  test('journals terminal statuses without raw daemon reasons', () => {
    const outcome = mdkTerminalOutcomeFromPayload(
      {
        reason: 'raw daemon failure with private route details',
        reasonRef: 'reason.public.treasury_payout.no_route',
        status: 'failed',
      },
      '2026-06-17T05:00:00.000Z',
    )

    expect(outcome).toEqual({
      observedAt: '2026-06-17T05:00:00.000Z',
      reasonRef: 'reason.public.treasury_payout.no_route',
      status: 'failed',
    })
  })

  test('ignores pending and unsafe reason refs', () => {
    expect(
      mdkTerminalOutcomeFromPayload(
        {
          reasonRef: 'invoice.private.raw',
          status: 'succeeded',
        },
        '2026-06-17T05:00:00.000Z',
      ),
    ).toEqual({
      observedAt: '2026-06-17T05:00:00.000Z',
      reasonRef: null,
      status: 'succeeded',
    })
    expect(
      mdkTerminalOutcomeFromPayload(
        { status: 'pending' },
        '2026-06-17T05:00:00.000Z',
      ),
    ).toBeNull()
  })

  test('renders a no-store internal status response from the journal', async () => {
    const response = durableMdkPaymentOutcomeResponse('pay_123', {
      observedAt: '2026-06-17T05:00:00.000Z',
      reasonRef: null,
      status: 'succeeded',
    })
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body).toEqual({
      journaled: true,
      paymentId: 'pay_123',
      reason: null,
      reasonRef: null,
      status: 'succeeded',
    })
  })
})
