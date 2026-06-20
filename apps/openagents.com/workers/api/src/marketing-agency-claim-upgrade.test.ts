import { describe, expect, test } from 'vitest'

import {
  assessMarketingAgencyPaidDeliveryClaim,
  MARKETING_AGENCY_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF,
  PAID_DELIVERY_GATE_OWNER_SIGN_OFF,
  PAID_DELIVERY_GATE_RECEIPT_VERIFIES,
  projectMarketingAgencyPaidDeliveryClaims,
} from './marketing-agency-claim-upgrade'

import { firstPaidMarketingAgencyDeliveryReceiptFixture } from './marketing-agency-delivery-receipt-fixture'

const receiptFixture = (overrides = {}) => ({
  ...firstPaidMarketingAgencyDeliveryReceiptFixture,
  ...overrides,
})

describe('marketing-agency campaign claim upgrade', () => {
  test('a fully gated, verified receipt with owner sign-off passes all gates', () => {
    const claim = assessMarketingAgencyPaidDeliveryClaim({
      receipt: receiptFixture(),
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
    const claim = assessMarketingAgencyPaidDeliveryClaim({
      receipt: receiptFixture(),
      receiptRef: 'receipt.fixture',
    })

    expect(claim.paidDeliverySubstantiated).toBe(false)
    expect(claim.failingGateRefs).toContain(PAID_DELIVERY_GATE_OWNER_SIGN_OFF)
    expect(claim.unclearedBlockerRefs).toContain(MARKETING_AGENCY_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF)
    expect(claim.gates.ownerSignOffPresent).toBe(false)
  })

  test('fails if receipt does not verify clean', () => {
    const claim = assessMarketingAgencyPaidDeliveryClaim({
      receipt: receiptFixture({ deliveryStage: 'drafted_for_review' }),
      receiptRef: 'receipt.fixture',
      ownerSignOffRef: 'owner.fixture',
    })

    expect(claim.paidDeliverySubstantiated).toBe(false)
    expect(claim.failingGateRefs).toContain(PAID_DELIVERY_GATE_RECEIPT_VERIFIES)
    expect(claim.unclearedBlockerRefs).toContain(MARKETING_AGENCY_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF)
    expect(claim.gates.receiptVerifiesClean).toBe(false)
  })

  test('projecting an empty list returns 0 claims and false', () => {
    const projection = projectMarketingAgencyPaidDeliveryClaims([])

    expect(projection.paidDeliveryClaimSubstantiated).toBe(false)
    expect(projection.totals.assessedCount).toBe(0)
    expect(projection.totals.substantiatedCount).toBe(0)
    expect(projection.unclearedBlockerRefs).toContain(MARKETING_AGENCY_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF)
  })

  test('projecting a mix of substantiated and withheld claims', () => {
    const projection = projectMarketingAgencyPaidDeliveryClaims([
      { receipt: receiptFixture(), receiptRef: 'r1' }, // withheld (no signoff)
      { receipt: receiptFixture(), receiptRef: 'r2', ownerSignOffRef: 'owner' }, // substantiated
    ])

    expect(projection.paidDeliveryClaimSubstantiated).toBe(true)
    expect(projection.totals.assessedCount).toBe(2)
    expect(projection.totals.substantiatedCount).toBe(1)
    expect(projection.totals.withheldCount).toBe(1)
    // The projection still surfaces the blocker because resolving it completely requires a promise state flip, but the projection alone just reports.
    expect(projection.unclearedBlockerRefs).toContain(MARKETING_AGENCY_PACK_FIRST_PAID_RECEIPT_BLOCKER_REF)
  })
})
