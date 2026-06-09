import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsMdkCoreHostedCheckoutPlan,
  OpenAgentsMdkCorePreparedCheckout,
  OpenAgentsMdkCorePreparedCheckoutProjection,
  OpenAgentsMdkCoreSignedCheckoutUrl,
  makeOpenAgentsMdkCoreCheckoutSigner,
  normalizeOpenAgentsMdkCoreCustomer,
  openAgentsMdkCoreCheckoutProjectionHasPrivateMaterial,
  prepareOpenAgentsMdkCoreCheckout,
  projectOpenAgentsMdkCorePreparedCheckout,
  resolveOpenAgentsMdkCoreCheckoutRoute,
  sanitizeOpenAgentsMdkCoreCheckoutPath,
  signOpenAgentsMdkCoreCheckoutUrl,
  validateOpenAgentsMdkCoreMetadata,
  verifyOpenAgentsMdkCoreCheckoutUrl,
} from './mdk-core-checkout-contract'

const amount = {
  amountMinorUnits: 2500,
  asset: 'usd',
  denomination: 'usd_cent',
} as const

const prepare = (input: Parameters<
  typeof prepareOpenAgentsMdkCoreCheckout
>[0]) => Effect.runPromise(prepareOpenAgentsMdkCoreCheckout(input))

