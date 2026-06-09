import { describe, expect, test } from 'vitest'

import {
  SiteCommerceManifestUnsafe,
  decodeSiteSourceCommerceManifest,
} from './site-commerce'

const validManifest = {
  payments: {
    enabled: true,
    provider: 'openagents_hosted',
    products: [
      {
        id: 'consultation_deposit',
        name: 'Consultation deposit',
        price: { asset: 'usd', amount: 5000 },
        checkoutPath: '/checkout/consultation-deposit',
        entitlementScope: 'product',
        agentReadable: true,
        settlementMode: 'checkout_only',
        customerDataRequirements: [
          {
            key: 'email',
            label: 'Email',
            required: true,
            kind: 'email',
          },
        ],
        publicProjectionState: 'listed',
      },
    ],
    paidActions: [
      {
        id: 'download_report',
        name: 'Download report',
        price: { asset: 'sats', amount: 100 },
        method: 'GET',
        path: '/api/reports/download',
        checkoutPath: '/checkout/download-report',
        entitlementScope: 'action',
        agentReadable: true,
        settlementMode: 'deferred',
        customerDataRequirements: [],
        publicProjectionState: 'proof_only',
      },
    ],
  },
}

describe('Site commerce manifest schema', () => {
  test('accepts a public-safe product and paid-action manifest', () => {
    expect(decodeSiteSourceCommerceManifest(validManifest)).toEqual(
      validManifest,
    )
  })

  test('rejects secret-shaped fields', () => {
    expect(() =>
      decodeSiteSourceCommerceManifest({
        ...validManifest,
        payments: {
          ...validManifest.payments,
          mdkAccessToken: 'MDK_ACCESS_TOKEN=secret-value',
        },
      }),
    ).toThrow(SiteCommerceManifestUnsafe)
  })

  test('rejects raw invoice and preimage material', () => {
    expect(() =>
      decodeSiteSourceCommerceManifest({
        payments: {
          ...validManifest.payments,
          products: [
            {
              ...validManifest.payments.products[0],
              checkoutPath: '/checkout/consultation-deposit',
              name: 'lnbc2500n1rawinvoice',
            },
          ],
        },
      }),
    ).toThrow(SiteCommerceManifestUnsafe)

    expect(() =>
      decodeSiteSourceCommerceManifest({
        payments: {
          ...validManifest.payments,
          paidActions: [
            {
              ...validManifest.payments.paidActions[0],
              name: 'payment_preimage=abc123',
            },
          ],
        },
      }),
    ).toThrow(SiteCommerceManifestUnsafe)
  })

  test('rejects checkout paths with query or result state', () => {
    expect(() =>
      decodeSiteSourceCommerceManifest({
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
    ).toThrow(SiteCommerceManifestUnsafe)
  })

  test('rejects non-positive prices', () => {
    expect(() =>
      decodeSiteSourceCommerceManifest({
        payments: {
          ...validManifest.payments,
          products: [
            {
              ...validManifest.payments.products[0],
              price: { asset: 'usd', amount: 0 },
            },
          ],
        },
      }),
    ).toThrow(SiteCommerceManifestUnsafe)
  })
})
