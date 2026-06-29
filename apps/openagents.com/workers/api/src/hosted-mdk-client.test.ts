import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { BuyerPaymentChallengeRecord } from './buyer-payment-ledger'
import {
  OpenAgentsHostedMdkCheckoutProjection,
  OpenAgentsHostedMdkCheckoutRequest,
  OpenAgentsHostedMdkCheckoutResponse,
  OpenAgentsHostedMdkCheckoutStatusResponse,
  hostedMdkCheckoutRequestFromPaymentChallenge,
  makeFakeOpenAgentsHostedMdkClient,
  makeOpenAgentsHostedMdkRouteClient,
  openAgentsHostedMdkCheckoutProjectionHasPrivateMaterial,
  openAgentsHostedMdkPayloadHasPrivateMaterial,
  projectOpenAgentsHostedMdkCheckoutResponse,
  validateOpenAgentsHostedMdkClientConfig,
} from './hosted-mdk-client'
import type { OpenAgentsL402CredentialPayload } from './l402-credential-service'
import type { OpenAgentsPaidEndpointProductRecord } from './paid-endpoint-product-catalog'

const amount = {
  amountMinorUnits: 25_000,
  asset: 'bitcoin',
  denomination: 'bitcoin_millisatoshi',
} as const

const challenge: BuyerPaymentChallengeRecord = {
  actorRef: 'agent:user_123',
  archivedAt: null,
  challengeRef: 'challenge.mdk_hosted.agent_api.proposals.1',
  createdAt: '2026-06-06T09:00:00.000Z',
  expiresAt: '2026-06-06T09:10:00.000Z',
  id: 'buyer_payment_challenge_mdk_1',
  idempotencyKeyHash: 'hash.mdk.checkout.1',
  metadataRefs: ['metadata.mdk.checkout.safe'],
  method: 'POST',
  ownerUserId: 'user_owner_123',
  path: '/api/agents/proposals',
  price: amount,
  productId: 'product.agent_api.proposals.day',
  publicProjectionJson: '{}',
  requestBodyDigest: 'sha256:request_body_digest',
  spendCap: amount,
  status: 'issued',
  surface: 'agent_api',
}

const product: OpenAgentsPaidEndpointProductRecord = {
  binding: {
    actionRef: null,
    kind: 'agent_api_endpoint',
    method: 'POST',
    pathTemplate: '/api/agents/proposals',
    resourceRef: 'resource.agent_api.proposals',
  },
  displayName: 'Agent proposal intake',
  entitlement: {
    durationSeconds: 86_400,
    kind: 'duration_quota',
    quotaUnits: 20,
    scopeRefs: ['entitlement.agent_api.proposals.day'],
  },
  internalEconomicsRefs: ['internal_economics.agent_api.proposals'],
  operatorNoteRefs: ['operator_note.payment_policy.reviewed'],
  price: amount,
  productId: challenge.productId,
  projectionPolicy: 'agent_visible',
  providerBindingRefs: ['provider_binding.openagents.hosted_mdk'],
  publicAgentDocRefs: ['docs.api.agent_proposals'],
  publicSummaryRef: 'summary.product.agent_api.proposals.day',
  spendCapHintRefs: ['spend_cap.bitcoin.max_25000_msat'],
  status: 'active',
  surface: 'agent_api',
}

const l402Payload: OpenAgentsL402CredentialPayload = {
  amount,
  challengeRef: challenge.challengeRef,
  credentialRef: 'credential.l402.agent_api.proposals.1',
  endpointRef: 'endpoint.agent_api.proposals',
  entitlementScopeRefs: ['entitlement.agent_api.proposals.day'],
  expiresAt: challenge.expiresAt,
  idempotencyKeyHash: challenge.idempotencyKeyHash,
  issuedAt: challenge.createdAt,
  method: 'POST',
  path: challenge.path,
  paymentHashRef: 'payment_hash.redacted.mdk.1',
  productId: challenge.productId,
  replayNonceRef: 'replay_nonce.l402.agent_api.proposals.1',
  requestBodyDigest: challenge.requestBodyDigest,
  version: 'oa-l402-v1',
}

