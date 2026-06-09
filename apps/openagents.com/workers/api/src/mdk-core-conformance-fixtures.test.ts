import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  credentialFailureL402Response,
  l402ResponseContractHasPrivateMaterial,
} from './l402-response-contract'
import {
  l402PayloadFromBuyerPaymentChallenge,
  makeOpenAgentsL402HmacSigningBoundary,
  mintOpenAgentsL402Credential,
  verifyOpenAgentsL402Credential,
} from './l402-credential-service'
import {
  makeFakeOpenAgentsHostedMdkClient,
  hostedMdkCheckoutRequestFromPaymentChallenge,
} from './hosted-mdk-client'
import {
  OpenAgentsMdkCoreConformanceFixture,
  OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES,
  OPENAGENTS_MDK_CORE_FIXTURE_AMOUNT_CHECKOUT,
  OPENAGENTS_MDK_CORE_FIXTURE_BITCOIN_AMOUNT,
  OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE,
  OPENAGENTS_MDK_CORE_FIXTURE_PRODUCT,
  OPENAGENTS_MDK_CORE_FIXTURE_PRODUCT_CHECKOUT,
  openAgentsMdkCoreConformanceFixtureHasPrivateMaterial,
} from './mdk-core-conformance-fixtures'
import {
  makeOpenAgentsMdkCoreCheckoutSigner,
  openAgentsMdkCoreCheckoutProjectionHasPrivateMaterial,
  prepareOpenAgentsMdkCoreCheckout,
  projectOpenAgentsMdkCorePreparedCheckout,
  resolveOpenAgentsMdkCoreCheckoutRoute,
  signOpenAgentsMdkCoreCheckoutUrl,
  validateOpenAgentsMdkCoreMetadata,
  verifyOpenAgentsMdkCoreCheckoutUrl,
} from './mdk-core-checkout-contract'

const expectedKinds = [
  'amount_checkout_creation',
  'customer_field_normalization',
  'error_envelope',
  'l402_token_parsing',
  'metadata_limits',
  'preimage_proof_boundary',
  'price_recheck',
  'product_checkout_creation',
  'safe_return_path',
  'sandbox_flag',
  'signed_checkout_url',
  'stale_challenge',
] as const

const mdkConfig = {
  configRef: 'config.mdk_hosted.fixture',
  credentialBindingRef: 'binding.mdk_hosted.fixture',
  environment: 'sandbox',
  providerRef: 'provider.mdk_hosted.fake',
  webhookBindingRef: null,
} as const

const l402Expected = {
  amount: OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE.price,
  challengeRef: OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE.challengeRef,
  endpointRef: 'endpoint.agent_api.proposals',
  entitlementScopeRefs: ['entitlement.agent_api.proposals.day'],
  method: OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE.method,
  nowIso: '2026-06-06T09:05:00.000Z',
  path: OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE.path,
  paymentProofRef: 'payment_proof.redacted.fixture.1',
  productId: OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE.productId,
  requestBodyDigest:
    OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE.requestBodyDigest,
  requirePaymentProof: true,
} as const

