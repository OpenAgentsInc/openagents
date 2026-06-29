import { describe, expect, test } from 'vitest'

import {
  assessEcommerceCampaignPaidDeliveryClaim,
  ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF,
  PAID_DELIVERY_GATE_OWNER_SIGN_OFF,
  PAID_DELIVERY_GATE_RECEIPT_VERIFIES,
  projectEcommerceCampaignPaidDeliveryClaims,
} from './ecommerce-campaign-claim-upgrade'
import {
  type EcommerceCampaignDeliveryReceiptDocument,
  ECOMMERCE_CAMPAIGN_DELIVERY_RECEIPT_DOC_VERSION,
  buildEcommerceCampaignDeliveryReceipt,
} from './ecommerce-campaign-delivery-receipt'

const receiptFixture = (overrides = {}): EcommerceCampaignDeliveryReceiptDocument => ({
  docVersion: ECOMMERCE_CAMPAIGN_DELIVERY_RECEIPT_DOC_VERSION,
  receipt: buildEcommerceCampaignDeliveryReceipt({
    workItemRef: 'work_item.fixture',
    outcomeKind: 'inventory_aware_ad_campaign',
    humanReviewAccepted: true,
    receiptedGates: {
      merchant_approval: true,
      channel_access: true,
      ad_account_access: true,
      spend_cap_accepted: true,
      publish_authority: true,
    },
    spendCapCents: 50000,
    spendObservedCents: 1000,
    publishedArtifactRefs: ['artifact.fixture'],
    statsWindow: null,
    attributionCaveat: 'fixture',
    stockoutFollowUp: 'fixture',
    paidSettlement: {
      amountCents: 1000,
      asset: 'usd',
      evidenced: true,
      publicPaymentRef: 'payment.fixture',
    },
    freshnessTimestamp: '2026-06-20T12:00:00Z',
    publicSourceRefs: [],
    ...overrides,
  }),
})

describe('e-commerce campaign claim upgrade', () => {
  test('a fully gated, verified receipt with owner sign-off passes all gates', () => {
    const claim = assessEcommerceCampaignPaidDeliveryClaim({
      document: receiptFixture(),
      receiptRef: 'receipt.fixture',
      ownerSignOffRef: 'owner.fixture',
    })

    expect(claim.paidDeliverySubstantiated).toBe(true)
    expect(claim.failingGateRefs).toEqual([])
    expect(claim.unclearedBlockerRefs).toEqual([])
    expect(claim.gates.receiptVerifiesClean).toBe(true)
    expect(claim.gates.ownerSignOffPresent).toBe(true)
    expect(claim.promiseState).toBe('yellow')
  })

  test('fails if owner sign-off is missing', () => {
    const claim = assessEcommerceCampaignPaidDeliveryClaim({
      document: receiptFixture(),
      receiptRef: 'receipt.fixture',
    })

    expect(claim.paidDeliverySubstantiated).toBe(false)
    expect(claim.failingGateRefs).toContain(PAID_DELIVERY_GATE_OWNER_SIGN_OFF)
    expect(claim.unclearedBlockerRefs).toContain(ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF)
    expect(claim.gates.ownerSignOffPresent).toBe(false)
  })

  test('fails if receipt does not verify clean', () => {
    const claim = assessEcommerceCampaignPaidDeliveryClaim({
      document: receiptFixture({ humanReviewAccepted: false }),
      receiptRef: 'receipt.fixture',
      ownerSignOffRef: 'owner.fixture',
    })

    expect(claim.paidDeliverySubstantiated).toBe(false)
    expect(claim.failingGateRefs).toContain(PAID_DELIVERY_GATE_RECEIPT_VERIFIES)
    expect(claim.unclearedBlockerRefs).toContain(ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF)
    expect(claim.gates.receiptVerifiesClean).toBe(false)
  })

  test('projecting an empty list returns 0 claims and false', () => {
    const projection = projectEcommerceCampaignPaidDeliveryClaims([])

    expect(projection.paidDeliveryClaimSubstantiated).toBe(false)
    expect(projection.totals.assessedCount).toBe(0)
    expect(projection.totals.substantiatedCount).toBe(0)
    expect(projection.unclearedBlockerRefs).toContain(ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF)
  })

  test('projecting a mix of substantiated and withheld claims', () => {
    const projection = projectEcommerceCampaignPaidDeliveryClaims([
      { document: receiptFixture(), receiptRef: 'r1' }, // withheld (no signoff)
      { document: receiptFixture(), receiptRef: 'r2', ownerSignOffRef: 'owner' }, // substantiated
    ])

    expect(projection.paidDeliveryClaimSubstantiated).toBe(true)
    expect(projection.totals.assessedCount).toBe(2)
    expect(projection.totals.substantiatedCount).toBe(1)
    expect(projection.totals.withheldCount).toBe(1)
    // The projection still surfaces the blocker because resolving it completely requires a promise state flip, but the projection alone just reports.
    expect(projection.unclearedBlockerRefs).toContain(ECOMMERCE_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF)
  })
})
