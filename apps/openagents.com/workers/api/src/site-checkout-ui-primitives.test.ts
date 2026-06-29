import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsSiteCheckoutUiPrimitiveContract,
  OpenAgentsSiteCheckoutUiPrimitiveProjection,
  OpenAgentsSiteCheckoutUiPrimitiveUnsafe,
  decodeOpenAgentsSiteCheckoutUiPrimitiveContract,
  openAgentsSiteCheckoutUiPrimitiveHasPrivateMaterial,
  projectOpenAgentsSiteCheckoutUiPrimitives,
  siteCheckoutUiPrimitivesFromCatalog,
} from './site-checkout-ui-primitives'
import { sitePaymentCatalogFromManifest } from './site-payment-catalog'

const catalog = sitePaymentCatalogFromManifest({
  createdAt: '2026-06-06T10:00:00.000Z',
  deploymentId: 'deployment.site_otec.v3',
  manifest: {
    payments: {
      agentReadable: true,
      enabled: true,
      metadataRefs: ['metadata.site_payment.site_otec.v3'],
      paidActions: [
        {
          actionRef: 'action.report.download',
          agentReadable: true,
          checkoutPath: '/checkout/download-report',
          customerDataRequirements: [],
          displayRef: 'display.download_report',
          entitlementScope: 'action',
          id: 'download_report',
          metadataRefs: ['metadata.action.download_report'],
          method: 'GET',
          path: '/api/actions/download-report',
          price: {
            amountMinorUnits: 25_000,
            asset: 'bitcoin',
            denomination: 'bitcoin_millisatoshi',
          },
          publicProjectionState: 'listed',
          sandbox: true,
          settlementMode: 'deferred',
        },
      ],
      products: [
        {
          agentReadable: true,
          checkoutPath: '/checkout/consultation-deposit',
          customerDataRequirements: [
            {
              key: 'email',
              kind: 'email',
              labelRef: 'label.customer.email',
              required: true,
            },
          ],
          displayRef: 'display.consultation_deposit',
          entitlementScope: 'product',
          id: 'consultation_deposit',
          metadataRefs: ['metadata.product.consultation_deposit'],
          price: {
            amountMinorUnits: 2500,
            asset: 'usd',
            denomination: 'usd_cent',
          },
          publicProjectionState: 'listed',
          sandbox: true,
          settlementMode: 'checkout_only',
        },
      ],
      provider: 'openagents_hosted_mdk',
      sandboxDefault: true,
    },
  },
  manifestRef: 'manifest.site_otec.payments.v3',
  orderRef: 'order.site_otec',
  siteId: 'site_otec',
  siteVersionId: 'version_site_otec_v3',
  sourceManifestDigest: 'sha256:site_otec_manifest_v3',
  status: 'active',
  updatedAt: '2026-06-06T10:01:00.000Z',
  workroomRef: 'workroom.site_otec',
} as const)

