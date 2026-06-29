import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_GENERATED_SITE_PAYMENT_HELPER_MDK_CORE_PARITY_REFS,
  OPENAGENTS_STATIC_SITE_PAYMENT_HELPER_EXAMPLE,
  OPENAGENTS_WFP_SITE_PAYMENT_HELPER_EXAMPLE,
  OpenAgentsGeneratedSitePaymentHelperErrorEnvelope,
  OpenAgentsGeneratedSitePaymentHelperRequestPlan,
  OpenAgentsGeneratedSitePaymentHelperUnsafe,
  assertGeneratedSitePaymentHelperSourceSafe,
  generatedSiteCheckoutIntentPlan,
  generatedSiteCheckoutReturnPlan,
  generatedSiteL402ChallengePlan,
  generatedSiteL402RedemptionPlan,
  generatedSitePaymentDiscoveryPlan,
  generatedSitePaymentHelperErrorEnvelope,
  generatedSitePaymentProofPlan,
} from './site-mdk-generated-helpers'
import {
  OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES,
} from './mdk-core-conformance-fixtures'
import {
  SiteCheckoutIntentRequest,
  SiteL402ChallengeRequest,
  SiteL402RedemptionRequest,
} from './site-commerce-routes'

const siteId = 'site_otec'

const checkoutBody = {
  cancelReturnPath: '/pricing',
  customerDataRefs: ['email'],
  expectedPrice: {
    amountMinorUnits: 2500,
    asset: 'usd',
    denomination: 'usd_cent',
  },
  itemKind: 'product',
  productId: 'consultation_deposit',
  siteVersionId: 'version_site_otec_v2',
  successReturnPath: '/checkout/thanks',
} as const

const challengeBody = {
  entitlementScope: 'action',
  method: 'POST',
  paidActionId: 'generate-report',
  path: '/api/actions/generate-report',
  price: {
    amount: 1200,
    asset: 'sats',
  },
  spendCap: {
    amount: 1200,
    asset: 'sats',
  },
} as const

const redemptionBody = {
  challengeExpiresAt: '2026-06-05T18:05:00.000Z',
  challengeId: 'site_l402_challenge_site_otec_generated_helper',
  credentialId: 'site_l402_credential_generated_helper',
  entitlementScope: 'action',
  method: 'POST',
  paidActionId: 'generate-report',
  path: '/api/actions/generate-report',
  paymentProofRef: 'mdk_payment_proof_generated123',
  price: {
    amount: 1200,
    asset: 'sats',
  },
} as const

