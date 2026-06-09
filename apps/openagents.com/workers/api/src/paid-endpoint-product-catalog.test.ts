import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsPaidEndpointProductCatalogUnsafe,
  OpenAgentsPaidEndpointProductProjection,
  decodeOpenAgentsPaidEndpointProductCatalog,
  openAgentsPaidEndpointProductProjectionHasPrivateMaterial,
  projectOpenAgentsPaidEndpointCatalog,
  projectOpenAgentsPaidEndpointProduct,
} from './paid-endpoint-product-catalog'

const validCatalog = {
  products: [
    {
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
      price: {
        amountMinorUnits: 500,
        asset: 'usd',
        denomination: 'usd_cent',
      },
      productId: 'product.agent_api.proposals.day',
      projectionPolicy: 'agent_visible',
      providerBindingRefs: ['provider_binding.openagents.hosted_api'],
      publicAgentDocRefs: ['docs.api.agent_proposals'],
      publicSummaryRef: 'summary.product.agent_api.proposals.day',
      spendCapHintRefs: ['spend_cap.credits.max_500'],
      status: 'active',
      surface: 'agent_api',
    },
    {
      binding: {
        actionRef: 'action.forum.post_reward',
        kind: 'forum_paid_action',
        method: null,
        pathTemplate: null,
        resourceRef: 'resource.forum.post_reward',
      },
      displayName: 'Forum post reward',
      entitlement: {
        durationSeconds: null,
        kind: 'resource',
        quotaUnits: null,
        scopeRefs: ['entitlement.forum.post_reward.single'],
      },
      internalEconomicsRefs: ['internal_economics.forum.post_reward'],
      operatorNoteRefs: [],
      price: {
        amountMinorUnits: 10_000,
        asset: 'bitcoin',
        denomination: 'bitcoin_millisatoshi',
      },
      productId: 'product.forum.post_reward.single',
      projectionPolicy: 'public_visible',
      providerBindingRefs: ['provider_binding.mdk.hosted'],
      publicAgentDocRefs: ['docs.forum.paid_actions'],
      publicSummaryRef: 'summary.product.forum.post_reward.single',
      spendCapHintRefs: ['spend_cap.bitcoin.small_reward'],
      status: 'active',
      surface: 'forum_paid_action',
    },
    {
      binding: {
        actionRef: 'action.runner.container_recovery',
        kind: 'runner_recovery',
        method: null,
        pathTemplate: null,
        resourceRef: 'resource.runner.container_recovery',
      },
      displayName: 'Runner recovery hold',
      entitlement: {
        durationSeconds: 3_600,
        kind: 'duration',
        quotaUnits: null,
        scopeRefs: ['entitlement.runner.recovery.hour'],
      },
      internalEconomicsRefs: ['internal_economics.runner.container_recovery'],
      operatorNoteRefs: ['operator_note.capacity_review_required'],
      price: {
        amountMinorUnits: 50,
        asset: 'credits',
        denomination: 'credit',
      },
      productId: 'product.runner.recovery.hour',
      projectionPolicy: 'operator_only',
      providerBindingRefs: ['provider_binding.cloudflare.container_runner'],
      publicAgentDocRefs: ['docs.runner.recovery'],
      publicSummaryRef: 'summary.product.runner.recovery.hour',
      spendCapHintRefs: ['spend_cap.credits.max_50'],
      status: 'draft',
      surface: 'runner',
    },
  ],
}

