import { describe, expect, test } from 'vitest'

import { ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF } from './prefilled-workspace-vertical-templates'
import {
  type EcommerceCampaignAuthorityGateId,
  type EcommerceCampaignDeliveryInput,
  EcommerceCampaignDeliveryReceiptInvariantError,
  buildEcommerceCampaignDeliveryReceipt,
  verifyEcommerceCampaignPaidDelivery,
} from './ecommerce-campaign-delivery-receipt'

const allBlockedGates: Record<EcommerceCampaignAuthorityGateId, boolean> = {
  merchant_approval: false,
  channel_access: false,
  ad_account_access: false,
  spend_cap_accepted: false,
  publish_authority: false,
}

const allReceiptedGates: Record<EcommerceCampaignAuthorityGateId, boolean> = {
  merchant_approval: true,
  channel_access: true,
  ad_account_access: true,
  spend_cap_accepted: true,
  publish_authority: true,
}

const baseInput = (
  overrides: Partial<EcommerceCampaignDeliveryInput> = {},
): EcommerceCampaignDeliveryInput => ({
  workItemRef: 'work_item.ecommerce.campaign.fixture',
  outcomeKind: 'inventory_aware_ad_campaign',
  humanReviewAccepted: false,
  receiptedGates: allBlockedGates,
  spendCapCents: 50_000,
  spendObservedCents: null,
  publishedArtifactRefs: [],
  statsWindow: null,
  attributionCaveat:
    'Attribution is modeled, not deterministic; treat as directional.',
  stockoutFollowUp: 'Re-check SKU stock before any re-run.',
  paidSettlement: {
    amountCents: 0,
    asset: 'usd',
    evidenced: false,
    publicPaymentRef: null,
  },
  freshnessTimestamp: '2026-06-20T12:00:00.000Z',
  publicSourceRefs: [
    'docs/blitz/forge/2026-06-16-ecommerce-prefilled-workspace.md',
  ],
  ...overrides,
})

describe('e-commerce campaign delivery receipt', () => {
  test('a fresh draft with all gates blocked is blocked, not delivered', () => {
    const receipt = buildEcommerceCampaignDeliveryReceipt(baseInput())

    expect(receipt.templateRef).toBe(ECOMMERCE_DESIGN_PARTNER_TEMPLATE_REF)
    expect(receipt.deliveryStage).toBe('blocked')
    expect(receipt.evidenceState).toBe('not_yet_evidenced')
    expect(receipt.noAutoPublish).toBe(true)
    expect(receipt.noAutoSpend).toBe(true)
    expect(receipt.outstandingAuthorityBlockers).toHaveLength(5)
    expect(verifyEcommerceCampaignPaidDelivery(receipt).length).toBeGreaterThan(
      0,
    )
  })

  test('reviewed draft with no blocked gates is drafted_for_review until paid', () => {
    const receipt = buildEcommerceCampaignDeliveryReceipt(
      baseInput({
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
      }),
    )

    expect(receipt.deliveryStage).toBe('drafted_for_review')
    expect(receipt.evidenceState).toBe('review_accepted')
    expect(receipt.outstandingAuthorityBlockers).toHaveLength(0)
    expect(verifyEcommerceCampaignPaidDelivery(receipt)).toContain(
      'buyer payment not recorded',
    )
  })

  test('a fully gated, reviewed, paid delivery verifies as evidenced', () => {
    const receipt = buildEcommerceCampaignDeliveryReceipt(
      baseInput({
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
        publishedArtifactRefs: ['campaign.meta.set.123'],
        spendObservedCents: 42_000,
        statsWindow: '2026-06-20/2026-06-27',
        paidSettlement: {
          amountCents: 25_000,
          asset: 'usd',
          evidenced: true,
          publicPaymentRef: 'payment.public.ref.abc',
        },
      }),
    )

    expect(receipt.deliveryStage).toBe('delivered')
    expect(receipt.evidenceState).toBe('paid')
    expect(verifyEcommerceCampaignPaidDelivery(receipt)).toEqual([])
  })

  test('rejects published artifacts while authority gates are blocked', () => {
    expect(() =>
      buildEcommerceCampaignDeliveryReceipt(
        baseInput({
          publishedArtifactRefs: ['campaign.meta.set.999'],
        }),
      ),
    ).toThrow(EcommerceCampaignDeliveryReceiptInvariantError)
  })

  test('rejects observed spend while authority gates are blocked', () => {
    expect(() =>
      buildEcommerceCampaignDeliveryReceipt(
        baseInput({ spendObservedCents: 1_000 }),
      ),
    ).toThrow(EcommerceCampaignDeliveryReceiptInvariantError)
  })

  test('rejects observed spend above the accepted spend cap', () => {
    expect(() =>
      buildEcommerceCampaignDeliveryReceipt(
        baseInput({
          receiptedGates: allReceiptedGates,
          spendCapCents: 10_000,
          spendObservedCents: 10_001,
        }),
      ),
    ).toThrow(EcommerceCampaignDeliveryReceiptInvariantError)
  })

  test('does not verify a paid delivery that lacks a dereferenceable payment ref', () => {
    const receipt = buildEcommerceCampaignDeliveryReceipt(
      baseInput({
        humanReviewAccepted: true,
        receiptedGates: allReceiptedGates,
        publishedArtifactRefs: ['campaign.meta.set.123'],
        paidSettlement: {
          amountCents: 25_000,
          asset: 'usd',
          evidenced: true,
          publicPaymentRef: null,
        },
      }),
    )

    expect(verifyEcommerceCampaignPaidDelivery(receipt)).toContain(
      'paid settlement recorded without a dereferenceable payment ref',
    )
  })
})