describe('OpenAgents MDK core checkout contract', () => {
  test('resolves route names from MDK-style handler, route, or target keys', async () => {
    await expect(Effect.runPromise(resolveOpenAgentsMdkCoreCheckoutRoute({
      handler: 'CREATE_CHECKOUT',
    }))).resolves.toBe('create_checkout')
    await expect(Effect.runPromise(resolveOpenAgentsMdkCoreCheckoutRoute({
      route: 'get_checkout',
    }))).resolves.toBe('get_checkout')
    await expect(Effect.runPromise(resolveOpenAgentsMdkCoreCheckoutRoute({
      target: 'confirm_checkout',
    }))).resolves.toBe('confirm_checkout')
    await expect(Effect.runPromise(resolveOpenAgentsMdkCoreCheckoutRoute({
      handler: 'pay_invoice',
    }))).rejects.toMatchObject({ reason: 'invalid_route' })
  })

  test('validates metadata limits and secret-shaped values', () => {
    expect(validateOpenAgentsMdkCoreMetadata({
      campaign: 'autopilot-sites',
      source_ref: 'source.public.ref',
    })).toBe(undefined)
    expect(validateOpenAgentsMdkCoreMetadata({
      customer_email: 'ben@example.com',
    })).toMatchObject({ reason: 'secret_leakage_detected' })
    expect(validateOpenAgentsMdkCoreMetadata(
      Object.fromEntries(Array.from({ length: 51 }, (_, index) => [
        `key_${index}`,
        'value',
      ])),
    )).toMatchObject({ reason: 'metadata_invalid' })
  })

  test('normalizes customer fields without exposing them in projections', async () => {
    const customer = normalizeOpenAgentsMdkCoreCustomer({
      'Billing Address': '  Austin  ',
      'External ID': '  ext_123  ',
      email: '',
      name: 'Ben',
    })
    const prepared = await prepare({
      amount,
      cancelRef: 'checkout.cancel.agent_api',
      checkoutPath: '/pay?step=review',
      customer,
      metadata: {
        source_ref: 'site_builder',
      },
      mode: 'amount',
      requireCustomerData: ['Billing Address', 'External ID', 'email'],
      returnRef: 'checkout.success.agent_api',
      sandbox: true,
      titleRef: 'title.agent_api.proposal',
    })
    const projection = projectOpenAgentsMdkCorePreparedCheckout(
      prepared,
      'public',
    )

    expect(customer).toEqual({
      billingAddress: 'Austin',
      externalId: 'ext_123',
      name: 'Ben',
    })
    expect(S.decodeUnknownSync(OpenAgentsMdkCorePreparedCheckout)(prepared))
      .toEqual(prepared)
    expect(prepared.checkoutPath).toBe('/pay')
    expect(projection).toMatchObject({
      customerFieldKeys: ['billingAddress', 'externalId', 'name'],
      customerValuesRedacted: true,
      metadataKeys: ['source_ref'],
      metadataValuesRedacted: true,
      requireCustomerData: ['billingAddress', 'externalId', 'email'],
    })
    expect(JSON.stringify(projection)).not.toContain('Austin')
    expect(S.decodeUnknownSync(OpenAgentsMdkCorePreparedCheckoutProjection)(
      projection,
    )).toEqual(projection)
    expect(openAgentsMdkCoreCheckoutProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('sanitizes checkout paths using MDK-compatible relative-path rules', () => {
    expect(sanitizeOpenAgentsMdkCoreCheckoutPath(undefined)).toBe('/checkout')
    expect(sanitizeOpenAgentsMdkCoreCheckoutPath('https://evil.test/pay'))
      .toBe('/checkout')
    expect(sanitizeOpenAgentsMdkCoreCheckoutPath('//evil.test/pay'))
      .toBe('/checkout')
    expect(sanitizeOpenAgentsMdkCoreCheckoutPath('/checkout/one?x=1#frag'))
      .toBe('/checkout/one')
  })

  test('signs and verifies checkout paths with Worker-compatible Web Crypto', async () => {
    const signer = await makeOpenAgentsMdkCoreCheckoutSigner({
      secretKeyMaterial: 'unit-test-mdk-core-secret',
      signerRef: 'signer.mdk_core.unit_test',
    })
    const signed = await Effect.runPromise(signOpenAgentsMdkCoreCheckoutUrl(
      {
        cancelRef: 'checkout.cancel.agent_api',
        checkoutPath: '/checkout/agent-api',
        checkoutRef: 'mdk_checkout.agent_api.proposals.1',
        expiresAt: '2026-06-06T09:10:00.000Z',
        issuedAt: '2026-06-06T09:00:00.000Z',
        returnRef: 'checkout.success.agent_api',
        sandbox: true,
      },
      signer,
    ))

    expect(S.decodeUnknownSync(OpenAgentsMdkCoreSignedCheckoutUrl)(signed))
      .toEqual(signed)
    expect(signed.signedPath).toContain('/checkout/agent-api?')
    expect(signed.signedPath).toContain('signature=')
    await expect(Effect.runPromise(verifyOpenAgentsMdkCoreCheckoutUrl(
      signed.signedPath,
      signer,
    ))).resolves.toBe(true)
    await expect(Effect.runPromise(verifyOpenAgentsMdkCoreCheckoutUrl(
      signed.signedPath.replace('sandbox=true', 'sandbox=false'),
      signer,
    ))).resolves.toBe(false)
    await expect(Effect.runPromise(verifyOpenAgentsMdkCoreCheckoutUrl(
      '/checkout/agent-api?action=createCheckout',
      signer,
    ))).rejects.toMatchObject({ reason: 'signature_missing' })
  })

  test('models the hosted checkout plan bridge without leaking MDK internals', async () => {
    const preparedCheckout = await prepare({
      mode: 'product',
      cancelRef: 'checkout.cancel.site',
      checkoutPath: '/checkout/product',
      metadata: {
        source_ref: 'site.public.product',
      },
      productId: 'product.site_checkout.report',
      productPriceRef: 'price.product.report.standard',
      returnRef: 'checkout.success.site',
      sandbox: true,
    })
    const signer = await makeOpenAgentsMdkCoreCheckoutSigner({
      secretKeyMaterial: 'unit-test-mdk-core-secret',
      signerRef: 'signer.mdk_core.unit_test',
    })
    const signedCheckoutUrl = await Effect.runPromise(
      signOpenAgentsMdkCoreCheckoutUrl(
        {
          cancelRef: 'checkout.cancel.site',
          checkoutPath: preparedCheckout.checkoutPath,
          checkoutRef: 'mdk_checkout.product.site.report.1',
          expiresAt: '2026-06-06T09:10:00.000Z',
          issuedAt: '2026-06-06T09:00:00.000Z',
          returnRef: 'checkout.success.site',
          sandbox: true,
        },
        signer,
      ),
    )
    const hostedRequest = {
      amount: {
        amountMinorUnits: 2500,
        asset: 'usd',
        denomination: 'usd_cent',
      },
      cancelRef: 'checkout.cancel.site',
      challengeExpiresAt: '2026-06-06T09:10:00.000Z',
      challengeRef: 'challenge.site_checkout.report.1',
      customerDataRefs: [],
      environment: 'sandbox',
      idempotencyKeyHash: 'hash.site.checkout.1',
      l402CredentialRef: null,
      metadataRefs: ['metadata.mdk_core.source_ref'],
      mode: 'product',
      productId: 'product.site_checkout.report',
      returnRef: 'checkout.success.site',
      sandbox: true,
      siteRef: 'site.report',
    } as const
    const plan = {
      hostedRequest,
      l402Payload: null,
      preparedCheckout,
      signedCheckoutUrl,
    }

    expect(S.decodeUnknownSync(OpenAgentsMdkCoreHostedCheckoutPlan)(plan))
      .toEqual(plan)
    expect(JSON.stringify(plan)).not.toMatch(
      /(lnbc|preimage|mnemonic|mdk_access_token|wallet_state)/i,
    )
  })
})
