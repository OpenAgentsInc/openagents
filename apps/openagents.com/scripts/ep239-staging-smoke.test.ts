import { describe, expect, test } from 'vitest'

const smoke = await import('./ep239-staging-smoke.mjs')

describe('Ep239 staging funded-loop smoke', () => {
  test('redact scrubs agent tokens, bearer headers, and stripe-style secrets', () => {
    const dirty = {
      authorization: 'Bearer oa_agent_super_secret_value_123',
      note: 'token oa_agent_abcDEF-_123 must not appear',
      stripe: 'sk-test_abcdEFGH1234',
    }
    const cleaned = smoke.redact(dirty)
    expect(cleaned).not.toContain('oa_agent_super_secret_value_123')
    expect(cleaned).not.toContain('oa_agent_abcDEF-_123')
    expect(cleaned).not.toContain('sk-test_abcdEFGH1234')
    expect(cleaned).toContain('[REDACTED]')
  })

  test('redact leaves non-secret refs (chatcmpl, receipt) intact for dereference', () => {
    const ref = {
      chatcmplId: 'chatcmpl_abc123',
      receiptRef: 'receipt.inference.charge.chatcmpl_abc123',
    }
    const cleaned = smoke.redact(ref)
    expect(cleaned).toContain('chatcmpl_abc123')
    expect(cleaned).toContain('receipt.inference.charge.chatcmpl_abc123')
  })

  test('presenceTag never echoes the value, only a length fingerprint', () => {
    const tag = smoke.presenceTag('oa_agent_secret')
    expect(tag).not.toContain('oa_agent_secret')
    expect(tag).toMatch(/^present\(len=\d+\)$/)
    expect(smoke.presenceTag(undefined)).toBe('absent')
    expect(smoke.presenceTag('')).toBe('absent')
  })

  test('assertStagingHost rejects production hosts', () => {
    expect(() =>
      smoke.assertStagingHost('https://openagents.com'),
    ).toThrowError(/production/i)
    expect(() =>
      smoke.assertStagingHost('https://auth.openagents.com/callback'),
    ).toThrowError(/production/i)
    expect(() =>
      smoke.assertStagingHost('https://www.openagents.com'),
    ).toThrowError(/production/i)
  })

  test('assertStagingHost accepts the isolated staging Worker host', () => {
    expect(
      smoke.assertStagingHost(
        'https://openagents-staging.openagents.workers.dev',
      ),
    ).toBe('openagents-staging.openagents.workers.dev')
  })

  test('parseArgs defaults to the staging Worker base url', () => {
    const options = smoke.parseArgs([])
    expect(options.baseUrl).toContain('openagents-staging')
    expect(options.json).toBe(false)
    expect(options.requireComplete).toBe(false)
    expect(options.help).toBe(false)
  })

  test('parseArgs honors --base-url, --json, --require-complete, Stripe receipt flags, and --help; rejects unknown flags', () => {
    const options = smoke.parseArgs([
      '--base-url',
      'https://x.example',
      '--json',
      '--require-complete',
      '--stripe-checkout-session-id',
      'cs_test_123',
      '--stripe-checkout-receipt-ref',
      'receipt.billing.stripe_checkout.cs_test_456',
      '--card-credit-spend-session-id',
      'cs_test_spend_123',
      '--card-credit-spend-receipt-ref',
      'receipt.inference.card_credit_spend.cs_test_spend_456',
      '--referral-payout-receipt-ref',
      'receipt.site_referral_payout.staging_test.settled_1',
    ])
    expect(options.baseUrl).toBe('https://x.example')
    expect(options.json).toBe(true)
    expect(options.requireComplete).toBe(true)
    expect(options.stripeCheckoutSessionId).toBe('cs_test_123')
    expect(options.stripeCheckoutReceiptRef).toBe(
      'receipt.billing.stripe_checkout.cs_test_456',
    )
    expect(options.cardCreditSpendSessionId).toBe('cs_test_spend_123')
    expect(options.cardCreditSpendReceiptRef).toBe(
      'receipt.inference.card_credit_spend.cs_test_spend_456',
    )
    expect(options.referralPayoutReceiptRef).toBe(
      'receipt.site_referral_payout.staging_test.settled_1',
    )
    expect(smoke.parseArgs(['--help']).help).toBe(true)
    expect(() => smoke.parseArgs(['--nope'])).toThrowError(/Unknown argument/)
  })

  test('stripeCheckoutReceiptRefForSession derives the public receipt ref', () => {
    expect(smoke.stripeCheckoutReceiptRefForSession('cs_test_123')).toBe(
      'receipt.billing.stripe_checkout.cs_test_123',
    )
  })

  test('cardCreditSpendReceiptRefForSession derives the composite public receipt ref', () => {
    expect(smoke.cardCreditSpendReceiptRefForSession('cs_test_123')).toBe(
      'receipt.inference.card_credit_spend.cs_test_123',
    )
  })

  test('buildAcceptanceGateSummary keeps #5520 incomplete when Stripe/referral receipts are unproven', () => {
    const summary = smoke.buildAcceptanceGateSummary({
      stripeTestCardCheckoutProven: false,
      operatorGrantProven: true,
      operatorGrantRefs: ['receipt.inference.usd_credit_grant.ep239-stg-1'],
      meteredSpendProven: false,
      referralAccrualProven: false,
      newSurfacesProven: true,
      promiseHonestyProven: true,
      promiseHonestyRefs: ['registry:2026-06-20.50'],
    })

    expect(summary.complete).toBe(false)
    expect(
      summary.gates.find(g => g.id === 'operator_grant_to_credit_bridge')
        ?.status,
    ).toBe('PROVEN')
    expect(
      summary.gates.find(g => g.id === 'card_to_credit_stripe_test')?.status,
    ).toBe('UNPROVEN')
    expect(
      summary.gates.find(g => g.id === 'referral_accrual_and_test_settlement')
        ?.blockerRefs,
    ).toContain('blocker.ep239_phase1.referral_test_payout_settlement_unproven')
  })

  test('buildAcceptanceGateSummary marks complete only when every named gate is proven', () => {
    const summary = smoke.buildAcceptanceGateSummary({
      stripeTestCardCheckoutProven: true,
      stripeTestCardCheckoutRefs: ['evidence.stripe_checkout_paid.cs_test_123'],
      operatorGrantProven: true,
      operatorGrantRefs: ['receipt.inference.usd_credit_grant.ep239-stg-1'],
      meteredSpendProven: true,
      meteredSpendRefs: [
        'receipt.inference.charge.chatcmpl_123',
        'receipt.inference.card_credit_spend.cs_test_123',
      ],
      referralAccrualProven: true,
      referralAccrualRefs: [
        'receipt.site_referral_payout.staging_test.settled_1',
      ],
      newSurfacesProven: true,
      newSurfaceRefs: ['ftjob_123', 'sbx_123'],
      promiseHonestyProven: true,
      promiseHonestyRefs: ['registry:2026-06-20.50'],
    })

    expect(summary.complete).toBe(true)
    expect(summary.gates.every(g => g.status === 'PROVEN')).toBe(true)
  })
})