describe('OpenAgents paid endpoint product catalog', () => {
  test('accepts stable products for agent APIs, Forum paid actions, and runner recovery', () => {
    const catalog = decodeOpenAgentsPaidEndpointProductCatalog(validCatalog)

    expect(catalog.products.map(product => product.productId)).toEqual([
      'product.agent_api.proposals.day',
      'product.forum.post_reward.single',
      'product.runner.recovery.hour',
    ])
  })

  test('rejects secret-shaped payment, provider, wallet, and customer material', () => {
    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          {
            ...validCatalog.products[0]!,
            publicAgentDocRefs: ['ben@example.com'],
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)

    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          {
            ...validCatalog.products[1]!,
            mdkAccessToken: 'MDK_ACCESS_TOKEN=secret',
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)

    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          {
            ...validCatalog.products[1]!,
            publicSummaryRef: 'lnbc2500n1rawinvoice',
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)
  })

  test('rejects unstable IDs, duplicate products, invalid prices, and endpoint bindings without paths', () => {
    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          {
            ...validCatalog.products[0]!,
            productId: 'Product With Spaces',
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)

    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          validCatalog.products[0]!,
          {
            ...validCatalog.products[1]!,
            productId: validCatalog.products[0]!.productId,
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)

    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          {
            ...validCatalog.products[0]!,
            price: {
              amountMinorUnits: 1.5,
              asset: 'usd',
              denomination: 'usd_cent',
            },
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)

    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          {
            ...validCatalog.products[0]!,
            binding: {
              ...validCatalog.products[0]!.binding,
              pathTemplate: null,
            },
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)
  })

  test('rejects price denomination mismatches and unsafe paths', () => {
    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          {
            ...validCatalog.products[1]!,
            price: {
              amountMinorUnits: 10_000,
              asset: 'bitcoin',
              denomination: 'credit',
            },
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)

    expect(() =>
      decodeOpenAgentsPaidEndpointProductCatalog({
        products: [
          {
            ...validCatalog.products[0]!,
            binding: {
              ...validCatalog.products[0]!.binding,
              pathTemplate: '/api/agents/proposals?raw_invoice=abc',
            },
          },
        ],
      }),
    ).toThrow(OpenAgentsPaidEndpointProductCatalogUnsafe)
  })

  test('projects public, agent, customer, and operator views without leaking private economics', () => {
    const catalog = decodeOpenAgentsPaidEndpointProductCatalog(validCatalog)
    const agentCatalog = projectOpenAgentsPaidEndpointCatalog(catalog, 'agent')
    const publicCatalog = projectOpenAgentsPaidEndpointCatalog(catalog, 'public')
    const customerCatalog = projectOpenAgentsPaidEndpointCatalog(
      catalog,
      'customer',
    )
    const operatorCatalog = projectOpenAgentsPaidEndpointCatalog(
      catalog,
      'operator',
    )

    expect(agentCatalog).toHaveLength(2)
    expect(publicCatalog).toHaveLength(2)
    expect(customerCatalog).toHaveLength(2)
    expect(operatorCatalog).toHaveLength(3)

    for (const projection of [...agentCatalog, ...publicCatalog, ...customerCatalog]) {
      expect(S.decodeUnknownSync(OpenAgentsPaidEndpointProductProjection)(
        projection,
      )).toEqual(projection)
      expect(projection.internalEconomicsRefs).toEqual([])
      expect(projection.operatorNoteRefs).toEqual([])
      expect(projection.providerBindingRefs).toEqual([])
      expect(openAgentsPaidEndpointProductProjectionHasPrivateMaterial(projection))
        .toBe(false)
    }

    const operatorProjection = operatorCatalog.find(
      product => product.productId === 'product.runner.recovery.hour',
    )

    expect(operatorProjection).toMatchObject({
      internalEconomicsRefs: ['internal_economics.runner.container_recovery'],
      operatorNoteRefs: ['operator_note.capacity_review_required'],
      providerBindingRefs: ['provider_binding.cloudflare.container_runner'],
    })
    expect(openAgentsPaidEndpointProductProjectionHasPrivateMaterial(
      operatorProjection!,
    )).toBe(false)
  })

  test('redacts unsafe refs from projections even if a caller bypasses decoding', () => {
    const catalog = decodeOpenAgentsPaidEndpointProductCatalog(validCatalog)
    const projection = projectOpenAgentsPaidEndpointProduct(
      {
        ...catalog.products[0]!,
        internalEconomicsRefs: [
          'internal_economics.agent_api.proposals',
          'raw_invoice_should_not_escape',
        ],
        providerBindingRefs: [
          'provider_binding.openagents.hosted_api',
          'provider_token_should_not_escape',
        ],
        publicAgentDocRefs: [
          'docs.api.agent_proposals',
          'customer@example.com',
        ],
      },
      'operator',
    )

    expect(projection.internalEconomicsRefs).toEqual([
      'internal_economics.agent_api.proposals',
    ])
    expect(projection.providerBindingRefs).toEqual([
      'provider_binding.openagents.hosted_api',
    ])
    expect(projection.publicAgentDocRefs).toEqual([
      'docs.api.agent_proposals',
    ])
    expect(openAgentsPaidEndpointProductProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })
})