describe('generated Site MDK helper contracts', () => {
  test('builds payment discovery, checkout, return, and proof plans with clean URLs', () => {
    const discovery = generatedSitePaymentDiscoveryPlan({ siteId })
    const checkout = generatedSiteCheckoutIntentPlan({
      body: checkoutBody,
      idempotencyKey: 'checkout-helper-1',
      siteId,
    })
    const status = generatedSiteCheckoutReturnPlan({
      checkoutIntentRef: 'site_checkout_intent_site_otec_helper',
      returnAction: 'status',
      siteId,
    })
    const proof = generatedSitePaymentProofPlan({
      checkoutIntentRef: 'site_checkout_intent_site_otec_helper',
      siteId,
    })

    expect(S.decodeUnknownSync(OpenAgentsGeneratedSitePaymentHelperRequestPlan)(discovery))
      .toEqual(discovery)
    expect(S.decodeUnknownSync(SiteCheckoutIntentRequest)(checkout.body))
      .toEqual(checkoutBody)
    expect(checkout).toMatchObject({
      body: checkoutBody,
      headers: {
        'content-type': 'application/json',
        'idempotency-key': 'checkout-helper-1',
      },
      helperKind: 'checkout_intent_create',
      method: 'POST',
      runtime: 'static_site_fetch',
      url: '/api/sites/site_otec/commerce/checkout-intents',
    })
    expect(status).toMatchObject({
      helperKind: 'checkout_return_read',
      method: 'GET',
      url:
        '/api/sites/site_otec/commerce/checkout-returns/site_checkout_intent_site_otec_helper/status',
    })
    expect(proof.url).toBe(
      '/api/sites/site_otec/commerce/payment-proofs/site_checkout_intent_site_otec_helper',
    )
    expect(JSON.stringify([discovery, checkout, status, proof])).not.toMatch(
      /(\?|#|lnbc|lntb|payment_hash|payment_preimage|mnemonic|wallet_secret|MDK_ACCESS_TOKEN)/i,
    )
  })

  test('builds L402 challenge and redemption plans that match route schemas', () => {
    const challenge = generatedSiteL402ChallengePlan({
      body: challengeBody,
      idempotencyKey: 'l402-helper-1',
      runtime: 'wfp_worker_fetch',
      siteId,
    })
    const redemption = generatedSiteL402RedemptionPlan({
      body: redemptionBody,
      idempotencyKey: 'l402-redeem-helper-1',
      runtime: 'wfp_worker_fetch',
      siteId,
    })

    expect(S.decodeUnknownSync(SiteL402ChallengeRequest)(challenge.body))
      .toEqual(challengeBody)
    expect(S.decodeUnknownSync(SiteL402RedemptionRequest)(redemption.body))
      .toEqual(redemptionBody)
    expect(challenge).toMatchObject({
      helperKind: 'l402_challenge_create',
      headers: {
        'content-type': 'application/json',
        'idempotency-key': 'l402-helper-1',
      },
      method: 'POST',
      runtime: 'wfp_worker_fetch',
      url: '/api/sites/site_otec/commerce/l402/challenges',
    })
    expect(redemption.url).toBe(
      '/api/sites/site_otec/commerce/l402/redemptions',
    )
  })

  test('rejects query-state paths, invalid idempotency keys, spend-cap overflow, and secret-shaped source', () => {
    expect(() =>
      generatedSiteCheckoutIntentPlan({
        body: {
          ...checkoutBody,
          successReturnPath: '/checkout/thanks?checkout_id=raw',
        },
        idempotencyKey: 'checkout-helper-2',
        siteId,
      }),
    ).toThrow(OpenAgentsGeneratedSitePaymentHelperUnsafe)
    expect(() =>
      generatedSitePaymentDiscoveryPlan({
        apiBaseUrl: 'https://example.com',
        siteId,
      }),
    ).toThrow(OpenAgentsGeneratedSitePaymentHelperUnsafe)
    expect(() =>
      generatedSiteCheckoutIntentPlan({
        body: checkoutBody,
        idempotencyKey: 'bad key with spaces',
        siteId,
      }),
    ).toThrow(OpenAgentsGeneratedSitePaymentHelperUnsafe)
    expect(() =>
      generatedSiteL402ChallengePlan({
        body: {
          ...challengeBody,
          spendCap: {
            amount: 100,
            asset: 'sats',
          },
        },
        idempotencyKey: 'l402-helper-2',
        siteId,
      }),
    ).toThrow(OpenAgentsGeneratedSitePaymentHelperUnsafe)
    expect(() =>
      assertGeneratedSitePaymentHelperSourceSafe(
        "import '@moneydevkit/lightning-js'; const value = 'MDK_ACCESS_TOKEN'",
      ),
    ).toThrow(OpenAgentsGeneratedSitePaymentHelperUnsafe)
  })

  test('returns redacted helper error envelopes', () => {
    const retryable = generatedSitePaymentHelperErrorEnvelope({
      helperKind: 'l402_challenge_create',
      status: 503,
    })
    const terminal = generatedSitePaymentHelperErrorEnvelope({
      helperKind: 'checkout_intent_create',
      status: 400,
    })

    expect(S.decodeUnknownSync(OpenAgentsGeneratedSitePaymentHelperErrorEnvelope)(retryable))
      .toEqual(retryable)
    expect(retryable.retryable).toBe(true)
    expect(terminal.retryable).toBe(false)
    expect(JSON.stringify([retryable, terminal])).not.toMatch(
      /(lnbc|payment_hash|preimage|mnemonic|wallet_secret|provider_grant)/i,
    )
  })

  test('keeps static and WFP generated helper examples source-safe', () => {
    expect(() =>
      assertGeneratedSitePaymentHelperSourceSafe(
        OPENAGENTS_STATIC_SITE_PAYMENT_HELPER_EXAMPLE,
      ),
    ).not.toThrow()
    expect(() =>
      assertGeneratedSitePaymentHelperSourceSafe(
        OPENAGENTS_WFP_SITE_PAYMENT_HELPER_EXAMPLE,
      ),
    ).not.toThrow()
    expect(OPENAGENTS_STATIC_SITE_PAYMENT_HELPER_EXAMPLE).not.toMatch(
      /(@moneydevkit|lightning-js|MDK_|mnemonic|payment_hash|preimage|wallet)/i,
    )
    expect(OPENAGENTS_WFP_SITE_PAYMENT_HELPER_EXAMPLE).not.toContain('?')
  })

  test('declares parity against implemented MDK core conformance fixtures', () => {
    const fixtureIds = new Set(
      OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES.map(fixture => fixture.id),
    )

    expect(
      OPENAGENTS_GENERATED_SITE_PAYMENT_HELPER_MDK_CORE_PARITY_REFS.every(ref =>
        fixtureIds.has(ref),
      ),
    ).toBe(true)
  })
})