describe('OpenAgents Site checkout UI primitives', () => {
  test('derives generated-source checkout primitives from the payment catalog', () => {
    const contract = siteCheckoutUiPrimitivesFromCatalog({
      cancelPath: '/checkout/cancelled',
      catalog,
      runtimeTarget: 'static',
      sourceSurface: 'generated_html',
      successPath: '/checkout/complete',
    })

    expect(S.decodeUnknownSync(OpenAgentsSiteCheckoutUiPrimitiveContract)(
      contract,
    )).toEqual(contract)
    expect(contract.primitives.map(primitive => primitive.primitiveKind))
      .toEqual([
        'product_card',
        'checkout_button',
        'checkout_form',
        'paid_action_prompt',
        'checkout_button',
        'success_state',
        'cancel_state',
        'entitlement_state',
      ])
    expect(contract.primitives[0]).toMatchObject({
      checkoutIntentEndpoint:
        '/api/sites/site_otec/commerce/checkout-intents',
      customerDataRequirementRefs: ['email'],
      displayRef: 'display.consultation_deposit',
      price: {
        asset: 'usd',
        denomination: 'usd_cent',
      },
      sourceSafe: true,
    })
    expect(contract.primitives[3]).toMatchObject({
      actionId: 'download_report',
      path: '/api/actions/download-report',
      primitiveKind: 'paid_action_prompt',
    })
  })

  test('projects public and agent views without leaking metadata to public', () => {
    const contract = siteCheckoutUiPrimitivesFromCatalog({
      cancelPath: '/checkout/cancelled',
      catalog,
      runtimeTarget: 'workers_for_platforms',
      sourceSurface: 'generated_react',
      successPath: '/checkout/complete',
    })
    const publicProjection = projectOpenAgentsSiteCheckoutUiPrimitives(
      contract,
      'public',
    )
    const agentProjection = projectOpenAgentsSiteCheckoutUiPrimitives(
      contract,
      'agent',
    )

    expect(S.decodeUnknownSync(OpenAgentsSiteCheckoutUiPrimitiveProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(publicProjection.primitives[0]?.agentMetadataRefs).toEqual([])
    expect(agentProjection.primitives[0]?.agentMetadataRefs).toEqual([
      'metadata.site_payment.site_otec.v3',
      'metadata.product.consultation_deposit',
    ])
    expect(openAgentsSiteCheckoutUiPrimitiveHasPrivateMaterial(publicProjection))
      .toBe(false)
  })

  test('supports tip, deposit, and subscription affordance contracts safely', () => {
    const contract = siteCheckoutUiPrimitivesFromCatalog({
      cancelPath: '/checkout/cancelled',
      catalog,
      runtimeTarget: 'static',
      sourceSurface: 'generated_html',
      successPath: '/checkout/complete',
    })
    const base = contract.primitives[0]!
    const affordanceContract = decodeOpenAgentsSiteCheckoutUiPrimitiveContract({
      ...contract,
      primitives: [
        { ...base, id: 'site_checkout_ui:site_otec:tip', primitiveKind: 'tip_affordance' },
        { ...base, id: 'site_checkout_ui:site_otec:deposit', primitiveKind: 'deposit_affordance' },
        { ...base, id: 'site_checkout_ui:site_otec:subscription', primitiveKind: 'subscription_affordance' },
      ],
    })

    expect(affordanceContract.primitives.map(primitive => primitive.primitiveKind))
      .toEqual([
        'tip_affordance',
        'deposit_affordance',
        'subscription_affordance',
      ])
  })

  test('rejects checkout query state, private customer values, and payment secrets', () => {
    const contract = siteCheckoutUiPrimitivesFromCatalog({
      cancelPath: '/checkout/cancelled',
      catalog,
      runtimeTarget: 'static',
      sourceSurface: 'generated_html',
      successPath: '/checkout/complete',
    })

    expect(() =>
      decodeOpenAgentsSiteCheckoutUiPrimitiveContract({
        ...contract,
        primitives: [
          {
            ...contract.primitives[0]!,
            successPath: '/checkout/complete?checkout_id=abc',
          },
        ],
      }),
    ).toThrow(OpenAgentsSiteCheckoutUiPrimitiveUnsafe)
    expect(() =>
      decodeOpenAgentsSiteCheckoutUiPrimitiveContract({
        ...contract,
        primitives: [
          {
            ...contract.primitives[0]!,
            customerEmail: 'customer@example.com',
          },
        ],
      }),
    ).toThrow(OpenAgentsSiteCheckoutUiPrimitiveUnsafe)
    expect(() =>
      decodeOpenAgentsSiteCheckoutUiPrimitiveContract({
        ...contract,
        primitives: [
          {
            ...contract.primitives[0]!,
            agentMetadataRefs: ['lnbc2500n1rawinvoice'],
          },
        ],
      }),
    ).toThrow(OpenAgentsSiteCheckoutUiPrimitiveUnsafe)
  })
})