const config = {
  configRef: 'config.mdk_hosted.sandbox',
  credentialBindingRef: 'binding.mdk_hosted.operator',
  environment: 'sandbox',
  providerRef: 'provider.mdk_hosted.fake',
  webhookBindingRef: null,
} as const

const makeRequest = (
  overrides: Partial<
    Parameters<typeof hostedMdkCheckoutRequestFromPaymentChallenge>[0]
  > = {},
) =>
  hostedMdkCheckoutRequestFromPaymentChallenge({
    cancelRef: 'return.cancel.agent_api.proposals',
    challenge,
    customerDataRefs: ['customer_data.email_required'],
    environment: 'sandbox',
    l402Payload,
    metadataRefs: ['metadata.operator.safe_checkout'],
    product,
    returnRef: 'return.success.agent_api.proposals',
    sandbox: true,
    siteRef: null,
    ...overrides,
  })

const runRequest = async (
  overrides: Partial<
    Parameters<typeof hostedMdkCheckoutRequestFromPaymentChallenge>[0]
  > = {},
) => Effect.runPromise(makeRequest(overrides))

describe('OpenAgents hosted MDK client contract', () => {
  test('accepts public binding refs that do not contain secret-shaped words', () => {
    expect(
      validateOpenAgentsHostedMdkClientConfig({
        configRef: 'config.openagents.mdk.cloudflare_container',
        credentialBindingRef:
          'credential_binding.openagents.mdk.cloudflare_worker_binding',
        environment: 'production',
        providerRef: 'provider.openagents.mdk.cloudflare_container',
        webhookBindingRef: null,
      }),
    ).toBeUndefined()
    expect(
      validateOpenAgentsHostedMdkClientConfig({
        configRef: 'config.openagents.mdk.cloudflare_container',
        credentialBindingRef:
          'credential_binding.openagents.mdk.cloudflare_worker_secrets',
        environment: 'production',
        providerRef: 'provider.openagents.mdk.cloudflare_container',
        webhookBindingRef: null,
      }),
    ).toMatchObject({
      detailRef: 'detail.mdk_hosted.config_secret_leakage',
      reason: 'secret_leakage_detected',
    })
  })

  test('creates deterministic fake hosted checkout refs from catalog, ledger, and L402 inputs', async () => {
    const request = await runRequest()
    const client = makeFakeOpenAgentsHostedMdkClient(config)
    const response = await Effect.runPromise(client.createCheckout(request))

    expect(
      S.decodeUnknownSync(OpenAgentsHostedMdkCheckoutRequest)(request),
    ).toEqual(request)
    expect(
      S.decodeUnknownSync(OpenAgentsHostedMdkCheckoutResponse)(response),
    ).toEqual(response)
    expect(request).toMatchObject({
      amount,
      challengeRef: challenge.challengeRef,
      l402CredentialRef: l402Payload.credentialRef,
      mode: 'l402_invoice',
      productId: challenge.productId,
    })
    expect(response).toMatchObject({
      acceptedWorkSettlementAuthority: false,
      checkoutRef:
        'mdk_checkout.product_agent_api_proposals_day_hash_mdk_checkout_1',
      checkoutLaunchPath:
        '/checkout/product_agent_api_proposals_day_hash_mdk_checkout_1',
      checkoutUrlRef:
        'mdk_checkout_url.product_agent_api_proposals_day_hash_mdk_checkout_1',
      environment: 'sandbox',
      invoiceRef:
        'mdk_invoice.redacted.product_agent_api_proposals_day_hash_mdk_checkout_1',
      paymentHashRef:
        'mdk_payment_hash.redacted.product_agent_api_proposals_day_hash_mdk_checkout_1',
      providerPayoutAuthority: false,
      settlementAuthority: 'buyer_payment_evidence_only',
      status: 'created',
    })
    expect(openAgentsHostedMdkPayloadHasPrivateMaterial(request)).toBe(false)
    expect(openAgentsHostedMdkPayloadHasPrivateMaterial(response)).toBe(false)
  })

  test('keeps public projections free of invoice and payment-hash refs', async () => {
    const request = await runRequest()
    const client = makeFakeOpenAgentsHostedMdkClient(config)
    const response = await Effect.runPromise(client.createCheckout(request))
    const publicProjection = projectOpenAgentsHostedMdkCheckoutResponse(
      response,
      'public',
    )
    const agentProjection = projectOpenAgentsHostedMdkCheckoutResponse(
      response,
      'agent',
    )

    expect(
      S.decodeUnknownSync(OpenAgentsHostedMdkCheckoutProjection)(
        publicProjection,
      ),
    ).toEqual(publicProjection)
    expect(publicProjection.invoiceRef).toBe(null)
    expect(publicProjection.paymentHashRef).toBe(null)
    expect(agentProjection.invoiceRef).toBe(response.invoiceRef)
    expect(agentProjection.paymentHashRef).toBe(response.paymentHashRef)
    expect(
      openAgentsHostedMdkCheckoutProjectionHasPrivateMaterial(publicProjection),
    ).toBe(false)
  })

  test('fails when hosted MDK configuration is missing or unavailable', async () => {
    const request = await runRequest()
    const missingConfigClient = makeFakeOpenAgentsHostedMdkClient({
      ...config,
      credentialBindingRef: null,
    })
    const unavailableClient = makeFakeOpenAgentsHostedMdkClient(config, {
      providerAvailable: false,
    })

    await expect(
      Effect.runPromise(missingConfigClient.createCheckout(request)),
    ).rejects.toMatchObject({ reason: 'missing_configuration' })
    await expect(
      Effect.runPromise(unavailableClient.createCheckout(request)),
    ).rejects.toMatchObject({ reason: 'provider_unavailable' })
  })

  test('fails unsupported assets, stale challenges, and provider rejection', async () => {
    const creditAmount = {
      amountMinorUnits: 10,
      asset: 'credits',
      denomination: 'credit',
    } as const
    const creditChallenge = {
      ...challenge,
      price: creditAmount,
      spendCap: creditAmount,
    }
    const creditProduct = {
      ...product,
      price: creditAmount,
    }
    const unsupportedRequest = await runRequest({
      challenge: creditChallenge,
      l402Payload: null,
      product: creditProduct,
    })
    const staleRequest = await runRequest({
      challenge: {
        ...challenge,
        expiresAt: '2026-06-06T08:59:00.000Z',
      },
      l402Payload: null,
    })
    const client = makeFakeOpenAgentsHostedMdkClient(config)
    const rejectingClient = makeFakeOpenAgentsHostedMdkClient(config, {
      rejectCheckout: true,
    })
    const validRequest = await runRequest()

    await expect(
      Effect.runPromise(client.createCheckout(unsupportedRequest)),
    ).rejects.toMatchObject({ reason: 'unsupported_asset_denomination' })
    await expect(
      Effect.runPromise(client.createCheckout(staleRequest)),
    ).rejects.toMatchObject({ reason: 'stale_challenge' })
    await expect(
      Effect.runPromise(rejectingClient.createCheckout(validRequest)),
    ).rejects.toMatchObject({ reason: 'provider_rejected' })
  })

  test('rejects secret-shaped metadata and product challenge mismatches', async () => {
    const unsafeRequest = await runRequest({
      l402Payload: null,
      metadataRefs: ['payment_preimage=secret'],
    })
    const client = makeFakeOpenAgentsHostedMdkClient(config)

    await expect(
      Effect.runPromise(client.createCheckout(unsafeRequest)),
    ).rejects.toMatchObject({ reason: 'secret_leakage_detected' })
    await expect(
      runRequest({
        product: {
          ...product,
          productId: 'product.agent_api.other.day',
        },
      }),
    ).rejects.toMatchObject({ reason: 'provider_rejected' })
  })

  test('creates and looks up live hosted checkout through MDK core route without leaking provider payment material', async () => {
    const calls: Array<{
      body: Record<string, unknown>
      headers: Record<string, string>
      url: string
    }> = []
    const fetch: typeof globalThis.fetch = async (url, init) => {
      const requestBody = JSON.parse(String(init?.body)) as Record<
        string,
        unknown
      >
      calls.push({
        body: requestBody,
        headers: Object.fromEntries(new Headers(init?.headers).entries()),
        url: String(url),
      })

      return new Response(
        JSON.stringify({
          data: {
            createdAt: '2026-06-06T09:01:00.000Z',
            expiresAt: '2026-06-06T09:11:00.000Z',
            id: 'checkout_live_123',
            invoice: {
              invoice: 'lnbc2500n1redacted-provider-fixture',
              paymentHash: 'provider-payment-hash-redacted-by-client',
            },
            sandbox: true,
            status:
              requestBody.handler === 'get_checkout'
                ? 'PAYMENT_RECEIVED'
                : 'PENDING_PAYMENT',
            type: 'AMOUNT',
          },
        }),
        {
          headers: { 'content-type': 'application/json' },
          status: 200,
        },
      )
    }
    const request = await runRequest()
    const client = makeOpenAgentsHostedMdkRouteClient(config, {
      checkoutPathBase: '/checkout',
      fetch,
      nowIso: () => '2026-06-06T09:00:00.000Z',
      routeSecret: 'unit-test-route-secret',
      routeUrl: 'https://mdk-sidecar.test/api/mdk',
    })
    const response = await Effect.runPromise(client.createCheckout(request))
    const status = await Effect.runPromise(
      client.getCheckoutStatus({
        checkoutRef: response.checkoutRef,
        environment: 'sandbox',
        providerRef: config.providerRef,
        sandbox: true,
        siteRef: null,
      }),
    )

    expect(client.implementationState).toBe('live_provider_configured')
    expect(calls).toHaveLength(2)
    expect(calls[0]).toMatchObject({
      body: {
        handler: 'create_checkout',
        params: {
          amount: 25,
          currency: 'SAT',
          requireCustomerData: ['customer_data.email_required'],
          sandbox: true,
          type: 'AMOUNT',
        },
      },
      headers: {
        'x-moneydevkit-webhook-secret': 'unit-test-route-secret',
      },
      url: 'https://mdk-sidecar.test/api/mdk',
    })
    expect(calls[0]?.body).toMatchObject({
      params: {
        metadata: {
          challenge_ref: request.challengeRef,
          idempotency_key_hash: request.idempotencyKeyHash,
          product_id: request.productId,
        },
      },
    })
    expect(calls[1]?.body).toEqual({
      checkoutId: 'checkout_live_123',
      handler: 'get_checkout',
    })
    expect(
      S.decodeUnknownSync(OpenAgentsHostedMdkCheckoutResponse)(response),
    ).toEqual(response)
    expect(
      S.decodeUnknownSync(OpenAgentsHostedMdkCheckoutStatusResponse)(status),
    ).toEqual(status)
    expect(response).toMatchObject({
      checkoutLaunchPath: '/checkout/checkout_live_123',
      checkoutRef: 'mdk_checkout.checkout_live_123',
      checkoutUrlRef: 'mdk_checkout_url.checkout_live_123',
      invoiceRef: 'mdk_invoice.redacted.checkout_live_123',
      paymentHashRef: 'mdk_payment_hash.redacted.checkout_live_123',
      providerPayoutAuthority: false,
      status: 'pending_payment',
    })
    expect(status).toMatchObject({
      checkoutRef: response.checkoutRef,
      providerRef: config.providerRef,
      status: 'payment_received',
    })
    expect(JSON.stringify(response)).not.toMatch(
      /(lnbc|provider-payment-hash|route-secret|mnemonic|payment_preimage)/i,
    )
    expect(openAgentsHostedMdkPayloadHasPrivateMaterial(response)).toBe(false)
  })

  test('omits sandbox from production MDK route checkout payloads', async () => {
    const calls: Array<Record<string, unknown>> = []
    const fetch: typeof globalThis.fetch = async (_url, init) => {
      const requestBody = JSON.parse(String(init?.body)) as Record<
        string,
        unknown
      >
      calls.push(requestBody)

      return new Response(
        JSON.stringify({
          data: {
            createdAt: '2026-06-06T09:01:00.000Z',
            expiresAt: '2026-06-06T09:11:00.000Z',
            id: 'checkout_live_production_123',
            status: 'PENDING_PAYMENT',
            type: 'AMOUNT',
          },
        }),
        {
          headers: { 'content-type': 'application/json' },
          status: 200,
        },
      )
    }
    const productionConfig = {
      ...config,
      environment: 'production' as const,
    }
    const request = await runRequest({
      environment: 'production',
      sandbox: false,
    })
    const client = makeOpenAgentsHostedMdkRouteClient(productionConfig, {
      checkoutPathBase: '/checkout',
      fetch,
      nowIso: () => '2026-06-06T09:00:00.000Z',
      routeSecret: 'unit-test-route-secret',
      routeUrl: 'https://mdk-sidecar.test/api/mdk',
    })

    await Effect.runPromise(client.createCheckout(request))

    expect(calls[0]).toMatchObject({
      handler: 'create_checkout',
      params: {
        amount: 25,
        currency: 'SAT',
        type: 'AMOUNT',
      },
    })
    expect(calls[0]?.params).not.toHaveProperty('sandbox')
  })

  test('maps MDK route failures and missing runtime config to typed client errors', async () => {
    const request = await runRequest()
    const rejectingClient = makeOpenAgentsHostedMdkRouteClient(config, {
      checkoutPathBase: '/checkout',
      fetch: async () =>
        new Response(JSON.stringify({ error: 'Invalid checkout params' }), {
          status: 400,
        }),
      nowIso: () => '2026-06-06T09:00:00.000Z',
      routeSecret: 'unit-test-route-secret',
      routeUrl: 'https://mdk-sidecar.test/api/mdk',
    })
    const missingRuntimeClient = makeOpenAgentsHostedMdkRouteClient(config, {
      checkoutPathBase: '/checkout',
      fetch: async () => new Response('{}'),
      nowIso: () => '2026-06-06T09:00:00.000Z',
      routeSecret: '',
      routeUrl: '',
    })

    await expect(
      Effect.runPromise(rejectingClient.createCheckout(request)),
    ).rejects.toMatchObject({ reason: 'provider_rejected' })
    await expect(
      Effect.runPromise(missingRuntimeClient.createCheckout(request)),
    ).rejects.toMatchObject({ reason: 'missing_configuration' })
  })

  test('bounds hung MDK route calls with a typed provider-unavailable timeout', async () => {
    const request = await runRequest()
    const hangingClient = makeOpenAgentsHostedMdkRouteClient(config, {
      checkoutPathBase: '/checkout',
      fetch: async () => new Promise<Response>(() => {}),
      nowIso: () => '2026-06-06T09:00:00.000Z',
      routeSecret: 'unit-test-route-secret',
      routeTimeoutMs: 1,
      routeUrl: 'https://mdk-sidecar.test/api/mdk',
    })

    await expect(
      Effect.runPromise(hangingClient.createCheckout(request)),
    ).rejects.toMatchObject({
      detailRef: 'detail.mdk_hosted.route_timeout',
      reason: 'provider_unavailable',
    })
  })
})