describe('OpenAgents MDK core conformance fixtures', () => {
  test('catalogs every required fixture and keeps fixture data public-safe', () => {
    expect(OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES.map(
      fixture => fixture.kind,
    ).sort()).toEqual([...expectedKinds].sort())

    const decodedFixtures = OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES.map(
      fixture => S.decodeUnknownSync(OpenAgentsMdkCoreConformanceFixture)(
        fixture,
      ),
    )

    expect(decodedFixtures).toEqual([
      ...OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES,
    ])
    expect(OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES.every(fixture =>
      fixture.assertionRefs.length > 0 && fixture.sourceRefs.length > 0,
    )).toBe(true)
    expect(OPENAGENTS_MDK_CORE_CONFORMANCE_FIXTURES.every(fixture =>
      !openAgentsMdkCoreConformanceFixtureHasPrivateMaterial(fixture),
    )).toBe(true)

    expect(openAgentsMdkCoreConformanceFixtureHasPrivateMaterial(
      OPENAGENTS_MDK_CORE_FIXTURE_AMOUNT_CHECKOUT,
    )).toBe(false)
    expect(openAgentsMdkCoreConformanceFixtureHasPrivateMaterial(
      OPENAGENTS_MDK_CORE_FIXTURE_PRODUCT_CHECKOUT,
    )).toBe(false)
  })

  test('executes human amount and product checkout creation fixtures', async () => {
    const amountPrepared = await Effect.runPromise(
      prepareOpenAgentsMdkCoreCheckout(
        OPENAGENTS_MDK_CORE_FIXTURE_AMOUNT_CHECKOUT,
      ),
    )
    const productPrepared = await Effect.runPromise(
      prepareOpenAgentsMdkCoreCheckout(
        OPENAGENTS_MDK_CORE_FIXTURE_PRODUCT_CHECKOUT,
      ),
    )
    const amountProjection = projectOpenAgentsMdkCorePreparedCheckout(
      amountPrepared,
      'public',
    )
    const productProjection = projectOpenAgentsMdkCorePreparedCheckout(
      productPrepared,
      'agent',
    )

    expect(amountPrepared).toMatchObject({
      checkoutPath: '/checkout/fixture',
      customerFieldKeys: ['externalId', 'name'],
      mode: 'amount',
      sandbox: true,
    })
    expect(productPrepared).toMatchObject({
      checkoutPath: '/checkout/product-fixture',
      mode: 'product',
      productId: 'product.site_checkout.fixture_report',
      sandbox: true,
    })
    expect(amountProjection.customerValuesRedacted).toBe(true)
    expect(productProjection.metadataValuesRedacted).toBe(true)
    expect(openAgentsMdkCoreCheckoutProjectionHasPrivateMaterial(
      amountProjection,
    )).toBe(false)
    expect(openAgentsMdkCoreCheckoutProjectionHasPrivateMaterial(
      productProjection,
    )).toBe(false)
  })

  test('exercises route, metadata, safe-path, sandbox, and signed-url fixtures', async () => {
    const signer = await makeOpenAgentsMdkCoreCheckoutSigner({
      secretKeyMaterial: 'unit-test-mdk-core-conformance-secret',
      signerRef: 'signer.mdk_core.conformance',
    })
    const signed = await Effect.runPromise(signOpenAgentsMdkCoreCheckoutUrl(
      {
        cancelRef: 'checkout.cancel.fixture',
        checkoutPath: '/checkout/fixture?ignored=true',
        checkoutRef: 'mdk_checkout.fixture.1',
        expiresAt: '2026-06-06T09:10:00.000Z',
        issuedAt: '2026-06-06T09:00:00.000Z',
        returnRef: 'checkout.success.fixture',
        sandbox: true,
      },
      signer,
    ))

    await expect(Effect.runPromise(resolveOpenAgentsMdkCoreCheckoutRoute({
      handler: 'create_checkout',
    }))).resolves.toBe('create_checkout')
    await expect(Effect.runPromise(resolveOpenAgentsMdkCoreCheckoutRoute({
      handler: 'pay_invoice',
    }))).rejects.toMatchObject({ reason: 'invalid_route' })
    expect(validateOpenAgentsMdkCoreMetadata({
      source_ref: 'fixture_checkout',
    })).toBe(undefined)
    expect(validateOpenAgentsMdkCoreMetadata(
      Object.fromEntries(Array.from({ length: 51 }, (_, index) => [
        `key_${index}`,
        'value',
      ])),
    )).toMatchObject({ reason: 'metadata_invalid' })
    expect(signed.checkoutPath).toBe('/checkout/fixture')
    expect(signed.signedPath).toContain('sandbox=true')
    await expect(Effect.runPromise(verifyOpenAgentsMdkCoreCheckoutUrl(
      signed.signedPath,
      signer,
    ))).resolves.toBe(true)
  })

  test('exercises hosted MDK stale challenge and typed error envelope fixtures', async () => {
    const staleChallenge = {
      ...OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE,
      expiresAt: '2026-06-06T08:59:00.000Z',
    }
    const request = await Effect.runPromise(
      hostedMdkCheckoutRequestFromPaymentChallenge({
        cancelRef: 'checkout.cancel.fixture',
        challenge: staleChallenge,
        environment: 'sandbox',
        l402Payload: null,
        metadataRefs: ['metadata.fixture.safe'],
        product: OPENAGENTS_MDK_CORE_FIXTURE_PRODUCT,
        returnRef: 'checkout.success.fixture',
        sandbox: true,
        siteRef: null,
      }),
    )
    const client = makeFakeOpenAgentsHostedMdkClient(mdkConfig)

    await expect(Effect.runPromise(client.createCheckout(request)))
      .rejects.toMatchObject({ reason: 'stale_challenge' })

    const response = credentialFailureL402Response({
      audience: 'agent',
      result: {
        credentialRef: 'credential.l402.fixture.1',
        payload: null,
        reasonRef: 'reason.l402_credential.proof_missing',
        status: 'proof_missing',
      },
    })

    expect(response).toMatchObject({
      errorKind: 'proof_missing',
      statusCode: 401,
    })
    expect(l402ResponseContractHasPrivateMaterial(response)).toBe(false)
  })

  test('exercises L402 token parsing, price re-check, and proof boundaries', async () => {
    const signer = await makeOpenAgentsL402HmacSigningBoundary({
      secretKeyMaterial: 'unit-test-l402-conformance-secret',
      signerRef: 'signer.l402.conformance',
    })
    const payload = l402PayloadFromBuyerPaymentChallenge({
      challenge: OPENAGENTS_MDK_CORE_FIXTURE_CHALLENGE,
      credentialRef: 'credential.l402.fixture.1',
      endpointRef: l402Expected.endpointRef,
      entitlementScopeRefs: l402Expected.entitlementScopeRefs,
      issuedAt: '2026-06-06T09:00:00.000Z',
      paymentHashRef: 'payment_hash.redacted.fixture.1',
      replayNonceRef: 'replay_nonce.l402.fixture.1',
    })
    const envelope = await mintOpenAgentsL402Credential(payload, signer)
    const valid = await verifyOpenAgentsL402Credential(
      envelope.credential,
      signer,
      l402Expected,
    )
    const amountMismatch = await verifyOpenAgentsL402Credential(
      envelope.credential,
      signer,
      {
        ...l402Expected,
        amount: {
          ...OPENAGENTS_MDK_CORE_FIXTURE_BITCOIN_AMOUNT,
          amountMinorUnits:
            OPENAGENTS_MDK_CORE_FIXTURE_BITCOIN_AMOUNT.amountMinorUnits + 1,
        },
      },
    )
    const proofMissing = await verifyOpenAgentsL402Credential(
      envelope.credential,
      signer,
      {
        ...l402Expected,
        paymentProofRef: null,
      },
    )
    const malformed = await verifyOpenAgentsL402Credential(
      'not-a-real-credential',
      signer,
      l402Expected,
    )

    expect(valid.status).toBe('valid')
    expect(amountMismatch.status).toBe('amount_mismatch')
    expect(proofMissing.status).toBe('proof_missing')
    expect(malformed.status).toBe('malformed')
    expect(JSON.stringify(envelope.payload)).not.toMatch(
      /(lnbc|payment_preimage=|mnemonic|mdk_access_token)/i,
    )
  })
})
