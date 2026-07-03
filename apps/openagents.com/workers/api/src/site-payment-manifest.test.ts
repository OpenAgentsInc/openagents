import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsSitePaymentManifestProjection,
  OpenAgentsSitePaymentManifestUnsafe,
  decodeOpenAgentsSitePaymentManifest,
  openAgentsSitePaymentManifestHasPrivateMaterial,
  projectOpenAgentsSitePaymentManifest,
} from './site-payment-manifest'

const validManifest = {
  payments: {
    agentReadable: true,
    enabled: true,
    metadataRefs: ['metadata.site_payment.otec.v1'],
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
} as const

describe('OpenAgents Site payment manifest', () => {
  test('decodes a source-visible Site payment manifest and projects it safely', () => {
    const manifest = decodeOpenAgentsSitePaymentManifest(validManifest)
    const publicProjection = projectOpenAgentsSitePaymentManifest(
      manifest,
      'public',
    )
    const agentProjection = projectOpenAgentsSitePaymentManifest(
      manifest,
      'agent',
    )

    expect(publicProjection).toMatchObject({
      enabled: true,
      paidActions: [
        {
          id: 'download_report',
          price: {
            asset: 'bitcoin',
            denomination: 'bitcoin_millisatoshi',
          },
        },
      ],
      products: [
        {
          id: 'consultation_deposit',
          price: {
            asset: 'usd',
            denomination: 'usd_cent',
          },
        },
      ],
      payoutModeGate: {
        activeMode: 'disabled',
        hostedDirectPayoutClaimAllowed: false,
        livePayoutClaimAllowed: false,
        state: 'blocked',
      },
      provider: 'openagents_hosted_mdk',
    })
    expect(publicProjection.payoutModeGate.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.mdk.hosted_programmatic_payouts_disabled',
      ]),
    )
    expect(agentProjection.products[0]?.checkoutPath).toBe(
      '/checkout/consultation-deposit',
    )
    expect(
      S.decodeUnknownSync(OpenAgentsSitePaymentManifestProjection)(
        publicProjection,
      ),
    ).toEqual(publicProjection)
    expect(JSON.stringify(publicProjection)).not.toContain('customerData')
    expect(
      openAgentsSitePaymentManifestHasPrivateMaterial(publicProjection),
    ).toBe(false)
  })

  test('rejects sats wording in source-visible payment prices', () => {
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          paidActions: [
            {
              ...validManifest.payments.paidActions[0],
              price: {
                amountMinorUnits: 25_000,
                asset: 'sats',
                denomination: 'sats',
              },
            },
          ],
        },
      }),
    ).toThrow()
  })

  test('projects subscription and retainer products with receipt-renewed recurrence', () => {
    const manifest = decodeOpenAgentsSitePaymentManifest({
      payments: {
        ...validManifest.payments,
        products: [
          {
            ...validManifest.payments.products[0],
            displayRef: 'display.monthly_retainer',
            id: 'monthly_retainer',
            metadataRefs: ['metadata.product.monthly_retainer'],
            recurringBilling: {
              billingKind: 'retainer',
              entitlementRenewalMode: 'renew_on_payment_receipt',
              interval: 'month',
              renewalReceiptScopeRefs: [
                'receipt_scope.business.retainer.renewal',
              ],
            },
          },
          {
            ...validManifest.payments.products[0],
            displayRef: 'display.membership_subscription',
            id: 'membership_subscription',
            metadataRefs: ['metadata.product.membership_subscription'],
            recurringBilling: {
              billingKind: 'subscription',
              entitlementRenewalMode: 'renew_on_payment_receipt',
              interval: 'year',
              renewalReceiptScopeRefs: [
                'receipt_scope.business.membership.renewal',
              ],
            },
          },
        ],
      },
    })
    const publicProjection = projectOpenAgentsSitePaymentManifest(
      manifest,
      'public',
    )

    expect(
      publicProjection.products.map(product => product.recurringBilling),
    ).toEqual([
      {
        billingKind: 'retainer',
        entitlementRenewalMode: 'renew_on_payment_receipt',
        interval: 'month',
        renewalReceiptScopeRefs: ['receipt_scope.business.retainer.renewal'],
      },
      {
        billingKind: 'subscription',
        entitlementRenewalMode: 'renew_on_payment_receipt',
        interval: 'year',
        renewalReceiptScopeRefs: ['receipt_scope.business.membership.renewal'],
      },
    ])
    expect(
      S.decodeUnknownSync(OpenAgentsSitePaymentManifestProjection)(
        publicProjection,
      ),
    ).toEqual(publicProjection)
  })

  test('projects customer-owned processor funnels with separate OpenAgents metering refs', () => {
    const manifest = decodeOpenAgentsSitePaymentManifest({
      payments: {
        ...validManifest.payments,
        customerOwnedProcessor: {
          chargeDestination: 'customer_account',
          customerProcessorAccountRef: 'processor_account.vertical_checkout',
          openAgentsMeteringRefs: [
            'metering.openagents.business_fulfillment.site_payment',
          ],
          processor: 'stripe_connect',
          processorConnectionRef: 'processor_connection.vertical_checkout',
          revenueOwner: 'customer',
        },
        provider: 'customer_owned_processor',
      },
    })
    const publicProjection = projectOpenAgentsSitePaymentManifest(
      manifest,
      'public',
    )

    expect(publicProjection.provider).toBe('customer_owned_processor')
    expect(publicProjection.customerOwnedProcessor).toEqual({
      chargeDestination: 'customer_account',
      meteringSeparated: true,
      openAgentsMeteringRefs: [
        'metering.openagents.business_fulfillment.site_payment',
      ],
      processor: 'stripe_connect',
      revenueOwner: 'customer',
    })
    expect(JSON.stringify(publicProjection)).not.toContain(
      'processor_connection.vertical_checkout',
    )
    expect(JSON.stringify(publicProjection)).not.toContain(
      'processor_account.vertical_checkout',
    )
    expect(
      openAgentsSitePaymentManifestHasPrivateMaterial(publicProjection),
    ).toBe(false)
  })

  test('rejects raw payment material and unsafe checkout paths', () => {
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          metadataRefs: ['lnbc2500n1rawinvoice'],
        },
      }),
    ).toThrow(OpenAgentsSitePaymentManifestUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          products: [
            {
              ...validManifest.payments.products[0],
              checkoutPath: '/checkout/success?checkout_id=abc',
            },
          ],
        },
      }),
    ).toThrow(OpenAgentsSitePaymentManifestUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          paidActions: [
            {
              ...validManifest.payments.paidActions[0],
              path: 'https://evil.test/api/actions/download-report',
            },
          ],
        },
      }),
    ).toThrow(OpenAgentsSitePaymentManifestUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          products: [
            {
              ...validManifest.payments.products[0],
              recurringBilling: {
                billingKind: 'subscription',
                entitlementRenewalMode: 'renew_on_payment_receipt',
                interval: 'month',
                renewalReceiptScopeRefs: ['customer_email.ben@example.com'],
              },
            },
          ],
        },
      }),
    ).toThrow(OpenAgentsSitePaymentManifestUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          customerOwnedProcessor: {
            chargeDestination: 'customer_account',
            customerProcessorAccountRef: 'acct_123raw',
            openAgentsMeteringRefs: [
              'metering.openagents.business_fulfillment.site_payment',
            ],
            processor: 'stripe_connect',
            processorConnectionRef: 'processor_connection.vertical_checkout',
            revenueOwner: 'customer',
          },
          provider: 'customer_owned_processor',
        },
      }),
    ).toThrow(OpenAgentsSitePaymentManifestUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          customerOwnedProcessor: {
            chargeDestination: 'customer_account',
            customerProcessorAccountRef: 'processor_account.vertical_checkout',
            openAgentsMeteringRefs: [],
            processor: 'stripe_connect',
            processorConnectionRef: 'processor_connection.vertical_checkout',
            revenueOwner: 'customer',
          },
          provider: 'customer_owned_processor',
        },
      }),
    ).toThrow(OpenAgentsSitePaymentManifestUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          provider: 'customer_owned_processor',
        },
      }),
    ).toThrow(OpenAgentsSitePaymentManifestUnsafe)
    expect(() =>
      decodeOpenAgentsSitePaymentManifest({
        payments: {
          ...validManifest.payments,
          customerOwnedProcessor: {
            chargeDestination: 'customer_account',
            customerProcessorAccountRef: 'processor_account.vertical_checkout',
            openAgentsMeteringRefs: [
              'metering.openagents.business_fulfillment.site_payment',
            ],
            processor: 'stripe_connect',
            processorConnectionRef: 'processor_connection.vertical_checkout',
            revenueOwner: 'customer',
          },
        },
      }),
    ).toThrow(OpenAgentsSitePaymentManifestUnsafe)
  })

  test('hides public-hidden products and actions from public projection', () => {
    const manifest = decodeOpenAgentsSitePaymentManifest({
      payments: {
        ...validManifest.payments,
        paidActions: [
          {
            ...validManifest.payments.paidActions[0],
            publicProjectionState: 'hidden',
          },
        ],
        products: [
          {
            ...validManifest.payments.products[0],
            publicProjectionState: 'hidden',
          },
        ],
      },
    })
    const publicProjection = projectOpenAgentsSitePaymentManifest(
      manifest,
      'public',
    )
    const operatorProjection = projectOpenAgentsSitePaymentManifest(
      manifest,
      'operator',
    )

    expect(publicProjection.products).toEqual([])
    expect(publicProjection.paidActions).toEqual([])
    expect(operatorProjection.products).toHaveLength(1)
    expect(operatorProjection.paidActions).toHaveLength(1)
  })
})
