import { describe, expect, test } from 'vitest'

const smoke = await import('./khala-billing-mpp-proof-smoke.mjs')

describe('Khala billing/MPP proof smoke helpers', () => {
  test('redact scrubs agent, bearer, payment, and Stripe secret-shaped values', () => {
    const cleaned = smoke.redact({
      agent: 'oa_agent_super_secret',
      bearer: 'Bearer oa_agent_another_secret',
      payment: 'Authorization: Payment abc.def.ghi',
      stripe: 'rk_live_hidden_value_123',
    })
    expect(cleaned).not.toContain('oa_agent_super_secret')
    expect(cleaned).not.toContain('oa_agent_another_secret')
    expect(cleaned).not.toContain('abc.def.ghi')
    expect(cleaned).not.toContain('rk_live_hidden_value_123')
    expect(cleaned).toContain('[REDACTED]')
  })

  test('parseArgs defaults to production and honors receipt flags', () => {
    const options = smoke.parseArgs([
      '--json',
      '--require-complete',
      '--stripe-checkout-session-id',
      'cs_test_123',
      '--card-credit-spend-receipt-ref',
      'receipt.inference.card_credit_spend.cs_test_123',
    ])
    expect(options.baseUrl).toBe('https://openagents.com')
    expect(options.json).toBe(true)
    expect(options.requireComplete).toBe(true)
    expect(options.stripeCheckoutSessionId).toBe('cs_test_123')
    expect(options.cardCreditSpendReceiptRef).toBe(
      'receipt.inference.card_credit_spend.cs_test_123',
    )
    expect(() => smoke.parseArgs(['--wat'])).toThrowError(/Unknown argument/)
  })

  test('receipt helpers derive public refs', () => {
    expect(smoke.stripeCheckoutReceiptRefForSession('cs_test_123')).toBe(
      'receipt.billing.stripe_checkout.cs_test_123',
    )
    expect(smoke.cardCreditSpendReceiptRefForSession('cs_test_123')).toBe(
      'receipt.inference.card_credit_spend.cs_test_123',
    )
  })

  test('classifies fail-safe inert MPP endpoint as passing', () => {
    const result = smoke.classifyMppUnauthenticatedResponse({
      body: { error: 'mpp_not_configured' },
      headers: new Headers(),
      status: 503,
    })
    expect(result).toMatchObject({ ok: true, status: 'inert' })
  })

  test('classifies 402 Payment challenge as passing', () => {
    const result = smoke.classifyMppUnauthenticatedResponse({
      body: { status: 402 },
      headers: new Headers({ 'www-authenticate': 'Payment id="pi_123"' }),
      status: 402,
    })
    expect(result).toMatchObject({ ok: true, status: 'armed_402' })
  })

  test('classifies a 200 without payment credential as unsafe', () => {
    const result = smoke.classifyMppUnauthenticatedResponse({
      body: { object: 'chat.completion' },
      headers: new Headers(),
      status: 200,
    })
    expect(result).toMatchObject({ ok: false, status: 'unexpected' })
  })

  test('summary is complete only without failures or skips', () => {
    expect(
      smoke.buildSummary([
        { name: 'a', status: 'PASS', details: {} },
        { name: 'b', status: 'SKIP', details: {} },
      ]).complete,
    ).toBe(false)
    expect(
      smoke.buildSummary([{ name: 'a', status: 'PASS', details: {} }]).complete,
    ).toBe(true)
  })
})
